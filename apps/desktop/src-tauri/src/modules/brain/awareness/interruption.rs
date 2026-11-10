//! Consumidor de `EventKey::Interruption` — port de `brain-event-handlers.ts::replyToInterruption` +
//! `constants.ts::interruptionPrompt`. Adição de escopo pedida por outro agente desta rodada (porte
//! do barge-in de voz, que publica esta chave); mora aqui e não em `modules/voice` porque é reação
//! do lado do CÉREBRO à interrupção, não a detecção dela.
//!
//! **Payload assumido** (combinado com o agente que porta a publicação, lado `voice`):
//! `{ transcript, is_interruption, spokenPrefix, timestamp }` — os mesmos 3 campos que
//! `replyToInterruption` lê no TS (`timestamp` não é usado, só citado por paridade). Se o payload
//! real divergir quando a publicação aterrissar, é aqui que se ajusta — sem chave nova em
//! `events/keys.rs` (já existe `EventKey::Interruption`).
//!
//! **Sem gate de cadência.** O TS também não aplica `passesCadence`/dedup aqui: uma interrupção real
//! do usuário sempre merece resposta, ao contrário de um candidato do tick periódico.

use serde_json::Value;

use super::prompts::interruption_reply;

/// `None` quando o payload não descreve uma interrupção válida (sem transcript, ou
/// `is_interruption` ausente/`false`) — mesmo filtro de `replyToInterruption`.
pub fn interruption_prompt_from_payload(payload: &Value) -> Option<String> {
    let transcript = payload.get("transcript").and_then(Value::as_str)?.trim();
    if transcript.is_empty() {
        return None;
    }
    if payload.get("is_interruption").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let spoken_prefix = payload.get("spokenPrefix").and_then(Value::as_str).unwrap_or("");
    match interruption_reply(transcript, spoken_prefix) {
        Ok(prompt) => Some(prompt),
        Err(error) => {
            log::error!("resposta a interrupção: prompt não carregou: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_a_prompt_only_when_transcript_and_is_interruption_are_both_present() {
        let payload = json!({ "transcript": "espera", "is_interruption": true, "spokenPrefix": "eu ia dizer" });
        let prompt = interruption_prompt_from_payload(&payload).unwrap();
        assert!(prompt.contains("espera"));
        assert!(prompt.contains("eu ia dizer"));
    }

    #[test]
    fn no_prompt_without_is_interruption_true() {
        assert!(interruption_prompt_from_payload(&json!({ "transcript": "oi", "is_interruption": false })).is_none());
        assert!(interruption_prompt_from_payload(&json!({ "transcript": "oi" })).is_none());
    }

    #[test]
    fn no_prompt_for_an_empty_or_missing_transcript() {
        assert!(interruption_prompt_from_payload(&json!({ "transcript": "  ", "is_interruption": true })).is_none());
        assert!(interruption_prompt_from_payload(&json!({ "is_interruption": true })).is_none());
    }

    #[test]
    fn missing_spoken_prefix_still_builds_a_prompt() {
        let prompt = interruption_prompt_from_payload(&json!({ "transcript": "para", "is_interruption": true })).unwrap();
        assert!(prompt.contains("para"));
    }
}
