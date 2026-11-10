//! O laço de um turno de conversa. Port de `brain/services/chat.service.ts::runSendMessageTurn` +
//! `chat-turn.runtime.ts` + `chat/chat-executor.ts`/`tool-loop/service.ts` (o laço multi-turn de
//! ferramentas que `executeLoop` delega a `toolLoop.runStream`).
//!
//! ## O que este laço faz, de ponta a ponta
//!
//! 1. Compõe o system prompt via `brain::persona::compose_system_prompt` — o `system.md` rico
//!    (15 placeholders) + humor/vínculo injetados, portado nesta rodada.
//! 2. Resolve provedor/modelo/credencial do `ConfigStore` do processo (`brain::llm_provider`) e passa
//!    pelo chokepoint de privacidade (`privacy::assert_network_allowed`) ANTES de qualquer
//!    requisição de rede, exatamente como `modules/llm.rs` já faz para o catálogo.
//! 3. Chama o provedor via `rig-core` em streaming (`DynModel::stream`), emitindo um
//!    [`TurnEvent::TextChunk`] por fragmento de texto — o equivalente local ao `STREAM_CHUNK` que o
//!    HUD escuta no EventBus TypeScript (ainda não portado; ver [`TurnObserver`]).
//! 4. Quando o modelo pede uma tool, executa via `tools::registry().execute(name, args)` (que já
//!    valida o argumento contra o JSON Schema antes do handler), devolve o resultado ao modelo como
//!    próxima mensagem e repete — até uma resposta sem tool call ou até `max_tool_steps`.
//! 5. Cada esboço/erro é honesto: sem tool alguma registrada, o catálogo enviado ao provedor é
//!    vazio (não inventa tool); falha de provedor devolve erro nomeado, não resposta fingida.
//!
//! ## O que este laço NÃO faz (e quem faz)
//!
//! - **Roteamento de modelo e teto de custo.** `enableRouting`/`maxCostUsd` do TS não existem em
//!   Rust: o turno usa o provedor/modelo configurados (`llm.provider`). O CUSTO é contado depois,
//!   por `chat::send` (`token_usage::cost_for` sobre [`TurnResult::usage`]), sem barrar o turno.
//! - **Provedores.** [`run_turn_llm`] resolve qualquer provedor do catálogo via `brain::llm_provider`
//!   e cai para Ollama só em falha ANTES de qualquer efeito colateral (`ProviderBeforeSideEffects`).
//! - **Histórico e pós-turno** moram fora daqui: `chat::send` passa o `history` do `HistoryStore`, e
//!   memória episódica + reflexão rodam em `brain::post_turn` (listener assinado ao fim do turno).
//! - **Persona/prompt.** `brain::persona` preenche os placeholders do `system.md`; o `.md` chega por
//!   `chat::load_prompt_no_handle` (ver o cabeçalho de `chat/prompt.rs`).
//! - **Retry em saída vazia.** [`run_turn`] refaz uma vez quando o texto final vem vazio e nenhuma
//!   tool rodou; a segunda vazia sobe `TurnError::EmptyOutput`.
//! - **Self-healing e monólogo.** [`failure_tracker`] (3 falhas seguidas da mesma tool a bloqueiam
//!   por 5 min, fora do catálogo a partir do passo seguinte) e [`monologue`] (`llm.enableMonologue`,
//!   default desligado, mesmo provedor/modelo do turno). Ver os cabeçalhos de cada um.
//!
//! ## Abort: por que corre em `select!`, não só numa flag lida entre itens
//!
//! `throwIfAborted` (`chat-turn-guard.ts`) documenta o defeito real que já aconteceu aqui: um abort
//! que só é observado DEPOIS que o laço de ferramentas "termina normalmente" deixa o modelo
//! continuar gerando, o texto interrompido vira resposta e o próximo turno responde de novo à
//! mesma pergunta — duas respostas para uma fala. Por isso [`run_turn`] corre a leitura do stream
//! (`stream.next()`) contra o sinal de abort em `tokio::select!`: no instante em que alguém chama
//! [`TurnAbortHandle::abort`], a próxima volta do laço vê a flag, chama
//! `drop(stream)` (largar o `Streamed` cancela a leitura e a conexão em curso) e devolve
//! `Err(TurnError::Aborted)` — nunca uma resposta parcial
//! travestida de sucesso. O mesmo sinal é checado de novo entre a execução de cada tool: abortar
//! enquanto uma tool está rodando não cancela a tool em si (já teve efeito colateral), mas impede
//! que o laço peça mais uma rodada ao provedor depois disso.

use crate::config::schemas::llm_defaults;
use futures::StreamExt;
use rig_core::completion::{CompletionRequest, Message, ToolDefinition, Usage};
use rig_core::message::{AssistantContent, ToolChoice, ToolName};
use rig_core::streaming::{Item, StreamEvent};
use serde_json::{json, Value};
use tokio::sync::watch;

use crate::config::store::ConfigStore;
use crate::modules::brain::llm_provider::ChatModel;
use crate::modules::brain::persona;

// `pub(crate)`: `post_turn::pipeline` reuses the same pure detector to decide whether a turn is
// memory-poisoned (an unresolved action claim) before storing it — see the wiring there.
pub(crate) mod action_claim;
mod effect_guard;
mod failure_tracker;
mod llm_gate;
mod monologue;
mod narration_filter;
mod tag_loop;
mod tool_step;

/// Porte literal do texto pedido pra tarefa — nudge de UMA rodada corretiva quando o turno afirma
/// uma ação sem tool call (ver [`action_claim`] e o uso em [`run_turn_once`]).
/// Both nudges are marked as internal: read as the user speaking, the model apologized to the user
/// for "esquecer de apertar o botão" (smoke 2026-09-26).
const ACTION_CLAIM_NUDGE: &str =
    "[INTERNAL SYSTEM NOTICE — not the user; do not comment on this notice or apologize] You said you did (or will do) an action but called no tool. Call the tool now or say clearly that you cannot.";

/// Blank or only emotion tags: nothing to say or show (a lone `<|THINKING|>` once ended a turn).
fn is_unspeakable(text: &str) -> bool {
    crate::modules::voice::strip_emotion_tags(text).trim().is_empty()
}

/// Step budget spent after tools ran: one last tool-free round so the user gets an answer built on
/// what the tools already did, instead of an error that throws that work away.
const WRAP_UP_NUDGE: &str =
    "[INTERNAL SYSTEM NOTICE — not the user; do not comment on this notice] The tool-call limit for this turn is reached. Reply to the user now, in short text, using only the tool results above. Do not call any tool.";

const EMPTY_AFTER_TOOLS_NUDGE: &str =
    "[INTERNAL SYSTEM NOTICE — not the user; do not comment on this notice] You replied with nothing. Reply to the user now, in short text, based on the tool results above — without calling them again.";

/// Último `system_prompt`/`user_content` que saiu para um provedor de verdade, estado de TURNO em
/// memória — nunca persistido, nunca reconstruído do disco. Porte de `getLastPromptTrace()`
/// (`brain` local no TS), consumido por `tools::memory_console::snapshot` e exibido no painel
/// "Último Prompt" das configurações. Fica em processo (não por `ConfigStore`) pelo mesmo motivo do
/// original: é dado de depuração do turno em curso, não configuração do usuário.
mod prompt_trace {
    use serde_json::{json, Value};
    use std::sync::{Mutex, OnceLock};

    fn state() -> &'static Mutex<Option<Value>> {
        static STATE: OnceLock<Mutex<Option<Value>>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(None))
    }

    pub(super) fn record(system_prompt: &str, user_content: &str) {
        let trace = json!({ "systemPrompt": system_prompt, "userContent": user_content });
        if let Ok(mut guard) = state().lock() {
            *guard = Some(trace);
        }
    }

    /// `pub(crate)`: `tools::memory_console` (irmão de `brain`, não descendente de `turn`) precisa
    /// ler daqui.
    pub(crate) fn last() -> Option<Value> {
        state().lock().ok().and_then(|guard| guard.clone())
    }
}

pub(crate) use prompt_trace::last as last_prompt_trace;

