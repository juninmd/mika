//! Rastreia se o usuário está falando agora, INDEPENDENTE de a Mika estar falando ou não — o sinal
//! que `brain::awareness::engine::ProactiveEngine::should_speak` precisa para não começar uma fala
//! proativa por cima do usuário (defects.md "barge-in", item 3: smoke-baseline.md #8 flagrou um
//! comentário proativo falado ENQUANTO o usuário ainda falava).
//!
//! **Por que não reusar `InterruptionDetector`.** Toda a máquina de estado de lá (`speaking`,
//! `streak`, `cooldown`) só importa quando `self.speaking` (a Mika) é verdadeiro — é o detector de
//! BARGE-IN, não um monitor geral de atividade de voz. O instante que a proatividade precisa vigiar
//! é exatamente o OPOSTO: a Mika está CALADA, prestes a puxar assunto. Um rastreador separado, mais
//! simples (só liga/desliga por janela), evita reaproveitar um estado que não foi desenhado para
//! isso.
//!
//! **Alimentado por `vad_audio.rs::process_payload`**, uma vez por janela, com a MESMA probabilidade
//! Silero e o MESMO limiar (`interruption_detector::NEURAL_SPEECH_THRESHOLD`) que o pipeline já
//! calcula ali — nenhuma segunda passada de VAD.

use std::sync::{Mutex, OnceLock};

/// A Mika não pode COMEÇAR uma fala proativa por até isto depois da última janela com voz do
/// usuário — smoke-baseline.md #8.
pub const GRACE_MS: u64 = 1_500;

/// Um heartbeat (janela recebida, com ou sem voz) mais velho que isto torna o estado rastreado não
/// confiável — válvula de segurança: se a sessão de VAD parar de alimentar este tracker (mic
/// desligado no meio de uma fala, sessão nunca recriada), uma transição "parou de falar" perdida não
/// pode calar a Mika para sempre. Bem acima de `GRACE_MS`: só entra em jogo quando o tracker parou de
/// receber janelas de vez, nunca numa pausa comum de conversa.
pub const STALE_MS: u64 = 30_000;

#[derive(Default)]
struct State {
    /// `Some(t)` = a última janela com voz aconteceu em `t`. Não é limpo por uma janela silenciosa —
    /// a decisão de "ainda dentro da graça" mora em [`should_defer`], não neste campo.
    last_voice_at_ms: Option<u64>,
    /// TODA janela, com ou sem voz — o heartbeat que `STALE_MS` vigia.
    last_window_at_ms: Option<u64>,
    /// Última janela com transiente de tecla sem voz (`speech_gate.rs`) — o que `transcribe.rs`
    /// pergunta para descartar um trecho que foi só digitação.
    last_keystroke_at_ms: Option<u64>,
}

pub struct UserSpeechActivity {
    state: Mutex<State>,
}

impl UserSpeechActivity {
    pub fn new() -> Self {
        Self { state: Mutex::new(State::default()) }
    }

    /// Chamado uma vez por janela do VAD, Mika falando ou não — ver o cabeçalho do arquivo.
    pub fn note_window(&self, is_voice: bool, keystroke: bool, now_ms: u64) {
        let Ok(mut state) = self.state.lock() else { return };
        state.last_window_at_ms = Some(now_ms);
        if is_voice {
            state.last_voice_at_ms = Some(now_ms);
        }
        if keystroke {
            state.last_keystroke_at_ms = Some(now_ms);
        }
    }

    /// `true` = a proatividade deve ADIAR (não descartar) a fala agora — ver [`should_defer`].
    pub fn should_defer_proactive_speech(&self, now_ms: u64) -> bool {
        let Ok(state) = self.state.lock() else { return false };
        should_defer(state.last_voice_at_ms, state.last_window_at_ms, now_ms)
    }

    /// Veredito do Silero para os últimos `window_ms` — ver [`voiced_within`].
    pub fn heard_voice_within(&self, window_ms: u64, now_ms: u64) -> Option<bool> {
        let Ok(state) = self.state.lock() else { return None };
        voiced_within(state.last_voice_at_ms, state.last_window_at_ms, window_ms, now_ms)
    }

    /// O VAD viu tecla nos últimos `window_ms` — `false` sem VAD vivo (falha aberto).
    pub fn heard_keystrokes_within(&self, window_ms: u64, now_ms: u64) -> bool {
        let Ok(state) = self.state.lock() else { return false };
        voiced_within(state.last_keystroke_at_ms, state.last_window_at_ms, window_ms, now_ms) == Some(true)
    }
}

impl Default for UserSpeechActivity {
    fn default() -> Self {
        Self::new()
    }
}

