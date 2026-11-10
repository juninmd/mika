//! Tempo até o primeiro áudio de UM turno de fala automática (item 4 da rodada, observabilidade
//! `specs/10-observability.md`) — a métrica gêmea de "tempo até o primeiro token" que
//! `brain::turn.rs` já loga, só que medida deste lado: quando a PRIMEIRA síntese de fala do turno
//! termina, não quando o texto chega. This one starts at `chat:thinking-start`, which the turn now
//! announces before composing the prompt — so it also counts prompt preparation, the wait the user
//! actually feels; `brain::turn.rs`'s first-token clock starts only once the model is called.
//!
//! Dois `Arc<Atomic*>`, não um campo em `TurnState`: o relógio é zerado pela task que ASSINA o
//! barramento (`spawn_auto_speak`'s no evento `ThinkingStart`), mas quem TERMINA de sintetizar e
//! precisa comparar contra ele é [`super::spawn_speak_worker`], numa task SEPARADA sem acesso ao
//! `TurnState` da primeira — ver o cabeçalho de `mod.rs` para o porquê da síntese morar numa fila.
//!
//! Decisão pura, log no chamador: `note_audio_ready` só devolve o `Some(ms)` da primeira síntese do
//! turno (`None` daí em diante, ou se nenhum turno começou) — quem loga é `spawn_speak_worker`,
//! mesmo padrão de `failure_tracker::record_failure` (devolve a dica, o chamador loga).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct TurnTiming {
    started_ms: Arc<AtomicU64>,
    /// Começa em `true` (nada para logar): um `SpeechRequest` fora de qualquer turno de chat não
    /// pode produzir uma linha "primeiro áudio" órfã.
    first_audio_logged: Arc<AtomicBool>,
}

impl TurnTiming {
    pub(super) fn new() -> Self {
        Self { started_ms: Arc::new(AtomicU64::new(0)), first_audio_logged: Arc::new(AtomicBool::new(true)) }
    }

    /// `chat:thinking-start`: novo turno, novo relógio.
    pub(super) fn mark_turn_started(&self, now_ms: u64) {
        self.started_ms.store(now_ms, Ordering::SeqCst);
        self.first_audio_logged.store(false, Ordering::SeqCst);
    }

    /// Chamado depois de QUALQUER síntese bem-sucedida; `compare_exchange` garante que só a
    /// primeira do turno vira `Some` — as seguintes (e qualquer chamada fora de um turno marcado)
    /// devolvem `None`.
    pub(super) fn note_audio_ready(&self, now_ms: u64) -> Option<u64> {
        self.first_audio_logged.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).ok()?;
        let started = self.started_ms.load(Ordering::SeqCst);
        (started > 0).then(|| now_ms.saturating_sub(started))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_audio_after_a_turn_start_reports_its_elapsed_ms() {
        let timing = TurnTiming::new();
        timing.mark_turn_started(1_000);
        assert_eq!(timing.note_audio_ready(1_180), Some(180));
    }

    #[test]
    fn a_second_synthesis_in_the_same_turn_reports_nothing() {
        let timing = TurnTiming::new();
        timing.mark_turn_started(1_000);
        assert_eq!(timing.note_audio_ready(1_100), Some(100));
        assert_eq!(timing.note_audio_ready(1_500), None, "só a primeira síntese do turno é notícia");
    }

    #[test]
    fn a_synthesis_before_any_turn_started_reports_nothing() {
        let timing = TurnTiming::new();
        assert_eq!(timing.note_audio_ready(1_000), None, "SpeechRequest solto, sem turno, não loga nada");
    }

    #[test]
    fn a_new_turn_start_re_arms_the_reporter() {
        let timing = TurnTiming::new();
        timing.mark_turn_started(1_000);
        timing.note_audio_ready(1_050);

        timing.mark_turn_started(2_000);

        assert_eq!(timing.note_audio_ready(2_090), Some(90), "o próximo turno tem de poder logar de novo");
    }
}