const OLLAMA_DEFAULT_BASE_URL: &str = llm_defaults::OLLAMA_BASE_URL;
const OLLAMA_DEFAULT_MODEL: &str = llm_defaults::OLLAMA_MODEL;

/// Teto de contexto pedido explicitamente ao Ollama. Mesma constante e mesmo motivo do TypeScript
/// (`packages/llm/src/llm-language-model.ts::OLLAMA_MAX_NUM_CTX`): sem `num_ctx` no request, o
/// Ollama assume 4096 — e o prompt real desta app mede ~12,3k tokens, o que já causou 400 silencioso
/// e desativação do provedor. `modules/llm.rs::fetch_ollama_capabilities` sabe a janela REAL de cada
/// modelo (via `/api/show`), mas chamá-lo a cada turno custaria uma requisição extra por mensagem;
/// este módulo pede sempre o teto, que é o mesmo fallback que o TS usa quando o catálogo não foi
/// consultado ainda.
const OLLAMA_MAX_NUM_CTX: u64 = 32_768;

/// Mesmo default do schema real (`config/schemas/llm.rs::llm()`, campo `maxToolSteps`) — usado só
/// como piso defensivo se o `ConfigStore` alguma vez devolver `None` para um campo com default
/// declarado (não deveria acontecer; `ConfigStore::get` já mescla defaults do schema).
const DEFAULT_MAX_TOOL_STEPS: u32 = 15;

// === Abort ======================================================================================

/// Lado de quem PODE abortar um turno em curso. Ver o porquê do `select!` no cabeçalho do arquivo.
pub struct TurnAbortHandle {
    aborted: watch::Sender<bool>,
}

/// Lado de quem OBSERVA o abort dentro do laço — clonável porque `run_turn` precisa de uma cópia
/// própria para correr em `select!` sem disputar `&mut` com quem detém o handle.
#[derive(Clone)]
pub struct TurnAbortSignal {
    aborted: watch::Receiver<bool>,
}

impl TurnAbortHandle {
    pub fn new() -> (Self, TurnAbortSignal) {
        let (tx, rx) = watch::channel(false);
        (Self { aborted: tx }, TurnAbortSignal { aborted: rx })
    }

    /// Idempotente: abortar duas vezes não é erro, só não muda nada na segunda.
    pub fn abort(&self) {
        let _ = self.aborted.send(true);
    }
}

impl TurnAbortSignal {
    pub fn is_aborted(&self) -> bool {
        *self.aborted.borrow()
    }

    /// Espera o sinal mudar. Uma vez abortado ele nunca volta a `false`, então uma segunda espera
    /// nesta mesma cópia nunca resolve de novo — inofensivo, porque `run_turn` sempre confere
    /// `is_aborted()` no TOPO do laço antes de voltar a esperar (nunca só dentro do `select!`).
    /// Handle descartado (`Err`) não é mudança: resolver aqui fazia o `select!` do laço girar sem
    /// nunca consumir o stream — como em `wait_aborted`, nunca resolve.
    async fn changed(&mut self) {
        if self.aborted.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }

    /// Resolve só quando abortado. Handle descartado sem abortar = nunca resolve (não é abort).
    async fn wait_aborted(&mut self) {
        if self.aborted.wait_for(|aborted| *aborted).await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

// === Eventos que o HUD escuta ===================================================================

/// Equivalente local aos eventos `THINKING_START`/`STREAM_CHUNK`/`THINKING_STOP` do EventBus
/// TypeScript (`packages/sdk/src/svc/events.ts`, ainda não portado). Quem reivindicar o canal IPC
/// de chat pluga um [`TurnObserver`] que publica no EventBus real — este módulo não conhece IPC nem
/// EventBus, de propósito (ver "Escopo desta rodada" no cabeçalho: `brain` não reivindica canal).
#[derive(Debug, Clone, PartialEq)]
pub enum TurnEvent {
    ThinkingStart,
    TextChunk(String),
    ToolCallStarted { name: String },
    /// `result` e `duration_ms` vão ao renderer: `match_effect` só aplica o efeito lendo o resultado.
    /// `action` é o `args.action` da chamada (só o `effect_guard` lê: leitura e efeito dividem a tool).
    ToolCallFinished { name: String, action: Option<String>, ok: bool, result: serde_json::Value, duration_ms: u64 },
    ThinkingStop,
}

pub trait TurnObserver: Send + Sync {
    fn emit(&self, event: TurnEvent);
}

/// Observer nulo — turno sem HUD acoplado (testes, uso programático).
pub struct NullObserver;

impl TurnObserver for NullObserver {
    fn emit(&self, _event: TurnEvent) {}
}

/// Announces `ThinkingStart` the moment a turn is accepted — before prompt composition (episode
/// embedding, model profile, monologue), which used to leave the avatar idle for seconds after the
/// user spoke — and keeps the thinking state paired from there: a repeated start or stop from the
/// loop underneath collapses, and [`Self::finish`] closes a turn that died before streaming.
struct ThinkingAnnouncer<'a> {
    inner: &'a dyn TurnObserver,
    thinking: std::sync::atomic::AtomicBool,
}

impl<'a> ThinkingAnnouncer<'a> {
    fn start(inner: &'a dyn TurnObserver) -> Self {
        inner.emit(TurnEvent::ThinkingStart);
        Self { inner, thinking: std::sync::atomic::AtomicBool::new(true) }
    }

    fn finish(&self) {
        if self.thinking.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.inner.emit(TurnEvent::ThinkingStop);
        }
    }
}

impl TurnObserver for ThinkingAnnouncer<'_> {
    fn emit(&self, event: TurnEvent) {
        let forward = match event {
            TurnEvent::ThinkingStart => !self.thinking.swap(true, std::sync::atomic::Ordering::SeqCst),
            TurnEvent::ThinkingStop => self.thinking.swap(false, std::sync::atomic::Ordering::SeqCst),
            _ => true,
        };
        if forward {
            self.inner.emit(event);
        }
    }
}

// === Config ======================================================================================

#[derive(Debug, Clone)]
pub struct TurnLlmConfig {
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
    pub num_ctx: u64,
    pub max_tool_steps: u32,
    /// `llm.topP` — vai em `options.top_p` (o `rig` põe tudo que não é `think`/`keep_alive` lá).
    pub top_p: Option<f64>,
    /// `llm.enableThinking` — ver [`resolve_ollama_config`] e o cabeçalho de
    /// `build_ollama_additional_params` para o incidente que este campo existe para fechar.
    pub enable_thinking: bool,
}

/// Duplica a leitura de `ConfigKey` que `modules/llm.rs` já faz para o catálogo Ollama
/// (`CFG_OLLAMA_BASE_URL`/`CFG_OLLAMA_API_KEY`, privadas àquele arquivo), mais `llm.temperature`/
/// `llm.maxTokens`/`llm.maxToolSteps`, que nenhum módulo porta ainda. Mesmo precedente já aceito no
/// repo para duplicar leitura pequena através de um limite de módulo que não pode mudar nesta
/// rodada (`chat.rs::prompts_root` cita o mesmo motivo sobre `health.rs`).
///
/// Modelo: `llm.ollamaModel`, depois `llm.model`, depois o default — `readModelForProvider` do TS
/// (`llm-factory-config.ts`). O wizard e o painel de conexão gravam `llm.ollamaModel`; ler só
/// `llm.model` ignorava o modelo escolhido ali.
///
/// `llm.maxTokens` é `float_range` no schema (`z.number()` sem `.int()`), não inteiro — por isso
/// `.as_f64()`, nunca `.as_u64()`: um valor como `4096.0` no JSON não decodifica como inteiro em
/// `serde_json`, e `.as_u64()` devolveria `None` para o próprio default do schema.
fn resolve_ollama_config(store: &ConfigStore) -> TurnLlmConfig {
    let get_str = |path: &str| store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty());

    TurnLlmConfig {
        model: get_str("llm.ollamaModel").or_else(|| get_str("llm.model")).unwrap_or_else(|| OLLAMA_DEFAULT_MODEL.to_string()),
        base_url: get_str("apiKeys.ollamaBaseUrl").unwrap_or_else(|| OLLAMA_DEFAULT_BASE_URL.to_string()),
        api_key: get_str("apiKeys.ollamaApiKey").unwrap_or_default(),
        temperature: store.get("llm.temperature").and_then(|v| v.as_f64()),
        max_tokens: store.get("llm.maxTokens").and_then(|v| v.as_f64()).map(|n| n as u64),
        num_ctx: store.get("llm.contextWindow").and_then(|v| v.as_u64()).filter(|&n| n > 0).unwrap_or(OLLAMA_MAX_NUM_CTX),
        max_tool_steps: store
            .get("llm.maxToolSteps")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MAX_TOOL_STEPS),
        // Mesmo default do schema (`config/schemas/llm.rs::llm()`, `enableThinking` = `false`) —
        // ver `build_ollama_additional_params` para o porquê de nunca deixar isto implícito.
        enable_thinking: store.get("llm.enableThinking").and_then(|v| v.as_bool()).unwrap_or(false),
        top_p: store.get("llm.topP").and_then(|v| v.as_f64()),
    }
}

