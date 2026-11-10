//! Porte fiel de `apps/desktop/src/main/services/voice/audio/barge-in-recorder.ts` — acumula
//! PCM16 mono 16 kHz num anel de pré-roll e, ao ser avisado de um barge-in, grava a janela seguinte
//! para transcrever depois (quem transcreve e o que fazer com o texto fica fora daqui, ver o
//! cabeçalho de `vad_pipeline.rs`).
//!
//! **Por que este arquivo é o dono do buffer, e não `crate::events`.** O comentário do lado TS é
//! explícito — "o áudio nunca é descartado" — porque cada amostra perdida no meio de uma captura
//! corrompe o trecho que seria transcrito. `crate::events::EventBus` é `tokio::sync::broadcast`:
//! DESCARTA para um assinante lento (ver o cabeçalho de `events.rs`), aceitável para `VadSample`,
//! não para o áudio bruto. Este recorder não é alimentado pelo barramento: `vad_pipeline.rs` chama
//! [`BargeInRecorder::absorb`] diretamente, em processo, na mesma chamada síncrona que já tem a
//! janela em mãos — sem canal, sem fila, sem ponto onde uma amostra possa ser descartada.
//!
//! **Sem o backstop de timer do TS.** `flushCapture` original também dispara por um `setTimeout`
//! solto, para o caso de o áudio parar de chegar no meio de uma captura. Aqui não existe esse
//! timer: o flush só acontece dentro de [`BargeInRecorder::absorb`], quando uma nova janela chega.
//! Isso é seguro enquanto o pipeline de captura continuar entregando janelas continuamente (mesma
//! garantia que `interruption_detector.rs` já assume); se o fluxo de áudio parar de vez no meio de
//! uma captura em curso, o flush por tempo decorrido nunca dispara — diferença de comportamento
//! documentada, não escondida.

use std::collections::VecDeque;

/// Mesmos números de `barge-in-recorder.ts` (`PRE_ROLL_MS`, `POST_BARGE_MS`, `DEFAULT_SAMPLE_RATE`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BargeInRecorderConfig {
    pub sample_rate: u32,
    pub pre_roll_ms: u64,
    pub post_barge_ms: u64,
}

impl Default for BargeInRecorderConfig {
    fn default() -> Self {
        Self { sample_rate: 16_000, pre_roll_ms: 300, post_barge_ms: 2_500 }
    }
}

fn samples_for(ms: u64, sample_rate: u32) -> usize {
    ((ms as f64 / 1000.0) * sample_rate as f64).round() as usize
}

pub struct BargeInRecorder {
    config: BargeInRecorderConfig,
    pre_roll_max_samples: usize,
    capture_max_samples: usize,
    pre_roll: VecDeque<i16>,
    capturing: bool,
    capture_started_at_ms: u64,
    capture: Vec<i16>,
}

impl BargeInRecorder {
    pub fn new(config: BargeInRecorderConfig) -> Self {
        let pre_roll_max_samples = samples_for(config.pre_roll_ms, config.sample_rate);
        let capture_max_samples = samples_for(config.post_barge_ms, config.sample_rate);
        Self {
            config,
            pre_roll_max_samples,
            capture_max_samples,
            pre_roll: VecDeque::with_capacity(pre_roll_max_samples),
            capturing: false,
            capture_started_at_ms: 0,
            capture: Vec::new(),
        }
    }

    /// Equivalente a assinar `Events.FORCE_STOP_TTS`: (re)inicia a janela de captura. Chamar de novo
    /// antes do primeiro flush substitui a captura em curso — mesmo comportamento do TS para dois
    /// `FORCE_STOP_TTS` seguidos (o timer/elapsed antigo é descartado, não somado).
    pub fn on_force_stop_tts(&mut self, now_ms: u64) {
        self.capturing = true;
        self.capture_started_at_ms = now_ms;
        self.capture.clear();
    }

    /// Equivalente a `BargeInRecorder.stop()`: cancela qualquer captura em curso e zera o pré-roll.
    /// Sem chamador em produção ainda — mesma lacuna de `VadPipeline::stop` (ver o cabeçalho de
    /// `vad_pipeline.rs`: nada encerra explicitamente uma `VadAudioSession`).
    #[allow(dead_code)]
    pub fn stop(&mut self) {
        self.pre_roll.clear();
        self.capture.clear();
        self.capturing = false;
    }

    /// Uma janela PCM16 mono 16 kHz. Fora de captura, alimenta o anel de pré-roll; em captura,
    /// acumula e devolve `Some(áudio mesclado)` quando o tempo decorrido ou o tamanho máximo é
    /// atingido — mesmas duas condições de `absorb`/`flushCapture` no TS (`elapsed >= POST_BARGE_MS
    /// || captureSamples >= postBargeMaxSamples`).
    pub fn absorb(&mut self, chunk: &[i16], now_ms: u64) -> Option<Vec<i16>> {
        if self.capturing {
            self.capture.extend_from_slice(chunk);
            let elapsed = now_ms.saturating_sub(self.capture_started_at_ms);
            if elapsed >= self.config.post_barge_ms || self.capture.len() >= self.capture_max_samples {
                return Some(self.flush());
            }
            return None;
        }

        self.pre_roll.extend(chunk.iter().copied());
        while self.pre_roll.len() > self.pre_roll_max_samples {
            self.pre_roll.pop_front();
        }
        None
    }

