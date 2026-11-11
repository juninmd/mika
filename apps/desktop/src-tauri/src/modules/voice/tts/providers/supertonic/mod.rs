//! Fachada pública do provedor Supertonic — o único caminho com gerador Rust real (ver
//! `../../dispatch.rs`). `generate` é a única função que `dispatch.rs` chama; o resto do submódulo
//! (`assets`, `model`, `wav`) é implementação.

mod assets;
mod model;
mod wav;

pub(crate) use model::evict_if_idle;

use std::path::PathBuf;

use crate::config::store::ConfigStore;
use crate::modules::voice::tts::abort_registry::CancelHandle;

pub struct SupertonicOptions {
    pub voice: String,
    pub language: String,
    pub speed: f32,
    pub steps: u32,
    pub cache_dir: PathBuf,
}

const SUPPORTED_LANGUAGES: [&str; 5] = ["en", "ko", "es", "pt", "fr"];

/// `dispatch.rs` pergunta antes de chamar [`generate`] para escolher o fallback local (Piper).
pub fn supports_language(language: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&language)
}

/// Chega à UI como o erro da fala: diz o idioma e a saída (provedor de nuvem), em vez de falar
/// errado em silêncio. Também é o erro de `dispatch.rs` quando nem o Piper tem voz verificada.
pub(crate) fn unsupported_language_message(store: &ConfigStore) -> String {
    let locale = crate::config::locale::app_locale(store);
    crate::modules::settings::t("voice.tts.supertonicUnsupportedLanguage", &locale)
        .replace("{{language}}", crate::config::locale::language_name(&locale))
}

/// Boot: carrega as três sessões ONNX (GPU) antes da primeira fala, que senão pagava essa carga
/// inteira antes do primeiro áudio. Só com o modelo já baixado — ver [`assets::installed_model_files`].
pub async fn prewarm(cache_dir: PathBuf) {
    let Some(paths) = assets::installed_model_files(&cache_dir) else { return };
    let started = std::time::Instant::now();
    match model::load(paths).await {
        Ok(()) => log::info!("Supertonic TTS carregado em {} ms", started.elapsed().as_millis()),
        Err(e) => log::warn!("Supertonic TTS não pré-carregou: {e}"),
    }
}