/// Encaixa o pedido no que o MODELO aguenta: `num_ctx` acima da janela de treino degrada a saída,
/// e `max_tokens` acima do teto de saída do provedor é 400. Perfil desconhecido não mexe em nada.
fn fit_to_model(num_ctx: &mut u64, max_tokens: &mut Option<u64>, profile: &crate::modules::llm::model_profile::ModelProfile) {
    if let Some(window) = profile.context_window {
        *num_ctx = (*num_ctx).min(window);
    }
    if let (Some(requested), Some(ceiling)) = (*max_tokens, profile.max_output_tokens) {
        *max_tokens = Some(requested.min(ceiling));
    }
}

pub(super) async fn resolve_ollama_config_for_model(store: &ConfigStore) -> TurnLlmConfig {
    let mut config = resolve_ollama_config(store);
    let profile = crate::modules::llm::model_profile::resolve(store, "ollama", &config.model).await;
    fit_to_model(&mut config.num_ctx, &mut config.max_tokens, &profile);
    config
}

/// `additional_params` do turno Ollama — pura e testável sem `ConfigStore`/rede, extraída de
/// [`run_turn_ollama`]/[`run_turn_ollama_raw`] para que o campo `think` não fique só coberto por um
/// teste de ponta a ponta contra rede real.
///
/// **O incidente que isto fecha.** Até esta rodada, `additional_params` só levava `num_ctx` — o
/// campo `think` nunca era enviado ao Ollama, então `llm.enableThinking` (exposto em Settings,
/// default `false` no schema) era pura decoração: o Ollama decidia sozinho se o modelo pensava,
/// pelo default DELE, não pelo do usuário. Para um modelo com capability `thinking` (ex.
/// `qwen3`/`qwen3.5`, incluindo o import GGUF que este app testou), isso mede: `num_predict`
/// (`llm.maxTokens`, default 4096) é o teto de tokens da resposta INTEIRA — pensamento + texto
/// final compartilham o mesmo orçamento. Um prompt real (`system.md` rico, ~3,5k tokens só de
/// preâmbulo, mais o catálogo de tools) empurra o modelo a "pensar" mais para decidir se chama uma
/// tool; reproduzido contra o Ollama local deste repo (`qwen3`-family, quant IQ1_S) com
/// `num_predict: 300`: `done_reason: "length"`, `thinking` com 1261 caracteres, `content` VAZIO — o
/// texto final nunca chegou a ser gerado. `run_turn` já trata isso honesto
/// (`TurnError::EmptyOutput`, nunca sucesso vazio), mas o usuário simplesmente não recebe resposta
/// nenhuma, turno após turno, com `enableThinking: false` salvo em Settings sem efeito nenhum.
fn build_ollama_additional_params(config: &TurnLlmConfig) -> Value {
    // Enabled → omit `think`: Ollama already lets thinking models think by default, and an explicit
    // `think: true` is a hard error ("does not support thinking") on models without that capability.
    let mut params = json!({ "num_ctx": config.num_ctx, "keep_alive": OLLAMA_KEEP_ALIVE });
    if !config.enable_thinking {
        params["think"] = json!(false);
    }
    if let Some(top_p) = config.top_p {
        params["top_p"] = json!(top_p);
    }
    params
}

/// Quanto o Ollama segura o modelo na memória depois do último pedido. O default dele (5 min)
/// descarrega o modelo numa pausa comum de jogo, e o turno seguinte paga a recarga inteira (segundos)
/// antes do primeiro token. 30 min, não "para sempre": num PC de jogo a VRAM volta para o jogo
/// quando a conversa realmente parou.
pub(crate) const OLLAMA_KEEP_ALIVE: &str = "30m";

/// `POST /api/generate` sem `prompt`: o Ollama só carrega o modelo e responde, sem gerar nada. Com o
/// MESMO `num_ctx`/`keep_alive` do turno — outro `num_ctx` faria o primeiro turno recarregar o runner.
pub(super) fn ollama_warmup_body(config: &TurnLlmConfig) -> Value {
    json!({ "model": config.model, "keep_alive": OLLAMA_KEEP_ALIVE, "options": { "num_ctx": config.num_ctx } })
}

fn build_ollama_model(config: &TurnLlmConfig) -> ChatModel {
    super::llm_provider::build_ollama_model(&config.base_url, &config.api_key, &config.model)
}

/// O `system.md` rico (`brain::persona::compose_system_prompt`), FALHA ALTA se o `.md` não for
/// encontrado — nunca degrada para um prompt vazio ou parcial em silêncio (mesmo contrato de
/// `load_prompt_no_handle`, ver o cabeçalho de `chat/prompt.rs`). `dynamic_context` (jogo ativo,
/// tela atual, gíria, entre outras fontes) é montado por `persona::compose_system_prompt`, não
/// aqui — este arquivo não tem contexto de jogo/tela PRÓPRIO; desde esta rodada, a tela chega ao
/// chat regular via `persona::screen_context` (fecha a lacuna que a antiga
/// `PersonaState.screen_context` deixava, ver o cabeçalho daquele arquivo), atrás das mesmas guardas
/// de visão/privacidade/frescor documentadas lá.
async fn compose_system_prompt(store: &ConfigStore, user_content: &str) -> Result<String, TurnError> {
    // A nota de crise (`crisis.rs`) NÃO entra aqui: vai por último, em [`finish_system_prompt`].
    persona::compose_system_prompt(store, user_content).await.map_err(TurnError::Provider)
}

/// Passo final da composição: healing/monólogo ([`append_turn_context`]), a nota de idioma da
/// mensagem (`reply_language.rs`, só quando ela foi escrita noutro idioma) e, por ÚLTIMO, a nota de
/// crise (`crisis.rs`). Última instrução do prompt é a que o modelo pequeno mais obedece — com o
/// monólogo ("provoque", "faça piada") depois dela, a nota ficava soterrada.
fn finish_system_prompt(prompt: String, healing_ctx: &str, monologue: Option<&str>, user_content: &str, store: &ConfigStore) -> String {
    let mut prompt = append_turn_context(prompt, healing_ctx, monologue);
    if let Some(note) = super::reply_language::note(store, user_content) {
        prompt.push_str("\n\n");
        prompt.push_str(&note);
    }
    super::crisis::with_note(prompt, user_content, &crate::config::locale::app_locale(store))
}

/// Anexa healing/monólogo ao prompt já composto — pura e testável sem rede nem `ConfigStore`: quem
/// chama já resolveu os dois fora daqui (healing é leitura síncrona de um `Mutex` de processo;
/// monólogo é uma chamada de LLM async, ver `monologue.rs`). Mesma ordem final de
/// `turn-context.ts::composeSystemPrompt`: prompt base (que já traz humor/vínculo, injetados DENTRO
/// de `persona::compose_system_prompt`) -> healing -> "## Internal Reasoning".
fn append_turn_context(mut prompt: String, healing_ctx: &str, monologue: Option<&str>) -> String {
    if !healing_ctx.is_empty() {
        prompt.push_str(healing_ctx);
    }
    if let Some(text) = monologue {
        prompt.push_str(&format!("\n\n## Internal Reasoning\n{text}"));
    }
    prompt
}

