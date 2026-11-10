//! `mika:chat-send` — roda um turno real sobre `brain::turn::run_turn_ollama`, traduz os eventos
//! de turno para o HUD e liga o par histórico-real/turno-cancelável que o resto do grupo
//! `mika:chat-*` precisava (ver o cabeçalho de `mod.rs`).

use crate::config::schemas::llm_defaults;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use super::history::ChatMessage;
use super::history_store::HistoryStore;
use super::turns;
use crate::config::store::ConfigStore;
use crate::modules::brain::turn;
use crate::modules::llm::provider_health;
use crate::events::{bus, EventKey};
use crate::protocol::{renderer_event_targets, InvokeContext};
use crate::shell_actions::is_owner_surface;

/// Espelha `DESKTOP_EVENT_CHANNELS.LlmProviderHealth` (`desktop-ipc-channels.ts`) —
/// `mika-runtime-system.ts::onProviderHealth` é quem escuta do lado renderer.
const PROVIDER_HEALTH_EVENT: &str = "llm:provider-health";

/// Resolução mínima de provider/model para a CHAVE de saúde (`provider_health`), não para montar o
/// cliente — duplica o mesmo pequeno recorte de `brain::llm_provider`/`brain::turn` (ambos privados
/// a `brain`, fora do alcance deste módulo; mesmo precedente de duplicação já aceito no repo para
/// esta leitura específica, ver os cabeçalhos daqueles dois arquivos). Só entra aqui o suficiente
/// para IDENTIFICAR qual provedor+modelo respondeu, não para requisitar nada.
pub(super) fn resolve_health_key(store: &ConfigStore) -> (String, String) {
    let get_str = |path: &str| store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty());
    let provider = match get_str("llm.provider").as_deref() {
        Some("google") => "google",
        Some("openrouter") => "openrouter",
        Some("openaiCompatible") => "openaiCompatible",
        Some("unsloth") => "unsloth",
        _ => "ollama",
    };
    let model = match provider {
        "google" => get_str("llm.googleModel").unwrap_or_else(|| llm_defaults::GOOGLE_MODEL.to_string()),
        "openrouter" => get_str("llm.openrouterModel").unwrap_or_else(|| llm_defaults::OPENROUTER_MODEL.to_string()),
        "openaiCompatible" => get_str("llm.openaiCompatibleModel").unwrap_or_else(|| llm_defaults::OPENAI_COMPATIBLE_MODEL.to_string()),
        "unsloth" => get_str("llm.unslothModel").unwrap_or_default(),
        _ => get_str("llm.ollamaModel").or_else(|| get_str("llm.model")).unwrap_or_else(|| llm_defaults::OLLAMA_MODEL.to_string()),
    };
    (provider.to_string(), model)
}

/// `renderer_event_targets`, nunca `app.emit` — mesma regra de superfície de
/// `assistant_bridge::emit_to_renderer`/`expression_bridge` (ver os cabeçalhos deles): `plugin-view`
/// não pode alcançar o estado de saúde do provedor do usuário.
fn emit_provider_health(app: &AppHandle, payload: &Value) {
    for label in renderer_event_targets(app.webview_windows().keys().cloned().collect()) {
        let _ = app.emit_to(label.as_str(), PROVIDER_HEALTH_EVENT, payload.clone());
    }
}

/// Espelha `DESKTOP_EVENT_CHANNELS` em `packages/sdk/src/contracts/desktop-ipc-channels.ts` — os
/// nomes exatos que `mika-runtime-chat.ts` (`onChatThinkingStart`/`onChatStreamChunk`/`onToolCall`/
/// `onToolResult`/`onChatThinkingStop`) escuta para os `TurnEvent` que este módulo traduz. Um
/// literal errado aqui não quebra a build: só deixa o HUD mudo, sem erro nenhum.
// `pub(super)`: `chat::proactive::send_proactive` reusa o MESMO mapeamento evento->canal/chave para
// um turno sem `InvokeContext` (ver o cabeçalho de `proactive.rs`) — sem isto ele teria de duplicar
// os 5 literais, que é exatamente o defeito de "canal errado não quebra a build" que o comentário
// abaixo descreve.
pub(super) const CHAT_THINKING_START_EVENT: &str = "mika:event:chat:thinking-start";
pub(super) const CHAT_STREAM_CHUNK_EVENT: &str = "mika:event:chat:stream-chunk";
pub(super) const CHAT_TOOL_CALL_EVENT: &str = "mika:event:chat:tool-call";
pub(super) const CHAT_TOOL_RESULT_EVENT: &str = "mika:event:chat:tool-result";
pub(super) const CHAT_THINKING_STOP_EVENT: &str = "mika:event:chat:thinking-stop";

