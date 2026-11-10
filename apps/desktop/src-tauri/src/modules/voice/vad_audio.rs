//! `voice:vad-audio` — o canal que faltava reivindicar. Dono de UMA [`VadPipeline`] (singleton de
//! processo, mesmo padrão de `modules/chat/turns.rs::registry`) e do handler que
//! `voice/mod.rs::VoiceModule::invoke` despacha para cá depois da guarda de superfície.
//!
//! Reproduz `apps/desktop/src/main/services/voice/ipc.ts::registerVoiceIPC` (rota `voiceVadAudio`,
//! linhas ~166-187):
//!
//! 1. **O áudio nunca é descartado.** [`process_payload`] publica `EventKey::VadAudio` com o
//!    payload CRU (`{ samples, amplitude }`), sempre, antes de qualquer janelamento.
//! 2. **Só `VadSample` é limitado a 100 Hz** ([`should_emit_sample`], mesmo teto de
//!    `ipc.ts::lastVadEmitMs`). `VadAudio` nunca passa por este throttle.
//! 3. **Barge-in aciona DUAS coisas** ([`react_to_barge_in`]): `EventKey::ForceStopTts` no
//!    barramento (que `voice::tts::bus_bridge::spawn_force_stop_listener`, já ligado em
//!    `modules/mod.rs`, converte em `abort_registry::force_stop_tts()` de verdade) E
//!    `chat::turns::abort_turn(window_label)` — o hook que `vad_pipeline.rs`/`turns.rs` já
//!    documentavam como a peça que faltava. Só o primeiro cala o TTS; sem o segundo, o turno de LLM
//!    em curso continuaria gerando texto para uma fala que ninguém mais vai ouvir.
//! 4. **Erro de inferência é logado e engolido, nunca derruba a rota** — mesmo `try/catch` que
//!    envolve o corpo inteiro do handler original. Só uma violação de CONTRATO (payload sem
//!    `samples`/`amplitude` no formato que o schema Zod já garante) propaga `Err`, mesmo critério
//!    de `extract_enabled` (`voice/mod.rs`) e `parse_payload` (`transcribe.rs`).
//! 5. **A Mika responde ao que o usuário disse ao interromper.** Antes desta rodada,
//!    `VadPipelineOutcome::barge_in_audio` (o áudio pós-barge-in que [`BargeInRecorder`] já
//!    gravava) era descartado — nada transcrevia, `EventKey::Interruption` nunca era publicado, e
//!    o Rust dava a mesma cara de "calou e não respondeu" que `project_resposta_que_significa_nao_fiz`
//!    já catalogou para outros módulos. Agora: quando o recorder fecha a captura,
//!    [`spawn_interruption_transcription`] transcreve FORA do caminho quente (`tauri::async_runtime::spawn`,
//!    nunca bloqueando a próxima janela do VAD), pula quem o STT contínuo já vai responder
//!    (`listening_state::global().is_listening()`), filtra alucinação/eco
//!    (`interruption_transcript::process_interruption_audio`, que também consulta
//!    `speak_guard::looks_like_self_echo`) e publica `EventKey::Interruption` com `{transcript,
//!    is_interruption: true, spokenPrefix, timestamp}` — mesma forma de `interruption-transcript.ts`.
//!    `spokenPrefix` vem de `speak_guard::spoken_prefix()`, o tracker de processo que
//!    `voice::auto_speak` alimenta a cada trecho falado (contrato documentado no cabeçalho de
//!    `speak_guard.rs`); sai `""` até esse módulo estar ligado a ele.
//!    `InterruptionSignal::SilenceEndpoint` (endpoint por silêncio sustentado, calculado por
//!    `check_silence_endpoint` mas nunca acionado até aqui) publica o mesmo evento direto, sem
//!    transcrição, com `{transcript: "", is_silence_endpoint: true, timestamp}` — porte de
//!    `interruption-detector.ts` linhas 161-165.
//!
//! **Janelamento.** `VadPipeline::process` só aceita janelas de `inference::vad::FRAME_SAMPLES`
//! (512 amostras @ 16 kHz); o renderer manda o tamanho de chunk que a captura tiver, não esse
//! número exato. [`VadAudioSession::pending`] acumula entre chamadas e drena uma janela completa
//! por vez — mesmo papel de `NeuralVadAnalyzer.push`/`pending` no TS, sem fila assíncrona: cada
//! janela é processada (`.await`) antes de continuar.
//!
//! **Formato das amostras.** Floats normalizados em `[-1, 1]` (mesmo formato que
//! `NeuralVadAnalyzer.push` recebe no TS) — `VadPipeline` só aceita PCM16 (contrato do módulo
//! `inference`, AGENTS.md §2.5). [`f32_to_pcm16`] é o inverso exato de
//! `inference::vad::pcm16_to_f32` (`s as f32 / 32_768.0`).
//!
//! **Por que `VadPipeline::process` recebe um barramento PRÓPRIO, nunca `bus()`.**
//! `VadStream::process` (`vad_bridge.rs`) também publica `VadAudio`/`VadSample` — instrumentação de
//! uma rodada anterior, sem throttle e com formato diferente do payload cru (`{ samples: janela de
//! 512, sampleRate }`, não `{ samples, amplitude }`). Os dois consumidores REAIS
//! (`BargeInRecorder`/`InterruptionDetector`) recebem a amostra por chamada direta, não pelo
//! barramento (ver `vad_pipeline.rs`), e nenhum assinante depende dessa instrumentação hoje. Passar
//! `bus()` aqui duplicaria `VadAudio`/`VadSample` sob a MESMA chave com formato e cadência
//! diferentes — a classe de defeito que `events/keys.rs` pede para evitar.
//! [`VadAudioSession::internal_sink`] é um `EventBus` isolado sem assinante: a publicação continua
//! acontecendo (não é erro, `EventBus::publish` não falha por falta de assinante), só que não vai a
//! lugar nenhum — silencia a instrumentação sem editar `vad_bridge.rs`/`vad_pipeline.rs` (portados,
//! testados, fora do escopo desta tarefa).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::{Mutex as AsyncMutex, OnceCell};

