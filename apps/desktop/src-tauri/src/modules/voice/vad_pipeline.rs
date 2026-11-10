//! Liga [`VadStream`] (Silero real, `vad_bridge.rs`) aos dois consumidores reais de
//! `voice:vad-audio` — [`BargeInRecorder`] e [`InterruptionDetector`], portados numa rodada
//! anterior de `barge-in-recorder.ts`/`interruption-detector.ts`. `voice:vad_audio.rs` (mesmo
//! diretório) é quem chama `process()` de produção, a partir do canal `voice:vad-audio` real —
//! ver o cabeçalho de lá para a atuação completa do barge-in.
//!
//! **Entrega sem perda, provada em teste.** [`VadPipeline::process`] chama
//! `BargeInRecorder::absorb`/`InterruptionDetector::feed_vad_sample` DIRETAMENTE, na mesma chamada
//! síncrona que já tem a amostra em mãos — nunca pelo `crate::events::EventBus` (que descarta para
//! assinante lento; aceitável para `VadSample`, não para o áudio bruto, ver o cabeçalho de
//! `events.rs`). O barramento continua recebendo a publicação de `VadStream::process` para outros
//! assinantes possíveis (ex.: um medidor de nível no HUD, que já tolera perda); os dois consumidores
//! reais deste arquivo não dependem dele.
//!
//! **Estado "Mika está falando" — decisão: barramento, não chamada direta.** Ao contrário de
//! `VadAudio`/`VadSample` (parágrafo acima), [`SpeechStarted`]/[`SpeechFinished`] chegam por
//! [`EventSubscription`], assinada uma vez em [`VadPipeline::new`] e drenada a cada [`process`]
//! (via [`apply_pending_speech_state`]). Três motivos, nenhum deles "por simetria com VadSample":
//! 1. `events/keys.rs` já classificou as duas chaves como "risco aceito, baixa frequência" — a
//!    decisão de tolerância a perda é do DONO do vocabulário, não deste arquivo, e ela já aponta
//!    para o barramento.
//! 2. Chamada DIRETA exigiria que `tts::speaker::speak` (quem sabe quando a Mika começa/termina de
//!    falar) segurasse uma referência à `VadPipeline` de uma sessão de captura específica — um
//!    acoplamento peça-a-peça entre TTS e uma sessão de VAD que nem sempre existe (o usuário pode
//!    estar com o microfone desligado enquanto a Mika fala). O barramento já é o desacoplamento que
//!    este crate usa para exatamente este caso (`events/mod.rs`, "por que este módulo existe").
//! 3. Ao contrário de `VadAudio` (perder UMA amostra corrompe o trecho gravado), perder
//!    `SpeechStarted`/`SpeechFinished` só importa se cair bem na janela de lag do anel de 256 — e
//!    [`process`] drena a cada ~32 ms enquanto o microfone estiver ligado, então a assinatura deste
//!    pipeline nunca fica atrás por mais que um punhado de mensagens.
//!
//! [`apply_pending_speech_state`] usa `tokio::time::timeout(Duration::ZERO, ..)` para um "peek" não
//! bloqueante — `EventSubscription` (fora deste diretório, `events/bus.rs`) não expõe `try_recv`
//! próprio, e adicionar um método lá está fora do escopo desta tarefa (só `modules/voice/` e
//! `sidecar-attrition.mts`). `timeout` com `Duration::ZERO` poll a future uma vez (implementação do
//! tokio testa o valor ANTES do timer): se a mensagem já está pronta no anel, devolve na hora; senão
//! devolve `Err(Elapsed)` sem esperar — nunca bloqueia `process()`.
//!
//! **Atuação sobre o turno.** `crate::modules::chat::turns::abort_turn(window_label)` é o hook
//! público que `chat/turns.rs` criou para isto — mas fica em `vad_audio.rs`, não aqui:
//! `VadPipeline` não tem (nem deveria ter) o `window_label` da invocação IPC.

use std::time::Duration;

use crate::events::{EventBus, EventKey};
use crate::modules::inference::vad::Pcm16Frame;

use super::barge_in_recorder::BargeInRecorder;
use super::interruption_detector::{InterruptionConfig, InterruptionDetector, InterruptionSignal};
use super::speech_gate::{FrameAnalyzer, SpeechGate};
use super::vad_bridge::VadStream;