/// Decisão pura, estado injetado — testável sem o `Mutex`/singleton. Ver `GRACE_MS`/`STALE_MS`.
fn should_defer(last_voice_at_ms: Option<u64>, last_window_at_ms: Option<u64>, now_ms: u64) -> bool {
    let Some(heartbeat) = last_window_at_ms else { return false }; // VAD nunca rodou: nada para vigiar.
    if now_ms.saturating_sub(heartbeat) > STALE_MS {
        return false; // tracker mudo há muito tempo — estado não confiável, falha aberto.
    }
    let Some(voiced_at) = last_voice_at_ms else { return false };
    now_ms.saturating_sub(voiced_at) < GRACE_MS
}

/// `None` = sem veredito (VAD nunca rodou ou parou de receber janelas): quem consulta falha aberto.
fn voiced_within(last_voice_at_ms: Option<u64>, last_window_at_ms: Option<u64>, window_ms: u64, now_ms: u64) -> Option<bool> {
    let heartbeat = last_window_at_ms?;
    if now_ms.saturating_sub(heartbeat) > STALE_MS {
        return None;
    }
    Some(last_voice_at_ms.is_some_and(|voiced_at| now_ms.saturating_sub(voiced_at) <= window_ms))
}

static GLOBAL: OnceLock<UserSpeechActivity> = OnceLock::new();

/// O tracker de processo que `vad_audio.rs` alimenta e `ProactiveEngine` lê — ver o cabeçalho do
/// arquivo. Testes usam `UserSpeechActivity::new()` isolado (mesmo motivo de `listening_state::global`).
pub fn global() -> &'static UserSpeechActivity {
    GLOBAL.get_or_init(UserSpeechActivity::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_window_ever_seen_never_defers() {
        assert!(!should_defer(None, None, 1_000));
    }

    #[test]
    fn currently_voiced_defers() {
        assert!(should_defer(Some(1_000), Some(1_000), 1_000));
    }

    #[test]
    fn within_the_grace_window_after_speech_stops_still_defers() {
        assert!(should_defer(Some(1_000), Some(2_490), 2_490), "1490ms depois da última janela com voz");
    }

    #[test]
    fn past_the_grace_window_no_longer_defers() {
        assert!(!should_defer(Some(1_000), Some(2_510), 2_510), "1510ms depois — passou dos 1500ms de graça");
    }

    /// Válvula de segurança: heartbeat velho (>30s) falha ABERTO, mesmo que a última janela vista
    /// tivesse voz — sem isto, uma sessão de VAD morta silenciaria a Mika para sempre.
    #[test]
    fn a_stale_heartbeat_fails_open_even_if_the_last_window_was_voiced() {
        assert!(!should_defer(Some(1_000), Some(1_000), 31_500));
    }

    #[test]
    fn a_heartbeat_just_inside_the_stale_ceiling_still_gates_when_voice_is_recent() {
        assert!(should_defer(Some(31_400), Some(31_400), 31_400));
    }

    #[test]
    fn note_window_feeds_the_real_instance_and_should_defer_reads_it_back() {
        let tracker = UserSpeechActivity::new();
        assert!(!tracker.should_defer_proactive_speech(0), "sem nenhuma janela ainda, nunca adia");

        tracker.note_window(true, false, 1_000);
        assert!(tracker.should_defer_proactive_speech(1_100), "dentro da graça logo após a voz");

        tracker.note_window(false, false, 3_000); // usuário parou de falar; heartbeat renovado
        assert!(
            !tracker.should_defer_proactive_speech(3_000),
            "3000-1000=2000ms > GRACE_MS, mesmo com o heartbeat fresco"
        );
    }

    /// Veredito do motor `silero` para a fala recortada: só existe se o VAD está vivo.
    #[test]
    fn voiced_within_answers_only_while_the_vad_is_alive() {
        assert_eq!(voiced_within(None, None, 2_000, 5_000), None, "VAD nunca rodou");
        assert_eq!(voiced_within(Some(1_000), Some(1_000), 2_000, 40_000), None, "heartbeat velho");
        assert_eq!(voiced_within(Some(4_000), Some(5_000), 2_000, 5_000), Some(true));
        assert_eq!(voiced_within(Some(1_000), Some(5_000), 2_000, 5_000), Some(false), "voz fora da janela");
        assert_eq!(voiced_within(None, Some(5_000), 2_000, 5_000), Some(false), "VAD vivo, nunca ouviu voz");
    }

    /// `transcribe.rs` só descarta por digitação quando o VAD VIU tecla na janela e nenhuma voz.
    #[test]
    fn keystrokes_within_is_answered_only_while_the_vad_is_alive() {
        let tracker = UserSpeechActivity::new();
        assert!(!tracker.heard_keystrokes_within(2_000, 5_000), "VAD nunca rodou");
        tracker.note_window(false, true, 4_000);
        tracker.note_window(false, false, 5_000);
        assert!(tracker.heard_keystrokes_within(2_000, 5_000));
        assert!(!tracker.heard_keystrokes_within(500, 5_000), "tecla fora da janela");
        assert!(!tracker.heard_keystrokes_within(2_000, 40_000), "heartbeat velho");
    }

    #[test]
    fn global_returns_the_same_process_wide_instance() {
        assert!(std::ptr::eq(global(), global()));
    }
}