/// Traduz cada [`turn::TurnEvent`] para o evento IPC que o renderer já escuta, só para a janela que
/// pediu o turno — mesmo padrão de `emit_to` que `voice::mod::handle_mic_state_request` já usa para
/// não ecoar em janela nenhuma que não perguntou.
struct IpcTurnObserver {
    app: AppHandle,
    window_label: String,
    user_content: String,
}

/// Núcleo puro do mapeamento — sem `AppHandle`, testável sem um app Tauri de verdade (este crate
/// não tem a feature `test-support` do `tauri` habilitada, então nada aqui pode depender de um
/// `AppHandle` real em teste).
pub(super) fn turn_event_ipc(event: turn::TurnEvent, user_content: &str) -> (&'static str, Value) {
    match event {
        turn::TurnEvent::ThinkingStart => (CHAT_THINKING_START_EVENT, json!({ "userContent": user_content })),
        turn::TurnEvent::TextChunk(text) => (CHAT_STREAM_CHUNK_EVENT, json!({ "text": text })),
        turn::TurnEvent::ToolCallStarted { name } => (CHAT_TOOL_CALL_EVENT, json!({ "name": name })),
        turn::TurnEvent::ToolCallFinished { name, ok, result, duration_ms, .. } => {
            (CHAT_TOOL_RESULT_EVENT, json!({ "name": name, "success": ok, "result": result, "durationMs": duration_ms }))
        }
        turn::TurnEvent::ThinkingStop => (CHAT_THINKING_STOP_EVENT, json!({})),
    }
}

/// O mesmo turno no barramento: a janela recebe pelo IPC, e quem vive no Rust (fala automática,
/// motor proativo, catálogo de pessoas) assina a chave.
pub(super) fn turn_event_bus(event: &turn::TurnEvent, user_content: &str) -> Option<(EventKey, Value)> {
    match event {
        turn::TurnEvent::ThinkingStart => Some((EventKey::ThinkingStart, json!({ "userContent": user_content }))),
        turn::TurnEvent::TextChunk(text) => Some((EventKey::StreamChunk, json!({ "text": text }))),
        turn::TurnEvent::ToolCallStarted { name } => Some((EventKey::ToolCall, json!({ "name": name }))),
        turn::TurnEvent::ToolCallFinished { name, ok, result, .. } => Some((EventKey::ToolResult, json!({ "name": name, "success": ok, "result": result }))),
        turn::TurnEvent::ThinkingStop => Some((EventKey::ThinkingStop, json!({}))),
    }
}
impl turn::TurnObserver for IpcTurnObserver {
    fn emit(&self, event: turn::TurnEvent) {
        if let Some((key, payload)) = turn_event_bus(&event, &self.user_content) {
            bus().publish(key, payload);
        }
        let (channel, payload) = turn_event_ipc(event, &self.user_content);
        let _ = self.app.emit_to(self.window_label.as_str(), channel, payload);
    }
}

