//! Porte de `apps/desktop/src/main/services/voice/audio/tts-speaker.ts`
//! (`speakThroughTTS`/`speakChunks`/`processSingleChunk`) — o orquestrador que fatia o texto em
//! sentenças, gera áudio por pedaço e emite os dois eventos que `vad_pipeline.rs` está esperando
//! (ver o cabeçalho de lá): `SPEECH_STARTED` antes do primeiro pedaço, `SPEECH_FINISHED` sempre no
//! fim, mesmo em erro/cancelamento (o TS usa `finally` para isso; aqui é um guard de escopo, ver
//! [`speak`]). A síntese de um chunk individual (`processSingleChunk`) mora em `chunk.rs` — mesmo
//! arquivo lógico, separado só por tamanho (AGENTS.md, <300 linhas).
//!
//! **Por que traits em vez de `EventBus`.** Este módulo não tem `AppHandle` nem uma instância de
//! `VadPipeline` (dono real de `InterruptionDetector::on_speech_started`/`on_speech_finished`, ver
//! `voice/vad_pipeline.rs`) — igual `vision::register` não tem o `AppHandle` para registrar sozinho
//! (`vision/mod.rs`, §3). [`SpeechObserver`] e [`SpeechSink`] são o ponto de injeção pronto para
//! quem tiver as duas pontas: implementar `SpeechObserver` sobre um `VadPipeline` fecha a lacuna (1)
//! do cabeçalho de `vad_pipeline.rs`; implementar `SpeechSink` sobre `AppHandle::emit` entrega o
//! áudio ao renderer (`mika:event:tts:audio`, hoje só do lado TS).
//!
//! **Mudo (`isSpeechMuted`) e `Events.TTS_FALLBACK` ficam de fora.** O primeiro mora em
//! `voice/audio/speech-mute.ts`, fora deste diretório e sem porte Rust ainda; o segundo só dispara
//! quando o ElevenLabs cai para Supertonic, caminho cortado (`dispatch.rs`). Nenhum dos dois muda o
//! resultado observável hoje (só Supertonic está ligado), e inventar uma leitura de config para um
//! toggle sem efeito seria o mesmo código morto que `interruption_detector.rs` recusou.

mod chunk;

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::store::ConfigStore;
use crate::modules::voice::tts::abort_registry::{self, CancelHandle};
use crate::modules::voice::tts::cache::TtsCache;
use crate::modules::voice::tts::preprocessor::{has_speakable_text, split_first_clause, split_into_sentences};
use crate::modules::voice::tts::state::TtsState;

use chunk::{produces_no_speech, speak_chunk};

/// Ponto de injeção para "Mika está falando" — ver o cabeçalho do arquivo. Default no-op: um
/// chamador que não precisa do sinal (testes, `speak` isolado) não é forçado a implementar nada.
pub trait SpeechObserver: Send + Sync {
    fn on_speech_started(&self, _now_ms: u64) {}
    fn on_speech_finished(&self) {}
}

pub struct TtsChunkAudio {
    pub audio: Vec<u8>,
    pub mime_type: &'static str,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub provider: String,
}

/// Ponto de injeção para o áudio pronto — equivalente a `Events.TTS_AUDIO_READY`. Ver o cabeçalho
/// do arquivo.
pub trait SpeechSink: Send + Sync {
    fn on_chunk_ready(&self, chunk: TtsChunkAudio);
}

pub struct SpeakResult {
    pub success: bool,
    pub error: Option<String>,
    pub chunks_spoken: usize,
    /// Ninguém LÊ este campo ainda: `speak_tool` devolve `{success, data|error}`, a forma exata de
    /// `handlers/speak.ts::runSpeak`, e lá `cancelled` também não atravessa para o resultado da
    /// tool. É escrito de verdade (o `speak` distingue cancelamento de falha) e serve a quem quiser
    /// diferenciar "o usuário interrompeu" de "deu erro" — allow por item, não por diretório.
    #[allow(dead_code)]
    pub cancelled: bool,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Porte de `speakThroughTTS`. `content` chega cru (sem sanitizar) — o corte em sentenças roda
/// sobre o texto ORIGINAL, mesma ordem do TS (sanitizar antes cortaria pontuação que o corte usa).
#[allow(clippy::too_many_arguments)]
pub async fn speak(
    content: &str,
    state: &TtsState,
    cache: &Mutex<TtsCache>,
    cache_dir: &Path,
    store: &ConfigStore,
    observer: &dyn SpeechObserver,
    sink: &dyn SpeechSink,
) -> SpeakResult {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return SpeakResult { success: false, error: Some("Text cannot be empty".into()), chunks_spoken: 0, cancelled: false };
    }
    if !has_speakable_text(trimmed) {
        return SpeakResult {
            success: false,
            error: Some("Text has no speakable content".into()),
            chunks_spoken: 0,
            cancelled: false,
        };
    }