pub struct VadPipelineOutcome {
    pub speech_probability: f32,
    /// Veredito robusto de voz do usuário nesta janela (Silero + [`SpeechGate`]) — o único que
    /// barge-in, atividade do usuário e STT consultam; `speech_probability` sozinha abre em tecla.
    pub user_voice: bool,
    /// Janela com transiente de tecla/clique e sem estrutura de voz — ver `speech_gate.rs`.
    pub keystroke: bool,
    pub interruption: InterruptionSignal,
    /// `Some` só na janela em que uma captura em curso atinge o teto de tempo/amostras — mesmo
    /// contrato de `BargeInRecorder::absorb`. `vad_audio.rs::react_to_barge_in`/`process_payload` é
    /// quem consome isto agora: dispara a transcrição fora do caminho quente e publica
    /// `EventKey::Interruption` quando o texto passa pelos filtros de `interruption_transcript.rs`.
    pub barge_in_audio: Option<Vec<i16>>,
}

/// Dono de UMA sessão de captura: uma `VadStream` (que por sua vez é dona da única `SileroVad` da
/// sessão — nunca uma por frame, ver o cabeçalho de `inference/vad.rs`), um `InterruptionDetector`,
/// um `BargeInRecorder` e a assinatura de `SpeechStarted`/`SpeechFinished` (ver o cabeçalho do
/// arquivo para o porquê de ser assinatura, não chamada direta).
pub struct VadPipeline {
    vad: VadStream,
    analyzer: FrameAnalyzer,
    gate: SpeechGate,
    detector: InterruptionDetector,
    recorder: BargeInRecorder,
    speech_state: crate::events::EventSubscription,
}

impl VadPipeline {
    /// Assina `SpeechStarted`/`SpeechFinished` em `bus` IMEDIATAMENTE — antes de qualquer `process()`
    /// — para não perder um par publicado entre a criação do pipeline e a primeira janela de áudio.
    pub fn new(vad: VadStream, detector: InterruptionDetector, recorder: BargeInRecorder, bus: &EventBus) -> Self {
        let gate = SpeechGate::new(detector.speech_threshold());
        Self { vad, analyzer: FrameAnalyzer::default(), gate, detector, recorder, speech_state: bus.subscribe() }
    }

    /// `voice.vadIgnoreTyping` — ver `SpeechGate::set_reject_typing`.
    pub fn set_reject_typing(&mut self, reject: bool) {
        self.gate.set_reject_typing(reject);
    }

    /// `voice.interruption*`/`voice.vad*` relidos a cada chamada. `true` quando a config MUDOU:
    /// igual não faz nada (`update_config` zera o streak em curso e este método roda por janela).
    pub fn set_interruption_config(&mut self, config: InterruptionConfig) -> bool {
        if self.detector.config() == config {
            return false;
        }
        self.detector.update_config(config);
        self.gate.set_threshold(config.speech_threshold);
        true
    }

    /// Sem chamador de produção ainda: só `speak()` sabe quando a Mika começa a falar, e nada
    /// ainda chama `speak()` em produção (`tts/mod.rs`, item 2 de "o que ainda falta"). Documentado
    /// aqui, não escondido — `vad_audio.rs` não é dono desse estado.
    #[allow(dead_code)]
    pub fn on_speech_started(&mut self, now_ms: u64) {
        self.detector.on_speech_started(now_ms);
    }

    /// Mesma lacuna de [`Self::on_speech_started`] — ver ali.
    #[allow(dead_code)]
    pub fn on_speech_finished(&mut self) {
        self.detector.on_speech_finished();
    }

    /// Renderer playback state, sent with every VAD window — see `InterruptionDetector::set_playback`.
    pub fn set_playback(&mut self, playing: bool, now_ms: u64) {
        self.detector.set_playback(playing, now_ms);
    }
    /// Chamado por `vad_audio.rs` quando o próprio `process()` já sinalizou `BargeIn` — inicia a
    /// captura pós-barge-in no `BargeInRecorder` (mesmo contrato de `Events.FORCE_STOP_TTS` no TS).
    pub fn on_force_stop_tts(&mut self, now_ms: u64) {
        self.recorder.on_force_stop_tts(now_ms);
    }

    /// Sem chamador em produção ainda: nada hoje encerra explicitamente uma `VadAudioSession`
    /// (`vad_audio.rs` é singleton de processo, dura a vida do app) — `voice:mic-state` só
    /// atualiza o HUD, não a sessão de captura. Documentado, não escondido; ver o cabeçalho de
    /// `vad_audio.rs`.
    #[allow(dead_code)]
    pub fn stop(&mut self) {
        self.recorder.stop();
    }