/// 04-memory.md §"Curto prazo": chamado DEPOIS que o turno já gravou as duas mensagens no
/// histórico — nunca atrasa a resposta atual. `overflow_for_summary` só devolve `Some` quando há
/// excedente novo além do que o resumo já cobre (ver o cabeçalho de `history_store.rs`); mesmo
/// desenho de `gamer::game_watcher::GameWatcher::spawn_llm_summary`: uma falha do LLM só loga e
/// deixa `summarized_through` parado, para a próxima chamada tentar de novo o mesmo trecho.
fn spawn_rolling_summary_if_overflowed(history: &Arc<HistoryStore>, store: &Arc<ConfigStore>, history_limit: usize) {
    if !super::rolling_summary::summarization_enabled(store) {
        return;
    }
    let overflow = match history.overflow_for_summary(history_limit) {
        Ok(Some(overflow)) => overflow,
        Ok(None) => return,
        Err(error) => {
            log::warn!("mika: falha ao checar excedente do resumo rolante: {error}");
            return;
        }
    };
    let (summarized_through, messages, epoch) = overflow;
    let history = Arc::clone(history);
    let store = Arc::clone(store);
    tauri::async_runtime::spawn(async move {
        let existing = history.rolling_summary().unwrap_or(None);
        let (_handle, signal) = turn::TurnAbortHandle::new();
        match super::rolling_summary::summarize_overflow_with_llm(&store, existing.as_deref(), &messages, signal).await {
            Ok(summary) if !summary.is_empty() => {
                match history.set_rolling_summary(&summary, summarized_through, epoch) {
                    Ok(true) => {}
                    Ok(false) => log::info!("mika: resumo rolante descartado — a conversa foi limpa enquanto ele era gerado"),
                    Err(error) => log::warn!("mika: falha ao salvar resumo rolante do chat: {error}"),
                }
            }
            Ok(_) => {}
            Err(error) => log::warn!("mika: resumo rolante do chat falhou, mantendo o histórico cru: {error}"),
        }
    });
}

/// Grava o par do turno no histórico. `push` escreve em SQLite sob um `Mutex` síncrono: fora do
/// worker async, que também serve `mika_invoke`.
async fn persist_turn(history: &Arc<HistoryStore>, messages: Vec<ChatMessage>) -> Result<(), String> {
    let history = Arc::clone(history);
    tauri::async_runtime::spawn_blocking(move || messages.into_iter().try_for_each(|message| history.push(message)))
        .await
        .map_err(|e| format!("gravar histórico: {e}"))?
}

pub(super) fn now_millis() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Espelha a checagem de `validateAttachments` de `chat/ipc.ts` só na medida do que este módulo
/// consegue ser honesto sobre: se HÁ anexo, é porque o usuário pediu para usá-lo, e este turno não
/// sabe lê-lo (sem visão/multimodal portado). Reportar isso é melhor que aceitar em silêncio e
/// responder como se a imagem nunca tivesse existido.
fn has_attachments(payload: &Value) -> bool {
    payload.get("attachments").and_then(Value::as_array).is_some_and(|list| !list.is_empty())
}

fn empty_message_response(tool_count: usize, attachments_present: bool) -> Value {
    let content = if attachments_present {
        "Anexos ainda não são suportados nesta versão. Envie o pedido em texto."
    } else {
        "Mensagem vazia. Envie um texto para conversar."
    };
    json!({
        "id": format!("empty-{}", now_millis()),
        "content": content,
        "toolCount": tool_count,
        "role": "system",
    })
}

/// `03-resilience.md`, "Latch fatal": resposta devolvida SEM chamar `turn::run_turn_llm` — o par
/// `provider:model` já está `disabled` (`provider_health::is_blocked`), então esta mensagem nunca
/// vira requisição de rede. `entry` é o snapshot devolvido por `is_blocked`, a mesma forma que
/// `ProviderHealthBanner` já lê (`reason`/`statusCode`).
fn blocked_by_health_response(tool_count: usize, entry: &provider_health::HealthEntry) -> Value {
    let json = entry.to_json();
    let reason = json["reason"].as_str().unwrap_or("motivo desconhecido");
    json!({
        "id": format!("blocked-{}", now_millis()),
        "content": format!("Provedor desativado: {reason}. Corrija a configuração ou tente novamente em Configurações."),
        "toolCount": tool_count,
        "role": "system",
    })
}

/// Sinal de crise na fala do usuário (`brain::crisis`): a resposta carrega a região dos recursos e o
/// renderer mostra o cartão. Vai na resposta do próprio `chat-send` (voz e texto convergem nele), sem
/// canal de evento novo; nada do texto do usuário é gravado.
fn attach_crisis_region(mut response: Value, region: Option<&'static str>) -> Value {
    if let (Some(region), Some(object)) = (region, response.as_object_mut()) {
        object.insert("crisisRegion".to_string(), json!(region));
    }
    response
}