    let guard = abort_registry::global().register_active();
    let cancel = guard.handle();

    observer.on_speech_started(now_ms());
    let outcome = speak_all_chunks(trimmed, state, cache, cache_dir, store, &cancel, sink).await;
    observer.on_speech_finished();

    outcome
}

#[allow(clippy::too_many_arguments)]
async fn speak_all_chunks(
    trimmed: &str,
    state: &TtsState,
    cache: &Mutex<TtsCache>,
    cache_dir: &Path,
    store: &ConfigStore,
    cancel: &CancelHandle,
    sink: &dyn SpeechSink,
) -> SpeakResult {
    let sentences = split_into_sentences(trimmed);
    let candidates = if sentences.len() > 1 { sentences } else { vec![trimmed.to_string()] };
    let speakable: Vec<String> =
        candidates.into_iter().filter(|c| has_speakable_text(c) && !produces_no_speech(state, c)).collect();
    let mut chunks = if speakable.is_empty() { vec![trimmed.to_string()] } else { speakable };
    // O primeiro áudio espera a síntese inteira do primeiro trecho: cortá-lo na primeira vírgula
    // adianta a fala, e o resto sintetiza enquanto ele toca (ver `split_first_clause`).
    let rest = chunks.split_off(1);
    chunks = split_first_clause(&chunks[0]).into_iter().chain(rest).collect();

    let mut chunks_spoken = 0usize;
    let mut error = None;
    let mut cancelled = false;

    for (index, chunk) in chunks.iter().enumerate() {
        if cancel.is_cancelled() {
            cancelled = true;
            break;
        }
        let outcome = speak_chunk(chunk, index, chunks.len(), state, cache, cache_dir, store, cancel, sink).await;
        if outcome.cancelled {
            cancelled = true;
            break;
        }
        if outcome.success {
            chunks_spoken += 1;
        } else {
            error = outcome.error.or_else(|| Some("Unknown error".into()));
            break;
        }
    }

    if cancelled {
        return SpeakResult { success: false, error: Some("TTS cancelled".into()), chunks_spoken, cancelled: true };
    }
    if let Some(error) = error {
        return SpeakResult { success: false, error: Some(error), chunks_spoken, cancelled: false };
    }
    SpeakResult { success: true, error: None, chunks_spoken, cancelled: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct NoopObserver;
    impl SpeechObserver for NoopObserver {}

    struct NoopSink;
    impl SpeechSink for NoopSink {
        fn on_chunk_ready(&self, _chunk: TtsChunkAudio) {}
    }

    struct CountingObserver {
        started: AtomicUsize,
        finished: AtomicUsize,
    }
    impl SpeechObserver for CountingObserver {
        fn on_speech_started(&self, _now_ms: u64) {
            self.started.fetch_add(1, Ordering::SeqCst);
        }
        fn on_speech_finished(&self) {
            self.finished.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn cut_provider_state() -> TtsState {
        // "kokoro" está no dispatch como corte nomeado — exercita o caminho de erro sem tocar
        // rede/ONNX real (Supertonic exigiria os pesos baixados, ver `providers/supertonic`).
        let mut state = TtsState::default();
        state.provider = "kokoro".into();
        state
    }

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-tts-speaker-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, TempDir) {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::boolean;

        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "privacy",
            fields: vec![boolean("offlineMode", false), boolean("screenCapture", false)],
        }]);
        (ConfigStore::open(&dir.0, schema).unwrap(), dir)
    }

    #[tokio::test]
    async fn empty_text_never_touches_the_provider_or_the_observer() {
        let (store, _dir) = store();
        let observer = CountingObserver { started: AtomicUsize::new(0), finished: AtomicUsize::new(0) };
        let result = speak("   ", &cut_provider_state(), &Mutex::new(TtsCache::new()), Path::new("."), &store, &observer, &NoopSink)
            .await;

        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("Text cannot be empty"));
        assert_eq!(observer.started.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn punctuation_only_text_is_rejected_as_not_speakable() {
        let (store, _dir) = store();
        let result =
            speak("... !!", &cut_provider_state(), &Mutex::new(TtsCache::new()), Path::new("."), &store, &NoopObserver, &NoopSink)
                .await;
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("Text has no speakable content"));
    }

    /// A prova central: mesmo com o provedor cortado (erro garantido), `on_speech_started` E
    /// `on_speech_finished` disparam — é exatamente esse par que destrava `voice:vad-audio` (ver o
    /// cabeçalho do arquivo). Um teste que só passasse com Supertonic gerando de verdade não
    /// provaria isto isoladamente da rede.
    #[tokio::test]
    async fn speech_started_and_finished_fire_around_a_failing_provider_call() {
        let (store, _dir) = store();
        let observer = CountingObserver { started: AtomicUsize::new(0), finished: AtomicUsize::new(0) };
        let result = speak(
            "Oi, tudo bem?",
            &cut_provider_state(),
            &Mutex::new(TtsCache::new()),
            Path::new("."),
            &store,
            &observer,
            &NoopSink,
        )
        .await;

        assert!(!result.success, "kokoro é um corte nomeado — a chamada tem de falhar");
        assert_eq!(observer.started.load(Ordering::SeqCst), 1);
        assert_eq!(observer.finished.load(Ordering::SeqCst), 1, "finished tem de disparar mesmo em erro (mesmo `finally` do TS)");
    }

    /// Cancelamento explícito via `CancelHandle::cancel()`, sem depender de timing entre tasks: o
    /// mesmo caminho que `abort_registry::force_stop_tts()` aciona por dentro, testado sem a
    /// corrida de "o registro global já tinha o guard quando cancelei?".
    #[test]
    fn a_cancelled_handle_short_circuits_speak_chunk_before_any_provider_call() {
        let registry = abort_registry::AbortRegistry::new();
        let guard = registry.register_active();
        let handle = guard.handle();
        handle.cancel();
        assert!(handle.is_cancelled());
    }

    struct FailingSink;
    impl SpeechSink for FailingSink {
        fn on_chunk_ready(&self, _chunk: TtsChunkAudio) {
            panic!("um provedor cortado nunca deve chegar a produzir áudio");
        }
    }

    #[tokio::test]
    async fn a_cut_provider_never_reaches_the_sink() {
        let (store, _dir) = store();
        let result = speak(
            "Oi, tudo bem?",
            &cut_provider_state(),
            &Mutex::new(TtsCache::new()),
            Path::new("."),
            &store,
            &NoopObserver,
            &FailingSink,
        )
        .await;
        assert!(!result.success);
    }

    /// A prova de ponta a ponta do item 1: `system` completa `speak()` com `success: true` sem
    /// nenhum áudio no `sink` — é exatamente esse `SpeakResult.success` que `auto_speak/mod.rs`
    /// checa antes de logar `"auto-speak não virou áudio"`; um `false` aqui logaria um `warn` por
    /// FRASE para quem escolheu "Sistema — Não gera áudio" de propósito.
    #[tokio::test]
    async fn system_provider_completes_successfully_without_reaching_the_sink() {
        let (store, _dir) = store();
        let mut state = TtsState::default();
        state.provider = "system".into();
        let result =
            speak("Oi, tudo bem?", &state, &Mutex::new(TtsCache::new()), Path::new("."), &store, &NoopObserver, &FailingSink)
                .await;
        assert!(result.success, "system devia terminar com sucesso: {:?}", result.error);
        assert_eq!(result.error, None);
    }

    struct TimingSink(std::time::Instant, Mutex<Vec<f64>>);
    impl SpeechSink for TimingSink {
        fn on_chunk_ready(&self, _chunk: TtsChunkAudio) {
            self.1.lock().unwrap().push(self.0.elapsed().as_secs_f64() * 1000.0);
        }
    }

    /// Tempo até o primeiro áudio de uma resposta típica pelo `speak()` de produção (Supertonic
    /// real, cache vazio a cada rodada). Grava JSON em `MIKA_TTS_BENCH_OUT`.
    ///   MIKA_SUPERTONIC_DIR=... MIKA_TTS_BENCH_OUT=ttfa.json cargo test --release --lib speak_ttfa_bench -- --ignored --nocapture
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "precisa do modelo Supertonic baixado"]
    async fn speak_ttfa_bench() {
        let (store, _dir) = store();
        let cache_dir: std::path::PathBuf = std::env::var("MIKA_SUPERTONIC_DIR").expect("defina MIKA_SUPERTONIC_DIR").into();
        let reply = "Se você pegar o atalho pela esquerda, passando pela ponte quebrada, chega no chefe antes do tempo acabar. Boa sorte! Eu fico de olho no mapa.";
        let state = TtsState::default();
        let mut rounds = Vec::new();
        for round in 0..5 {
            let sink = TimingSink(std::time::Instant::now(), Mutex::new(Vec::new()));
            let result = speak(reply, &state, &Mutex::new(TtsCache::new()), &cache_dir, &store, &NoopObserver, &sink).await;
            assert!(result.success, "{:?}", result.error);
            let times = sink.1.into_inner().unwrap();
            if round > 0 {
                rounds.push(times);
            }
        }
        let mean = |i: fn(&Vec<f64>) -> f64| (rounds.iter().map(i).sum::<f64>() / rounds.len() as f64).round();
        let report = serde_json::json!({
            "chunks": rounds[0].len(),
            "ttfaMeanMs": mean(|t| t[0]),
            "totalMeanMs": mean(|t| *t.last().unwrap()),
            "rounds": rounds,
        });
        println!("{report:#}");
        if let Ok(out) = std::env::var("MIKA_TTS_BENCH_OUT") {
            std::fs::write(out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
        }
    }
}