/// Espelha `llmFactory.isMonologueEnabled()` (`ConfigKey.LlmEnableMonologue` = `llm.enableMonologue`,
/// default `false` — ver o cabeçalho de `monologue.rs`).
fn monologue_enabled(store: &ConfigStore) -> bool {
    store.get("llm.enableMonologue").and_then(|v| v.as_bool()).unwrap_or(false)
}

// === Resultado e erro ===========================================================================

#[derive(Debug, Clone, Default)]
struct TurnStep {
    /// Nomes das tools executadas nesta rodada, na ordem em que o modelo pediu.
    pub tool_calls: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TurnResult {
    pub text: String,
    pub usage: Usage,
    /// `true` só quando [`run_turn_llm`] respondeu via failover para Ollama depois do provedor
    /// CONFIGURADO ter falhado (`TurnError::ProviderBeforeSideEffects`) — nunca setado por
    /// [`run_turn`] diretamente (sempre `false` na construção). `chat::send::handle_chat_send`
    /// consulta isto para NÃO marcar o par configurado como saudável: o usuário recebeu resposta,
    /// mas o provedor que ele escolheu continua quebrado, e mascarar isso violaria o próprio
    /// mecanismo de saúde (02-providers.md "MUST: ollama é o alvo final do failover" +
    /// 03-resilience.md).
    pub served_by_failover: bool,
}

#[derive(Debug)]
pub enum TurnError {
    /// `TurnAbortHandle::abort` foi chamado durante a geração ou entre execuções de tool.
    Aborted,
    /// Erro do provedor (rede, autenticação, resposta malformada) ou do cliente `rig-core`, DEPOIS
    /// de algum efeito colateral já ter acontecido neste turno (texto já emitido ao observer e/ou
    /// tool já executada) — nunca seguro para failover automático (repetir duplicaria o efeito).
    Provider(String),
    /// Mesmo erro de [`Self::Provider`], mas ANTES de qualquer efeito colateral: nenhum texto saiu,
    /// nenhuma tool rodou. É a ÚNICA classe segura para `run_turn_llm` tentar de novo inteira contra
    /// outro provedor (02-providers.md, "MUST: `ollama` é o alvo final do failover") — repetir o
    /// turno do zero não duplica nada, porque nada aconteceu ainda.
    ProviderBeforeSideEffects(String),
    /// Resposta final sem texto — nunca devolvido como sucesso vazio, MESMO quando uma ou mais
    /// tools rodaram nesta tentativa (a lista de nomes viaja aqui, `[]` quando nenhuma tool
    /// executou): sem isto, `run_turn`'s retry-once não tinha como resumir "rodei X mas não veio
    /// texto" na segunda tentativa também vazia — via `Ok(vec![])`, sucesso mudo por até 140s
    /// (achado real, ver o relatório da tarefa).
    EmptyOutput(Vec<String>),
    /// `max_tool_steps` esgotado sem uma resposta final (o modelo continuava pedindo tools).
    MaxToolStepsExceeded,
}

impl std::fmt::Display for TurnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aborted => write!(f, "turno abortado"),
            // Redigido aqui, no `Display`: a URL de requisição do Gemini carrega `?key=<segredo>` e este
            // texto chega ao chat, ao HUD e aos logs.
            Self::Provider(err) | Self::ProviderBeforeSideEffects(err) => write!(f, "erro do provedor: {}", crate::modules::system::log_redact::redact(err)),
            Self::EmptyOutput(tools) if tools.is_empty() => write!(f, "saída vazia do modelo (sem texto e sem tool)"),
            Self::EmptyOutput(tools) => write!(f, "saída vazia do modelo depois de rodar: {}", tools.join(", ")),
            Self::MaxToolStepsExceeded => write!(f, "limite de passos de tool excedido sem resposta final"),
        }
    }
}

impl std::error::Error for TurnError {}

// === O laço =====================================================================================

/// Núcleo testável do laço: recebe um [`ChatModel`] já montado (qualquer `Model<Wire, Transport>` do
/// `rig-core` apagado em `DynModel`), sem tocar `ConfigStore` nem rede — quem chama já resolveu
/// provedor/credencial e montou o prompt. Testes deste arquivo passam um modelo de mentira (dublê de
/// teste é prática sancionada por `AGENTS.md` — "zero mock" é sobre o CAMINHO DE PRODUÇÃO, não
/// sobre dublê em teste).
///
/// Saída vazia sem tool nenhuma é refeita UMA vez (porte de `retryEmptyOutput`): o abort é checado
/// de novo no início da segunda tentativa. Um turno que rodou tool já teve sua segunda chance no
/// próprio contexto (`run_turn_once`) e nunca é refeito — isso repetiria a tool; vazio mesmo assim,
/// [`empty_output_fallback`] devolve um resumo curto. Só um turno que nunca rodou tool alguma, nas
/// duas tentativas, ainda vira `Err(EmptyOutput)` de fato (nada pra resumir).
pub async fn run_turn(
    model: ChatModel,
    system_prompt: String,
    history: Vec<Message>,
    user_content: &str,
    tools: Vec<ToolDefinition>,
    max_tool_steps: u32,
    temperature: Option<f64>,
    max_tokens: Option<u64>,
    additional_params: Option<Value>,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    let first = run_turn_once(
        model.clone(),
        system_prompt.clone(),
        history.clone(),
        user_content,
        tools.clone(),
        max_tool_steps,
        temperature,
        max_tokens,
        additional_params.clone(),
        abort.clone(),
        observer,
    )
    .await;
    // A turn that ran tools already had its in-context retry; rerunning it would repeat them.
    let first = match first {
        Err(TurnError::EmptyOutput(tool_names)) if !tool_names.is_empty() => return Ok(empty_output_fallback(tool_names)),
        other => other,
    };
    if !matches!(first, Err(TurnError::EmptyOutput(_))) {
        return first;
    }
    log::warn!("brain:turn: saída vazia do modelo, tentando mais uma vez");
    let second =
        run_turn_once(model, system_prompt, history, user_content, tools, max_tool_steps, temperature, max_tokens, additional_params, abort, observer)
            .await;
    match second {
        Err(TurnError::EmptyOutput(tool_names)) if !tool_names.is_empty() => Ok(empty_output_fallback(tool_names)),
        other => other,
    }
}

/// Frase curta em pt-BR pra quando o modelo rodou tool(s) mas não formulou nenhum texto final —
/// nunca "" (o achado real: silêncio de sucesso por até 140s depois da 3ª chamada de
/// `vision_capture`). Não afirma sucesso: a tool pode ter falhado ou sido negada na aprovação
/// (smoke 2026-09-26: "Rodei code" depois de duas negações) — só nomeia o que foi tentado.
fn empty_output_fallback(tool_names: Vec<String>) -> TurnResult {
    log::warn!("brain:turn: saída vazia depois de {tool_names:?} — resumindo em texto em vez de devolver vazio");
    TurnResult {
        text: format!("Tentei usar {} mas não consegui montar uma resposta em texto agora — pode perguntar de novo?", tool_names.join(", ")),
        usage: Usage::new(),
        served_by_failover: false,
    }
}