/// Porte de `generateSupertonicAudio`. A marcação `<lang>texto</lang>` do TS
/// (`` `<${language}>${trimmed}</${language}>` ``) é o próprio contrato de entrada do tokenizer do
/// modelo — mantida aqui pelo mesmo motivo que lá: é o modelo, não uma escolha de app, que espera
/// esse formato.
pub async fn generate(text: &str, opts: &SupertonicOptions, store: &ConfigStore, cancel: &CancelHandle) -> Result<Vec<u8>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("texto vazio — nada para sintetizar".to_string());
    }
    if opts.cache_dir.as_os_str().is_empty() {
        return Err("supertonic sem diretório de cache configurado — nenhum áudio gerado".to_string());
    }

    // Sem fallback para "pt": fonemas de português lendo japonês é o defeito que isto impede.
    let language = opts.language.as_str();
    if !supports_language(language) {
        return Err(unsupported_language_message(store));
    }
    let tagged = format!("<{language}>{trimmed}</{language}>");

    let paths = assets::ensure_model_files(&opts.cache_dir, store).await?;
    let voice_embedding = assets::ensure_voice(&opts.cache_dir, &opts.voice, store).await?;

    let (samples, sample_rate) =
        model::synthesize(paths, tagged, voice_embedding, opts.speed, opts.steps.max(1), cancel.clone()).await?;

    if samples.is_empty() {
        return Err("supertonic devolveu forma de onda vazia".to_string());
    }
    wav::encode_wav_pcm16(&samples, sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cancel_handle() -> CancelHandle {
        // `AbortRegistry` real, só para obter um `CancelHandle` — os testes aqui nunca chegam a
        // cancelar de verdade (falham antes, na validação de entrada).
        let registry = Box::leak(Box::new(crate::modules::voice::tts::abort_registry::AbortRegistry::new()));
        registry.register_active().handle()
    }

    fn store() -> (ConfigStore, std::path::PathBuf) {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::{boolean, choice};
        use std::sync::atomic::{AtomicU32, Ordering as O};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, O::Relaxed);
        let dir = std::env::temp_dir().join(format!("mika-supertonic-facade-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![
            Namespace { name: "privacy", fields: vec![boolean("offlineMode", false), boolean("screenCapture", false)] },
            Namespace { name: "app", fields: vec![choice("locale", "pt-BR", &["pt-BR", "ja-JP"])] },
        ]);
        (ConfigStore::open(&dir, schema).unwrap(), dir)
    }

    #[tokio::test]
    async fn empty_text_is_rejected_before_touching_disk_or_network() {
        let (store, dir) = store();
        let opts = SupertonicOptions { voice: "F3".into(), language: "pt".into(), speed: 1.0, steps: 20, cache_dir: dir.clone() };
        let err = generate("   ", &opts, &store, &cancel_handle()).await.unwrap_err();
        assert!(err.contains("vazio"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Supertonic só fala en/ko/es/pt/fr. Para o resto a fala falha dizendo o motivo — antes caía em
    /// "pt" e lia japonês com fonemas de português.
    #[tokio::test]
    async fn an_unsupported_language_is_a_named_error_not_portuguese_phonemes() {
        let (store, dir) = store();
        store.set("app.locale", serde_json::json!("ja-JP")).unwrap();
        let opts = SupertonicOptions { voice: "F3".into(), language: "ja".into(), speed: 1.0, steps: 20, cache_dir: dir.clone() };
        let err = generate("こんにちは", &opts, &store, &cancel_handle()).await.unwrap_err();
        assert!(err.contains("Japanese"), "o erro deve nomear o idioma: {err}");
        assert!(err.contains("Supertonic"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn missing_cache_dir_degrades_named_instead_of_panicking() {
        let (store, dir) = store();
        let opts = SupertonicOptions { voice: "F3".into(), language: "pt".into(), speed: 1.0, steps: 20, cache_dir: "".into() };
        let err = generate("oi", &opts, &store, &cancel_handle()).await.unwrap_err();
        assert!(err.contains("cache"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sintetiza no modelo real pelo caminho de produção (GPU quando houver). A difusão parte de
    /// ruído aleatório, então não há amostra a comparar: o que se cobra é o defeito que o DirectML
    /// já produziu em outro modelo aqui — saída que roda rápido e sai vazia ou muda.
    ///   MIKA_SUPERTONIC_DIR=%APPDATA%/com.mika.desktop/supertonic cargo test --lib supertonic -- --ignored
    #[tokio::test]
    #[ignore = "precisa do modelo Supertonic baixado pelo app"]
    async fn real_model_synthesizes_audible_speech() {
        let (store, dir) = store();
        let cache_dir = std::env::var("MIKA_SUPERTONIC_DIR").expect("defina MIKA_SUPERTONIC_DIR").into();
        let opts = SupertonicOptions { voice: "F3".into(), language: "pt".into(), speed: 1.0, steps: 20, cache_dir };
        let wav = generate("Oi, tudo bem? Vamos jogar mais uma partida hoje?", &opts, &store, &cancel_handle()).await.expect("síntese");
        let mut reader = hound::WavReader::new(std::io::Cursor::new(wav)).expect("WAV válido");
        let rate = reader.spec().sample_rate as f32;
        let samples: Vec<f32> = reader.samples::<i16>().map(|s| f32::from(s.expect("amostra")) / 32768.0).collect();
        let seconds = samples.len() as f32 / rate;
        let rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        assert!((1.5..15.0).contains(&seconds), "duração implausível para a frase: {seconds:.2} s");
        assert!(rms > 0.01, "áudio praticamente mudo: RMS {rms:.4}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Benchmark de latência pelo caminho de produção: boot (`prewarm`) → primeira fala da sessão
    /// (o tempo até o primeiro áudio que o usuário sente) → falas seguintes já quentes. Grava JSON
    /// em `MIKA_TTS_BENCH_OUT` para comparar antes/depois de uma mudança no pipeline.
    ///   MIKA_SUPERTONIC_DIR=... MIKA_TTS_BENCH_OUT=bench.json cargo test --release --lib tts_latency_bench -- --ignored --nocapture
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "precisa do modelo Supertonic (baixa na primeira vez)"]
    async fn tts_latency_bench() {
        let (store, dir) = store();
        let cache_dir: PathBuf = std::env::var("MIKA_SUPERTONIC_DIR").expect("defina MIKA_SUPERTONIC_DIR").into();
        assets::ensure_model_files(&cache_dir, &store).await.expect("modelo");
        let opts = SupertonicOptions { voice: "F3".into(), language: "pt".into(), speed: 1.0, steps: 20, cache_dir: cache_dir.clone() };
        assets::ensure_voice(&cache_dir, &opts.voice, &store).await.expect("voz");
        let sentences = ["Boa, essa foi por pouco!", "Oi, tudo bem? Vamos jogar mais uma partida hoje?", "Se você pegar o atalho pela esquerda, chega no chefe antes do tempo acabar."];
        let started = std::time::Instant::now();
        prewarm(cache_dir.clone()).await;
        let prewarm_ms = started.elapsed().as_millis();
        let mut runs = Vec::new();
        for round in 0..4 {
            for text in sentences {
                let t = std::time::Instant::now();
                let wav = generate(text, &opts, &store, &cancel_handle()).await.expect("síntese");
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let audio_ms = (wav.len() - 44) as f64 / 2.0 / 44.1;
                runs.push(serde_json::json!({"round": round, "text": text, "ms": ms.round(), "audioMs": audio_ms.round(), "rtf": ms / audio_ms}));
            }
        }
        let warm: Vec<f64> = runs.iter().skip(sentences.len()).map(|r| r["ms"].as_f64().unwrap()).collect();
        let report = serde_json::json!({
            "prewarmMs": prewarm_ms,
            "firstSpeechMs": runs[0]["ms"],
            "warmMeanMs": (warm.iter().sum::<f64>() / warm.len() as f64).round(),
            "warmMeanRtf": runs.iter().skip(sentences.len()).map(|r| r["rtf"].as_f64().unwrap()).sum::<f64>() / warm.len() as f64,
            "runs": runs,
        });
        println!("{report:#}");
        if let Ok(out) = std::env::var("MIKA_TTS_BENCH_OUT") {
            std::fs::write(out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
