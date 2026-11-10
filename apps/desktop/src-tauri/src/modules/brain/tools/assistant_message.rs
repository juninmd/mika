//! Payload de `EventKey::AssistantMessage`, compartilhado por `content::daily_recap` e
//! `collaboration_session::run_debate` — as duas tools desta rodada cujo texto gerado por LLM só
//! chega à UI se publicado aqui (ver `events/keys.rs` para a chave e `modules/chat/
//! assistant_bridge.rs` para quem encaminha isto ao IPC do renderer). Extraído porque as duas
//! chamadas de `eventBusPort()?.emit(Events.ASSISTANT_MESSAGE, ...)` do TS
//! (`content-pipeline.service.ts::generateDailyRecap`, `collaboration.service.ts::nextTurn`)
//! publicam o MESMO formato — `collaboration` só soma `isCollaboration: true`.
//!
//! **`&EventBus` injetado, não `crate::events::bus()` direto** — mesmo padrão de
//! `voice::tts::bus_bridge::EventBusObserver`. Produção sempre passa `crate::events::bus()`
//! (`content.rs`/`collaboration_session.rs`); os testes deste arquivo passam uma instância isolada
//! (`EventBus::new()`), o que os tira da disputa por singleton de processo que `events/bus.rs`
//! documenta (`cargo test` roda o binário inteiro em paralelo).

use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::events::{EventBus, EventKey};

pub(super) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Porte de `content-pipeline.service.ts::generateDailyRecap`'s
/// `eventBusPort()?.emit(Events.ASSISTANT_MESSAGE, { content, timestamp })`.
pub(super) fn publish_recap(bus: &EventBus, content: &str) {
    bus.publish(EventKey::AssistantMessage, json!({ "content": content, "timestamp": now_ms() }));
}

/// Porte de `collaboration.service.ts::nextTurn`'s
/// `eventBusPort()?.emit(Events.ASSISTANT_MESSAGE, { content: \`[${currentAgent}]: ${lastMessage}\`,
/// timestamp, isCollaboration: true })`. `formatted_content` já vem como `[agente]: fala` — quem
/// chama monta essa string (ver `collaboration_session.rs::run_debate`).
pub(super) fn publish_debate_turn(bus: &EventBus, formatted_content: &str) {
    bus.publish(
        EventKey::AssistantMessage,
        json!({ "content": formatted_content, "timestamp": now_ms(), "isCollaboration": true }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_recap_has_content_and_timestamp_without_the_collaboration_flag() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();

        publish_recap(&bus, "recap de teste");

        let envelope = sub.recv().await.expect("publish_recap devia publicar");
        assert_eq!(envelope.key, EventKey::AssistantMessage);
        assert_eq!(envelope.payload["content"], json!("recap de teste"));
        assert!(envelope.payload["timestamp"].is_u64());
        assert!(envelope.payload.get("isCollaboration").is_none(), "recap não é debate");
    }

    #[tokio::test]
    async fn publish_debate_turn_carries_the_formatted_content_and_the_collaboration_flag() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();

        publish_debate_turn(&bus, "[Mika]: e aí, bora jogar RPG?");

        let envelope = sub.recv().await.expect("publish_debate_turn devia publicar");
        assert_eq!(envelope.key, EventKey::AssistantMessage);
        assert_eq!(envelope.payload["content"], json!("[Mika]: e aí, bora jogar RPG?"));
        assert_eq!(envelope.payload["isCollaboration"], json!(true));
    }
}