async fn run_turn_once(
    model: ChatModel,
    system_prompt: String,
    history: Vec<Message>,
    user_content: &str,
    tools: Vec<ToolDefinition>,
    max_tool_steps: u32,
    temperature: Option<f64>,
    max_tokens: Option<u64>,
    additional_params: Option<Value>,
    mut abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    if abort.is_aborted() {
        return Err(TurnError::Aborted);
    }

    observer.emit(TurnEvent::ThinkingStart);

    // Observabilidade (specs/10-observability.md, tier humano — `info` cai em "human" por nível,
    // ver `log_buffer::audience_for`): tempo até o primeiro token e ms totais + nº de tools no fim.
    // Nunca o CONTEÚDO do turno na linha, só números.
    let turn_started = std::time::Instant::now();
    // Mesmos números, agora também no anel que a aba LLM mostra (`llm::turn_metrics`).
    crate::modules::llm::turn_metrics::begin_turn();
    let mut first_token_logged = false;

    let mut messages = history;
    messages.push(Message::user(user_content));

    let mut steps: Vec<TurnStep> = Vec::new();
    let mut total_usage = Usage::new();
    // Trava de UMA rodada corretiva por turno — ver o bloco `action_claim` abaixo.
    let mut action_claim_nudge_used = false;
    let mut empty_after_tools_nudge_used = false;
    let mut wrap_up_started = false;
    let mut force_tool_call = false;
    let mut succeeded_calls: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    // Revisão (blocker): `steps` deixa de estar vazio assim que o nudge empilha o
    // `TurnStep::default()` de contabilidade (linha abaixo), mesmo sem NENHUM efeito colateral real
    // ter acontecido — `steps.is_empty()` sozinho não serve mais pra decidir se um erro de provedor
    // é seguro pra failover (`TurnError::ProviderBeforeSideEffects`). Rastreado à parte, só vira
    // `true` quando uma tool de verdade é executada.
    let mut any_tool_ran = false;
    // Revisão (major): só a PRIMEIRA rodada (antes do nudge) pode virar um claim descartado — e só
    // quando o pedido do usuário torna isso possível (`action_claim::claims_untooled_action` exige
    // as duas metades). Fora desse caso estreito, o texto continua saindo em tempo real como
    // sempre; computado uma vez, `user_content` não muda dentro do turno.
    let action_requested = action_claim::user_requested_an_action(user_content);

    let final_text = loop {
        if steps.len() as u32 >= max_tool_steps {
            if !any_tool_ran || wrap_up_started {
                observer.emit(TurnEvent::ThinkingStop);
                return Err(TurnError::MaxToolStepsExceeded);
            }
            wrap_up_started = true;
            messages.push(Message::user(WRAP_UP_NUDGE));
        }

        // Tools bloqueadas (3+ falhas seguidas, ver `failure_tracker.rs`) somem do catálogo enviado
        // ao provedor NESTE passo — o modelo não pode nem tentar chamar de novo o que já falhou
        // repetido. Filtrado a cada volta (não uma vez fora do laço) porque o bloqueio pode
        // acontecer NO MEIO do turno, no passo anterior deste mesmo laço.
        let active_tools: Vec<ToolDefinition> = if wrap_up_started {
            Vec::new()
        } else {
            tools.iter().filter(|t| !failure_tracker::failure_tracker().is_blocked(&t.name)).cloned().collect()
        };

        // `preamble` entra como a PRIMEIRA mensagem `system` da conversa; um resumo rolante de
        // `HistoryStore::to_turn_history` vem logo depois, e os provedores juntam as `system` iniciais.
        let mut request = CompletionRequest::from(messages.clone())
            .preamble(system_prompt.clone())
            .tools(active_tools)
            .temperature(temperature)
            .max_tokens(max_tokens)
            .additional_params(additional_params.clone());
        // The text nudge alone got answered as conversation ("vacilei… vou mandar agora") with still
        // no call; OpenAI-compatible servers honor this, Ollama ignores it with a warning.
        if std::mem::take(&mut force_tool_call) && !tools.is_empty() {
            request = request.tool_choice(ToolChoice::Required);
        }

        // Abrir o stream só codifica o pedido; rede e HTTP falham no PRIMEIRO item (tratado abaixo,
        // com a mesma regra de "antes de qualquer efeito colateral").
        let mut stream = match model.stream(request) {
            Ok(stream) => stream,
            Err(err) => {
                observer.emit(TurnEvent::ThinkingStop);
                // Nenhuma TOOL rodou ainda neste turno -> nenhum efeito colateral externo -> seguro
                // para `run_turn_llm` tentar de novo inteiro contra outro provedor. `any_tool_ran`,
                // não `steps.is_empty()` (revisão: o nudge de `action_claim` empilha um `TurnStep`
                // de contabilidade sem NENHUMA tool ter rodado — ver o comentário na declaração).
                return Err(if !any_tool_ran { TurnError::ProviderBeforeSideEffects(err.to_string()) } else { TurnError::Provider(err.to_string()) });
            }
        };

        let mut step_text = String::new();
        let mut step_tool_calls = Vec::new();
        // Fresh per step (per call to the model): a leaked reasoning paragraph in the FOLLOW-UP
        // step after a tool result is just this step's own leading paragraph, caught the same way —
        // see narration_filter.rs's header for why state never carries across steps.
        let mut narration = narration_filter::NarrationFilter::new();
        let mut tag_loop = tag_loop::TagLoopGuard::default();
        // Revisão (blocker): só a rodada que ainda PODE virar um claim descartado (primeira rodada,
        // nudge ainda não usado, e o usuário de fato pediu uma ação) segura o texto localmente —
        // sem isto, `TurnEvent::TextChunk` já tinha saído para o observer/barramento ANTES desta
        // rodada ser julgada, e é dali que `post_turn::assembler::TurnAssembler` monta o texto
        // gravado como memória: um claim rejeitado + a resposta final da rodada seguinte viravam
        // uma única string concatenada, gravada como memória mesmo o claim tendo sido corrigido
        // (`used_tools=true` deixa passar `should_record_turn`). Toda outra rodada continua saindo
        // em tempo real, sem esta rodada segurar nada.
        let hold_text = action_requested && steps.is_empty() && !action_claim_nudge_used;

        loop {
            if abort.is_aborted() {
                // Largar o stream cancela a leitura e a conexão (sai de escopo no `return`).
                observer.emit(TurnEvent::ThinkingStop);
                return Err(TurnError::Aborted);
            }

            let item = tokio::select! {
                biased;
                _ = abort.changed() => continue,
                item = stream.next() => item,
            };

            let Some(item) = item else { break };
            if !first_token_logged && matches!(item, Ok(Item::Event(StreamEvent::Text { .. })) | Ok(Item::Event(StreamEvent::End { content: AssistantContent::ToolCall(_), .. }))) {
                first_token_logged = true;
                let first_token_ms = turn_started.elapsed().as_millis() as u64;
                log::info!("turno: primeiro token em {first_token_ms}ms");
                crate::modules::llm::turn_metrics::record(crate::modules::llm::turn_metrics::Mark::FirstToken, first_token_ms);
            }
            match item {
                Ok(Item::Event(StreamEvent::Text { text, .. })) => {
                    let spoken = narration.push(&text);
                    step_text.push_str(&spoken);
                    if !spoken.is_empty() && !hold_text {
                        observer.emit(TurnEvent::TextChunk(spoken));
                    }
                    if tag_loop.push(&text) {
                        log::warn!("turno: stream degenerou em tags de emoção repetidas — cortado");
                        break;
                    }
                }
                // A tool call só vale quando a parte TERMINA: aí os argumentos já são o JSON inteiro.
                Ok(Item::Event(StreamEvent::End { content: AssistantContent::ToolCall(tool_call), .. })) => {
                    step_tool_calls.push(tool_call);
                }
                // Início/fragmentos de parte, raciocínio e itens não modelados não mudam a decisão do
                // laço (falar vs. chamar tool); o texto e as calls completos chegam em `Text`/`End`.
                Ok(_) => {}
                Err(err) => {
                    observer.emit(TurnEvent::ThinkingStop);
                    let no_side_effects_yet = !any_tool_ran && step_text.is_empty() && step_tool_calls.is_empty();
                    return Err(if no_side_effects_yet { TurnError::ProviderBeforeSideEffects(err.to_string()) } else { TurnError::Provider(err.to_string()) });
                }
            }
        }

        // O uso fecha com o fim do provedor (`Finish`); num stream cortado acima (`tag_loop`) esse fim
        // nunca chegou e o uso fica zerado, como a `StreamFinal` ausente já ficava. Depois disto o
        // stream é largado, o que fecha a conexão se o corte deixou resposta pendente.
        total_usage += stream.partial().usage;
        drop(stream);

        let trailing = narration.finish();
        step_text.push_str(&trailing);
        if !trailing.is_empty() && !hold_text {
            observer.emit(TurnEvent::TextChunk(trailing));
        }

        if step_tool_calls.is_empty() {
            // Achado real (smoke item 6): "abre a calculadora" -> zero tool call, texto afirmando
            // que já abriu (a calculadora nunca abriu). UMA rodada corretiva, só na 1ª vez neste
            // turno — nunca um laço. Com `hold_text` o texto desta rodada NUNCA saiu como
            // `TextChunk` (ver a declaração acima) — nem falado, nem gravável em memória — então
            // descartar aqui é `continue` puro, sem nenhum "desfazer" a mais.
            if steps.is_empty() && !action_claim_nudge_used && action_claim::claims_untooled_action(user_content, &step_text) {
                action_claim_nudge_used = true;
                steps.push(TurnStep::default());
                messages.push(Message::from(vec![AssistantContent::text(step_text)]));
                messages.push(Message::user(ACTION_CLAIM_NUDGE));
                force_tool_call = true;
                continue;
            }
            // Empty after a tool ran: one reminder in this same context, never a rerun of the turn
            // (a rerun called the tool again — the side effect repeated).
            if is_unspeakable(&step_text) && any_tool_ran && !empty_after_tools_nudge_used {
                empty_after_tools_nudge_used = true;
                messages.push(Message::user(EMPTY_AFTER_TOOLS_NUDGE));
                continue;
            }
            // Resposta final de verdade — se o texto ficou retido (`hold_text`), este é o momento
            // em que ele sai, de uma vez (a rodada não vai ser descartada).
            if hold_text && !step_text.is_empty() {
                observer.emit(TurnEvent::TextChunk(step_text.clone()));
            }
            steps.push(TurnStep::default());
            break step_text;
        }

        // A tool call during the tool-free wrap-up round is the model ignoring the notice: no answer.
        if wrap_up_started {
            observer.emit(TurnEvent::ThinkingStop);
            return Err(TurnError::MaxToolStepsExceeded);
        }

        // Rodada com tool call de verdade — efeito colateral externo prestes a acontecer. Nunca
        // mais elegível para failover automático (ver `any_tool_ran` acima), e qualquer texto retido
        // desta MESMA rodada (ex.: "abre a calculadora" -> tool call na 1ª tentativa) sai agora,
        // antes do `ToolCallStarted` — mesma ordem que `voice::auto_speak` já espera (texto antes da
        // tool descarregar a fala pendente).
        any_tool_ran = true;
        if hold_text && !step_text.is_empty() {
            observer.emit(TurnEvent::TextChunk(step_text.clone()));
        }

        let mut assistant_content = Vec::new();
        if !step_text.trim().is_empty() {
            assistant_content.push(AssistantContent::text(step_text));
        }
        let executed = tool_step::run(&step_tool_calls, &abort, observer, &mut succeeded_calls, tool_step::TOOL_CALL_TIMEOUT).await?;
        assistant_content.extend(executed.calls);
        messages.push(Message::from(assistant_content));
        messages.push(Message::from(executed.results));
        steps.push(TurnStep { tool_calls: executed.executed });
    };

    observer.emit(TurnEvent::ThinkingStop);

    let tool_count: usize = steps.iter().map(|step| step.tool_calls.len()).sum();
    let total_ms = turn_started.elapsed().as_millis() as u64;
    log::info!("turno: {total_ms}ms total, {tool_count} tool(s)");
    crate::modules::llm::turn_metrics::record(crate::modules::llm::turn_metrics::Mark::Total, total_ms);

    // A call written as text never ran and is not speech: it is neither shown nor stored.
    let final_text = crate::modules::voice::tts::tool_call_text::strip_tool_call_text(&final_text).trim().to_string();
    if is_unspeakable(&final_text) {
        // Nomes na ordem em que rodaram, achatados de todos os passos — é só o que
        // `empty_output_fallback` (chamado por `run_turn`) precisa pra resumir "tentei usar X" sem
        // propagar silêncio como sucesso.
        let tool_names = steps.iter().flat_map(|step| step.tool_calls.clone()).collect();
        return Err(TurnError::EmptyOutput(tool_names));
    }

    // A claim still unresolved here is kept out of memory by `post_turn::pipeline::should_record_turn`.
    Ok(TurnResult { text: final_text, usage: total_usage, served_by_failover: false })
}