    fn flush(&mut self) -> Vec<i16> {
        self.capturing = false;
        let mut merged = Vec::with_capacity(self.pre_roll.len() + self.capture.len());
        merged.extend(self.pre_roll.iter().copied());
        merged.extend(self.capture.iter().copied());
        self.pre_roll.clear();
        self.capture.clear();
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorder() -> BargeInRecorder {
        BargeInRecorder::new(BargeInRecorderConfig::default())
    }

    fn chunk(value: i16, len: usize) -> Vec<i16> {
        vec![value; len]
    }

    #[test]
    fn audio_before_force_stop_tts_only_feeds_the_pre_roll_never_the_detector() {
        let mut r = recorder();
        for _ in 0..10 {
            assert_eq!(r.absorb(&chunk(1, 512), 0), None);
        }
    }

    #[test]
    fn pre_roll_keeps_only_the_most_recent_window() {
        let mut r = BargeInRecorder::new(BargeInRecorderConfig { pre_roll_ms: 10, sample_rate: 1_000, post_barge_ms: 1_000 });
        // 10ms @ 1kHz = 10 amostras de pré-roll. Manda 30 amostras: só as 10 últimas devem sobrar.
        r.absorb(&chunk(1, 20), 0);
        r.absorb(&chunk(2, 10), 0);
        r.on_force_stop_tts(0);
        let merged = r.absorb(&chunk(0, 0), 2_000).expect("elapsed >= post_barge_ms força o flush");
        assert_eq!(merged, chunk(2, 10));
    }

    #[test]
    fn flushed_audio_includes_the_pre_roll_captured_before_force_stop_tts() {
        let mut r = BargeInRecorder::new(BargeInRecorderConfig { pre_roll_ms: 1_000, sample_rate: 1_000, post_barge_ms: 1_000 });
        r.absorb(&chunk(9, 5), 0); // pré-roll
        r.on_force_stop_tts(100);
        r.absorb(&chunk(1, 5), 200);
        let merged = r.absorb(&chunk(1, 5), 1_200).expect("flush por tempo decorrido");
        assert_eq!(merged, [chunk(9, 5), chunk(1, 5), chunk(1, 5)].concat());
    }

    #[test]
    fn flush_fires_on_elapsed_time_even_before_the_sample_ceiling() {
        let mut r = BargeInRecorder::new(BargeInRecorderConfig { pre_roll_ms: 0, sample_rate: 16_000, post_barge_ms: 50 });
        r.on_force_stop_tts(0);
        assert_eq!(r.absorb(&chunk(1, 100), 10), None);
        let merged = r.absorb(&chunk(1, 100), 60).expect("50ms decorridos");
        assert_eq!(merged.len(), 200);
    }

    #[test]
    fn flush_fires_on_the_sample_ceiling_even_before_the_time_elapses() {
        let mut r = BargeInRecorder::new(BargeInRecorderConfig { pre_roll_ms: 0, sample_rate: 1_000, post_barge_ms: 60_000 });
        r.on_force_stop_tts(0);
        // Teto de amostras = 60s * 1kHz = 60_000; entregue de uma vez, tempo decorrido é zero.
        let merged = r.absorb(&chunk(1, 60_000), 0).expect("teto de amostras atingido");
        assert_eq!(merged.len(), 60_000);
    }

    #[test]
    fn a_second_force_stop_tts_before_the_first_flush_restarts_the_capture() {
        let mut r = BargeInRecorder::new(BargeInRecorderConfig { pre_roll_ms: 0, sample_rate: 1_000, post_barge_ms: 1_000 });
        r.on_force_stop_tts(0);
        r.absorb(&chunk(1, 100), 100);
        r.on_force_stop_tts(200); // reinicia: descarta o `chunk(1, 100)` acumulado
        r.absorb(&chunk(2, 50), 300);
        let merged = r.absorb(&chunk(2, 50), 1_300).expect("flush por tempo decorrido desde o SEGUNDO force-stop");
        assert_eq!(merged, chunk(2, 100));
    }

    #[test]
    fn stop_discards_pre_roll_and_any_capture_in_progress() {
        let mut r = recorder();
        r.absorb(&chunk(1, 512), 0);
        r.on_force_stop_tts(0);
        r.absorb(&chunk(1, 512), 10);
        r.stop();

        assert_eq!(r.absorb(&chunk(0, 0), 100_000), None, "sem captura em curso depois de stop()");
    }

    #[test]
    fn no_audio_window_is_ever_lost_between_pre_roll_and_capture() {
        // `sample_rate: 512` faz de cada chunk de 512 amostras exatamente 1000ms de áudio — só para
        // deixar as contas de pré-roll/teto redondas neste teste.
        let mut r = BargeInRecorder::new(BargeInRecorderConfig { pre_roll_ms: 3_000, sample_rate: 512, post_barge_ms: 3_000 });
        let pre = [chunk(0, 512), chunk(1, 512), chunk(2, 512)];
        // pre_roll_max_samples = 1536 = exatamente os 3 chunks: nenhum é descartado pelo anel.
        for (i, c) in pre.iter().enumerate() {
            assert_eq!(r.absorb(c, i as u64 * 1_000), None);
        }
        r.on_force_stop_tts(3_000);

        let post = [chunk(3, 512), chunk(4, 512), chunk(5, 512)];
        assert_eq!(r.absorb(&post[0], 4_000), None);
        assert_eq!(r.absorb(&post[1], 5_000), None);
        // capture_max_samples = 1536 = exatamente os 3 chunks de captura: flush no 3º, nem antes nem depois.
        let merged = r.absorb(&post[2], 6_000).expect("teto de amostras da captura atingido no 3º chunk");

        let mut expected = Vec::new();
        for c in pre.iter().chain(post.iter()) {
            expected.extend_from_slice(c);
        }
        assert_eq!(merged, expected, "cada amostra enviada, pré-roll e captura, tem de aparecer no áudio mesclado");
    }
}