/// O cartão de crise não pode depender de o turno ter dado certo: falha ao ler o histórico, ao
/// registrar o turno ou ao gravar o par (`Err` do `?`) também o leva. Com sinal de crise o `Err`
/// vira uma resposta `role: "system"` (mesma convenção do turno que falhou) com a região; sem sinal
/// segue como `Err` do IPC.
fn with_crisis_region(result: Result<Value, String>, region: Option<&'static str>, tool_count: usize) -> Result<Value, String> {
    match (result, region) {
        (Ok(response), region) => Ok(attach_crisis_region(response, region)),
        (Err(error), None) => Err(error),
        (Err(error), region) => {
            let content = crate::modules::system::log_redact::redact(&error);
            let failure = json!({ "id": format!("error-{}", now_millis()), "content": content, "toolCount": tool_count, "role": "system" });
            Ok(attach_crisis_region(failure, region))
        }
    }
}

/// `mika:chat-send`: roda um turno real (`turn::run_turn_ollama`) e devolve a forma de
/// `desktopChatSendResponseSchema` (`id`, `content`, `toolCount`, `role`) — `role: "system"` tanto
/// para a mensagem vazia quanto para um turno que falhou, mesma convenção do `chat/ipc.ts` original
/// ("`system` = o turno falhou e `content` é a mensagem de erro").
pub async fn handle_chat_send(
    app: &AppHandle,
    ctx: &InvokeContext,
    payload: Value,
    history: &Arc<HistoryStore>,
    turn_registry: &turns::Turns,
) -> Result<Value, String> {
    if !is_owner_surface(ctx.window_type) {
        return Err(format!("mika:chat-send recusado para {:?}", ctx.window_type));
    }

    let message = payload.get("message").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let attachments_present = has_attachments(&payload);
    let tool_count = crate::modules::tools::registry().get_all().len();

    if message.is_empty() {
        return Ok(empty_message_response(tool_count, attachments_present));
    }

    let store = crate::state::config_store(app)?;
    let crisis_region = crate::modules::brain::crisis::detect(&message)
        .then(|| crate::modules::brain::crisis::region(&crate::config::locale::app_locale(&store)));

    let result = run_chat_turn(app, ctx, &store, message, tool_count, history, turn_registry).await;
    with_crisis_region(result, crisis_region, tool_count)
}