/// O catálogo do registro compartilhado no formato do provedor, em ordem estável: `get_all` sai de um
/// `HashMap`, e a ordem das tools muda o prefixo do prompt — turno a turno o KV-cache do servidor
/// (llama.cpp `id_slot`, Ollama) invalidava no catálogo inteiro sem nenhuma tool ter mudado.
fn chat_tool_definitions() -> Vec<ToolDefinition> {
    let mut items = crate::modules::tools::registry().get_all();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    // `ToolName` recusa nome vazio: uma tool sem nome não existe para o modelo (e não tem como ser chamada).
    items
        .into_iter()
        .filter_map(|item| Some(ToolDefinition::new(ToolName::new(item.name).ok()?, item.description.unwrap_or_default(), item.input_schema)))
        .collect()
}

/// Fachada real: resolve config Ollama do `ConfigStore` do processo, passa pelo chokepoint de
/// privacidade ANTES de qualquer requisição, compõe o prompt via
/// [`compose_system_prompt`] (o `system.md` rico + humor/vínculo), monta o
/// catálogo de tools do registro compartilhado e roda [`run_turn`]
/// com o cliente `rig-core` real. `store` sempre vem de `crate::state::config_store(app)` do lado
/// de quem chama — nunca `ConfigStore::open` (duas instâncias divergem em silêncio; ver o porquê em
/// `state::config_store`).
pub async fn run_turn_ollama(
    store: &ConfigStore,
    history: Vec<Message>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    let config = resolve_ollama_config(store);

    crate::modules::privacy::assert_network_allowed(store, &config.base_url, "brain:turn:ollama").map_err(TurnError::Provider)?;
    let prompt_started = std::time::Instant::now();

    // O monólogo vira DADO passado a `append_turn_context`, não algo que `compose_system_prompt`
    // busca sozinho (pedido explícito do coordenador). Gate: `llm.enableMonologue` (default
    // desligado, ver o cabeçalho de `monologue.rs` para o custo/latência quando ligado). Perfil do
    // modelo + monólogo correm JUNTO com a composição do prompt (cujo embedding de episódios é a
    // outra espera de rede do turno): nenhuma das duas depende da outra.
    let prepare = async {
        let config = resolve_ollama_config_for_model(store).await;
        let model = build_ollama_model(&config);
        let monologue_text = if monologue_enabled(store) { monologue::generate(model.clone(), user_content, &history).await } else { None };
        (config, model, monologue_text)
    };
    let (system_prompt, (config, model, monologue_text)) = tokio::join!(compose_system_prompt(store, user_content), prepare);
    let system_prompt = finish_system_prompt(system_prompt?, &failure_tracker::failure_tracker().self_healing_context(), monologue_text.as_deref(), user_content, store);
    log::debug!("turno: prompt montado em {}ms", prompt_started.elapsed().as_millis());
    prompt_trace::record(&system_prompt, user_content);
    let additional_params = Some(build_ollama_additional_params(&config));

    let tools = chat_tool_definitions();

    run_turn(
        model,
        system_prompt,
        history,
        user_content,
        tools,
        config.max_tool_steps,
        config.temperature,
        config.max_tokens,
        additional_params,
        abort,
        observer,
    )
    .await
}