use crate::config::store::ConfigStore;
use crate::events::{bus, EventBus, EventKey};
use crate::modules::chat::turns as chat_turns;
use crate::modules::inference::vad::{Pcm16Frame, FRAME_SAMPLES};
use crate::protocol::InvokeContext;

use super::barge_in_recorder::{BargeInRecorder, BargeInRecorderConfig};
use super::interruption_detector::{InterruptionConfig, InterruptionDetector, InterruptionSignal, NEURAL_SPEECH_THRESHOLD};
use super::interruption_transcript::{self, InterruptionAudio};
use super::user_speech_activity;
use super::vad_bridge::VadStream;
use super::vad_pipeline::VadPipeline;

/// Mesmo rate que todo o resto deste arquivo já assume (janela do VAD, `f32_to_pcm16`,
/// `Pcm16Frame` @ 16 kHz) — nomeado aqui só para o áudio pós-barge-in não carregar um `16_000`
/// solto sem explicação no meio da chamada de transcrição.
const CAPTURE_SAMPLE_RATE_HZ: u32 = 16_000;

/// Mesma escolha de `interruption-transcript.ts` (linha 30, `{ minFillerMs: 0 }`): a janela do
/// barge-in é curta e fixa (`BargeInRecorderConfig::post_barge_ms`), então mesmo uma frase padrão
/// CURTA já é suspeita — piso diferente do que o STT contínuo usaria.
const INTERRUPTION_MIN_FILLER_MS: u64 = 0;

/// Mesmo teto de `ipc.ts::lastVadEmitMs` (`now - lastVadEmitMs < 10` é suprimido) — só `VadSample`.
const VAD_SAMPLE_THROTTLE_MS: u64 = 10;

/// Dono de UMA sessão de captura pela vida do processo — mesma justificativa de singleton que
/// `chat/turns.rs::TURNS`: duas sessões divergiriam em qual delas tem o estado recorrente do VAD e
/// o streak de interrupção certos.
struct VadAudioSession {
    pipeline: VadPipeline,
    /// Amostras que ainda não completaram uma janela de `FRAME_SAMPLES` — ver o cabeçalho do
    /// arquivo ("Janelamento").
    pending: Vec<f32>,
    last_sample_emit_ms: Option<u64>,
    /// Sem assinante, de propósito — ver o cabeçalho do arquivo ("Por que `VadPipeline::process`
    /// recebe um barramento PRÓPRIO"). Recebe a instrumentação interna de `VadStream::process` para
    /// não duplicar `VadAudio`/`VadSample` no barramento real com formato e cadência diferentes.
    internal_sink: EventBus,
    /// Para onde `interruption_transcript::process_interruption_audio` procura os pesos do
    /// Whisper — mesmo `data_dir` que `ensure_session` já recebia, só guardado para a transcrição
    /// pós-barge-in (que roda fora da chamada de `handle`, ver [`spawn_interruption_transcription`]).
    data_dir: PathBuf,
    /// Os três campos abaixo são relidos do `ConfigStore` a cada chamada ([`Self::refresh_settings`]).
    asr_language: Option<String>,
    /// The barge-in transcribes with the engine the user picked, not Whisper.
    asr_provider: String,
    /// Which Whisper size to load when `asr_provider` resolves to a local Whisper engine
    /// (`voice.whisperModel`, `config/schemas/voice.rs`).
    whisper_model: String,
    /// Epoch ms do `InterruptionSignal::BargeIn` mais recente (code review, major) — capturado na
    /// DETECÇÃO, várias janelas antes do recorder fechar a captura e a transcrição publicar
    /// `EventKey::Interruption`. `awareness::engine::on_interruption` usa isto (`bargeInAtMs` no
    /// payload) para provar que um turno real já tinha TERMINADO antes mesmo da resposta de
    /// interrupção começar a esperar o LLM — ver `chat::proactive::run_proactive_turn`.
    barge_in_detected_at_ms: Option<u64>,
}

impl VadAudioSession {
    /// Relê as chaves de voz do store. Chamado a cada chamada de `voice:vad-audio` (leituras baratas,
    /// em memória): antes elas eram lidas só ao abrir a sessão e mudar o controle na UI não surtia efeito.
    fn refresh_settings(&mut self, store: &ConfigStore) {
        self.pipeline.set_interruption_config(interruption_config_from_store(store));
        self.pipeline.set_reject_typing(super::speech_gate::ignore_typing(store));
        self.asr_language = super::transcribe::configured_language(store);
        self.asr_provider = super::transcribe::resolve_provider(store);
        self.whisper_model = super::transcribe::resolve_whisper_model(store);
    }
}

static SESSION: OnceCell<AsyncMutex<VadAudioSession>> = OnceCell::const_new();