/// O turno em si (latch de saúde, histórico, LLM, gravação). Separado de [`handle_chat_send`] para
/// que TODO desfecho — inclusive o `Err` de um `?` — passe por [`with_crisis_region`].
async fn run_chat_turn(
    app: &AppHandle,
    ctx: &InvokeContext,
    store: &Arc<ConfigStore>,
    message: String,
    tool_count: usize,
    history: &Arc<HistoryStore>,
    turn_registry: &turns::Turns,
) -> Result<Value, String> {
    // `03-resilience.md`, "Latch fatal": consulta ANTES de qualquer I/O — um par `disabled` nunca
    // sai para a rede. Resolvido aqui (não mais só depois do turno) exatamente para poder checar
    // isto antes de `run_turn_llm`.
    let (health_provider, health_model) = resolve_health_key(&store);
    if let Some(entry) = provider_health::is_blocked(&health_provider, &health_model) {
        return Ok(blocked_by_health_response(tool_count, &entry));
    }

    let observer = IpcTurnObserver { app: app.clone(), window_label: ctx.window_label.clone(), user_content: message.clone() };
    // Histórico de turnos ANTERIORES de verdade, não mais `vec![]` — ver o cabeçalho de `mod.rs`.
    // `llm.historyLimit` (04-memory.md §"Curto prazo") corta para os mais recentes — ver o cabeçalho
    // de `HistoryStore::to_turn_history`.
    let history_limit = store.get("llm.historyLimit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let history_vec = history.to_turn_history(history_limit)?;

    let (handle, signal) = turn::TurnAbortHandle::new();
    let turn_guard = turns::register_guarded(turn_registry, &ctx.window_label, handle)?;
    // `run_turn_llm` resolve QUALQUER provider configurado (`llm.provider`), não mais só Ollama —
    // ver `brain/llm_provider.rs`. Isso vira a asserção oposta em
    // `modules/plugins/mod.rs::plugin_test_llm_branch_needs_multi_provider_but_rust_chat_is_ollama_only`
    // (fora do meu escopo; reportado, não editado).
    // Trace id do turno inteiro: todo `log::` emitido enquanto esta future for retomada (em
    // qualquer worker thread) carrega o mesmo id em `LogRecord::trace_id` — ver `system::log_trace`.
    let trace_id = crate::modules::system::log_trace::new_id();
    let outcome = crate::modules::system::log_trace::scoped(
        trace_id,
        turn::run_turn_llm(&store, history_vec, &message, signal, &observer),
    )
    .await;
    drop(turn_guard); // desregistra aqui; em panic/future descartada, o `Drop` faz o mesmo

    let response = match outcome {
        Ok(result) => {
            // Failover é sempre para o Ollama local: custo zero, não o preço do par configurado.
            let cost = if result.served_by_failover {
                Some(0.0)
            } else {
                crate::modules::token_usage::cost_for(app, &health_provider, &health_model, &result.usage)
            };
            crate::modules::token_usage::record(app, "chat", &health_provider, &health_model, &result.usage, cost);
            let context_window = crate::modules::llm::model_profile::effective_context_window(&store, &health_provider, &health_model).await;
            // `served_by_failover`: a resposta veio do Ollama de socorro, não do par CONFIGURADO —
            // `turn::run_turn_llm` já gravou a falha real dele antes do failover. Marcar
            // `health_provider`/`health_model` como saudável aqui apagaria esse registro e
            // esconderia que o provedor escolhido pelo usuário continua quebrado (ver o comentário
            // de `TurnResult::served_by_failover`).
            if result.served_by_failover {
                // A falha do par configurado já foi gravada por `turn::run_turn_llm` antes do
                // failover — emite AGORA para o banner não esperar o próximo turno bloqueado.
                if let Some(entry) = provider_health::get(&health_provider, &health_model) {
                    emit_provider_health(app, &entry.to_json());
                }
            } else if provider_health::record_success(&health_provider, &health_model).is_some() {
                emit_provider_health(app, &provider_health::healthy_payload(&health_provider, &health_model));
            }
            // Só um turno BEM-SUCEDIDO vira memória para o próximo — ver o cabeçalho de `mod.rs`.
            let id = format!("turn-{}", now_millis());
            persist_turn(history, vec![
                ChatMessage { id: format!("user-{}", now_millis()), role: "user".to_string(), content: message, timestamp_ms: now_millis() },
                ChatMessage { id: id.clone(), role: "assistant".to_string(), content: result.text.clone(), timestamp_ms: now_millis() },
            ])
            .await?;
            crate::modules::brain::guards::repetition_guard().remember(&result.text);
            super::assistant_bridge::emit_to_renderer(app, &super::assistant_bridge::turn_payload(&result.text, now_millis(), &result.usage, cost, context_window));
            spawn_rolling_summary_if_overflowed(history, &store, history_limit);
            json!({ "id": id, "content": result.text, "toolCount": tool_count, "role": "assistant" })
        }
        Err(err) => {
            // Abort é o usuário mandando parar (barge-in, cancelar) — não é falha real de provedor,
            // não pinta a bandeja e não conta como strike de saúde. `EmptyOutput`/
            // `MaxToolStepsExceeded` também não: o provedor RESPONDEU, só que sem uma resposta
            // final — problema de qualidade de saída, não de o provedor estar fora do ar. Só
            // `TurnError::Provider` (rede, autenticação, HTTP de erro) é falha real de provedor.
            // Mensagem localizada e redigida (`turn_error.rs`): o erro cru do provedor pode ter a URL
            // com `?key=<segredo>` e nunca vira texto de chat.
            let error_text = super::turn_error::user_message(&err, &health_provider, &crate::config::locale::app_locale(&store));
            if !matches!(err, turn::TurnError::Aborted) {
                bus().publish(EventKey::ChatError, json!({ "message": error_text }));
            }
            // Filtro ZDR sem endpoint para o modelo (404 do OpenRouter) é política, não saúde: a
            // mensagem acionável já foi para o chat, e o par não ganha strike.
            let counted = match &err {
                turn::TurnError::Provider(reason) if !provider_health::is_data_policy_mismatch(reason) => Some(reason),
                _ => None,
            };
            if let Some(reason) = counted {
                let status_code = provider_health::extract_http_status(reason);
                // O banner de saúde também mostra o motivo ao usuário: guarda a versão redigida.
                let reason = &crate::modules::system::log_redact::redact(reason);
                let entry = provider_health::record_failure(&health_provider, &health_model, reason, status_code);
                let entry_json = entry.to_json();
                // "Disciplina de log" (03-resilience.md): uma linha na TRANSIÇÃO para `disabled`,
                // nunca por tentativa — depois disto o latch (`is_blocked`, checado no topo desta
                // função) barra qualquer nova chamada antes de rede, então `record_failure` só volta
                // a bater aqui de novo depois de um reset/sucesso/mudança de config.
                if entry_json["state"] == "disabled" {
                    // Tier humano: efeito explícito — o chat some de verdade neste par, não só
                    // "desativado" cru.
                    crate::human_log!(
                        log::Level::Warn,
                        "mika: parei de tentar {health_provider}:{health_model} no chat porque \
                         desativou depois de falhas repetidas ({reason})"
                    );
                }
                emit_provider_health(app, &entry_json);
            }
            json!({
            "id": format!("error-{}", now_millis()),
            "content": error_text,
            "toolCount": tool_count,
            "role": "system",
            })
        }
    };
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- crisis ---------------------------------------------------------------------------------

    #[test]
    fn a_crisis_region_rides_the_response_only_when_the_message_carried_the_signal() {
        let base = json!({ "id": "turn-1", "content": "oi", "toolCount": 0, "role": "assistant" });
        assert_eq!(with_crisis_region(Ok(base.clone()), None, 0).unwrap(), base);
        let flagged = with_crisis_region(Ok(base), Some("br"), 0).unwrap();
        assert_eq!(flagged["crisisRegion"], json!("br"));
        assert_eq!(flagged["content"], json!("oi"));
    }

    /// O achado: `to_turn_history(..)?`/`register_guarded(..)?`/`persist_turn(..)?` voltavam `Err` antes
    /// de a região entrar na resposta, e a pessoa em crise via só um erro — sem o cartão. Com sinal,
    /// a falha vira resposta `system` COM a região (e sem vazar segredo); sem sinal segue `Err`.
    #[test]
    fn a_failure_after_a_crisis_signal_still_carries_the_card() {
        let failure = with_crisis_region(Err("gravar histórico: disco cheio".to_string()), Some("us"), 3).unwrap();
        assert_eq!(failure["crisisRegion"], json!("us"));
        assert_eq!(failure["role"], json!("system"));
        assert_eq!(failure["toolCount"], json!(3));
        assert!(failure["content"].as_str().unwrap().contains("disco cheio"), "{failure}");
        assert_eq!(with_crisis_region(Err("disco cheio".to_string()), None, 3), Err("disco cheio".to_string()));
    }

    // -- turn_event_ipc ---------------------------------------------------------------------------

    #[test]
    fn turn_event_ipc_maps_thinking_start_with_the_user_content() {
        let (channel, payload) = turn_event_ipc(turn::TurnEvent::ThinkingStart, "oi mika");
        assert_eq!(channel, "mika:event:chat:thinking-start");
        assert_eq!(payload, json!({ "userContent": "oi mika" }));
    }

    #[test]
    fn turn_event_ipc_maps_text_chunk() {
        let (channel, payload) = turn_event_ipc(turn::TurnEvent::TextChunk("olá, ".into()), "oi");
        assert_eq!(channel, "mika:event:chat:stream-chunk");
        assert_eq!(payload, json!({ "text": "olá, " }));
    }

    #[test]
    fn turn_event_ipc_maps_tool_call_started_and_finished() {
        let (started_channel, started_payload) =
            turn_event_ipc(turn::TurnEvent::ToolCallStarted { name: "get_time".into() }, "oi");
        assert_eq!(started_channel, "mika:event:chat:tool-call");
        assert_eq!(started_payload, json!({ "name": "get_time" }));

        let (finished_channel, finished_payload) =
            turn_event_ipc(turn::TurnEvent::ToolCallFinished { name: "get_time".into(), action: None, ok: true, result: json!({ "time": "12:00" }), duration_ms: 7 }, "oi");
        assert_eq!(finished_channel, "mika:event:chat:tool-result");
        assert_eq!(finished_payload, json!({ "name": "get_time", "success": true, "result": { "time": "12:00" }, "durationMs": 7 }));
    }

    #[test]
    fn turn_event_ipc_maps_thinking_stop_with_an_empty_payload() {
        let (channel, payload) = turn_event_ipc(turn::TurnEvent::ThinkingStop, "oi");
        assert_eq!(channel, "mika:event:chat:thinking-stop");
        assert_eq!(payload, json!({}));
    }

    /// Os 5 eventos que `IpcTurnObserver` emite têm de bater com os nomes reais do contrato —
    /// inventar um aqui deixaria o HUD mudo sem erro nenhum (ver o cabeçalho do arquivo).
    #[test]
    fn event_channel_literals_exist_in_the_ts_contract() {
        let channels_contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");

        for event_channel in [
            CHAT_THINKING_START_EVENT,
            CHAT_STREAM_CHUNK_EVENT,
            CHAT_TOOL_CALL_EVENT,
            CHAT_TOOL_RESULT_EVENT,
            CHAT_THINKING_STOP_EVENT,
            PROVIDER_HEALTH_EVENT,
        ] {
            assert!(
                channels_contract.contains(&format!("'{event_channel}'")),
                "evento {event_channel} não existe em desktop-ipc-channels.ts — o HUD ficaria mudo"
            );
        }
    }

    /// A fala automática vive do barramento: se o turno só fosse à janela, a Mika escreveria a
    /// resposta e nunca a diria.
    #[test]
    fn the_turn_reaches_the_bus_except_the_tool_result() {
        let chunk = turn_event_bus(&turn::TurnEvent::TextChunk("Oi.".into()), "oi");
        assert_eq!(chunk, Some((EventKey::StreamChunk, json!({ "text": "Oi." }))));
        assert_eq!(turn_event_bus(&turn::TurnEvent::ThinkingStart, "oi").map(|(k, _)| k), Some(EventKey::ThinkingStart));
        assert_eq!(turn_event_bus(&turn::TurnEvent::ThinkingStop, "oi").map(|(k, _)| k), Some(EventKey::ThinkingStop));
        let tool = turn_event_bus(&turn::TurnEvent::ToolCallStarted { name: "get_time".into() }, "oi");
        assert_eq!(tool.map(|(k, _)| k), Some(EventKey::ToolCall));
        let result = turn_event_bus(&turn::TurnEvent::ToolCallFinished { name: "x".into(), action: None, ok: true, result: json!({ "a": 1 }), duration_ms: 3 }, "oi");
        assert_eq!(result, Some((EventKey::ToolResult, json!({ "name": "x", "success": true, "result": { "a": 1 } }))));
    }

    // -- empty_message_response / has_attachments --------------------------------------------------

    #[test]
    fn has_attachments_is_false_for_missing_empty_or_absent_field() {
        assert!(!has_attachments(&json!({})));
        assert!(!has_attachments(&json!({ "attachments": [] })));
        assert!(!has_attachments(&json!({ "attachments": "not-an-array" })));
    }

    #[test]
    fn has_attachments_is_true_for_a_non_empty_array() {
        assert!(has_attachments(&json!({ "attachments": [{ "type": "image" }] })));
    }

    /// Espelha o placeholder de `chat/ipc.ts` para mensagem vazia sem anexo — mesmo texto, mesmo
    /// `role: "system"`.
    #[test]
    fn empty_message_response_without_attachments_matches_the_ts_placeholder() {
        let response = empty_message_response(3, false);
        assert_eq!(response["role"], "system");
        assert_eq!(response["toolCount"], 3);
        assert_eq!(response["content"], "Mensagem vazia. Envie um texto para conversar.");
        assert!(response["id"].as_str().unwrap().starts_with("empty-"));
    }

    /// Anexo sem texto: honesto sobre não suportar ainda, não finge que leu a imagem.
    #[test]
    fn empty_message_response_with_attachments_says_attachments_are_not_supported_yet() {
        let response = empty_message_response(0, true);
        assert_eq!(response["role"], "system");
        assert!(response["content"].as_str().unwrap().contains("Anexos"));
    }

    // -- blocked_by_health_response ------------------------------------------------------------------

    /// A resposta do latch cita o motivo CRU do provedor (mesmo texto que `ProviderHealthBanner`
    /// mostra) — nunca um erro genérico, mesmo MUST de "Superfície de UI" em 03-resilience.md.
    #[test]
    fn blocked_by_health_response_cites_the_raw_provider_reason() {
        let entry = provider_health::record_failure("test-blocked-response", "model-x", "modelo inválido (404)", Some(404));
        let response = blocked_by_health_response(2, &entry);
        assert_eq!(response["role"], "system");
        assert_eq!(response["toolCount"], 2);
        assert!(response["content"].as_str().unwrap().contains("modelo inválido (404)"));
        assert!(response["id"].as_str().unwrap().starts_with("blocked-"));
        provider_health::reset("test-blocked-response", "model-x");
    }

    // -- resolve_health_key -------------------------------------------------------------------------

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-chat-send-test-{}-{unique}", std::process::id()));
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

    #[test]
    fn resolve_health_key_defaults_to_ollama_with_the_documented_default_model() {
        let (store, _dir) = store();
        assert_eq!(resolve_health_key(&store), ("ollama".to_string(), "qwen3.5:4b".to_string()));
    }

    /// A chave de saúde segue a mesma prioridade de `brain::turn::resolve_ollama_config`:
    /// `llm.ollamaModel` (wizard/painel de conexão) vence `llm.model` (legado).
    #[test]
    fn resolve_health_key_prefers_ollama_model_over_the_legacy_model_field() {
        let (store, _dir) = store();
        store.set("llm.model", json!("legacy")).unwrap();
        store.set("llm.ollamaModel", json!("qwen2.5:7b")).unwrap();
        assert_eq!(resolve_health_key(&store), ("ollama".to_string(), "qwen2.5:7b".to_string()));
    }

    #[test]
    fn resolve_health_key_reads_the_configured_remote_provider_and_model() {
        let (store, _dir) = store();
        store.set("llm.provider", json!("openrouter")).unwrap();
        store.set("llm.openrouterModel", json!("anthropic/claude-3.5")).unwrap();
        assert_eq!(resolve_health_key(&store), ("openrouter".to_string(), "anthropic/claude-3.5".to_string()));
    }

    /// Unsloth não tem modelo padrão: o id é o que ele tem carregado (`GET /v1/models`), então a
    /// chave de saúde sem modelo escolhido fica vazia em vez de inventar um nome.
    #[test]
    fn resolve_health_key_reads_the_unsloth_provider_and_its_own_model_field() {
        let (store, _dir) = store();
        store.set("llm.provider", json!("unsloth")).unwrap();
        assert_eq!(resolve_health_key(&store), ("unsloth".to_string(), String::new()));
        store.set("llm.unslothModel", json!("gemma-4-26B-A4B-it-GGUF")).unwrap();
        assert_eq!(resolve_health_key(&store), ("unsloth".to_string(), "gemma-4-26B-A4B-it-GGUF".to_string()));
    }

    #[test]
    fn resolve_health_key_falls_back_to_ollama_for_an_unknown_provider_value() {
        let (store, _dir) = store();
        // O `ConfigStore` já recusaria um valor fora do enum do schema — este teste cobre só o
        // fallback do próprio `resolve_health_key`, mesmo padrão de `normalize_provider`.
        assert_eq!(resolve_health_key(&store).0, "ollama");
    }
}
