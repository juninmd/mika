//! A sessão de captura real: [`VadStream`] roda `SileroVad` (`inference::vad`) por janela PCM16 e
//! publica no barramento (`crate::events`) o equivalente a `Events.VAD_AUDIO`/`Events.VAD_SAMPLE`
//! (`ipc.ts::registerVoiceIPC`, rota `voiceVadAudio`).
//!
//! Em produção quem o usa é [`super::vad_pipeline::VadPipeline`] (`open` + `process` por baixo),
//! dono da única sessão, alimentado pelo canal `voice:vad-audio` que `vad_audio.rs` reivindica —
//! `barge_in_recorder.rs`/`interruption_detector.rs` consomem o resultado lá, por chamada direta.
//! A publicação aqui vai para o `EventBus` que o chamador injeta — em produção, o sink interno sem
//! assinante de `vad_audio.rs` (ver o cabeçalho de lá); só o teste a observa.
//!
//! Campos que publica e os que fica devendo, contra o payload real de `ipc.ts`:
//! - `VAD_AUDIO` real: `{ samples, amplitude }`. Aqui: `{ samples, sampleRate }` — `amplitude` é
//!   calculada no capturador do renderer (medidor de nível), não pelo VAD.
//! - `VAD_SAMPLE` real: `{ level, speechProbability, timestamp }`, throttlado a 100 Hz. Aqui, só
//!   `{ speechProbability }` — `level` é o mesmo `amplitude` acima, e `timestamp`/`at_ms` já vêm no
//!   [`crate::events::EventEnvelope`]. Sem throttle: uma janela Silero (32 ms) já é ~31 Hz, abaixo
//!   do teto de 100 Hz do lado TS.

use serde_json::json;

use crate::events::{EventBus, EventKey};
use crate::modules::inference::vad::{Pcm16Frame, SileroVad};

/// Uma `SileroVad` por STREAM de captura, nunca por frame — o LSTM do modelo carrega estado entre
/// janelas (ver o cabeçalho de `inference/vad.rs`); recriar a cada frame zeraria esse estado e a
/// detecção ficaria sempre baixa. `VadStream` é o dono da única instância da sessão de captura.
pub struct VadStream {
    vad: SileroVad,
}

impl VadStream {
    /// Abre (ou reusa, via `SileroVad::load`) a sessão Silero compartilhada do processo.
    pub async fn open(model_path: &std::path::Path) -> anyhow::Result<Self> {
        Ok(Self { vad: SileroVad::load(model_path).await? })
    }

    /// Roda uma janela PCM16 mono 16 kHz pela mesma instância (estado recorrente preservado) e
    /// publica no barramento passado — injetado, não o singleton `crate::events::bus()` direto,
    /// para o chamador real escolher a instância (produção usa `events::bus()`; teste, uma isolada
    /// — mesmo padrão de `TurnObserver` injetado em `brain/turn.rs::run_turn`).
    pub async fn process(&mut self, frame: Pcm16Frame<'_>, bus: &EventBus) -> anyhow::Result<f32> {
        let probability = self.vad.probability(frame).await?;

        // `VAD_AUDIO` nunca é descartado por probabilidade no lado TS — todo payload recebido é
        // repassado, e o gate de fala roda depois, no consumidor. Reproduzido aqui sem threshold.
        bus.publish(
            EventKey::VadAudio,
            json!({ "samples": frame.samples, "sampleRate": frame.sample_rate }),
        );
        bus.publish(EventKey::VadSample, json!({ "speechProbability": probability }));

        Ok(probability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    // Mesmo fixture rastreado que `inference/vad.rs` usa — real, não sintético (ver o porquê no
    // cabeçalho de lá: rede é falha alheia à mudança, e pular sem o arquivo seria verde otimista).
    const REAL_MODEL_SHA256: &str = "1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3";

    fn ensure_real_model() -> PathBuf {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/models/silero_vad.onnx");
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("fixture do Silero ausente em {}: {e}. É arquivo rastreado no repositório.", path.display()));

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        assert_eq!(hex::encode(hasher.finalize()), REAL_MODEL_SHA256, "fixture do Silero não bate com o SHA-256 esperado");
        path
    }

    fn silent_window() -> Vec<i16> {
        vec![0i16; crate::modules::inference::vad::FRAME_SAMPLES]
    }

    fn frame(samples: &[i16]) -> Pcm16Frame<'_> {
        Pcm16Frame { samples, sample_rate: 16_000, channels: 1 }
    }