/// Porte de `getInterruptionConfig` (`voice-barge-in.ts`) — mesmas seis chaves, mesmo namespace
/// `voice.*`. Relida a cada chamada (`refresh_settings`); `VadPipeline::set_interruption_config` só
/// aplica quando mudou, para não zerar o streak de barge-in por janela. `store.get` já devolve o default do schema quando o valor
/// nunca foi salvo (`ConfigStore::open` mescla sobre `schema.defaults()`), então o `.unwrap_or`
/// aqui só cobre o caso do TIPO vir errado (config corrompido à mão), não o de "nunca configurado".
fn interruption_config_from_store(store: &ConfigStore) -> InterruptionConfig {
    let default = InterruptionConfig::default();
    InterruptionConfig {
        enabled: store.get("voice.interruptionEnabled").and_then(|v| v.as_bool()).unwrap_or(default.enabled),
        vad_threshold: store
            .get("voice.interruptionThreshold")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(default.vad_threshold),
        trigger_streak: store
            .get("voice.interruptionTriggerStreak")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or(default.trigger_streak),
        cooldown_ms: store.get("voice.interruptionCooldownMs").and_then(|v| v.as_u64()).unwrap_or(default.cooldown_ms),
        endpointing_enabled: store
            .get("voice.vadEndpointingEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(default.endpointing_enabled),
        silence_ms: store.get("voice.vadSilenceMs").and_then(|v| v.as_u64()).unwrap_or(default.silence_ms),
        speech_threshold: store
            .get("voice.vadSpeechThreshold")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(default.speech_threshold),
    }
}

/// Abre a sessão Silero real na primeira chamada (mesmo padrão de `inference::vad::shared_session`
/// e `tts/providers/supertonic/model.rs::SHARED`); chamadas seguintes reusam a mesma instância —
/// inclusive a config de interrupção lida abaixo, só válida no boot (ver `interruption_config_from_store`).
async fn ensure_session(data_dir: &Path, store: &ConfigStore) -> Result<&'static AsyncMutex<VadAudioSession>, String> {
    let path = data_dir.to_path_buf();
    let interruption_config = interruption_config_from_store(store);
    let asr_language = super::transcribe::configured_language(store);
    let asr_provider = super::transcribe::resolve_provider(store);
    let whisper_model = super::transcribe::resolve_whisper_model(store);
    SESSION
        .get_or_try_init(move || async move {
            let model_path = crate::modules::desktop::vad_model_path(&path);
            let vad = VadStream::open(&model_path).await.map_err(|e| e.to_string())?;
            let pipeline = VadPipeline::new(
                vad,
                InterruptionDetector::new(interruption_config),
                BargeInRecorder::new(BargeInRecorderConfig::default()),
                bus(),
            );
            Ok::<_, String>(AsyncMutex::new(VadAudioSession {
                pipeline,
                pending: Vec::new(),
                last_sample_emit_ms: None,
                internal_sink: EventBus::new(),
                data_dir: path,
                asr_language,
                asr_provider,
                whisper_model,
                barge_in_detected_at_ms: None,
            }))
        })
        .await
}

/// Teto de amostras por chamada: 1s de áudio a 16 kHz (`CAPTURE_SAMPLE_RATE_HZ`), bem acima do que
/// o `AudioWorklet` do renderer manda por callback (frações de `FRAME_SAMPLES`, 512 amostras/32ms —
/// `inference::vad::FRAME_SAMPLES`). Sem este teto, `voice:vad-audio` — canal chamado em alta
/// frequência (até 100 Hz, ver o cabeçalho do arquivo) — aceitava um array de qualquer tamanho por
/// chamada, ida de graça pelo `min`/`max` que o schema TS nunca declarou.
const MAX_SAMPLES_PER_CALL: usize = CAPTURE_SAMPLE_RATE_HZ as usize;

fn parse_samples(payload: &Value) -> Result<Vec<f32>, String> {
    let raw = payload["samples"]
        .as_array()
        .ok_or_else(|| "'samples' ausente ou não é array".to_string())?;
    if raw.len() > MAX_SAMPLES_PER_CALL {
        return Err(format!(
            "'samples' excede o teto de {MAX_SAMPLES_PER_CALL} amostras por chamada (~1s @ {CAPTURE_SAMPLE_RATE_HZ}Hz)"
        ));
    }
    raw.iter()
        .map(|v| v.as_f64().map(|f| f as f32))
        .collect::<Option<Vec<f32>>>()
        .ok_or_else(|| "'samples' contém elemento não numérico".to_string())
}

/// Mesmas amostras de [`parse_samples`], vindas como corpo binário (`mika_send_pcm`): Float32
/// little-endian cru, sem JSON. Mesmo teto por chamada.
pub fn decode_f32_le(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.len() % 4 != 0 {
        return Err(format!("corpo binário de {} bytes não é múltiplo de 4 (Float32)", bytes.len()));
    }
    if bytes.len() / 4 > MAX_SAMPLES_PER_CALL {
        return Err(format!(
            "'samples' excede o teto de {MAX_SAMPLES_PER_CALL} amostras por chamada (~1s @ {CAPTURE_SAMPLE_RATE_HZ}Hz)"
        ));
    }
    Ok(bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

fn parse_amplitude(payload: &Value) -> Result<f32, String> {
    payload["amplitude"]
        .as_f64()
        .map(|f| f as f32)
        .ok_or_else(|| "'amplitude' ausente ou não numérico".to_string())
}

/// Inverso de `inference::vad::pcm16_to_f32` — ver o cabeçalho do arquivo ("Formato das amostras").
fn f32_to_pcm16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s * 32_768.0).round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16)
        .collect()
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// `true` = pode publicar `VadSample` agora. Mesma regra de `ipc.ts`: sem emissão anterior, ou
/// pelo menos `VAD_SAMPLE_THROTTLE_MS` decorridos desde a última.
fn should_emit_sample(last_emit_ms: Option<u64>, now_ms: u64) -> bool {
    match last_emit_ms {
        None => true,
        Some(last) => now_ms.saturating_sub(last) >= VAD_SAMPLE_THROTTLE_MS,
    }
}

/// A ATUAÇÃO do barge-in — ver o item 3 do cabeçalho do arquivo. `pipeline.on_force_stop_tts`
/// inicia a captura pós-barge-in (`BargeInRecorder`, para transcrever depois); as outras duas
/// chamadas são o que faz o sinal detectado virar efeito real em vez de só um valor calculado.
fn react_to_barge_in(pipeline: &mut VadPipeline, bus: &EventBus, window_label: &str, now_ms: u64) {
    pipeline.on_force_stop_tts(now_ms);
    bus.publish(EventKey::ForceStopTts, json!({}));
    if let Err(e) = chat_turns::abort_turn(window_label) {
        log::error!("VAD audio handler error: falha ao abortar o turno em curso: {e}");
    }
}