/// Fachada MULTI-PROVEDOR: lê `llm.provider` do `ConfigStore` e resolve QUALQUER um dos 5
/// provedores reais (`google`/`openrouter`/`ollama`/`openaiCompatible`/`unsloth`) — porte de
/// `resolveLanguageModelForProvider` (ver `modules/brain/llm_provider.rs` para os 3 remotos e o
/// porquê da distinção de chokepoint entre eles). `"ollama"` (e qualquer valor desconhecido/ausente,
/// mesmo fallback de `llm_provider::resolve_provider_id`) delega DIRETO a [`run_turn_ollama`] — zero
/// diff de comportamento para quem já usa Ollama hoje, byte a byte o mesmo caminho já testado acima.
///
/// **Failover automático para Ollama** (02-providers.md, "MUST: `ollama` é o alvo final do
/// failover"): só quando o provedor remoto configurado falha ANTES de qualquer efeito colateral
/// (`TurnError::ProviderBeforeSideEffects` — ver o enum) o turno inteiro é refeito contra Ollama.
/// Uma falha DEPOIS de texto já emitido ou tool já executada (`TurnError::Provider`) nunca tenta de
/// novo — refazer duplicaria o efeito colateral. Se o failover TAMBÉM falhar, o erro devolvido ao
/// chamador é o do provedor ORIGINAL (o que o usuário configurou), não o do Ollama de socorro —
/// `chat::send::handle_chat_send` grava saúde só para o par configurado, e é essa mensagem que faz
/// sentido no banner.
pub async fn run_turn_llm(
    store: &ConfigStore,
    history: Vec<Message>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    let announcer = ThinkingAnnouncer::start(observer);
    // Guarda por resultado (`effect_guard`): tool de efeito só com falha não termina em "pronto, fiz".
    let notice = crate::modules::settings::t("chat.effectFailed", &crate::config::locale::app_locale(store));
    let guard = effect_guard::EffectGuard::new(&announcer, notice);
    let result = llm_gate::in_chat_turn(run_turn_llm_routed(store, history, user_content, abort, &guard)).await;
    let result = guard.finish(result);
    announcer.finish();
    result
}

async fn run_turn_llm_routed(
    store: &ConfigStore,
    history: Vec<Message>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    let provider = super::llm_provider::resolve_provider_id(store);
    if provider == "ollama" {
        return run_turn_ollama(store, history, user_content, abort, observer).await;
    }

    match run_turn_remote(store, &provider, history.clone(), user_content, abort.clone(), observer).await {
        Err(TurnError::ProviderBeforeSideEffects(reason)) => {
            // Tier humano (specs/10-observability.md, "Dois públicos, dois tiers"): frase com o
            // EFEITO explícito ("caí para X"), não o evento cru ("provider failover").
            crate::human_log!(
                log::Level::Warn,
                "mika: caí para o Ollama local porque o provedor {provider} falhou antes de \
                 qualquer efeito colateral ({reason}) (02-providers.md)"
            );
            let model = super::llm_provider::resolve_remote_config(store, &provider).model;
            let fallback = run_turn_ollama(store, history, user_content, abort, observer).await;
            failover_outcome(fallback, &provider, &model, reason)
        }
        other => other,
    }
}

/// Resultado do failover. Mesmo com o Ollama de socorro respondendo, o par CONFIGURADO continua
/// quebrado e a saúde não pode "esquecer" isso (ver `TurnResult::served_by_failover`) — então a
/// falha é gravada aqui. Se o socorro também falhou, NÃO grava: o `TurnError::Provider` sobe e
/// `chat::send` grava a mesma falha; gravar nos dois lugares contava um turno como dois strikes.
fn failover_outcome(fallback: Result<TurnResult, TurnError>, provider: &str, model: &str, reason: String) -> Result<TurnResult, TurnError> {
    match fallback {
        Ok(mut result) => {
            if !crate::modules::llm::provider_health::is_data_policy_mismatch(&reason) {
                let status_code = crate::modules::llm::provider_health::extract_http_status(&reason);
                crate::modules::llm::provider_health::record_failure(provider, model, &reason, status_code);
            }
            result.served_by_failover = true;
            Ok(result)
        }
        // Parada pedida pelo usuário durante o socorro não é falha de provedor nenhum.
        Err(TurnError::Aborted) => Err(TurnError::Aborted),
        Err(_) => Err(TurnError::Provider(reason)),
    }
}

/// Corpo real da resolução remota — extraído de [`run_turn_llm`] só para o wrapper de failover
/// acima poder chamar de novo com `ollama` sem duplicar este bloco.
async fn run_turn_remote(
    store: &ConfigStore,
    provider: &str,
    history: Vec<Message>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    let mut config = super::llm_provider::resolve_remote_config(store, provider);
    let model = super::llm_provider::build_remote_model(store, provider, &config).map_err(TurnError::ProviderBeforeSideEffects)?;
    let prompt_started = std::time::Instant::now();

    // Mesmo wiring de `run_turn_ollama` — ver os comentários lá (monólogo como dado para
    // `append_turn_context`, perfil + monólogo em paralelo com a composição do prompt).
    let prepare = async {
        let profile = crate::modules::llm::model_profile::resolve(store, provider, &config.model).await;
        let monologue_text = if monologue_enabled(store) { monologue::generate(model.clone(), user_content, &history).await } else { None };
        (profile, monologue_text)
    };
    let (system_prompt, (profile, monologue_text)) = tokio::join!(compose_system_prompt(store, user_content), prepare);
    fit_to_model(&mut 0, &mut config.max_tokens, &profile);
    let system_prompt = finish_system_prompt(system_prompt?, &failure_tracker::failure_tracker().self_healing_context(), monologue_text.as_deref(), user_content, store);
    log::debug!("turno: prompt montado em {}ms", prompt_started.elapsed().as_millis());
    prompt_trace::record(&system_prompt, user_content);

    let tools = chat_tool_definitions();

    run_turn(
        model,
        system_prompt,
        history,
        user_content,
        tools,
        config.max_tool_steps,
        config.temperature,
        config.max_tokens,
        build_remote_additional_params(provider, store, PromptFamily::Chat, &config.model, &profile),
        abort,
        observer,
    )
    .await
}

/// Composição do system prompt de [`run_turn_ollama_raw`] — porte de
/// `` buildSystemPrompt(opts.store, options?.systemPrompt ? `\n\n## CURRENT TASK\n${options.systemPrompt}` : '') ``
/// (`chat-generate.runtime.ts::generateRawOnce`): a tarefa é o `dynamicContext`, então cai no bloco
/// volátil ANTES de `system-tail.md`, não depois dele. Sem jogo/gíria e sem humor/vínculo —
/// `generateRaw` usa `buildSystemPrompt` puro, sem o `injectPersonaState` que só o turno de chat
/// aplica ([`compose_system_prompt`]). Extraída à parte para ser testável sem rede.
fn build_raw_system_prompt(store: &ConfigStore, task_prompt: Option<&str>) -> Result<String, TurnError> {
    let dynamic_context = task_prompt.map(|task| format!("\n\n## CURRENT TASK\n{task}")).unwrap_or_default();
    persona::system_prompt::build_full_system_prompt(store, &dynamic_context).map_err(TurnError::Provider)
}

/// Fachada "tool-free": mesmo caminho de [`run_turn_ollama`] (config Ollama, chokepoint de
/// privacidade, cliente `rig-core`), mas SEM tools e com [`build_raw_system_prompt`] no lugar do
/// prompt de persona puro — porte de `ChatService.generateRaw`
/// (`chat-generate.runtime.ts::runGenerateRaw`/`generateRawOnce`), usado por
/// `ContentPipelineService`/`CollaborationService` (recap, post, abertura/turno de debate — ver
/// `tools/content.rs`/`tools/collaboration.rs`).
///
/// Saída vazia é refeita uma vez dentro de [`run_turn`], como `retryEmptyOutput` no TS.
///
/// O `system.md` rico É usado aqui (via [`build_raw_system_prompt`]), com "## CURRENT TASK" no bloco
/// volátil quando `task_prompt` vem.
///
/// `max_tool_steps` é sempre `1` (não `config.max_tool_steps`) e `tools`/`history` ficam vazios:
/// igual ao `maxToolSteps: 1`/`aiTools: {}` hardcoded em `generateRawOnce` — geração tool-free de
/// verdade, não um laço de ferramentas que por acaso não encontra nenhuma.
pub async fn run_turn_ollama_raw(
    store: &ConfigStore,
    task_prompt: Option<&str>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    let config = resolve_ollama_config(store);

    crate::modules::privacy::assert_network_allowed(store, &config.base_url, "brain:turn:ollama-raw").map_err(TurnError::Provider)?;
    let config = resolve_ollama_config_for_model(store).await;

    let model = build_ollama_model(&config);
    let system_prompt = build_raw_system_prompt(store, task_prompt)?;
    let additional_params = Some(build_ollama_additional_params(&config));

    run_turn(model, system_prompt, vec![], user_content, vec![], 1, config.temperature, config.max_tokens, additional_params, abort, observer).await
}

