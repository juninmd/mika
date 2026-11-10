//! Porte de `voice/audio/speech-mute.ts` (que embrulha `createMuteTimer` do SDK): o prazo até
//! quando a Mika fica calada.
//!
//! **Por que isto existe.** A ação `mute` da tool `voice` já publicava `EventKey::MuteSpeech` com
//! o mesmo payload do `runMute` do TS — e a tabela de paridade do módulo dizia "portado". Só que
//! ninguém ASSINAVA a chave: o evento entrava no barramento e morria ali. Mandar a Mika calar a
//! boca por 10 minutos devolvia sucesso e ela continuava falando, sem erro e sem log. Paridade de
//! nome não é paridade de comportamento — o que se mede é a AÇÃO, não o evento publicado.
//!
//! O estado é um instante, não um booleano: `untilMs` no futuro = calada; passou do prazo, volta a
//! falar sozinha, sem ninguém ter de desligar. Mesma semântica do `MuteTimer` do TS, e o mesmo
//! motivo de não haver timer nenhum aqui — o prazo é CONSULTADO na hora de falar.

use std::sync::atomic::{AtomicI64, Ordering};

use serde_json::Value;

static MUTED_UNTIL_MS: AtomicI64 = AtomicI64::new(0);

/// O novo prazo a partir do payload de `MuteSpeech`. `{ muted: false }` (ou `untilMs` ausente)
/// zera; `{ muted: true, untilMs }` guarda o instante.
///
/// Pura para ser testável sem estático de processo — `cargo test` roda em paralelo, e um teste que
/// dependesse do estático viraria flaky pelo vizinho.
fn deadline_from(payload: &Value) -> i64 {
    if payload.get("muted").and_then(Value::as_bool) == Some(false) {
        return 0;
    }
    payload.get("untilMs").and_then(Value::as_i64).unwrap_or(0).max(0)
}

/// Aplica um `MuteSpeech` recebido do barramento.
pub(crate) fn apply(payload: &Value) {
    let current = MUTED_UNTIL_MS.load(Ordering::Relaxed);
    MUTED_UNTIL_MS.store(next_deadline(current, payload), Ordering::Relaxed);
}
/// Comando explícito substitui o prazo; silêncio antecipado (`source: "mute_learning"`) só estica.
fn next_deadline(current: i64, payload: &Value) -> i64 {
    let requested = deadline_from(payload);
    if payload.get("source").and_then(Value::as_str) == Some("mute_learning") {
        return current.max(requested);
    }
    requested
}

/// O prazo é COMPARADO, não é um booleano: venceu a hora, a Mika volta a falar sem ninguém
/// desligar nada. Pura pelo mesmo motivo de [`deadline_from`].
fn muted_at(deadline_ms: i64, now_ms: i64) -> bool {
    deadline_ms > now_ms
}

/// `true` enquanto o prazo não venceu — consultado por `speak_tool::run_speak`, o funil único por
/// onde toda fala passa (tool `speak`, ação `voice.speak` e pedido de fala do barramento).
pub(crate) fn is_muted(now_ms: i64) -> bool {
    muted_at(MUTED_UNTIL_MS.load(Ordering::Relaxed), now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_future_deadline_mutes_and_a_past_one_does_not() {
        assert_eq!(deadline_from(&json!({ "muted": true, "untilMs": 1_700_000_000_000_i64 })), 1_700_000_000_000);
        // Desligar explicitamente zera, mesmo com `untilMs` no payload — é o caso de `minutes <= 0`
        // em `run_mute`, que manda `{ muted: false, untilMs: 0 }`.
        assert_eq!(deadline_from(&json!({ "muted": false, "untilMs": 1_700_000_000_000_i64 })), 0);
        assert_eq!(deadline_from(&json!({})), 0);
        // Negativo não vira silêncio eterno por acidente de sinal.
        assert_eq!(deadline_from(&json!({ "muted": true, "untilMs": -5 })), 0);
    }

    /// Silêncio antecipado (`mute_learning`) só estica o prazo: um mudo de 10 min pedido pelo
    /// usuário não pode virar 20 s porque um boss apareceu. O comando explícito continua mandando.
    #[test]
    fn an_anticipated_mute_never_shortens_the_current_deadline() {
        let anticipated = json!({ "muted": true, "untilMs": 20_000, "source": "mute_learning" });
        assert_eq!(next_deadline(600_000, &anticipated), 600_000);
        assert_eq!(next_deadline(5_000, &anticipated), 20_000);
        let explicit = json!({ "muted": true, "untilMs": 20_000 });
        assert_eq!(next_deadline(600_000, &explicit), 20_000);
        assert_eq!(next_deadline(600_000, &json!({ "muted": false })), 0);
    }

    /// O prazo vence sozinho. Testado na função PURA, nunca no estático: `cargo test` roda o
    /// binário inteiro em paralelo, e um teste que deixasse o silêncio ligado por um instante
    /// faria qualquer teste de fala vizinho falhar sem razão aparente.
    #[test]
    fn the_mute_expires_by_itself() {
        assert!(muted_at(2_000, 1_999));
        assert!(!muted_at(2_000, 2_000));
        assert!(!muted_at(2_000, 2_001));
        // Sem prazo nenhum (nunca silenciada, ou silêncio desligado) não cala nem no instante 0.
        assert!(!muted_at(0, 0));
    }
}