/// Publica `EventKey::Interruption` por silêncio sustentado — porte direto do corpo do
/// `setInterval` de `startSilenceDetection` (`interruption-detector.ts`, linhas 161-165): sem
/// transcrição, `transcript` sempre vazio, `is_interruption` NUNCA presente (o TS não seta o
/// campo; `voice-barge-in.ts` trata a ausência como falso) — só `is_silence_endpoint: true`.
fn publish_silence_endpoint(bus: &EventBus, at_ms: u64) {
    bus.publish(EventKey::Interruption, json!({ "transcript": "", "is_silence_endpoint": true, "timestamp": at_ms }));
}

/// Transcreve o áudio pós-barge-in e publica `EventKey::Interruption` FORA do caminho quente —
/// porte de `barge-in-recorder.ts::flushCapture` chamando `detector.processInterruptionAudio(...)`
/// sem `await` (`void ...catch(...)`). `tauri::async_runtime::spawn`, nunca `tokio::spawn` (mesmo
/// motivo de `tts/bus_bridge.rs::spawn_force_stop_listener`: este handler roda a partir de
/// `mika_invoke`, dentro do reator, mas a regra do projeto é a mesma função de spawn em toda
/// `modules/voice/`, sem exceção por chamador).
///
/// `spokenPrefix` sai de `speak_guard::spoken_prefix()` — o tracker de processo que
/// `voice::auto_speak` alimenta a cada trecho sintetizado (ver o cabeçalho de `speak_guard.rs`).
/// Lido DEPOIS da transcrição (mesmo instante em que o TS lê `autoSpeak.spokenPrefix()`, dentro do
/// handler de `Events.FORCE_STOP_TTS`), então reflete o que já tinha sido dito quando o texto
/// finalmente chega, não quando a captura começou.
fn spawn_interruption_transcription(
    bus: &'static EventBus,
    data_dir: PathBuf,
    samples: Vec<i16>,
    language: Option<String>,
    provider: String,
    whisper_model: String,
    window_label: String,
    barge_in_at_ms: Option<u64>,
) {
    tauri::async_runtime::spawn(async move {
        let audio = InterruptionAudio { samples, sample_rate: CAPTURE_SAMPLE_RATE_HZ };
        let Some(transcript) = interruption_transcript::process_interruption_audio(
            data_dir,
            audio,
            language,
            &provider,
            &whisper_model,
            INTERRUPTION_MIN_FILLER_MS,
        )
        .await
        else {
            return;
        };
        bus.publish(
            EventKey::Interruption,
            interruption_event_payload(&transcript, &super::speak_guard::spoken_prefix(), now_ms(), &window_label, barge_in_at_ms),
        );
    });
}

/// Extraído para ser testável sem rodar transcrição real (item pesado, sem fixture de STT neste
/// arquivo) — a ÚNICA coisa nova aqui é `window_label`, que `brain::awareness::engine::on_interruption`
/// usa para registrar o handle de abort sob a MESMA janela (defects.md "barge-in", item 2), e que o
/// renderer usa para descartar seu próprio envio duplicado da mesma fala (item 1, `interruption-dedupe.ts`).
/// `barge_in_at_ms` (code review, major) é o instante da DETECÇÃO do barge-in, não desta publicação
/// (`at_ms`/`timestamp`, sempre mais tarde — depois da gravação e da transcrição): `None` quando o
/// recorder flush não tinha uma detecção correspondente registrada (sessão nova/teste sintético).
fn interruption_event_payload(transcript: &str, spoken_prefix: &str, at_ms: u64, window_label: &str, barge_in_at_ms: Option<u64>) -> Value {
    json!({
        "transcript": transcript,
        "is_interruption": true,
        "spokenPrefix": spoken_prefix,
        "timestamp": at_ms,
        "bargeInAtMs": barge_in_at_ms,
        "window_label": window_label,
    })
}