    /// Mesma gravação real (fala em pt-BR, 16 kHz mono) que `inference/vad.rs` e o E2E de voz usam.
    fn load_real_speech_fixture() -> Vec<i16> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/voice/fala-usuario-pt-br.wav");
        let mut reader =
            hound::WavReader::open(&path).unwrap_or_else(|e| panic!("abrir fixture de voz real {}: {e}", path.display()));
        reader.samples::<i16>().map(|s| s.expect("amostra PCM16 válida no fixture")).collect()
    }

    fn window(samples: &[i16], index: usize) -> &[i16] {
        let size = crate::modules::inference::vad::FRAME_SAMPLES;
        &samples[index * size..(index + 1) * size]
    }

    #[tokio::test]
    async fn process_publishes_vad_audio_and_vad_sample_with_real_inference() {
        let model_path = ensure_real_model();
        let mut stream = VadStream::open(&model_path).await.expect("carregar VAD real");
        let bus = EventBus::new();
        // UM assinante — `broadcast` faz fan-out da MESMA mensagem para todo subscriber; dois
        // assinantes aqui não filtrariam por chave, os dois veriam a mesma sequência de eventos.
        let mut sub = bus.subscribe();

        let window = silent_window();
        let probability = stream.process(frame(&window), &bus).await.expect("inferência real");
        assert!((0.0..=1.0).contains(&probability));

        // Ordem de publicação de `VadStream::process`: `VadAudio` primeiro, `VadSample` depois.
        let audio = sub.recv().await.expect("VadAudio publicado");
        assert_eq!(audio.key, EventKey::VadAudio);
        assert_eq!(audio.payload["sampleRate"], serde_json::json!(16_000));
        assert_eq!(audio.payload["samples"].as_array().unwrap().len(), window.len());

        let sample = sub.recv().await.expect("VadSample publicado");
        assert_eq!(sample.key, EventKey::VadSample);
        assert_eq!(sample.payload["speechProbability"], serde_json::json!(probability));
    }

    /// Prova que `VadStream` reusa a MESMA `SileroVad` entre chamadas — mesmo teste de
    /// `inference/vad.rs::recurrent_state_changes_the_next_window_probability`, mas através do
    /// wrapper. Se `process` recriasse a instância a cada chamada, o estado da primeira janela
    /// nunca chegaria à segunda, e as duas leituras da MESMA janela (com e sem histórico) sairiam
    /// idênticas.
    #[tokio::test]
    async fn process_reuses_state_across_frames_instead_of_one_instance_per_frame() {
        let model_path = ensure_real_model();
        let bus = EventBus::new();
        let samples = load_real_speech_fixture();
        assert!(samples.len() >= crate::modules::inference::vad::FRAME_SAMPLES * 2, "fixture curta demais para duas janelas");

        let mut sequential = VadStream::open(&model_path).await.expect("carregar VAD real");
        sequential.process(frame(window(&samples, 0)), &bus).await.expect("primeira janela");
        let with_state = sequential.process(frame(window(&samples, 1)), &bus).await.expect("segunda janela, com estado");

        let mut isolated = VadStream::open(&model_path).await.expect("carregar segunda instância");
        let without_state = isolated.process(frame(window(&samples, 1)), &bus).await.expect("mesma janela, sem o estado anterior");

        assert_ne!(
            with_state, without_state,
            "a mesma janela devia dar probabilidades diferentes com e sem o estado da janela anterior — \
             prova que process() não recria a SileroVad a cada frame"
        );
    }
}