/// O par multi-provedor de [`run_turn_ollama_raw`] — sem tools, sem histórico, um turno só.
///
/// Existe para o botão "Validar" de Configurações (`settings/plugin_test.rs`). Sem ele aquela tela
/// teria de resolver provedor por conta própria, e **a tela que testa a conexão resolveria o
/// provedor por um caminho diferente do que a conversa usa** — o teste passaria a medir outra
/// coisa. Nesta mesma rodada o botão de STT já tinha ficado para trás da feature que testava; o
/// jeito de isso não se repetir é a resolução morar num lugar só.
pub async fn run_turn_llm_raw(
    store: &ConfigStore,
    task_prompt: Option<&str>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    run_turn_llm_raw_family(PromptFamily::Background, store, task_prompt, user_content, abort, observer).await
}

/// Mesma fachada de [`run_turn_llm_raw`], mas com a família de prompt explícita (ver
/// [`PromptFamily`]/[`kv_cache_id_slot`], item 6) — `chat::proactive` é a ÚNICA chamadora da
/// fachada raw cujo prompt é da mesma família do chat principal (persona), então passa
/// `PromptFamily::Chat` em vez do `Background` que [`run_turn_llm_raw`] já assume por padrão para
/// as outras ~15 chamadoras (extração, reflexão, consolidação, recap/colaboração, resumos de jogo).
pub(crate) async fn run_turn_llm_raw_family(
    prompt_family: PromptFamily,
    store: &ConfigStore,
    task_prompt: Option<&str>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    // A proactive line (`PromptFamily::Chat`) is stale once the user speaks; the rest is replayed.
    let on_preempt = if prompt_family == PromptFamily::Chat { llm_gate::Preempted::Drop } else { llm_gate::Preempted::Requeue };
    llm_gate::run_background(&abort, on_preempt, || run_turn_llm_raw_routed(prompt_family, store, task_prompt, user_content, abort.clone(), observer)).await
}

async fn run_turn_llm_raw_routed(
    prompt_family: PromptFamily,
    store: &ConfigStore,
    task_prompt: Option<&str>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    let announcer = ThinkingAnnouncer::start(observer);
    let result = run_turn_llm_raw_provider(prompt_family, store, task_prompt, user_content, abort, &announcer).await;
    announcer.finish();
    result
}

async fn run_turn_llm_raw_provider(
    prompt_family: PromptFamily,
    store: &ConfigStore,
    task_prompt: Option<&str>,
    user_content: &str,
    abort: TurnAbortSignal,
    observer: &dyn TurnObserver,
) -> Result<TurnResult, TurnError> {
    let provider = super::llm_provider::resolve_provider_id(store);
    if provider == "ollama" {
        return run_turn_ollama_raw(store, task_prompt, user_content, abort, observer).await;
    }

    let mut config = super::llm_provider::resolve_remote_config(store, &provider);
    let model = super::llm_provider::build_remote_model(store, &provider, &config).map_err(TurnError::Provider)?;
    let profile = crate::modules::llm::model_profile::resolve(store, &provider, &config.model).await;
    fit_to_model(&mut 0, &mut config.max_tokens, &profile);
    let system_prompt = build_raw_system_prompt(store, task_prompt)?;
    let additional_params = build_remote_additional_params(&provider, store, prompt_family, &config.model, &profile);

    run_turn(model, system_prompt, vec![], user_content, vec![], 1, config.temperature, config.max_tokens, additional_params, abort, observer).await
}

/// Roda uma chamada de LLM feita fora das fachadas deste arquivo (visão) na fila de fundo —
/// cede ao turno de chat e não concorre com as outras tarefas de fundo. Ver `turn/llm_gate.rs`.
/// `None` = passou do teto de 180 s e foi cancelada.
pub async fn as_background_llm_call<F: std::future::Future>(attempt: impl FnMut() -> F) -> Option<F::Output> {
    llm_gate::as_background(attempt).await
}

/// Roda `work` como turno de primeiro plano: não espera a fila de fundo, e as tarefas de fundo
/// esperam por ele. Para ação pedida pelo usuário que usa `run_turn_llm_raw` (botão "Validar").
pub async fn run_as_foreground<F: std::future::Future>(work: F) -> F::Output {
    llm_gate::in_foreground(work).await
}

/// Turnos de chat já iniciados — quem gera fala de fundo compara antes/depois (ver `llm_gate`).
pub fn chat_turns_started() -> u64 {
    llm_gate::chat_turns_started()
}

/// `true` quando um turno de chat REAL terminou no mesmo instante ou depois de `since_ms` (epoch
/// ms) — pega o caso que `chat_turns_started() != turns_before` sozinho não pega: um turno que já
/// tinha COMEÇADO E TERMINADO antes de `turns_before` ser capturado (code review, major: a resposta
/// de interrupção do barge-in só captura `turns_before` depois de decodificar o STT, bem mais devagar
/// que o hangover de silêncio do STT contínuo do renderer para a MESMA fala). Ver `llm_gate`.
pub fn chat_turn_completed_since(since_ms: u64) -> bool {
    llm_gate::chat_turn_completed_since(since_ms)
}

/// Família de prompt de um turno/completude, para o `id_slot` de KV-cache (ver
/// [`kv_cache_id_slot`]) — a mesma classificação orienta qual slot llama.cpp reusa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptFamily {
    /// Conversa real com a Mika: o chat do usuário (`run_turn_remote`) E a jogada proativa
    /// (`chat::proactive`) — mesmo prompt de persona, mesmo slot, para o prefixo em comum
    /// (histórico/persona) continuar batendo no cache entre os dois.
    Chat,
    /// Tudo que não é conversa: extração de fatos, reflexão, consolidação, recap/colaboração,
    /// resumos de jogo (`run_turn_llm_raw`, salvo a exceção acima) e a análise de visão
    /// (`vision::llm_client`) — prompts de tarefa bem diferentes do de persona, mesmo slot entre si.
    Background,
}

/// llama.cpp aceita e IGNORA um `id_slot` fora do range de slots paralelos configurados
/// (verificado ao vivo nesta rodada — ver o relatório da tarefa), então mandar um índice fixo por
/// família nunca quebra um servidor com menos slots do que o esperado; só melhora a localidade de
/// KV-cache quando o servidor TEM slots de sobra. Só `openaiCompatible` fala esse dialeto —
/// Ollama/Google/OpenRouter não têm o conceito de slot e tratariam o campo como corpo inválido, não
/// como parâmetro ignorado.
pub(crate) fn kv_cache_id_slot(provider: &str, prompt_family: PromptFamily) -> Option<u32> {
    (provider == "openaiCompatible").then_some(match prompt_family {
        PromptFamily::Chat => 0,
        PromptFamily::Background => 1,
    })
}

/// `additional_params` dos provedores remotos. `num_ctx` é opção do Ollama — mandá-la para Gemini
/// ou OpenRouter é corpo inválido, não parâmetro ignorado —, então só `openaiCompatible` recebe
/// algo: `chat_template_kwargs.enable_thinking`, que llama.cpp/vLLM repassam ao template (sem isso
/// `llm.enableThinking=false` não valia ali: Gemma 4 gerava ~300 tokens de raciocínio para um "oi"
/// (284 → 3, medido) e às vezes vazava o rascunho ("*Draft:*…") no texto da resposta) e `id_slot`
/// (ver [`kv_cache_id_slot`]) — os dois convivem no MESMO objeto, nunca duas chamadas separadas de
/// `additional_params` (a segunda substituiria a primeira, não mescla).
fn build_remote_additional_params(
    provider: &str,
    store: &ConfigStore,
    prompt_family: PromptFamily,
    model: &str,
    profile: &crate::modules::llm::model_profile::ModelProfile,
) -> Option<Value> {
    let settings = super::provider_params::SamplingSettings::from_store(store);
    let mut params = super::provider_params::remote_params(provider, model, &settings, profile);
    if let Some(slot) = kv_cache_id_slot(provider, prompt_family) {
        params.insert("id_slot".to_string(), json!(slot));
    }
    (!params.is_empty()).then_some(Value::Object(params))
}

#[cfg(test)]
mod tests;