    /// Drena TODO `SpeechStarted`/`SpeechFinished` já publicado desde a última chamada e aplica ao
    /// detector, antes de avaliar a amostra nova — ver o cabeçalho do arquivo para o mecanismo do
    /// "peek" não bloqueante.
    async fn apply_pending_speech_state(&mut self) {
        loop {
            match tokio::time::timeout(Duration::ZERO, self.speech_state.recv()).await {
                Ok(Some(envelope)) => match envelope.key {
                    EventKey::SpeechStarted => {
                        let at_ms = envelope.payload["atMs"].as_u64().unwrap_or(envelope.at_ms);
                        self.detector.on_speech_started(at_ms);
                    }
                    EventKey::SpeechFinished => self.detector.on_speech_finished(),
                    _ => {} // barramento compartilhado: outras chaves não dizem respeito a este pipeline.
                },
                // `Ok(None)`: publicador fechado (não acontece em produção, `bus()` é `'static`).
                // `Err(_)`: nada pendente agora — não é erro, é o caso comum entre duas falas da Mika.
                Ok(None) | Err(_) => return,
            }
        }
    }

    /// Uma janela PCM16 mono 16 kHz. Roda a inferência Silero real (via `VadStream`, que também
    /// publica no barramento compartilhado) e entrega a MESMA amostra, sem intermediário, aos dois
    /// consumidores reais — ver o cabeçalho do arquivo para por que a entrega aqui não pode passar
    /// pelo `EventBus`.
    pub async fn process(&mut self, frame: Pcm16Frame<'_>, bus: &EventBus, now_ms: u64) -> anyhow::Result<VadPipelineOutcome> {
        self.apply_pending_speech_state().await;

        let samples = frame.samples;
        let features = self.analyzer.analyze(samples);
        let speech_probability = self.vad.process(frame, bus).await?;
        let user_voice = self.gate.update(speech_probability, &features);

        let barge_in_audio = self.recorder.absorb(samples, now_ms);

        // Barge-in nunca por probabilidade crua: janela que o gate não confirmou chega como 0.
        let gated = if user_voice { speech_probability } else { 0.0 };
        let interruption = match self.detector.feed_vad_sample(None, Some(gated), now_ms) {
            InterruptionSignal::None => self.detector.check_silence_endpoint(now_ms),
            signal => signal,
        };

        Ok(VadPipelineOutcome { speech_probability, user_voice, keystroke: features.keystroke(), interruption, barge_in_audio })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::voice::barge_in_recorder::BargeInRecorderConfig;
    use crate::modules::voice::interruption_detector::InterruptionConfig;
    use std::path::{Path, PathBuf};

    // Mesmo fixture rastreado de `vad_bridge.rs` — real, não sintético.
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

    fn synthetic_frame(marker: i16) -> Vec<i16> {
        vec![marker; crate::modules::inference::vad::FRAME_SAMPLES]
    }

    /// O `bus` vem de FORA e é criado antes do pipeline, de propósito: `new` assina na hora, e um
    /// barramento criado depois deixaria a assinatura apontando para outro canal — exatamente o
    /// defeito que a assinatura ansiosa veio consertar.
    fn pipeline(recorder: BargeInRecorder, vad: VadStream, bus: &EventBus) -> VadPipeline {
        VadPipeline::new(vad, InterruptionDetector::new(InterruptionConfig::default()), recorder, bus)
    }

    #[tokio::test]
    async fn process_reports_real_probability_and_leaves_idle_consumers_idle() {
        let vad = VadStream::open(&ensure_real_model()).await.expect("carregar VAD real");
        let bus = EventBus::new();
        let mut pipe = pipeline(BargeInRecorder::new(BargeInRecorderConfig::default()), vad, &bus);

        let frame = silent_window();
        let outcome = pipe
            .process(Pcm16Frame { samples: &frame, sample_rate: 16_000, channels: 1 }, &bus, 0)
            .await
            .expect("inferência real");

        assert!((0.0..=1.0).contains(&outcome.speech_probability));
        assert_eq!(outcome.interruption, InterruptionSignal::None, "Mika não está falando: sem barge-in possível");
        assert_eq!(outcome.barge_in_audio, None, "sem force-stop-tts: nenhuma captura em curso");
    }

    /// A prova central do arquivo: mesmo com o barramento compartilhado saturado (capacidade 1, um
    /// assinante que nunca drena — o pior caso que `events.rs` documenta), o `BargeInRecorder` não
    /// perde nenhuma amostra, porque ele nunca recebe pelo barramento.
    #[tokio::test]
    async fn barge_in_audio_survives_even_when_the_shared_bus_is_lagging() {
        let vad = VadStream::open(&ensure_real_model()).await.expect("carregar VAD real");
        let recorder_cfg = BargeInRecorderConfig { sample_rate: 16_000, pre_roll_ms: 0, post_barge_ms: 100 };
        let bus = EventBus::with_capacity(1);
        let mut pipe = pipeline(BargeInRecorder::new(recorder_cfg), vad, &bus);

        // Assinante nunca drenado: cada `process()` publica DUAS mensagens (VadAudio, VadSample) —
        // já na segunda janela este assinante está atrasado, e nunca se recupera.
        let _slow_subscriber = bus.subscribe();

        pipe.on_force_stop_tts(0);

        let mut expected = Vec::new();
        let mut merged = None;
        for i in 0..4i16 {
            let frame = synthetic_frame(i);
            expected.extend_from_slice(&frame);
            let now_ms = (i as u64 + 1) * 32; // 32ms por janela de 512 amostras @ 16kHz
            let outcome = pipe
                .process(Pcm16Frame { samples: &frame, sample_rate: 16_000, channels: 1 }, &bus, now_ms)
                .await
                .expect("inferência real");
            if let Some(audio) = outcome.barge_in_audio {
                merged = Some(audio);
                break;
            }
        }

        let merged = merged.expect("teto de captura (100ms) atingido dentro de 4 janelas de 32ms");
        assert_eq!(merged, expected, "o recorder não pode perder amostra nenhuma mesmo com o barramento saturado");
    }

    struct Run {
        user_voice: Vec<bool>,
        keystroke: Vec<bool>,
        raw: Vec<f32>,
        barge_ins: usize,
    }

    /// O arquivo inteiro pelo pipeline de produção, com a Mika "falando" o tempo todo (barge-in
    /// armado) e o limiar default do schema (`voice.vadSpeechThreshold` = 0.35).
    async fn run_file(model: &Path, pcm: &[i16]) -> Run {
        let bus = EventBus::new();
        let vad = VadStream::open(model).await.expect("carregar VAD real");
        let config = InterruptionConfig { speech_threshold: 0.35, ..InterruptionConfig::default() };
        let recorder = BargeInRecorder::new(BargeInRecorderConfig::default());
        let mut pipe = VadPipeline::new(vad, InterruptionDetector::new(config), recorder, &bus);
        pipe.set_playback(true, 0);
        let mut run = Run { user_voice: Vec::new(), keystroke: Vec::new(), raw: Vec::new(), barge_ins: 0 };
        for (i, window) in pcm.chunks_exact(crate::modules::inference::vad::FRAME_SAMPLES).enumerate() {
            let now_ms = 1_000 + i as u64 * 32; // fora da janela de graça do eco (300 ms)
            let frame = Pcm16Frame { samples: window, sample_rate: 16_000, channels: 1 };
            let out = pipe.process(frame, &bus, now_ms).await.expect("inferência real");
            run.user_voice.push(out.user_voice);
            run.keystroke.push(out.keystroke);
            run.raw.push(out.speech_probability);
            if matches!(out.interruption, InterruptionSignal::BargeIn { .. }) {
                run.barge_ins += 1;
                pipe.set_playback(true, now_ms); // continua "falando": conta cada barge-in que escaparia
            }
        }
        run
    }

    fn first_time(flags: impl Iterator<Item = bool>) -> Option<f32> {
        flags.enumerate().find(|(_, v)| *v).map(|(i, _)| i as f32 * 0.032)
    }

    /// O que o detector ANTES deste gate fazia: 3 janelas seguidas de Silero cru ≥ 0.35.
    fn raw_barge_in_start(raw: &[f32]) -> Option<f32> {
        let streak = raw.windows(3).position(|w| w.iter().all(|p| *p >= 0.35))?;
        Some((streak + 2) as f32 * 0.032)
    }

    /// Gabarito `e2e/fixtures/voice/noise/vad-cases.json` (`pnpm vad:fixtures`): zero início de fala
    /// e zero barge-in nos arquivos sem fala; nos com fala, voz dentro de cada trecho e nunca
    /// começando antes de `início − 0.2 s`. E o STT no motor `energy` (default): o arquivo inteiro
    /// como UM trecho que o gate de nível do renderer gravaria — teclado/mouse sem fala é descartado
    /// (`transcribe.rs::neural_vad_rejects`), fala nunca. Imprime o antes (Silero cru) e o depois.
    #[tokio::test]
    async fn noise_fixtures_speech_decision_matches_the_answer_key() {
        let model = ensure_real_model();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/voice/noise");
        let key: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("vad-cases.json")).expect("gabarito")).expect("json");
        let mut failures = Vec::new();
        for case in key["cases"].as_array().expect("cases") {
            let file = case["file"].as_str().expect("file");
            let mut reader = hound::WavReader::open(dir.join(file)).expect("wav");
            let pcm: Vec<i16> = reader.samples::<i16>().map(|s| s.expect("pcm16")).collect();
            let run = run_file(&model, &pcm).await;
            let segments: Vec<(f32, f32)> = case["speechSegments"]
                .as_array()
                .expect("segments")
                .iter()
                .map(|s| (s[0].as_f64().unwrap() as f32, s[1].as_f64().unwrap() as f32))
                .collect();
            let start = first_time(run.user_voice.iter().copied());
            let clicky = case["noise"].as_array().expect("noise").iter().any(|n| n == "keyboard" || n == "mouse");
            let voiced = run.user_voice.iter().any(|v| *v);
            let stt_drop = super::super::transcribe::neural_vad_rejects("energy", true, Some(voiced), run.keystroke.iter().any(|k| *k));
            if stt_drop != (segments.is_empty() && clicky) {
                failures.push(format!("{file}: STT (energia) descartaria = {stt_drop}"));
            }
            eprintln!(
                "VAD {file:<26} antes: fala@0.5 {:?} barge-in {:?} | depois: fala {start:?} barge-ins {} teclas {} STT-descarta {stt_drop}",
                first_time(run.raw.iter().map(|p| *p >= 0.5)),
                raw_barge_in_start(&run.raw),
                run.barge_ins,
                run.keystroke.iter().filter(|k| **k).count(),
            );
            if segments.is_empty() {
                if start.is_some() || run.barge_ins > 0 {
                    failures.push(format!("{file}: sem fala, mas o gate abriu em {start:?} ({} barge-ins)", run.barge_ins));
                }
                continue;
            }
            for (from, to) in &segments {
                let covered = run.user_voice.iter().enumerate().any(|(i, v)| *v && (*from..=*to).contains(&(i as f32 * 0.032)));
                if !covered {
                    failures.push(format!("{file}: trecho {from}-{to}s sem voz detectada"));
                }
            }
            if start.is_some_and(|t| t < segments[0].0 - 0.2) {
                failures.push(format!("{file}: fala começou em {start:?}, antes de {}s", segments[0].0 - 0.2));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// Silero REAL sobre os 36 trens de teclado sintéticos (`speech_gate::synth::keyboards`) e sobre
    /// a fala humana versionada com os de pico 12000 por cima: teclado sozinho nunca vira voz nem barge-in;
    /// fala + teclado sempre vira. Imprime quantos teriam vazado pelo Silero cru (o antes).
    #[tokio::test]
    async fn real_silero_on_synthetic_typing_and_real_speech_over_typing() {
        use crate::modules::voice::speech_gate::synth::{keyboards, mix};
        let model = ensure_real_model();
        let speech: Vec<f32> = load_speech_fixture().iter().map(|&s| f32::from(s)).collect();
        let (mut raw_leaks, mut failures) = (0, Vec::new());
        for set in keyboards() {
            let alone = run_file(&model, &set.pcm).await;
            raw_leaks += usize::from(raw_barge_in_start(&alone.raw).is_some());
            if alone.user_voice.iter().any(|v| *v) || alone.barge_ins > 0 {
                failures.push(format!("só teclado virou voz: {}", set.label));
            }
            if set.amp != 12_000.0 {
                continue; // fala + teclado: um volume por ritmo/tecla basta, e cada um custa 6 s de Silero
            }
            let keys: Vec<f32> = set.pcm.iter().map(|&s| f32::from(s)).collect();
            let over = run_file(&model, &speech_gate_pcm(&mix(&speech, &keys))).await;
            if !over.user_voice.iter().any(|v| *v) || over.barge_ins == 0 {
                failures.push(format!("fala + teclado não virou voz/barge-in: {}", set.label));
            }
        }
        eprintln!("VAD sintético: {raw_leaks}/36 trens de teclado abririam barge-in pelo Silero cru; depois: {}", failures.len());
        assert!(failures.is_empty(), "{failures:#?}");
    }

    fn speech_gate_pcm(x: &[f32]) -> Vec<i16> {
        crate::modules::voice::speech_gate::synth::to_pcm16(x)
    }

    fn load_speech_fixture() -> Vec<i16> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/voice/fala-usuario-pt-br.wav");
        let mut reader = hound::WavReader::open(&path).expect("fixture de fala real");
        reader.samples::<i16>().map(|s| s.expect("pcm16")).collect()
    }
}
