//! Ponte EventBus -> IPC para `EventKey::AssistantMessage` — sem isto, `brain::tools::content`
//! (`daily_recap`) e `brain::tools::collaboration` (`collaboration_session::run_debate`) publicam
//! no barramento de processo (`crate::events`) e nenhuma janela jamais recebe. Espelha
//! `registerRendererEventBridge` (`apps/desktop/src/main/infrastructure/renderer-event-bridge.ts`)
//! só para ESTE canal — o bridge genérico completo (todo `MIKA_BRIDGED_EVENTS`, ~20 chaves) é maior
//! que o escopo desta rodada; ver o cabeçalho de `crate::events` ("ligar cada publicador/assinante
//! real ao canal IPC continua sendo trabalho de quem tiver o módulo específico... no escopo").
//!
//! **Canal**: `MIKA_BRIDGED_EVENT_CHANNELS['chat:assistant-message']`
//! (`apps/desktop/src/shared/mika-event-channels.ts`) resolve para
//! `DESKTOP_EVENT_CHANNELS.ChatAssistantMessage` (`packages/sdk/src/contracts/desktop-ipc-
//! channels.ts`) = `'mika:event:chat:assistant-message'`. `mika-runtime-chat.ts::onAssistantMessage`
//! é quem escuta do lado renderer, com payload `{ content, timestamp, usage? }` — `usage` nunca
//! sai daqui (só o turno principal de chat mede uso de token; recap/debate não).
//!
//! **`emit_to` nos alvos do TS, nunca `app.emit`.** `chat:assistant-message` está FORA de
//! `ALL_WINDOW_EVENTS` no TS, então `sendRendererEvent` manda só para a janela "main" quando ela
//! existe, e no fallback filtra `plugin-view`. [`assistant_message_targets`] reproduz essa regra.
//!
//! A primeira versão deste arquivo usava `app.emit`, que alcança TODA janela viva — e a diferença
//! foi anotada como "simplificação nomeada". Não é simplificação: `plugin-view` é exatamente a
//! superfície que este repositório trata como não-confiável, a que `is_owner_surface` barra em 10
//! rotas de IPC. O texto que a Mika diz é conversa do usuário, e entregá-lo a uma view de plugin
//! que o TS nunca alcançou é alargar superfície — só que por acidente de API, sem rota nova e sem
//! nada falhando, que é o jeito mais fácil de isso passar despercebido.
//!
//! **Não fecha a outra metade do TS.** `voice-auto-speak-events.ts` TAMBÉM assina
//! `Events.ASSISTANT_MESSAGE` para falar o texto em voz alta (`auto-speak.service.ts`) — isto aqui
//! só resolve "o texto chega ao painel de chat", não "a Mika fala o recap/debate". Ligar isso é
//! trabalho de `modules/voice`, fora do escopo desta rodada (ver o pedido original: só
//! `events/`, `modules/chat/` e as duas tools de `modules/brain/tools`).
//!
//! **Sem teste de fiação real.** `app.emit` exige um `AppHandle` de verdade; este crate não
//! habilita a feature `test-support` do `tauri` (mesma restrição documentada em
//! `chat/send.rs`/`chat/history_store.rs`). O que fica testável sem isso — o filtro de chave e o
//! literal do canal contra o contrato TS — está coberto abaixo.

use tauri::{AppHandle, Emitter, Manager};

use crate::events::{EventBus, EventKey};
use crate::protocol::renderer_event_targets;

/// Espelha `DESKTOP_EVENT_CHANNELS.ChatAssistantMessage`.
const ASSISTANT_MESSAGE_IPC_EVENT: &str = "mika:event:chat:assistant-message";

fn is_assistant_message(key: EventKey) -> bool {
    key == EventKey::AssistantMessage
}

/// Assina `bus` e reenvia cada `EventKey::AssistantMessage` para todas as janelas vivas. Chamado só
/// em produção (`modules/mod.rs::register_config_bound_tools`, o único ponto com `AppHandle`) —
/// mesmo padrão de `voice::tts::bus_bridge::spawn_force_stop_listener`.
pub(crate) fn spawn_assistant_message_bridge(bus: &'static EventBus, app: AppHandle) {
    // Assina ANTES do `tokio::spawn`, não dentro do bloco async: `broadcast` não entrega
    // retroativamente, e `tokio::spawn` só AGENDA a task — mesmo aviso do cabeçalho de
    // `voice::tts::bus_bridge::spawn_force_stop_listener`.
    let mut sub = bus.subscribe();
    // `tauri::async_runtime::spawn`, NUNCA `tokio::spawn` — ver o gate em `modules/mod.rs`: esta
    // função roda em `lib.rs::setup`, fora do contexto do reator, e `tokio::spawn` ali panica.
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if is_assistant_message(envelope.key) {
                emit_to_renderer(&app, &envelope.payload);
            }
        }
    });
}

/// Sends one assistant message straight to the chat surfaces, bypassing the bus: a normal chat
/// turn must reach the renderer (bubble, idle status, session usage) like TS's
/// `chat-history.service` did, but must not wake `auto_speak`, which already spoke it from the stream.
pub(crate) fn emit_to_renderer(app: &AppHandle, payload: &serde_json::Value) {
    for label in assistant_message_targets(app) {
        let _ = app.emit_to(label.as_str(), ASSISTANT_MESSAGE_IPC_EVENT, payload.clone());
    }
}