/// O corpo do handler, decoupled de `AppHandle`/`ensure_session` para poder ser testado com uma
/// sessão construída direto sobre um `EventBus` isolado (ver `mod tests` abaixo) — mesmo motivo de
/// `VadPipeline::process` aceitar `bus` injetado em vez do singleton. `bus` precisa ser `'static`
/// porque [`spawn_interruption_transcription`] o captura numa task desacoplada.
async fn process_payload(
    session: &mut VadAudioSession,
    bus: &'static EventBus,
    window_label: &str,
    samples: Vec<f32>,
    amplitude: f32,
    now_ms: u64,
) {
    // O áudio nunca é descartado: publicado sempre, com o payload cru, antes de qualquer
    // janelamento ou inferência — mesmo contrato de `ipc.ts` (item 1 do cabeçalho do arquivo).
    if bus.has_subscribers() {
        bus.publish(EventKey::VadAudio, json!({ "samples": &samples, "amplitude": amplitude }));
    }

    session.pending.extend(samples);
    while session.pending.len() >= FRAME_SAMPLES {
        let window_f32: Vec<f32> = session.pending.drain(..FRAME_SAMPLES).collect();
        let window_i16 = f32_to_pcm16(&window_f32);
        let frame = Pcm16Frame { samples: &window_i16, sample_rate: 16_000, channels: 1 };

        // `internal_sink`, não `bus`: ver o cabeçalho do arquivo ("Por que `VadPipeline::process`
        // recebe um barramento PRÓPRIO") — a instrumentação interna de `VadStream::process` não
        // pode duplicar o que este handler já publicou acima/abaixo no barramento real.
        let outcome = match session.pipeline.process(frame, &session.internal_sink, now_ms).await {
            Ok(outcome) => outcome,
            Err(e) => {
                log_failure(&format!("inferência do VAD falhou: {e}"));
                continue;
            }
        };

        if should_emit_sample(session.last_sample_emit_ms, now_ms) {
            session.last_sample_emit_ms = Some(now_ms);
            bus.publish(
                EventKey::VadSample,
                json!({ "level": amplitude, "speechProbability": outcome.speech_probability, "timestamp": now_ms }),
            );
        }

        // Independente da Mika estar falando ou não — ao contrário de `InterruptionDetector`, que só
        // acumula streak enquanto `self.speaking` (ver o cabeçalho de `user_speech_activity.rs`).
        // Veredito do `SpeechGate` (voz, não tecla — `speech_gate.rs`), nenhuma segunda inferência.
        let is_voice = outcome.user_voice && outcome.speech_probability >= NEURAL_SPEECH_THRESHOLD;
        user_speech_activity::global().note_window(is_voice, outcome.keystroke, now_ms);
        if is_voice {
            // Carrega o modelo do chat enquanto o usuário ainda fala — ver `brain::ollama_warmup`.
            crate::modules::brain::ollama_warmup::nudge();
        }

        match outcome.interruption {
            // `at_ms` guardado ANTES de reagir: é a detecção mais cedo possível deste barge-in (code
            // review, major) — bem antes do recorder fechar a captura e a transcrição publicar o
            // evento, janelas abaixo.
            InterruptionSignal::BargeIn { at_ms, .. } => {
                session.barge_in_detected_at_ms = Some(at_ms);
                react_to_barge_in(&mut session.pipeline, bus, window_label, now_ms);
            }
            InterruptionSignal::SilenceEndpoint { at_ms } => publish_silence_endpoint(bus, at_ms),
            InterruptionSignal::None => {}
        }

        // Independente do sinal DESTA janela: o recorder só devolve `Some` quando a captura pós
        // barge-in (iniciada janelas atrás, em `react_to_barge_in` -> `on_force_stop_tts`) atinge o
        // teto de tempo/amostras — ver o cabeçalho de `vad_pipeline.rs::VadPipelineOutcome`. `take()`:
        // esta detecção fica consumida por ESTA captura, uma próxima janela de barge-in grava a sua.
        if let Some(samples) = outcome.barge_in_audio {
            spawn_interruption_transcription(
                bus,
                session.data_dir.clone(),
                samples,
                session.asr_language.clone(),
                session.asr_provider.clone(),
                session.whisper_model.clone(),
                window_label.to_string(),
                session.barge_in_detected_at_ms.take(),
            );
        }
    }
}

/// `true` quando a causa é NOVA, atualizando o estado. Pura (o estado entra por parâmetro) para
/// ser testável sem estático de processo — teste que depende de estático compartilhado vira
/// flaky pelo vizinho quando o `cargo test` roda em paralelo.
fn is_new_reason(last: &mut Option<String>, reason: &str) -> bool {
    if last.as_deref() == Some(reason) {
        return false;
    }
    *last = Some(reason.to_string());
    true
}

/// Falha deste handler é logada e engolida (item 4 do cabeçalho) — mas ele roda por CHUNK de
/// áudio, ~50 vezes por segundo, e a causa costuma ser ESTÁVEL: modelo do VAD ainda não baixado,
/// por exemplo. Medido no boot de um perfil novo em 2026-09-22: 107 linhas de log em 3 segundos,
/// todas idênticas, enterrando qualquer outro erro junto.
///
/// A primeira ocorrência de cada causa continua em `error`. A repetição cai para `debug`, e uma
/// causa DIFERENTE volta a `error` — a segunda causa é informação nova, não ruído.
fn log_failure(reason: &str) {
    static LAST: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

    let is_new = LAST.lock().map_or(true, |mut last| is_new_reason(&mut last, reason));
    if is_new {
        log::error!("VAD audio handler error: {reason}");
    } else {
        log::debug!("VAD audio handler error (repetido): {reason}");
    }
}

/// Ponto de entrada chamado por `voice/mod.rs::VoiceModule::invoke` — a guarda de superfície
/// (`assertOwnerSurface(context, 'voiceVadAudio')` no TS, `is_owner_surface` aqui) já rodou lá
/// antes de chegar aqui, mesma convenção de `transcribe::handle`.
pub async fn handle(app: &AppHandle, ctx: &InvokeContext, payload: &Value) -> Result<Value, String> {
    let samples = parse_samples(payload)?;
    let amplitude = parse_amplitude(payload)?;
    handle_samples(app, ctx, samples, amplitude, payload["playing"].as_bool()).await
}

/// Caminho binário (`bridge::mika_send_pcm`): sem passar pelo roteador de canais, então a guarda de
/// superfície que `voice/mod.rs::VoiceModule::invoke` aplica antes do `match` é refeita aqui.
pub async fn handle_binary(app: &AppHandle, ctx: &InvokeContext, bytes: &[u8], amplitude: f32, playing: Option<bool>) -> Result<(), String> {
    if !crate::shell_actions::is_owner_surface(ctx.window_type) {
        return Err(format!("voice:vad-audio recusado para {:?}", ctx.window_type));
    }
    handle_samples(app, ctx, decode_f32_le(bytes)?, amplitude, playing).await.map(|_| ())
}

