use super::*;
use async_trait::async_trait;
use rig_core::driver::{Exchange, Opened, Opening, Transport};
use rig_core::error::ProviderError;
use rig_core::test_utils::{mock_final, MockFrame, MockScript, MockStreamEvent};
use rig_core::Model;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// -- ConfigStore de teste ---------------------------------------------------------------------

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-brain-turn-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> (ConfigStore, TempDir) {
    let dir = TempDir::new();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    (store, dir)
}

// -- FakeModel: dublê de teste do modelo, sem rede nenhuma ---------------------------------------
//
// É o TRANSPORTE de um `Model<MockScript, _>` do `rig-core` (feature `test-utils`): o wire e o
// decodificador são os do próprio crate (`MockScript`), então o laço de `run_turn` recebe os mesmos
// `StreamEvent`/`Streamed` que receberia de um provedor real. O que o dublê acrescenta é o que o
// roteiro pronto do `rig-core` não tem: atraso entre fragmentos, falha na N-ésima chamada e o
// registro de `tool_choice`/nº de tools de cada pedido.

#[derive(Clone)]
enum FakeChunk {
    Text(&'static str),
    ToolCall { id: &'static str, name: &'static str, args: Value },
}

#[derive(Clone)]
struct FakeModel {
    /// Uma entrada por chamada a `.stream()` — cada rodada do laço consome a próxima.
    script: Arc<Mutex<std::collections::VecDeque<Vec<FakeChunk>>>>,
    delay: Option<Duration>,
    calls: Arc<AtomicUsize>,
    /// Índice (0-based) da chamada a `.stream()` que deve devolver `Err` em vez de um stream —
    /// simula erro de rede/HTTP sem precisar de um servidor de mentira. `None` nunca falha
    /// (comportamento padrão, igual antes desta rodada).
    fail_stream_at: Option<usize>,
    /// `tool_choice` de cada chamada a `.stream()`, na ordem.
    tool_choices: Arc<Mutex<Vec<Option<rig_core::message::ToolChoice>>>>,
    /// Quantas tools o pedido de cada chamada a `.stream()` anunciou, na ordem.
    tool_counts: Arc<Mutex<Vec<usize>>>,
}

impl FakeModel {
    fn new(script: Vec<Vec<FakeChunk>>) -> Self {
        Self { script: Arc::new(Mutex::new(script.into())), delay: None, calls: Arc::new(AtomicUsize::new(0)), fail_stream_at: None, tool_choices: Arc::default(), tool_counts: Arc::default() }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// A chamada de índice `at` a `.stream()` devolve `Err` em vez de abrir o stream —
    /// `run_turn`/`ProviderBeforeSideEffects` (ver o cabeçalho do arquivo).
    fn failing_stream_at(mut self, at: usize) -> Self {
        self.fail_stream_at = Some(at);
        self
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// O dublê como o modelo que `run_turn` recebe em produção.
    fn model(&self) -> ChatModel {
        Model::new(MockScript::default(), self.clone()).erase()
    }
}

impl Transport<MockScript> for FakeModel {
    fn send(&self, request: CompletionRequest, exchange: Exchange) -> Opening<MockFrame> {
        assert_eq!(exchange.mode, rig_core::wire::Mode::Streaming, "run_turn só usa stream()");
        self.tool_choices.lock().unwrap().push(request.tool_choice.clone());
        self.tool_counts.lock().unwrap().push(request.tools.len());
        let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_stream_at == Some(call_index) {
            return Opening::failed(ProviderError::Provider("erro simulado de teste".to_string()));
        }
        let chunks = self.script.lock().unwrap().pop_front().unwrap_or_default();
        let delay = self.delay;

        Opening::ready(Opened::new(async_stream::stream! {
            for chunk in chunks {
                if let Some(d) = delay {
                    tokio::time::sleep(d).await;
                }
                yield Ok(MockFrame::Event(match chunk {
                    FakeChunk::Text(text) => MockStreamEvent::text(text),
                    FakeChunk::ToolCall { id, name, args } => MockStreamEvent::tool_call(id, name, args),
                }));
            }
            yield Ok(MockFrame::Event(MockStreamEvent::FinalResponse(mock_final(Usage::new()))));
        }))
    }
}

struct CollectingObserver {
    events: Mutex<Vec<TurnEvent>>,
}

impl CollectingObserver {
    fn new() -> Self {
        Self { events: Mutex::new(Vec::new()) }
    }

    fn events(&self) -> Vec<TurnEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl TurnObserver for CollectingObserver {
    fn emit(&self, event: TurnEvent) {
        self.events.lock().unwrap().push(event);
    }
}

async fn run(model: FakeModel, max_tool_steps: u32) -> (Result<TurnResult, TurnError>, CollectingObserver, TurnAbortHandle) {
    let (handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();
    let result = run_turn(
        model.model(),
        "system".to_string(),
        vec![],
        "olá",
        vec![],
        max_tool_steps,
        None,
        None,
        None,
        signal,
        &observer,
    )
    .await;
    (result, observer, handle)
}

#[tokio::test]
async fn plain_text_turn_returns_the_streamed_text_with_no_tool_calls() {
    let model = FakeModel::new(vec![vec![FakeChunk::Text("Oi, "), FakeChunk::Text("tudo bem?")]]);
    let (result, observer, _handle) = run(model, 6).await;

    let turn = result.unwrap();
    assert_eq!(turn.text, "Oi, tudo bem?");

    let events = observer.events();
    assert!(!events.iter().any(|e| matches!(e, TurnEvent::ToolCallStarted { .. })), "turno sem tool não anuncia tool");
    assert_eq!(events.first(), Some(&TurnEvent::ThinkingStart));
    assert_eq!(events.last(), Some(&TurnEvent::ThinkingStop));
    assert!(events.contains(&TurnEvent::TextChunk("Oi, ".to_string())));
}

/// A prova de FIAÇÃO pedida pela rodada de higiene de fala: `narration_filter` não é só correto
/// isolado (`narration_filter.rs` já prova isso) — o raciocínio vazado tem de morrer ANTES de
/// virar `TextChunk` (o evento que UI e TTS consomem) e antes de `turn.text` (que `chat/send.rs`
/// grava em histórico/memória). Um teste que só chamasse `NarrationFilter` direto não pegaria um
/// `mod narration_filter;` declarado mas nunca construído no laço de streaming.
#[tokio::test]
async fn narration_filter_is_wired_so_leaked_reasoning_never_reaches_text_chunk_events_or_the_final_text() {
    let model = FakeModel::new(vec![vec![
        FakeChunk::Text("<|THINKING|> [thinking] O usuário quer saber das horas.\n\n"),
        FakeChunk::Text("<|SMUG|> São 10h!"),
    ]]);
    let (result, observer, _handle) = run(model, 6).await;

    let turn = result.unwrap();
    assert_eq!(turn.text, "<|SMUG|> São 10h!", "turn.text alimenta histórico/memória em chat/send.rs — não pode carregar a narração");

    let spoken: String = observer
        .events()
        .into_iter()
        .filter_map(|event| match event {
            TurnEvent::TextChunk(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(spoken, "<|SMUG|> São 10h!", "nenhum TextChunk (UI/TTS) pode carregar a narração vazada");
}

/// Wiring of `tag_loop`: past a degenerate run of tags the stream is cut, so whatever the model
/// would still produce (30 s of it in the smoke) is never read.
#[tokio::test]
async fn a_degenerate_run_of_emotion_tags_ends_the_step() {
    let model = FakeModel::new(vec![vec![
        FakeChunk::Text("<|THINKING|> <|THINKING|> <|THINKING|> <|THINKING|> "),
        FakeChunk::Text("<|THINKING|> <|THINKING|> <|THINKING|> <|THINKING|> "),
        FakeChunk::Text("never read"),
    ]]);
    let (result, _observer, _handle) = run(model, 6).await;
    // Cut before "never read"; what is left is tags only, so it is empty, never shown as text.
    assert!(matches!(result, Err(TurnError::EmptyOutput(_))), "{result:?}");
}

/// O laço de ferramentas de ponta a ponta: o modelo pede uma tool, o registro executa de
/// verdade (sem dublê nesta parte — `ToolRegistry` é o real), o resultado volta ao modelo, e a
/// segunda rodada produz o texto final.
#[tokio::test]
async fn tool_call_round_trips_through_the_real_tool_registry_before_the_final_answer() {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "get_time_test_turn".into(),
            description: Some("devolve um horário fixo".into()),
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(FixedTimeTool),
    );

    let model = FakeModel::new(vec![
        vec![FakeChunk::ToolCall { id: "call-1", name: "get_time_test_turn", args: json!({}) }],
        vec![FakeChunk::Text("São 10h.")],
    ]);
    let (result, observer, _handle) = run(model, 6).await;

    let turn = result.unwrap();
    assert_eq!(turn.text, "São 10h.");

    let events = observer.events();
    assert!(events.contains(&TurnEvent::ToolCallStarted { name: "get_time_test_turn".into() }));
    assert!(events.iter().any(|e| matches!(e, TurnEvent::ToolCallFinished { name, ok: true, result, .. } if name == "get_time_test_turn" && !result.is_null())), "o resultado da tool tem de chegar ao observer");
}

struct FixedTimeTool;

#[async_trait]
impl crate::modules::tools::ToolHandler for FixedTimeTool {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        Ok(json!({ "time": "10:00" }))
    }
}

/// Smoke 2026-09-26: after the text nudge Gemma answered the nudge ("vacilei feio… vou mandar
/// agora") and still called nothing. The corrective round forces a tool call.
#[tokio::test]
async fn the_corrective_round_after_an_untooled_claim_requires_a_tool_call() {
    let model = FakeModel::new(vec![vec![FakeChunk::Text("Vou abrir a calculadora pra você!")], vec![FakeChunk::Text("ok")]]);
    let (handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();
    let desktop = ToolDefinition::new(ToolName::new("desktop").unwrap(), "", json!({ "type": "object" }));
    let _ = run_turn(model.model(), "system".into(), vec![], "abre a calculadora", vec![desktop], 6, None, None, None, signal, &observer).await;
    drop(handle);
    let choices = model.tool_choices.lock().unwrap().clone();
    assert_eq!(choices, vec![None, Some(rig_core::message::ToolChoice::Required)]);
}

/// Smoke 2026-09-27: after three searches the whole final reply was `<|THINKING|>` — nothing to
/// say or show, yet it counted as an answer.
#[tokio::test]
async fn a_final_reply_made_only_of_emotion_tags_counts_as_empty() {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "tag_only_test_turn".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(FixedTimeTool),
    );
    let model = FakeModel::new(vec![
        vec![FakeChunk::ToolCall { id: "call-t", name: "tag_only_test_turn", args: json!({}) }],
        vec![FakeChunk::Text("<|THINKING|>")],
        vec![FakeChunk::Text("São 10h!")],
    ]);
    let (result, _observer, _handle) = run(model, 6).await;
    assert_eq!(result.unwrap().text, "São 10h!");
}

/// Smoke 2026-09-27: a toolless reminder turn wrote Gemma's call syntax as its whole reply and
/// it was shown and stored; a call written as text is not an answer.
#[tokio::test]
async fn a_reply_that_is_only_a_textual_tool_call_counts_as_empty() {
    let model = FakeModel::new(vec![vec![FakeChunk::Text(r#"<|tool_call>call:reminder_manager{action: "get_reminders"}<tool_call|>"#)]]);
    let (result, _observer, _handle) = run(model, 6).await;
    assert!(matches!(result, Err(TurnError::EmptyOutput(_))), "{result:?}");
}

/// Background turns (facts, people, reflection) reply in JSON the caller parses; a bare object is
/// the answer, not a leaked call, and used to come back as `EmptyOutput`.
#[tokio::test]
async fn a_structured_json_reply_is_returned_verbatim_not_counted_as_empty() {
    let model = FakeModel::new(vec![vec![FakeChunk::Text(r#"{"facts": []}"#)]]);
    let (result, _observer, _handle) = run(model, 6).await;
    assert_eq!(result.unwrap().text, r#"{"facts": []}"#);
}

#[tokio::test]
async fn a_textual_tool_call_is_cut_out_of_a_real_reply() {
    let model = FakeModel::new(vec![vec![FakeChunk::Text(r#"Bebe água! <|tool_call>call:x{a: 1}<tool_call|>"#)]]);
    let (result, _observer, _handle) = run(model, 6).await;
    assert_eq!(result.unwrap().text, "Bebe água!");
}

struct CountingTool(Arc<std::sync::atomic::AtomicUsize>);

#[async_trait]
impl crate::modules::tools::ToolHandler for CountingTool {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(json!({ "success": true }))
    }
}

/// Smoke 2026-09-26: the step after `desktop launch calc` called the same launch again and two
/// calculators opened. An identical call that already succeeded in this turn is not re-run.
#[tokio::test]
async fn an_identical_tool_call_that_already_succeeded_in_the_turn_is_not_run_again() {
    let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "launch_once_test_turn".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(CountingTool(Arc::clone(&runs))),
    );
    let model = FakeModel::new(vec![
        vec![FakeChunk::ToolCall { id: "call-a", name: "launch_once_test_turn", args: json!({ "target": "calc" }) }],
        vec![FakeChunk::ToolCall { id: "call-b", name: "launch_once_test_turn", args: json!({ "target": "calc" }) }],
        vec![FakeChunk::Text("Pronto!")],
    ]);
    let (result, _observer, _handle) = run(model, 6).await;

    assert_eq!(result.unwrap().text, "Pronto!");
    assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 1);
}

/// Desde a `rig-core` 0.43 uma call cortada no meio (teto de tokens) chega como os argumentos que deu
/// para ler + `invalid_arguments`, e não como erro de stream: rodar a tool com esse prefixo faria uma
/// ação pela metade. A tool não roda, e o modelo recebe a falha.
#[tokio::test]
async fn a_tool_call_with_truncated_json_arguments_is_never_run() {
    let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    register_test_tool("truncated_args_test_turn", Arc::new(CountingTool(Arc::clone(&runs))));
    let cut_off = rig_core::message::ToolCall::from_wire("t1", rig_core::message::ToolFunction::parse(ToolName::new("truncated_args_test_turn").unwrap(), r#"{"target": "ca"#));
    assert!(cut_off.function.invalid_arguments.is_some(), "o fixture precisa ser um JSON cortado");
    let (_handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();

    let step = tool_step::run(&[cut_off], &signal, &observer, &mut std::collections::HashMap::new(), Duration::from_secs(5)).await.unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 0);
    assert!(observer.events().iter().any(|e| matches!(e, TurnEvent::ToolCallFinished { ok: false, .. })));
    assert_eq!(step.results.len(), 1, "o modelo recebe o resultado (a falha) da call");
}

/// Corrige o achado real do smoke: um turno que rodou tool mas terminou com texto vazio NÃO é
/// sucesso mudo (3ª chamada de `vision_capture`, resposta final "" chegando 140s depois). A
/// segunda chance é UMA chamada a mais com os resultados já no histórico — nunca refazer o turno
/// do zero, que chamava a tool de novo (smoke 2026-09-26: segundo diálogo de aprovação para o
/// mesmo código; aprovado, seria o clique repetido).
#[tokio::test]
async fn a_turn_that_ran_a_tool_and_ended_empty_is_retried_in_context_never_rerun() {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "silent_action_test_turn".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(FixedTimeTool),
    );

    let model = FakeModel::new(vec![
        vec![FakeChunk::ToolCall { id: "call-2", name: "silent_action_test_turn", args: json!({}) }],
        vec![], // fim vazio
        vec![], // vazio de novo depois do lembrete — aciona o resumo em vez de ""
        vec![FakeChunk::ToolCall { id: "call-3", name: "silent_action_test_turn", args: json!({}) }],
    ]);
    let (result, observer, _handle) = run(model.clone(), 6).await;

    let turn = result.unwrap();
    assert!(!turn.text.is_empty(), "nunca uma string vazia depois de rodar tool");
    assert!(turn.text.contains("silent_action_test_turn"), "o resumo nomeia a tool");
    assert!(!turn.text.contains("Rodei"), "o resumo não afirma sucesso: a tool pode ter sido negada");
    let runs = observer.events().into_iter().filter(|e| matches!(e, TurnEvent::ToolCallStarted { .. })).count();
    assert_eq!(runs, 1, "o efeito colateral nunca repete");
    assert_eq!(model.call_count(), 3, "tool + vazio + um lembrete no mesmo contexto");
}

/// A mesma retomada, mas a SEGUNDA tentativa recupera com texto de verdade — prova que o retry
/// continua sendo uma segunda CHANCE real, não só um caminho pro resumo de fallback.
#[tokio::test]
async fn a_turn_that_ran_a_tool_and_ended_empty_can_recover_with_real_text_on_retry() {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "silent_action_test_turn_2".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(FixedTimeTool),
    );

    let model = FakeModel::new(vec![
        vec![FakeChunk::ToolCall { id: "call-4", name: "silent_action_test_turn_2", args: json!({}) }],
        vec![], // fim vazio da 1ª tentativa
        vec![FakeChunk::Text("Prontinho!")],
    ]);
    let (result, _observer, _handle) = run(model, 6).await;

    assert_eq!(result.unwrap().text, "Prontinho!");
}

#[tokio::test]
async fn empty_text_with_no_tool_call_is_a_named_error_not_a_silent_empty_success() {
    let model = FakeModel::new(vec![vec![], vec![]]);
    let (result, _observer, _handle) = run(model.clone(), 6).await;

    assert!(matches!(result, Err(TurnError::EmptyOutput(ref names)) if names.is_empty()), "sem tool alguma nas duas tentativas, continua erro nomeado");
    assert_eq!(model.call_count(), 2, "uma segunda tentativa, nunca um laço");
}

/// Mesmo helper de `run`, mas com `user_content` livre — os testes de `action_claim` precisam
/// de um pedido de ação real ("abre a calculadora"), não o "olá" fixo do helper padrão.
async fn run_with_user_content(model: FakeModel, user_content: &str, max_tool_steps: u32) -> (Result<TurnResult, TurnError>, TurnAbortHandle) {
    let (handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();
    let result = run_turn(model.model(), "system".to_string(), vec![], user_content, vec![], max_tool_steps, None, None, None, signal, &observer).await;
    (result, handle)
}

/// O achado real do smoke (item 6): "abre a calculadora" -> zero tool call, texto afirmando
/// que já abriu. UMA rodada corretiva força o modelo a chamar a tool de verdade — nunca vira
/// resposta final aceita como está.
#[tokio::test]
async fn an_untooled_action_claim_forces_one_corrective_round_before_the_final_answer() {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "calculator_test_turn".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(FixedTimeTool),
    );

    let model = FakeModel::new(vec![
        vec![FakeChunk::Text("Abrindo a calculadora agora!")], // claim, zero tool call
        vec![FakeChunk::ToolCall { id: "call-5", name: "calculator_test_turn", args: json!({}) }],
        vec![FakeChunk::Text("Prontinho, abri a calculadora pra você!")],
    ]);
    let (result, _handle) = run_with_user_content(model.clone(), "abre a calculadora", 6).await;

    let turn = result.unwrap();
    assert_eq!(turn.text, "Prontinho, abri a calculadora pra você!");
    assert_eq!(model.call_count(), 3, "1 claim rejeitado + 1 tool call (a rodada corretiva chama a tool de verdade) + 1 resposta final");
}

/// Blocker (revisão): o texto do claim REJEITADO nunca pode chegar ao observer como
/// `TextChunk` — é dali que `post_turn::assembler::TurnAssembler` monta o par gravado como
/// memória (concatenando todo `StreamChunk` entre um `ThinkingStart`/`ThinkingStop`), então um
/// vazamento aqui gravava "Abrindo a calculadora agora!Prontinho, abri a calculadora pra
/// você!" como memória — a mesma classe de "memória envenenada" que este item existe pra
/// prevenir, reaparecendo no PRÓPRIO caminho de sucesso do nudge.
#[tokio::test]
async fn a_rejected_claim_never_reaches_the_observer_as_a_text_chunk() {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "calculator_test_turn_no_leak".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(FixedTimeTool),
    );

    let model = FakeModel::new(vec![
        vec![FakeChunk::Text("Abrindo a calculadora agora!")], // claim, zero tool call
        vec![FakeChunk::ToolCall { id: "call-no-leak", name: "calculator_test_turn_no_leak", args: json!({}) }],
        vec![FakeChunk::Text("Prontinho, abri a calculadora pra você!")],
    ]);

    let (handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();
    let result =
        run_turn(model.model(), "system".to_string(), vec![], "abre a calculadora", vec![], 6, None, None, None, signal, &observer).await;
    drop(handle);

    let turn = result.unwrap();
    assert_eq!(turn.text, "Prontinho, abri a calculadora pra você!");

    let streamed_text: String = observer
        .events()
        .into_iter()
        .filter_map(|event| match event {
            TurnEvent::TextChunk(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(
        streamed_text, "Prontinho, abri a calculadora pra você!",
        "o claim rejeitado vazou pro observer — é dali que a memória lê o texto do turno"
    );
}

/// Major (revisão): um erro de provedor na rodada CORRETIVA (pós-nudge), antes de qualquer tool
/// call, tem de continuar elegível a failover — nada externo aconteceu ainda. Antes desta
/// correção, `steps.is_empty()` já estava `false` só pelo `TurnStep::default()` de contabilidade
/// que o próprio nudge empilha, classificando o erro como `Provider` (nunca refeito contra
/// outro provedor) mesmo sem nenhum efeito colateral real.
#[tokio::test]
async fn a_provider_error_on_the_corrective_round_before_any_tool_call_stays_eligible_for_failover() {
    let model = FakeModel::new(vec![vec![FakeChunk::Text("Abrindo a calculadora agora!")]]).failing_stream_at(1);
    let (result, _handle) = run_with_user_content(model, "abre a calculadora", 6).await;

    assert!(
        matches!(result, Err(TurnError::ProviderBeforeSideEffects(_))),
        "sem tool alguma rodada ainda, o erro na rodada corretiva tem de continuar seguro pro failover: {result:?}"
    );
}

/// Se o modelo repete o claim MESMO depois do nudge, o turno não vira um laço — mas o
/// texto final tem de continuar reconhecível como claim sem tool, pra
/// `post_turn::pipeline::should_record_turn` nunca gravar essa afirmação falsa em memória.
#[tokio::test]
async fn a_claim_that_repeats_after_the_nudge_is_marked_unresolved_never_a_silent_success() {
    let model = FakeModel::new(vec![
        vec![FakeChunk::Text("Abrindo a calculadora agora!")],
        vec![FakeChunk::Text("Já abri, pode conferir!")], // repete o claim, ainda sem tool
    ]);
    let (result, _handle) = run_with_user_content(model.clone(), "abre a calculadora", 6).await;

    let turn = result.unwrap();
    assert!(action_claim::claims_untooled_action("abre a calculadora", &turn.text), "claim repetido sem tool call tem de ficar reconhecível");
    assert_eq!(model.call_count(), 2, "só UMA rodada corretiva, nunca um laço mesmo quando o modelo insiste");
}

/// Pedido comum sem verbo de ação nenhum nunca aciona o nudge — prova que o wiring não
/// interfere numa conversa normal.
#[tokio::test]
async fn plain_conversation_never_triggers_the_action_claim_nudge() {
    let model = FakeModel::new(vec![vec![FakeChunk::Text("Oi, tudo bem?")]]);
    let (result, _handle) = run_with_user_content(model.clone(), "oi, tudo bem?", 6).await;

    assert_eq!(result.unwrap().text, "Oi, tudo bem?");
    assert_eq!(model.call_count(), 1, "sem pedido de ação, nenhuma rodada extra");
}

/// Porte de `retryEmptyOutput`: modelo local às vezes devolve vazio uma vez e responde na seguinte.
#[tokio::test]
async fn empty_output_is_retried_once_before_giving_up() {
    let model = FakeModel::new(vec![vec![], vec![FakeChunk::Text("Oi!")]]);
    let (result, _observer, _handle) = run(model.clone(), 6).await;

    assert_eq!(result.unwrap().text, "Oi!");
    assert_eq!(model.call_count(), 2);
}

// -- ProviderBeforeSideEffects: a classe segura para failover automático -----------------------

/// **MUST** (02-providers.md, failover automático): um erro de provedor na PRIMEIRA chamada —
/// antes de qualquer texto ou tool — é a classe segura para `run_turn_llm` repetir inteira
/// contra outro provedor, porque nada aconteceu ainda.
/// Failover que também falha não grava strike aqui — `chat::send` grava o mesmo erro, e dois
/// registros por turno desligavam o provedor com ~1,5 falha real em vez de 3.
#[test]
fn a_failed_failover_leaves_the_strike_to_the_caller() {
    let (provider, model) = ("failover-test-dup", "m-dup");
    let outcome = failover_outcome(Err(TurnError::Provider("ollama fora".into())), provider, model, "google 503".into());

    assert!(matches!(outcome, Err(TurnError::Provider(ref r)) if r == "google 503"));
    assert!(crate::modules::llm::provider_health::get(provider, model).is_none(), "o strike é do chamador, não daqui");
}

/// O usuário cancelou ENQUANTE o Ollama de socorro respondia: não é falha do provedor configurado
/// (nem do socorro) — virar `Provider(motivo do google)` mostrava "erro do provedor" no chat, publicava
/// `ChatError` e dava um strike por uma parada que o próprio usuário pediu.
#[test]
fn an_abort_during_the_failover_stays_an_abort() {
    let (provider, model) = ("failover-test-abort", "m-abort");
    let outcome = failover_outcome(Err(TurnError::Aborted), provider, model, "google 503".into());

    assert!(matches!(outcome, Err(TurnError::Aborted)), "{outcome:?}");
    assert!(crate::modules::llm::provider_health::get(provider, model).is_none());
}

#[test]
fn a_served_failover_records_the_configured_pair_once() {
    let (provider, model) = ("failover-test-ok", "m-ok");
    let served = TurnResult { text: "oi".into(), usage: Usage::new(), served_by_failover: false };
    let outcome = failover_outcome(Ok(served), provider, model, "google 503".into()).expect("socorro respondeu");

    assert!(outcome.served_by_failover);
    let entry = crate::modules::llm::provider_health::get(provider, model).expect("par configurado gravado");
    assert_eq!(entry.to_json()["failureCount"], json!(1));
}

/// B2: o 404 do OpenRouter "No endpoints found matching your data policy" é o filtro ZDR do usuário
/// funcionando (modelo sem endpoint de retenção zero), não um provedor quebrado: não conta strike,
/// senão três mensagens "desativam" o par e o banner some com a causa real.
#[test]
fn a_zdr_policy_mismatch_never_counts_against_the_provider_health() {
    let (provider, model) = ("failover-test-zdr", "m-zdr");
    let served = TurnResult { text: "oi".into(), usage: Usage::new(), served_by_failover: false };
    let reason = "HttpError: Invalid status code 404 Not Found with message: No endpoints found matching your data policy (Zero data retention).";
    let outcome = failover_outcome(Ok(served), provider, model, reason.into()).expect("socorro respondeu");

    assert!(outcome.served_by_failover, "o socorro local continua valendo");
    assert!(crate::modules::llm::provider_health::get(provider, model).is_none(), "404 de política de dados não é falha de saúde");
}

#[tokio::test]
async fn a_provider_error_on_the_very_first_call_is_before_any_side_effects() {
    let model = FakeModel::new(vec![]).failing_stream_at(0);
    let (result, _observer, _handle) = run(model, 6).await;

    assert!(matches!(result, Err(TurnError::ProviderBeforeSideEffects(_))), "esperava ProviderBeforeSideEffects, veio {result:?}");
}

/// Um erro de provedor DEPOIS de uma tool já ter executado (efeito colateral real) nunca é
/// `ProviderBeforeSideEffects` — repetir o turno inteiro duplicaria a tool.
#[tokio::test]
async fn a_provider_error_after_a_tool_call_already_ran_is_not_before_side_effects() {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "ran_once_test_turn".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(FixedTimeTool),
    );
    let model = FakeModel::new(vec![vec![FakeChunk::ToolCall { id: "call-1", name: "ran_once_test_turn", args: json!({}) }]]).failing_stream_at(1);
    let (result, _observer, _handle) = run(model, 6).await;

    assert!(matches!(result, Err(TurnError::Provider(_))), "esperava Provider (efeito colateral já aconteceu), veio {result:?}");
}

fn looping_tool(name: &str) -> ToolDefinition {
    register_test_tool(name, Arc::new(FixedTimeTool));
    ToolDefinition::new(ToolName::new(name).unwrap(), "", json!({ "type": "object" }))
}

fn call(id: &'static str, name: &'static str) -> Vec<FakeChunk> {
    vec![FakeChunk::ToolCall { id, name, args: json!({}) }]
}

async fn run_with_tools(model: FakeModel, tools: Vec<ToolDefinition>, max_tool_steps: u32) -> Result<TurnResult, TurnError> {
    let (_handle, signal) = TurnAbortHandle::new();
    run_turn(model.model(), "system".into(), vec![], "olá", tools, max_tool_steps, None, None, None, signal, &NullObserver).await
}

/// Spent budget after tools ran used to throw the whole turn away (the user saw an error for work
/// that had already happened). Now one tool-free round turns what the tools did into an answer.
#[tokio::test]
async fn spending_the_step_budget_ends_in_a_tool_free_wrap_up_not_an_error() {
    let tool = looping_tool("loop_forever_test_turn");
    let model = FakeModel::new(vec![call("a", "loop_forever_test_turn"), call("b", "loop_forever_test_turn"), vec![FakeChunk::Text("Fechei com o que consegui.")]]);
    let result = run_with_tools(model.clone(), vec![tool], 2).await;

    assert_eq!(result.unwrap().text, "Fechei com o que consegui.");
    assert_eq!(*model.tool_counts.lock().unwrap(), vec![1, 1, 0], "a rodada de fechamento anuncia zero tools");
}

/// The wrap-up is one round, not a second budget: a model that still calls tools gets the named error.
#[tokio::test]
async fn a_model_that_ignores_the_wrap_up_still_ends_in_the_named_error() {
    let tool = looping_tool("loop_forever_test_turn");
    let model = FakeModel::new(vec![call("a", "loop_forever_test_turn"), call("b", "loop_forever_test_turn"), call("c", "loop_forever_test_turn")]);
    let result = run_with_tools(model, vec![tool], 2).await;
    assert!(matches!(result, Err(TurnError::MaxToolStepsExceeded)), "{result:?}");
}

/// Three refusals in the approval dialog are not three failures of the tool.
#[tokio::test]
async fn user_refusals_never_block_the_tool() {
    register_test_tool("refused_test_turn", Arc::new(DeniedTool));
    let (_handle, signal) = TurnAbortHandle::new();
    for id in ["r1", "r2", "r3", "r4"] {
        let call = rig_core::message::ToolCall::from_wire(id, rig_core::message::ToolFunction::new(ToolName::new("refused_test_turn").unwrap(), json!({ "n": id })));
        tool_step::run(&[call], &signal, &NullObserver, &mut std::collections::HashMap::new(), Duration::from_secs(5)).await.unwrap();
    }
    assert!(!failure_tracker::failure_tracker().is_blocked("refused_test_turn"));
}

struct DeniedTool;

#[async_trait]
impl crate::modules::tools::ToolHandler for DeniedTool {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        Ok(json!({ "success": false, "error": "user_denied" }))
    }
}

/// O ponto central do abort: uma chamada ainda em voo é CANCELADA, não deixada terminar em
/// segundo plano. O modelo de teste atrasa cada fragmento; abortamos antes do segundo chegar e
/// provamos que o terceiro (que provaria uma "segunda resposta") nunca é consumido.
// Relógio virtual: o abort (15ms) cai SEMPRE entre o 1º e o 2º fragmento (40ms), em qualquer máquina.
#[tokio::test(start_paused = true)]
async fn aborting_mid_stream_cancels_the_call_instead_of_yielding_a_second_response() {
    let model = FakeModel::new(vec![vec![
        FakeChunk::Text("primeiro "),
        FakeChunk::Text("segundo "),
        FakeChunk::Text("terceiro"),
    ]])
    .with_delay(Duration::from_millis(40));

    let (handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();

    let turn = tokio::spawn(run_turn(
        model.model(),
        "system".to_string(),
        vec![],
        "olá",
        vec![],
        6,
        None,
        None,
        None,
        signal,
        // `&'static` seria necessário para atravessar `spawn`; um observer local descartável
        // evita a complicação — o que este teste prova é o resultado, não os eventos emitidos.
        &NullObserver,
    ));

    tokio::time::sleep(Duration::from_millis(15)).await;
    handle.abort();

    let result = turn.await.unwrap();
    assert!(matches!(result, Err(TurnError::Aborted)));

    // Só UMA chamada a `.stream()` aconteceu — o laço nunca reagiu ao abort tentando de novo,
    // o que seria a própria "segunda resposta para uma fala" que este teste existe para pegar.
    assert_eq!(model.call_count(), 1);

    // Dá tempo de qualquer poll perdido ainda em voo terminar, sem que isso produza mais
    // eventos depois do abort já observado.
    tokio::time::sleep(Duration::from_millis(80)).await;
    drop(observer);
}

/// Ponta a ponta do rastreador de falhas DENTRO do laço real: três chamadas seguidas da MESMA
/// tool falhando (mesmo registro/execução de verdade, sem dublê nessa parte) cruzam
/// `MAX_FAILURES_BEFORE_BLOCK` e a tool fica bloqueada — a mesma tool não pode mais ser chamada
/// (`is_blocked`) nem apareceria no catálogo do próximo passo (`run_turn` filtra por
/// `is_blocked` antes de cada `CompletionRequest`, ver o comentário acima do filtro).
#[tokio::test]
async fn three_failures_of_the_same_tool_block_it_via_the_real_registry() {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition {
            name: "always_fails_test_turn".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        Arc::new(AlwaysFailsTool),
    );

    let model = FakeModel::new(vec![
        vec![FakeChunk::ToolCall { id: "f1", name: "always_fails_test_turn", args: json!({}) }],
        vec![FakeChunk::ToolCall { id: "f2", name: "always_fails_test_turn", args: json!({}) }],
        vec![FakeChunk::ToolCall { id: "f3", name: "always_fails_test_turn", args: json!({}) }],
        vec![FakeChunk::Text("desisto.")],
    ]);
    let (result, _observer, _handle) = run(model, 6).await;

    assert_eq!(result.unwrap().text, "desisto.");
    assert!(failure_tracker::failure_tracker().is_blocked("always_fails_test_turn"), "3 falhas deveriam bloquear a tool");
}

struct AlwaysFailsTool;

#[async_trait]
impl crate::modules::tools::ToolHandler for AlwaysFailsTool {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        Err("falha simulada".to_string())
    }
}

// -- append_turn_context / monologue_enabled ------------------------------------------------

/// Mesma ordem final de `composeSystemPrompt` no TS: healing antes, monólogo depois.
#[test]
fn append_turn_context_orders_healing_before_monologue() {
    let out = append_turn_context("base".to_string(), "\n[SYSTEM] Blocked tools this session: x. Do NOT call them.", Some("vou provocar"));
    let healing_at = out.find("[SYSTEM] Blocked tools").unwrap();
    let monologue_at = out.find("## Internal Reasoning").unwrap();
    assert!(healing_at < monologue_at);
    assert!(out.starts_with("base\n[SYSTEM]"));
    assert!(out.ends_with("## Internal Reasoning\nvou provocar"));
}

#[test]
fn append_turn_context_is_a_no_op_when_both_are_empty() {
    assert_eq!(append_turn_context("base".to_string(), "", None), "base");
}

/// B1: a nota de crise é a ÚLTIMA instrução do prompt — depois de healing e do monólogo ("provoque",
/// "faça piada"), senão o modelo pequeno obedece o que veio por último e a nota fica soterrada.
#[test]
fn the_crisis_note_is_the_last_instruction_of_the_prompt() {
    let (store, _dir) = store();
    let healing = "\n[SYSTEM] Blocked tools this session: x. Do NOT call them.";
    let out = finish_system_prompt("base".to_string(), healing, Some("vou provocar"), "quero me matar", &store);
    let note_at = out.find("CRISIS NOTICE").expect("a nota tem de estar no prompt");
    assert!(note_at > out.find("## Internal Reasoning").unwrap() && note_at > out.find("[SYSTEM] Blocked tools").unwrap(), "{out}");
    assert!(out.trim_end().ends_with("Do not call tools in this reply."), "{out}");
    let calm = finish_system_prompt("base".to_string(), "", Some("vou provocar"), "bora jogar", &store);
    assert!(!calm.contains("CRISIS NOTICE") && calm.ends_with("vou provocar"));
}

#[test]
fn monologue_enabled_defaults_to_false_and_honors_the_config_flag() {
    let (store, _dir) = store();
    assert!(!monologue_enabled(&store), "llm.enableMonologue é false por padrão no schema");

    store.set("llm.enableMonologue", json!(true)).unwrap();
    assert!(monologue_enabled(&store));
}

// -- resolve_ollama_config -----------------------------------------------------------------

#[test]
fn resolve_ollama_config_falls_back_to_documented_defaults() {
    let (store, _dir) = store();
    let config = resolve_ollama_config(&store);

    assert_eq!(config.model, OLLAMA_DEFAULT_MODEL);
    assert_eq!(config.base_url, OLLAMA_DEFAULT_BASE_URL);
    assert_eq!(config.api_key, "");
    assert_eq!(config.num_ctx, OLLAMA_MAX_NUM_CTX);
    assert_eq!(config.max_tool_steps, DEFAULT_MAX_TOOL_STEPS);
    assert!(!config.enable_thinking, "llm.enableThinking é false por padrão no schema");
}

/// O núcleo do defeito desta rodada: `llm.enableThinking` salvo em Settings tem de mudar o
/// `additional_params` que sai para o Ollama — antes desta rodada o campo era lido em lugar
/// nenhum, e um modelo com capability `thinking` sempre pensava pelo default DELE, nunca pelo
/// do usuário.
#[test]
fn resolve_ollama_config_reads_enable_thinking_from_the_store() {
    let (store, _dir) = store();
    store.set("llm.enableThinking", json!(true)).unwrap();
    assert!(resolve_ollama_config(&store).enable_thinking);
}

// -- kv_cache_id_slot (item 6) ---------------------------------------------------------------

#[test]
fn kv_cache_id_slot_is_none_for_every_provider_except_openai_compatible() {
    for provider in ["google", "openrouter", "ollama"] {
        assert_eq!(kv_cache_id_slot(provider, PromptFamily::Chat), None, "provider: {provider}");
        assert_eq!(kv_cache_id_slot(provider, PromptFamily::Background), None, "provider: {provider}");
    }
}

#[test]
fn kv_cache_id_slot_picks_slot_zero_for_chat_and_slot_one_for_background() {
    assert_eq!(kv_cache_id_slot("openaiCompatible", PromptFamily::Chat), Some(0));
    assert_eq!(kv_cache_id_slot("openaiCompatible", PromptFamily::Background), Some(1));
}

// -- build_remote_additional_params ---------------------------------------------------------

fn unknown() -> crate::modules::llm::model_profile::ModelProfile {
    crate::modules::llm::model_profile::ModelProfile::default()
}

/// `enableThinking=false` precisa chegar ao llama.cpp/vLLM (Gemma 4 gastava ~300 tokens de
/// raciocínio num "oi" e vazava o rascunho na fala); o dialeto do template (`chat_template_kwargs`)
/// e o `id_slot` nunca vão para Gemini/OpenRouter, que recusam corpo desconhecido.
#[test]
fn remote_params_speak_each_provider_dialect_and_keep_llama_fields_out_of_the_others() {
    let (store, _dir) = store();
    let off = json!({ "top_p": 0.95, "chat_template_kwargs": { "enable_thinking": false }, "id_slot": 0 });
    assert_eq!(build_remote_additional_params("openaiCompatible", &store, PromptFamily::Chat, "gemma", &unknown()), Some(off));
    assert_eq!(build_remote_additional_params("google", &store, PromptFamily::Chat, "gemini-2.5-flash", &unknown()), Some(json!({ "generationConfig": { "topP": 0.95 } })));
    assert_eq!(build_remote_additional_params("openrouter", &store, PromptFamily::Chat, "x/y", &unknown()), Some(json!({ "provider": { "zdr": true, "data_collection": "deny" }, "top_p": 0.95 })));
}

/// Política ZDR do OpenRouter (PRIVACY.md §3): o catálogo já filtra `zdr=true`, mas a
/// política só vale se o CORPO do pedido de chat a carregar. Prova no fio: um OpenRouter real
/// (cliente `rig`) apontado para um servidor em loopback, sem rede externa, e o JSON que chega.
#[tokio::test]
async fn openrouter_chat_request_body_carries_the_zdr_and_no_data_collection_policy() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let wire = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut raw = Vec::new();
        let mut buf = [0u8; 8192];
        while let Ok(n) = stream.read(&mut buf) {
            raw.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&raw).to_string();
            let Some((head, body)) = text.split_once("\r\n\r\n") else { continue };
            let length = head.to_ascii_lowercase().split("content-length:").nth(1).and_then(|v| v.trim().split_whitespace().next().and_then(|n| n.parse::<usize>().ok())).unwrap_or(0);
            if body.len() >= length || n == 0 {
                break;
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
        String::from_utf8_lossy(&raw).split_once("\r\n\r\n").map(|(_, body)| body.to_string()).unwrap_or_default()
    });

    let (store, _dir) = store();
    let params = build_remote_additional_params("openrouter", &store, PromptFamily::Chat, "x/y", &unknown());
    // O MESMO wire que `build_remote_model` monta para `openrouter` (dialeto OpenRouter, Chat
    // Completions), só com a base apontada para o loopback.
    let model = rig_core::providers::openai::OpenAIConfig::with_key(&rig_core::providers::openai::wire::OPENROUTER, "chave-de-teste")
        .with_base_url(format!("http://{addr}"))
        .connect(rig_reqwest::ReqwestClient::from(crate::modules::llm::http_client().clone()))
        .chat("x/y");
    let _ = model.stream(CompletionRequest::new("oi").additional_params(params)).unwrap().finish().await;

    let body: Value = serde_json::from_str(&wire.join().unwrap()).expect("corpo JSON do pedido");
    assert_eq!(body["provider"], json!({ "zdr": true, "data_collection": "deny" }), "corpo: {body}");
}

/// `id_slot` é ortogonal a `enableThinking` — ainda vai no corpo mesmo com o thinking ligado
/// (o "default do servidor" vale só para `chat_template_kwargs`, nunca esvazia o `id_slot`).
#[test]
fn remote_params_still_carry_id_slot_when_thinking_is_left_enabled() {
    let (store, _dir) = store();
    store.set("llm.enableThinking", json!(true)).unwrap();
    assert_eq!(build_remote_additional_params("openaiCompatible", &store, PromptFamily::Chat, "gemma", &unknown()), Some(json!({ "top_p": 0.95, "id_slot": 0 })));
}

/// A família de prompt escolhe o slot — chat e background nunca disputam o mesmo `id_slot`.
#[test]
fn remote_params_use_the_slot_of_the_given_prompt_family() {
    let (store, _dir) = store();
    store.set("llm.enableThinking", json!(true)).unwrap();
    assert_eq!(build_remote_additional_params("openaiCompatible", &store, PromptFamily::Background, "gemma", &unknown()), Some(json!({ "top_p": 0.95, "id_slot": 1 })));
}

// -- build_ollama_additional_params ---------------------------------------------------------

/// Disabled thinking is always EXPLICIT (`think: false`, accepted by every model); enabled omits the
/// field, because `think: true` makes Ollama reject non-thinking models such as qwen2.5. Ver o cabeçalho de `build_ollama_additional_params` para o incidente reproduzido (thinking
/// consome `num_predict` inteiro, `content` sai vazio, `enableThinking: false` do usuário nunca
/// chegava a valer).
#[test]
fn build_ollama_additional_params_forces_thinking_off_but_never_forces_it_on() {
    let mut config = TurnLlmConfig {
        model: OLLAMA_DEFAULT_MODEL.to_string(),
        base_url: OLLAMA_DEFAULT_BASE_URL.to_string(),
        api_key: String::new(),
        temperature: None,
        max_tokens: None,
        num_ctx: OLLAMA_MAX_NUM_CTX,
        max_tool_steps: DEFAULT_MAX_TOOL_STEPS,
        enable_thinking: false,
        top_p: None,
    };
    assert_eq!(build_ollama_additional_params(&config), json!({ "num_ctx": OLLAMA_MAX_NUM_CTX, "think": false, "keep_alive": OLLAMA_KEEP_ALIVE }));

    config.enable_thinking = true;
    assert_eq!(build_ollama_additional_params(&config), json!({ "num_ctx": OLLAMA_MAX_NUM_CTX, "keep_alive": OLLAMA_KEEP_ALIVE }));

    config.top_p = Some(0.8);
    assert_eq!(build_ollama_additional_params(&config), json!({ "num_ctx": OLLAMA_MAX_NUM_CTX, "top_p": 0.8, "keep_alive": OLLAMA_KEEP_ALIVE }));
}

/// O aquecimento só serve se carregar o modelo do MESMO jeito que o turno vai pedir: um
/// `num_ctx` diferente faz o Ollama recarregar o runner no primeiro turno (o custo que o
/// aquecimento existia para tirar), e sem `keep_alive` o modelo sai da VRAM em 5 min.
#[test]
fn the_warmup_request_loads_the_model_with_the_turns_own_num_ctx_and_keep_alive() {
    let (store, _dir) = store();
    store.set("llm.contextWindow", json!(16384)).unwrap();
    let config = resolve_ollama_config(&store);
    let turn = build_ollama_additional_params(&config);
    let body = ollama_warmup_body(&config);
    assert_eq!(body["model"], json!(config.model));
    assert_eq!(body["options"]["num_ctx"], turn["num_ctx"]);
    assert_eq!(body["keep_alive"], turn["keep_alive"]);
    assert!(body.get("prompt").is_none(), "sem prompt o Ollama só carrega o modelo, não gera nada");
}

/// `llm.topP` e `llm.contextWindow` saídos da aba LLM chegam ao request do Ollama.
#[test]
fn resolve_ollama_config_reads_top_p_and_the_chosen_context_window() {
    let (store, _dir) = store();
    store.set("llm.topP", json!(0.8)).unwrap();
    store.set("llm.contextWindow", json!(65536)).unwrap();
    let config = resolve_ollama_config(&store);
    assert_eq!((config.top_p, config.num_ctx), (Some(0.8), 65536));
}

/// O contexto pedido nunca passa da janela de treino do modelo, e o teto de saída do provedor
/// corta `maxTokens` (acima dele é 400); sem perfil, nada muda.
#[test]
fn fit_to_model_caps_context_and_output_at_what_the_model_supports() {
    let profile = crate::modules::llm::model_profile::ModelProfile { context_window: Some(8192), max_output_tokens: Some(2048), ..Default::default() };
    let (mut num_ctx, mut max_tokens) = (32_768, Some(4096));
    fit_to_model(&mut num_ctx, &mut max_tokens, &profile);
    assert_eq!((num_ctx, max_tokens), (8192, Some(2048)));

    let (mut num_ctx, mut max_tokens) = (4096, Some(1024));
    fit_to_model(&mut num_ctx, &mut max_tokens, &profile);
    assert_eq!((num_ctx, max_tokens), (4096, Some(1024)), "pedido menor que o teto fica como está");

    let (mut num_ctx, mut max_tokens) = (32_768, Some(4096));
    fit_to_model(&mut num_ctx, &mut max_tokens, &unknown());
    assert_eq!((num_ctx, max_tokens), (32_768, Some(4096)));
}

#[test]
fn resolve_ollama_config_reads_overrides_from_the_store() {
    let (store, _dir) = store();
    store.set("llm.model", json!("qwen128k:latest")).unwrap();
    store.set("apiKeys.ollamaBaseUrl", json!("http://127.0.0.1:11500")).unwrap();
    store.set("llm.maxToolSteps", json!(3)).unwrap();

    let config = resolve_ollama_config(&store);
    assert_eq!(config.model, "qwen128k:latest");
    assert_eq!(config.base_url, "http://127.0.0.1:11500");
    assert_eq!(config.max_tool_steps, 3);
}

#[test]
fn the_model_picked_in_the_wizard_or_connection_panel_wins_over_llm_model() {
    let (store, _dir) = store();
    store.set("llm.model", json!("legacy")).unwrap();
    store.set("llm.ollamaModel", json!("qwen2.5:7b")).unwrap();
    assert_eq!(resolve_ollama_config(&store).model, "qwen2.5:7b");
}

/// A janela de contexto é sempre pedida explicitamente — nunca fica a critério do default do
/// Ollama (4096). Ver o porquê no cabeçalho do arquivo.
#[test]
fn resolve_ollama_config_always_requests_a_context_window() {
    let (store, _dir) = store();
    assert!(resolve_ollama_config(&store).num_ctx > 4096);
}

// -- run_turn_ollama: o chokepoint de privacidade nunca é contornável -------------------------

/// NEGATIVO — o caminho que o chokepoint existe para negar: modo offline ligado e endpoint
/// remoto (não-loopback) tem de falhar ANTES de qualquer tentativa de rede, com o motivo do
/// gate na mensagem — não um erro genérico de conexão recusada que pareceria "o servidor caiu".
#[tokio::test]
async fn run_turn_ollama_refuses_a_remote_endpoint_under_offline_mode_before_touching_the_network() {
    let (store, _dir) = store();
    store.set("privacy.offlineMode", json!(true)).unwrap();
    store.set("apiKeys.ollamaBaseUrl", json!("http://ollama-remoto.invalid:11434")).unwrap();

    let (_handle, signal) = TurnAbortHandle::new();
    let result = run_turn_ollama(&store, vec![], "olá", signal, &NullObserver).await;

    match result {
        Err(TurnError::Provider(message)) => assert!(message.contains("privacy.offlineMode")),
        other => panic!("esperava TurnError::Provider citando o chokepoint, veio {other:?}"),
    }
}

/// "Thinking" used to appear only after prompt composition (episode embedding, model profile,
/// monologue) — seconds of an avatar doing nothing after the user spoke. The turn announces it
/// the moment it is accepted, and a turn that dies before streaming still closes it.
#[tokio::test]
async fn a_chat_turn_announces_thinking_before_any_preparation_and_always_closes_it() {
    let (store, _dir) = store();
    store.set("privacy.offlineMode", json!(true)).unwrap();
    store.set("apiKeys.ollamaBaseUrl", json!("http://ollama-remoto.invalid:11434")).unwrap();

    let (_handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();
    let result = run_turn_llm(&store, vec![], "olá", signal, &observer).await;

    assert!(result.is_err(), "refused by the offline gate before composing the prompt");
    assert_eq!(observer.events(), vec![TurnEvent::ThinkingStart, TurnEvent::ThinkingStop]);
}

/// Mesmo chokepoint, caso positivo — sem chamar `run_turn_ollama` inteiro (que dispararia uma
/// requisição HTTP real para `localhost:11434` caso passasse do gate): testa só a fronteira que
/// `run_turn_ollama` percorre antes de qualquer rede, com o mesmo `base_url` que
/// `resolve_ollama_config` resolveria por padrão.
#[test]
fn the_privacy_gate_allows_the_default_ollama_base_url_even_under_offline_mode() {
    let (store, _dir) = store();
    store.set("privacy.offlineMode", json!(true)).unwrap();

    let config = resolve_ollama_config(&store);
    assert_eq!(config.base_url, OLLAMA_DEFAULT_BASE_URL);
    assert!(crate::modules::privacy::assert_network_allowed(&store, &config.base_url, "brain:turn:ollama").is_ok());
}

// -- run_turn_ollama_raw: geração tool-free de `content`/`collaboration` -----------------------

#[test]
fn build_raw_system_prompt_appends_the_task_section_only_when_given() {
    let (store, _dir) = store();

    let base = build_raw_system_prompt(&store, None).expect("system.md deveria existir via fallback de dev");
    assert!(!base.contains("## CURRENT TASK"));
    assert!(!base.contains("## Bond With The User"), "geração raw não injeta humor/vínculo, igual ao TS");

    // Antes esta asserção cobrava a tarefa no FIM do prompt, depois do "FINAL REMINDER" — a
    // aproximação do port antigo. No TS a tarefa é `dynamicContext`: entra antes do tail.
    let with_task = build_raw_system_prompt(&store, Some("resuma o dia")).unwrap();
    let task_at = with_task.find("## CURRENT TASK\nresuma o dia").expect("tarefa ausente");
    assert!(task_at < with_task.find("## CRITICAL STYLE RULES").unwrap());
    assert!(with_task.ends_with(base.rsplit("\n\n").next().unwrap()), "o tail continua fechando o prompt");
}

/// Mesmo chokepoint de [`run_turn_ollama`], mesma prova: `content`/`collaboration` nunca
/// alcançam rede sob modo offline com endpoint remoto, mesmo sendo uma fachada diferente.
#[tokio::test]
async fn run_turn_ollama_raw_refuses_a_remote_endpoint_under_offline_mode_before_touching_the_network() {
    let (store, _dir) = store();
    store.set("privacy.offlineMode", json!(true)).unwrap();
    store.set("apiKeys.ollamaBaseUrl", json!("http://ollama-remoto.invalid:11434")).unwrap();

    let (_handle, signal) = TurnAbortHandle::new();
    let result = run_turn_ollama_raw(&store, Some("tarefa"), "oi", signal, &NullObserver).await;

    match result {
        Err(TurnError::Provider(message)) => assert!(message.contains("privacy.offlineMode")),
        other => panic!("esperava TurnError::Provider citando o chokepoint, veio {other:?}"),
    }
}

// -- abort observado em todas as esperas do laço ----------------------------------------------

/// `changed()` retornava na hora com o sender dropado (`Err` engolido) e o `select! { biased; … }`
/// do laço de streaming girava sem consumir o stream. Handle dropado não é abort: nunca resolve.
#[tokio::test(start_paused = true)]
async fn changed_never_resolves_once_the_abort_handle_is_dropped() {
    let (handle, mut signal) = TurnAbortHandle::new();
    drop(handle);
    let resolved = tokio::time::timeout(Duration::from_millis(50), signal.changed()).await;
    assert!(resolved.is_err(), "handle dropado sem abortar não pode acordar o select do laço");
}

#[tokio::test]
async fn the_stream_is_consumed_to_the_end_even_when_the_abort_handle_was_dropped() {
    let model = FakeModel::new(vec![vec![FakeChunk::Text("Oi, "), FakeChunk::Text("tudo "), FakeChunk::Text("bem?")]]).with_delay(Duration::from_millis(5));
    let (handle, signal) = TurnAbortHandle::new();
    drop(handle);
    let observer = CollectingObserver::new();
    let run = run_turn(model.model(), "system".to_string(), vec![], "olá", vec![], 6, None, None, None, signal, &observer);
    let result = tokio::time::timeout(Duration::from_secs(5), run).await.expect("o laço não pode girar sem consumir o stream");
    assert_eq!(result.unwrap().text, "Oi, tudo bem?");
}

struct HangsForeverTool;

#[async_trait]
impl crate::modules::tools::ToolHandler for HangsForeverTool {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        std::future::pending().await
    }
}

fn register_test_tool(name: &str, handler: Arc<dyn crate::modules::tools::ToolHandler>) {
    crate::modules::tools::registry().register(
        crate::modules::tools::ToolDefinition { name: name.into(), description: None, input_schema: json!({ "type": "object" }), plugin_id: "test".into(), requires_window: false },
        handler,
    );
}

/// "Parar" durante uma tool travada tem de encerrar o turno: antes, `registry().execute(..).await`
/// não olhava o abort e o turno ficava preso até a tool (nunca) voltar.
#[tokio::test]
async fn aborting_while_a_tool_hangs_ends_the_turn_instead_of_waiting_for_the_tool() {
    register_test_tool("hangs_test_turn", Arc::new(HangsForeverTool));
    let model = FakeModel::new(vec![vec![FakeChunk::ToolCall { id: "h1", name: "hangs_test_turn", args: json!({}) }]]);
    let (handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        handle.abort();
    });

    let run = run_turn(model.model(), "system".to_string(), vec![], "olá", vec![], 6, None, None, None, signal, &observer);
    let result = tokio::time::timeout(Duration::from_secs(5), run).await.expect("o abort tem de soltar o turno da tool travada");

    assert!(matches!(result, Err(TurnError::Aborted)), "{result:?}");
    assert_eq!(observer.events().last(), Some(&TurnEvent::ThinkingStop));
}

struct PanickingTool;

#[async_trait]
impl crate::modules::tools::ToolHandler for PanickingTool {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        panic!("tool quebrada de propósito")
    }
}

/// Um panic dentro de uma tool vira resultado de tool com falha: o modelo recebe o erro, o turno
/// continua e o HUD sai do "pensando" (`ThinkingStop`).
#[tokio::test]
async fn a_panicking_tool_fails_the_call_but_the_turn_continues_and_stops_thinking() {
    register_test_tool("panics_test_turn", Arc::new(PanickingTool));
    let model = FakeModel::new(vec![
        vec![FakeChunk::ToolCall { id: "p1", name: "panics_test_turn", args: json!({}) }],
        vec![FakeChunk::Text("A tool quebrou, mas sigo aqui.")],
    ]);
    let (result, observer, _handle) = run(model, 6).await;

    assert_eq!(result.unwrap().text, "A tool quebrou, mas sigo aqui.");
    let events = observer.events();
    assert!(events.iter().any(|e| matches!(e, TurnEvent::ToolCallFinished { name, ok: false, result, .. }
        if name == "panics_test_turn" && result["error"].as_str().is_some_and(|m| m.contains("tool panicked")))), "{events:?}");
    assert_eq!(events.last(), Some(&TurnEvent::ThinkingStop));
}

/// Tool que passa do teto: a chamada falha com `timeout` (o modelo lê o erro e o turno segue), em
/// vez de prender o turno para sempre.
#[tokio::test]
async fn a_tool_that_outlives_the_limit_fails_the_call_with_a_timeout() {
    register_test_tool("times_out_test_turn", Arc::new(HangsForeverTool));
    let call = rig_core::message::ToolCall::from_wire("t1", rig_core::message::ToolFunction::new(ToolName::new("times_out_test_turn").unwrap(), json!({})));
    let (_handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();

    let step = tool_step::run(&[call], &signal, &observer, &mut std::collections::HashMap::new(), Duration::from_millis(50)).await.expect("timeout é falha de tool, não do turno");

    assert_eq!(step.executed, vec!["times_out_test_turn".to_string()]);
    let events = observer.events();
    assert!(events.iter().any(|e| matches!(e, TurnEvent::ToolCallFinished { ok: false, result, .. }
        if result["error"].as_str().is_some_and(|m| m.starts_with("timeout:")))), "{events:?}");
}

/// O abort larga a task de uma tool só quando largar é inofensivo: tool `destructive` e ação que o
/// broker pergunta ao usuário (risco por AÇÃO — `clear_episodic`, `habits clear`, `export_data`) rodam
/// até o fim; consultar não.
#[test]
fn an_abort_never_drops_a_call_the_broker_gates_but_may_drop_a_plain_read() {
    for (name, args) in [
        ("memory_console", json!({ "action": "clear_episodic" })),
        ("person_catalog", json!({ "action": "delete" })),
        ("habits", json!({ "action": "clear" })),
        ("export_data", json!({})),
        ("desktop", json!({ "action": "launch", "target": "calc" })),
    ] {
        assert!(crate::modules::tools::registry().must_finish_once_started(name, &args), "{name} {args}: largar no meio deixa efeito parcial");
    }
    for (name, args) in [("memory_console", json!({ "action": "search" })), ("habits", json!({ "action": "get" })), ("web", json!({ "action": "search", "query": "tempo" }))] {
        assert!(!crate::modules::tools::registry().must_finish_once_started(name, &args), "{name} {args}: leitura é segura de largar");
    }
}

/// O erro cru do provedor chega ao chat, ao log e ao HUD pelo `Display`: a URL do Gemini carrega
/// `?key=<segredo>` e não pode aparecer em nenhum deles.
#[test]
fn a_provider_error_never_displays_the_api_key_from_the_request_url() {
    let raw = "HttpError: error sending request for url (https://generativelanguage.googleapis.com/v1beta/models/m:streamGenerateContent?key=SEGREDO123&alt=sse)";
    for err in [TurnError::Provider(raw.to_string()), TurnError::ProviderBeforeSideEffects(raw.to_string())] {
        let shown = err.to_string();
        assert!(!shown.contains("SEGREDO123"), "{shown}");
        assert!(shown.contains("generativelanguage.googleapis.com"), "o resto do diagnóstico continua: {shown}");
    }
}

// -- provedores reais contra um servidor em loopback ------------------------------------------------
//
// Prova no fio do que a migração para o `Model<Wire, Transport>` do `rig-core` mudou de lugar: o
// caminho, os cabeçalhos e o corpo que cada provedor recebe, e o stream (texto, tool call em
// fragmentos, uso) que volta. Sem rede externa: o "provedor" é um `TcpListener` de roteiro.

/// Um servidor HTTP/1.1 que responde cada conexão com o próximo `(content-type, corpo)` do roteiro e
/// devolve, por pedido, `(linha de requisição + cabeçalhos em minúsculas, corpo)`.
fn scripted_server(responses: Vec<(&'static str, String)>) -> (std::net::SocketAddr, std::thread::JoinHandle<Vec<(String, String)>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for (content_type, reply) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut raw = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                raw.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&raw).to_string();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let length = head.to_ascii_lowercase().split("content-length:").nth(1).and_then(|v| v.split_whitespace().next().and_then(|n| n.parse::<usize>().ok())).unwrap_or(0);
                    if body.len() >= length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&raw).to_string();
            let (head, body) = text.split_once("\r\n\r\n").map(|(h, b)| (h.to_ascii_lowercase(), b.to_string())).unwrap_or_default();
            seen.push((head, body));
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
            let _ = stream.write_all(response.as_bytes());
        }
        seen
    });
    (addr, handle)
}

fn sse(chunks: &[Value]) -> String {
    chunks.iter().map(|chunk| format!("data: {chunk}\n\n")).chain(std::iter::once("data: [DONE]\n\n".to_string())).collect()
}

fn chat_chunk(delta: Value, finish: Option<&str>) -> Value {
    json!({ "id": "c1", "object": "chat.completion.chunk", "created": 1, "model": "m", "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }] })
}

fn usage_chunk(prompt: u64, completion: u64) -> Value {
    json!({ "id": "c1", "object": "chat.completion.chunk", "created": 1, "model": "m", "choices": [], "usage": { "prompt_tokens": prompt, "completion_tokens": completion, "total_tokens": prompt + completion } })
}

/// O caminho de produção inteiro para `openaiCompatible`: `build_remote_model` (guarda SSRF incluída)
/// -> `run_turn` -> Chat Completions em streaming. Uma tool call que chega em FRAGMENTOS de
/// argumentos, executada pelo registro real, o resultado devolvido ao modelo na rodada seguinte, e o
/// uso das duas rodadas somado. É a prova do formato de stream que o `FakeModel` (que já fala em
/// `StreamEvent`) não alcança.
#[tokio::test]
async fn openai_compatible_streams_text_and_fragmented_tool_calls_over_the_real_wire() {
    register_test_tool("wire_time_test_turn", Arc::new(FixedTimeTool));
    let first = sse(&[
        chat_chunk(json!({ "role": "assistant", "tool_calls": [{ "index": 0, "id": "call_1", "type": "function", "function": { "name": "wire_time_test_turn", "arguments": "" } }] }), None),
        chat_chunk(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "{\"tz\":" } }] }), None),
        chat_chunk(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "\"UTC\"}" } }] }), None),
        chat_chunk(json!({}), Some("tool_calls")),
        usage_chunk(10, 5),
    ]);
    let second = sse(&[chat_chunk(json!({ "role": "assistant", "content": "São " }), None), chat_chunk(json!({ "content": "10h." }), None), chat_chunk(json!({}), Some("stop")), usage_chunk(20, 3)]);
    let (addr, server) = scripted_server(vec![("text/event-stream", first), ("text/event-stream", second)]);

    let (store, _dir) = store();
    store.set("llm.provider", json!("openaiCompatible")).unwrap();
    store.set("apiKeys.openaiCompatibleBaseUrl", json!(format!("http://{addr}/v1"))).unwrap();
    store.set("apiKeys.openaiCompatibleApiKey", json!("chave-de-teste")).unwrap();
    let config = super::super::llm_provider::resolve_remote_config(&store, "openaiCompatible");
    let model = super::super::llm_provider::build_remote_model(&store, "openaiCompatible", &config).unwrap();
    assert!(!format!("{model:?}").contains("chave-de-teste"), "o Debug do modelo nunca carrega a chave");

    let tool = ToolDefinition::new(ToolName::new("wire_time_test_turn").unwrap(), "hora", json!({ "type": "object" }));
    let (_handle, signal) = TurnAbortHandle::new();
    let observer = CollectingObserver::new();
    let params = Some(json!({ "top_p": 0.9, "chat_template_kwargs": { "enable_thinking": false }, "id_slot": 0 }));
    let result = run_turn(model, "system".into(), vec![], "que horas são?", vec![tool], 6, Some(0.4), Some(256), params, signal, &observer).await.unwrap();

    assert_eq!(result.text, "São 10h.");
    assert_eq!((result.usage.input_tokens, result.usage.output_tokens), (Some(30), Some(8)), "o uso das duas rodadas soma");
    assert!(observer.events().iter().any(|e| matches!(e, TurnEvent::ToolCallFinished { name, ok: true, .. } if name == "wire_time_test_turn")));

    let seen = server.join().unwrap();
    let (head, body) = &seen[0];
    assert!(head.starts_with("post /v1/chat/completions "), "{head}");
    assert!(head.contains("authorization: bearer chave-de-teste"), "{head}");
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "gpt-5.4-mini");
    assert_eq!(body["stream"], true);
    assert_eq!(body["temperature"], 0.4);
    assert!(body["max_completion_tokens"] == 256 || body["max_tokens"] == 256, "teto de saída: {body}");
    assert_eq!((&body["top_p"], &body["chat_template_kwargs"], &body["id_slot"]), (&json!(0.9), &json!({ "enable_thinking": false }), &json!(0)), "parâmetros extras no topo do corpo: {body}");
    assert_eq!(body["messages"][0]["role"], "system");
    assert!(body["messages"][0]["content"].to_string().contains("system"), "o preâmbulo vai como a primeira mensagem system: {body}");
    assert_eq!(body["tools"][0]["function"]["name"], "wire_time_test_turn");

    // A segunda rodada devolve ao modelo a call (com os argumentos já inteiros) e o resultado da tool.
    let second_body: Value = serde_json::from_str(&seen[1].1).unwrap();
    let messages = second_body["messages"].as_array().unwrap();
    let assistant = messages.iter().find(|m| m["role"] == "assistant").expect("a call do modelo volta no histórico");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "wire_time_test_turn");
    assert_eq!(serde_json::from_str::<Value>(assistant["tool_calls"][0]["function"]["arguments"].as_str().unwrap()).unwrap(), json!({ "tz": "UTC" }));
    let tool_message = messages.iter().find(|m| m["role"] == "tool").expect("o resultado da tool volta ao modelo");
    assert!(tool_message["content"].as_str().unwrap().contains("10:00"), "{tool_message}");
}

/// Ollama pelo `/api/chat` nativo: `think`/`keep_alive` no topo, `num_ctx`/`top_p`/temperatura/teto em
/// `options` — a divisão que `build_ollama_additional_params` pressupõe — e SEM `Authorization` quando
/// não há chave. O stream é NDJSON, com o uso no registro `done`.
#[tokio::test]
async fn ollama_native_chat_carries_think_keep_alive_and_options_and_streams_ndjson() {
    let ndjson = [
        json!({ "model": "m", "created_at": "2026-01-01T00:00:00Z", "message": { "role": "assistant", "content": "Oi, " }, "done": false }),
        json!({ "model": "m", "created_at": "2026-01-01T00:00:01Z", "message": { "role": "assistant", "content": "tudo bem?" }, "done": false }),
        json!({ "model": "m", "created_at": "2026-01-01T00:00:02Z", "message": { "role": "assistant", "content": "" }, "done": true, "done_reason": "stop", "prompt_eval_count": 12, "eval_count": 4 }),
    ]
    .iter()
    .map(|line| format!("{line}\n"))
    .collect::<String>();
    let (addr, server) = scripted_server(vec![("application/x-ndjson", ndjson)]);

    let config = TurnLlmConfig {
        model: "qwen3.5:4b".into(),
        base_url: format!("http://{addr}"),
        api_key: String::new(),
        temperature: Some(0.5),
        max_tokens: Some(128),
        num_ctx: 8192,
        max_tool_steps: 6,
        top_p: Some(0.9),
        enable_thinking: false,
    };
    let (_handle, signal) = TurnAbortHandle::new();
    let result = run_turn(build_ollama_model(&config), "system".into(), vec![], "oi", vec![], 6, config.temperature, config.max_tokens, Some(build_ollama_additional_params(&config)), signal, &NullObserver).await.unwrap();
    assert_eq!(result.text, "Oi, tudo bem?");
    assert_eq!((result.usage.input_tokens, result.usage.output_tokens), (Some(12), Some(4)));

    let seen = server.join().unwrap();
    let (head, body) = &seen[0];
    assert!(head.starts_with("post /api/chat "), "{head}");
    assert!(!head.contains("authorization"), "sem chave configurada o cabeçalho não vai: {head}");
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!((&body["think"], &body["keep_alive"], &body["stream"]), (&json!(false), &json!(OLLAMA_KEEP_ALIVE), &json!(true)), "{body}");
    assert_eq!(body["options"], json!({ "num_ctx": 8192, "top_p": 0.9, "temperature": 0.5, "num_predict": 128 }), "{body}");
}