/// `{content, timestamp, usage}` — `onAssistantMessage`'s contract (`UsageMetadata` is camelCase).
/// `contextWindow` feeds the renderer's context-usage badge, which stays hidden without it.
pub(crate) fn turn_payload(
    content: &str,
    timestamp_ms: u64,
    usage: &rig_core::completion::Usage,
    cost_usd: Option<f64>,
    context_window: Option<u64>,
) -> serde_json::Value {
    // Contador não informado (`None`) vai como 0, o que o renderer sempre recebeu.
    let (input, output, total) = (usage.input_tokens.unwrap_or(0), usage.output_tokens.unwrap_or(0), usage.total_tokens.unwrap_or(0));
    let mut usage_json = serde_json::json!({
        "promptTokens": input,
        "completionTokens": output,
        "totalTokens": if total == 0 { input + output } else { total },
    });
    if let Some(cost) = cost_usd {
        usage_json["costUsd"] = serde_json::json!(cost);
    }
    if let Some(window) = context_window {
        usage_json["contextWindow"] = serde_json::json!(window);
    }
    serde_json::json!({ "content": content, "timestamp": timestamp_ms, "usage": usage_json })
}

/// Espelha `sendRendererEvent` para uma chave FORA de `ALL_WINDOW_EVENTS` (que é o caso de
/// `chat:assistant-message`): janela `main` quando existe; senão, toda janela que não seja
/// `plugin-view`.
///
/// **Por que não `app.emit`.** `app.emit` alcança TODA janela viva, inclusive `plugin-view` — e
/// plugin-view é justamente a superfície que este repositório trata como não-confiável, a mesma que
/// `assertOwnerSurface`/`is_owner_surface` barram em 10 rotas de IPC. Mandar a fala da Mika para lá
/// entregaria conversa do usuário a uma view de plugin que o TS nunca alcançou. Alargar superfície
/// por acidente de API é mais fácil de não notar do que uma rota nova, porque nada falha.
/// A regra em si vive em [`crate::protocol::renderer_event_targets`], não aqui: é política de
/// SUPERFÍCIE, compartilhada com `features::character::expression_bridge`, e uma regra de confiança
/// duplicada erra uma cópia de cada vez. Os casos que a provam moram junto dela.
fn assistant_message_targets(app: &AppHandle) -> Vec<String> {
    renderer_event_targets(app.webview_windows().keys().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_payload_matches_the_on_assistant_message_contract() {
        let mut usage = rig_core::completion::Usage::new();
        usage.input_tokens = Some(10);
        usage.output_tokens = Some(5);
        let payload = turn_payload("oi", 42, &usage, Some(0.0), Some(32768));
        assert_eq!(payload["content"], "oi");
        assert_eq!(payload["timestamp"], 42);
        assert_eq!(payload["usage"]["promptTokens"], 10);
        assert_eq!(payload["usage"]["completionTokens"], 5);
        assert_eq!(payload["usage"]["totalTokens"], 15);
        assert_eq!(payload["usage"]["costUsd"], 0.0);
        assert_eq!(payload["usage"]["contextWindow"], 32768);
        assert!(turn_payload("oi", 42, &usage, None, None)["usage"].get("contextWindow").is_none());
    }

    #[test]
    fn only_assistant_message_is_forwarded() {
        assert!(is_assistant_message(EventKey::AssistantMessage));
        assert!(!is_assistant_message(EventKey::Notification));
        assert!(!is_assistant_message(EventKey::MoodChanged));
    }

    /// Um literal errado aqui deixaria o painel de chat mudo sem erro nenhum — mesma checagem de
    /// `chat/send.rs::event_channel_literals_exist_in_the_ts_contract`.
    #[test]
    fn channel_literal_exists_in_the_ts_contract() {
        let channels_contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(
            channels_contract.contains(&format!("'{ASSISTANT_MESSAGE_IPC_EVENT}'")),
            "canal {ASSISTANT_MESSAGE_IPC_EVENT} não existe em desktop-ipc-channels.ts"
        );
    }

    /// Esta ponte NÃO pode voltar a usar `app.emit`, que alcança toda janela viva — inclusive
    /// `plugin-view`. Os casos da regra de alvo moram em `protocol.rs`, com a regra; o que se prova
    /// aqui é que este arquivo continua passando por ela, e não por uma cópia local ou pelo emit
    /// aberto. Gate de fonte porque a diferença não falha em teste nenhum: `app.emit` entregaria a
    /// fala da Mika a mais janelas, com sucesso, em silêncio.
    #[test]
    fn the_bridge_routes_through_the_shared_surface_rule_and_never_broadcasts() {
        let source = include_str!("assistant_bridge.rs");
        let production: String = source
            .split("mod tests {")
            .next()
            .unwrap_or_default()
            .lines()
            .map(|line| line.split("//").next().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(production.contains("renderer_event_targets("), "a ponte deixou de usar a regra compartilhada de alvo");
        assert!(!production.contains("app.emit("), "`app.emit` alcança plugin-view — use `emit_to` nos alvos de renderer_event_targets");
    }
}