async fn handle_samples(app: &AppHandle, ctx: &InvokeContext, samples: Vec<f32>, amplitude: f32, playing: Option<bool>) -> Result<Value, String> {
    // "Pausar mic e tela" (bandeja): o áudio chega do renderer mas nada dele entra no VAD.
    if crate::modules::privacy::senses::paused() {
        return Ok(json!({ "ok": true }));
    }
    let data_dir = match crate::state::app_data_dir(app) {
        Ok(d) => d,
        Err(e) => {
            log_failure(&format!("não resolveu app_data_dir: {e}"));
            return Ok(json!({ "ok": true }));
        }
    };

    let store = match crate::state::config_store(app) {
        Ok(s) => s,
        Err(e) => {
            log_failure(&format!("config store indisponível: {e}"));
            return Ok(json!({ "ok": true }));
        }
    };

    let session = match ensure_session(&data_dir, &store).await {
        Ok(s) => s,
        Err(e) => {
            log_failure(&format!("sessão do VAD indisponível: {e}"));
            return Ok(json!({ "ok": true }));
        }
    };

    let mut guard = session.lock().await;
    guard.refresh_settings(&store);
    // Optional: a renderer that does not send it keeps the synthesis-only "speaking" signal.
    if let Some(playing) = playing {
        guard.pipeline.set_playback(playing, now_ms());
    }
    process_payload(&mut guard, bus(), &ctx.window_label, samples, amplitude, now_ms()).await;
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::brain::turn;
    use std::path::PathBuf;

    // --- lógica pura, sem sessão nem modelo -------------------------------------------------

    /// O caso que motivou o dedup: mesma causa a cada chunk de áudio (~50/s) só pode logar uma
    /// vez. E o caso que ele não pode estragar: causa nova volta a logar, senão o segundo defeito
    /// ficaria invisível atrás do primeiro.
    #[test]
    fn a_repeated_failure_logs_once_but_a_different_cause_logs_again() {
        let mut last = None;
        assert!(is_new_reason(&mut last, "modelo do VAD ausente"));
        assert!(!is_new_reason(&mut last, "modelo do VAD ausente"));
        assert!(!is_new_reason(&mut last, "modelo do VAD ausente"));
        assert!(is_new_reason(&mut last, "inferência do VAD falhou"));
        assert!(!is_new_reason(&mut last, "inferência do VAD falhou"));
        // Voltar à causa anterior é informação nova de novo: a falha ALTERNOU.
        assert!(is_new_reason(&mut last, "modelo do VAD ausente"));
    }

    /// defects.md "barge-in", item 2: `window_label` precisa chegar ao payload publicado para o
    /// motor de consciência registrar o handle de abort sob a MESMA janela do barge-in.
    #[test]
    fn interruption_event_payload_carries_the_window_label() {
        let payload = interruption_event_payload("para", "eu ia dizer", 1_000, "main", Some(700));
        assert_eq!(payload["transcript"], json!("para"));
        assert_eq!(payload["is_interruption"], json!(true));
        assert_eq!(payload["spokenPrefix"], json!("eu ia dizer"));
        assert_eq!(payload["timestamp"], json!(1_000));
        assert_eq!(payload["window_label"], json!("main"));
        assert_eq!(payload["bargeInAtMs"], json!(700));
    }

    #[test]
    fn parse_samples_requires_a_numeric_array() {
        assert!(parse_samples(&json!({})).is_err());
        assert!(parse_samples(&json!({ "samples": "not-an-array" })).is_err());
        assert!(parse_samples(&json!({ "samples": [1.0, "x", 2.0] })).is_err());
        assert_eq!(parse_samples(&json!({ "samples": [0.1, -0.2] })).unwrap(), vec![0.1_f32, -0.2_f32]);
    }

    /// NEGATIVO — sem `MAX_SAMPLES_PER_CALL`, `voice:vad-audio` (canal chamado até 100x/s, ver o
    /// cabeçalho do arquivo) aceitava um array de qualquer tamanho por chamada: nenhum teto do lado
    /// Rust nem do schema TS (`desktopVadAudioPayloadSchema`, só `z.union([Float32Array, number[]])`).
    #[test]
    fn parse_samples_rejects_arrays_over_the_per_call_cap() {
        let oversized = vec![0.0_f64; MAX_SAMPLES_PER_CALL + 1];
        let error = parse_samples(&json!({ "samples": oversized })).unwrap_err();
        assert!(error.contains("excede o teto"), "{error}");
    }

    #[test]
    fn parse_samples_accepts_arrays_at_the_per_call_cap() {
        let at_cap = vec![0.0_f64; MAX_SAMPLES_PER_CALL];
        assert_eq!(parse_samples(&json!({ "samples": at_cap })).unwrap().len(), MAX_SAMPLES_PER_CALL);
    }

    /// Paridade JSON x binário: o mesmo quadro, pelos dois caminhos, dá as mesmas amostras e a
    /// mesma recusa no teto — o fallback JSON e o `mika_send_pcm` não podem divergir.
    #[test]
    fn binary_decode_matches_the_json_path() {
        let frame = [0.0_f32, 0.1, -0.2, 1.0, -1.0, 0.000_123_4, f32::MIN_POSITIVE];
        let bytes: Vec<u8> = frame.iter().flat_map(|s| s.to_le_bytes()).collect();
        let via_json = parse_samples(&json!({ "samples": frame })).unwrap();
        assert_eq!(decode_f32_le(&bytes).unwrap(), via_json);

        let over = MAX_SAMPLES_PER_CALL + 1;
        assert!(decode_f32_le(&vec![0u8; over * 4]).unwrap_err().contains("excede o teto"));
        assert_eq!(decode_f32_le(&vec![0u8; MAX_SAMPLES_PER_CALL * 4]).unwrap().len(), MAX_SAMPLES_PER_CALL);
    }

    #[test]
    fn binary_decode_rejects_a_truncated_float() {
        assert!(decode_f32_le(&[0, 0, 0, 0, 1]).unwrap_err().contains("múltiplo de 4"));
        assert!(decode_f32_le(&[]).unwrap().is_empty());
    }

    #[test]
    fn parse_amplitude_requires_a_number() {
        assert!(parse_amplitude(&json!({})).is_err());
        assert!(parse_amplitude(&json!({ "amplitude": "loud" })).is_err());
        assert_eq!(parse_amplitude(&json!({ "amplitude": 0.42 })).unwrap(), 0.42_f32);
    }

    #[test]
    fn f32_to_pcm16_round_trips_through_the_full_range() {
        let pcm = f32_to_pcm16(&[-1.0, 0.0, 1.0]);
        assert_eq!(pcm[1], 0);
        assert!(pcm[0] < -32_000, "extremo negativo devia ficar perto de i16::MIN, deu {}", pcm[0]);
        assert!(pcm[2] > 32_000, "extremo positivo devia ficar perto de i16::MAX, deu {}", pcm[2]);
    }

    /// Entrada fora de `[-1, 1]` (possível por ruído/ganho no capturador) não pode dar overflow —
    /// `clamp` é o que garante isso antes do `as i16`.
    #[test]
    fn f32_to_pcm16_clamps_out_of_range_input() {
        let pcm = f32_to_pcm16(&[-5.0, 5.0]);
        assert_eq!(pcm[0], i16::MIN);
        assert_eq!(pcm[1], i16::MAX);
    }

    /// O ponto central do throttle: mesma regra de `ipc.ts::lastVadEmitMs`, só `VadSample` passa
    /// por aqui — `VadAudio` nunca chama esta função.
    #[test]
    fn should_emit_sample_throttles_to_100hz() {
        assert!(should_emit_sample(None, 0), "primeira amostra nunca é suprimida");
        assert!(!should_emit_sample(Some(0), 5), "dentro dos 10ms de throttle devia suprimir");
        assert!(should_emit_sample(Some(0), 10), "exatamente no teto devia liberar, mesma regra do TS (>=, não >)");
        assert!(should_emit_sample(Some(0), 11), "acima do teto devia liberar");
    }

    // --- orquestração real: VadPipeline real (modelo Silero rastreado), EventBus isolado -----

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

    async fn real_session() -> VadAudioSession {
        let vad = VadStream::open(&ensure_real_model()).await.expect("carregar VAD real");
        // Descartado logo após a construção: `VadPipeline::new` só usa a referência para assinar
        // `SpeechStarted`/`SpeechFinished` (irrelevante para estes testes, que não exercitam isso).
        let construction_bus = crate::events::EventBus::new();
        let pipeline = VadPipeline::new(
            vad,
            InterruptionDetector::new(InterruptionConfig::default()),
            BargeInRecorder::new(BargeInRecorderConfig::default()),
            &construction_bus,
        );
        VadAudioSession {
            pipeline,
            pending: Vec::new(),
            last_sample_emit_ms: None,
            internal_sink: EventBus::new(),
            data_dir: PathBuf::new(),
            asr_language: Some("pt".to_string()),
            asr_provider: "nemotron".to_string(),
            whisper_model: "onnx-community/whisper-base".to_string(),
            barge_in_detected_at_ms: None,
        }
    }

    /// `voice.interruption*`/`voice.vad*`/`voice.asr*` eram lidos só no boot da sessão: mexer no controle
    /// da UI salvava o valor e nada mudava (botão morto). Cada chamada agora relê o store.
    #[tokio::test]
    async fn changing_voice_settings_reaches_the_already_open_session() {
        let dir = std::env::temp_dir().join(format!("mika-vad-audio-settings-{}", std::process::id()));
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        let mut session = real_session().await;
        session.refresh_settings(&store);
        assert!(!session.pipeline.set_interruption_config(interruption_config_from_store(&store)), "config igual não mexe no streak em curso");

        store.set("voice.interruptionEnabled", json!(false)).unwrap();
        store.set("voice.vadSpeechThreshold", json!(0.8)).unwrap();
        store.set("voice.asrLanguage", json!("ja")).unwrap();
        store.set("voice.asrProvider", json!("local-whisper")).unwrap();
        let changed = interruption_config_from_store(&store);
        assert!(!changed.enabled && (changed.speech_threshold - 0.8).abs() < 1e-6);

        session.refresh_settings(&store);

        assert_eq!(session.asr_language.as_deref(), Some("ja"));
        assert_eq!(session.asr_provider, "local-whisper");
        assert!(!session.pipeline.set_interruption_config(changed), "a sessão já está com a config nova");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Prova 1: o áudio nunca é descartado (uma publicação de `VadAudio` por chamada, sempre), mas
    /// `VadSample` respeita o teto de 100 Hz — duas chamadas na mesma janela de 10ms produzem duas
    /// `VadAudio` e só UMA `VadSample`.
    #[tokio::test]
    async fn only_vad_sample_is_throttled_never_vad_audio() {
        let mut session = real_session().await;
        // `'static`: `process_payload` agora pode disparar `spawn_interruption_transcription`, que
        // exige um barramento que sobreviva à função — mesmo padrão de `tts/bus_bridge.rs` (ver o
        // cabeçalho deste arquivo, "Prova 3" abaixo, e o dos testes de lá: `Box::leak`).
        let bus: &'static EventBus = Box::leak(Box::new(EventBus::new()));
        let mut sub = bus.subscribe();

        let window = vec![0.0_f32; FRAME_SAMPLES];
        process_payload(&mut session, bus, "test-window", window.clone(), 0.1, 1_000).await;
        process_payload(&mut session, bus, "test-window", window, 0.1, 1_005).await;

        let (mut vad_audio, mut vad_sample) = (0, 0);
        while let Ok(Some(envelope)) = tokio::time::timeout(std::time::Duration::from_millis(200), sub.recv()).await {
            match envelope.key {
                EventKey::VadAudio => vad_audio += 1,
                EventKey::VadSample => vad_sample += 1,
                _ => {}
            }
        }

        assert_eq!(vad_audio, 2, "áudio nunca é descartado, mesmo dentro da janela de throttle");
        assert_eq!(vad_sample, 1, "o segundo VadSample, 5ms depois, devia ser suprimido pelo throttle de 100Hz");
    }

    /// Prova 2: barge-in publica `ForceStopTts` E chama `chat::turns::abort_turn` — as DUAS ações,
    /// não uma. Sinal sintético (não áudio real): a probabilidade que dispara `BargeIn` já está
    /// exaustivamente coberta em `interruption_detector.rs`; este teste prova só a reação nova.
    #[tokio::test]
    async fn barge_in_publishes_force_stop_tts_and_aborts_the_turn() {
        let mut pipeline_owner = real_session().await;
        let bus = EventBus::new();
        let mut sub = bus.subscribe();

        // Janela única, dedicada a este teste: `chat::turns::registry()` é singleton de processo
        // (mesmo motivo do aviso em `events/bus.rs`), mas é um HashMap por chave — outro teste
        // rodando em paralelo com outro `window_label` nunca colide aqui.
        let window_label = "vad-audio-test::barge-in";
        let (handle, signal) = turn::TurnAbortHandle::new();
        chat_turns::register_turn(chat_turns::registry(), window_label, handle).unwrap();
        assert!(!signal.is_aborted());

        react_to_barge_in(&mut pipeline_owner.pipeline, &bus, window_label, 1_000);

        let envelope = sub.recv().await.expect("ForceStopTts devia ser publicado");
        assert_eq!(envelope.key, EventKey::ForceStopTts);
        assert!(signal.is_aborted(), "abort_turn devia abortar o turno registrado para esta janela");
        assert!(
            chat_turns::registry().lock().unwrap().get(window_label).is_none(),
            "o turno abortado tem de sair do registro"
        );
    }

    /// Prova 3: silêncio sustentado publica `EventKey::Interruption` com a forma exata do TS —
    /// `is_silence_endpoint: true`, `transcript` vazio, SEM `is_interruption` (o TS nunca seta esse
    /// campo neste caminho; `voice-barge-in.ts` trata a ausência como falso).
    #[tokio::test]
    async fn publish_silence_endpoint_matches_the_ts_payload_shape() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();

        publish_silence_endpoint(&bus, 42_000);

        let envelope = sub.recv().await.expect("Interruption devia ser publicado de imediato");
        assert_eq!(envelope.key, EventKey::Interruption);
        assert_eq!(envelope.payload["transcript"], json!(""));
        assert_eq!(envelope.payload["is_silence_endpoint"], json!(true));
        assert_eq!(envelope.payload["timestamp"], json!(42_000));
        assert!(envelope.payload.get("is_interruption").is_none());
    }

    // --- config: `interruption_config_from_store` ------------------

    struct TempConfigDir(PathBuf);

    impl TempConfigDir {
        fn new() -> Self {
            // Contador, não relógio: no Windows testes paralelos liam o mesmo nanossegundo.
            static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-vad-audio-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempConfigDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_store() -> (ConfigStore, TempConfigDir) {
        let dir = TempConfigDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        (store, dir)
    }

    /// Sem nada salvo, `ConfigStore::get` já devolve o default do schema — mesmo comportamento que
    /// `transcribe.rs::resolve_provider` já depende (`ConfigStore::open` mescla sobre
    /// `schema.defaults()`). Os valores batem com `config/schemas/voice.rs`, não com
    /// `InterruptionConfig::default()` — os dois schemas divergem em `interruptionThreshold`
    /// (0.35 aqui, 0.15 lá), divergência pré-existente entre `interruption-options.ts` e
    /// `voice.schema.ts` que este porte reproduz, não inventa.
    #[test]
    fn interruption_config_from_store_reads_schema_defaults_when_unset() {
        let (store, _dir) = test_store();
        let config = interruption_config_from_store(&store);
        assert_eq!(config.enabled, true);
        assert!((config.vad_threshold - 0.35).abs() < f32::EPSILON, "default do schema é 0.35, deu {}", config.vad_threshold);
        assert_eq!(config.trigger_streak, 3);
        assert_eq!(config.cooldown_ms, 1_200);
        assert_eq!(config.endpointing_enabled, true);
        // 1_200, não 1_500: schema default alinhado ao valor que o listening path principal já
        // usava de fato (config/schemas/voice.rs, commit que tornou `vadSilenceMs` real).
        assert_eq!(config.silence_ms, 1_200);
        // 0.35, não o `NEURAL_SPEECH_THRESHOLD` de 0.5 do detector: mesma divergência de schemas
        // documentada acima para `interruptionThreshold`.
        assert!((config.speech_threshold - 0.35).abs() < f32::EPSILON, "default do schema é 0.35, deu {}", config.speech_threshold);
    }

    #[test]
    fn interruption_config_from_store_honors_saved_overrides() {
        let (store, _dir) = test_store();
        store.set("voice.interruptionEnabled", json!(false)).unwrap();
        store.set("voice.interruptionThreshold", json!(0.5)).unwrap();
        store.set("voice.interruptionTriggerStreak", json!(5)).unwrap();
        store.set("voice.interruptionCooldownMs", json!(2_000)).unwrap();
        store.set("voice.vadEndpointingEnabled", json!(false)).unwrap();
        store.set("voice.vadSilenceMs", json!(3_000)).unwrap();
        store.set("voice.vadSpeechThreshold", json!(0.6)).unwrap();

        let config = interruption_config_from_store(&store);
        assert_eq!(config.enabled, false);
        assert!((config.vad_threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.trigger_streak, 5);
        assert_eq!(config.cooldown_ms, 2_000);
        assert_eq!(config.endpointing_enabled, false);
        assert_eq!(config.silence_ms, 3_000);
        assert!((config.speech_threshold - 0.6).abs() < f32::EPSILON);
    }

}
