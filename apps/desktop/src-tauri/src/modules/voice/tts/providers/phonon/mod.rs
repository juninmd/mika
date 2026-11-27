//! Fachada do provedor Phonon (`ptts` 0.7.1) — TTS neural LOCAL em Rust puro, sem Python e sem
//! PyTorch. É o terceiro caminho com gerador real em processo, ao lado de Supertonic e Piper.
//!
//! **Nada é embarcado no instalador.** Os checkpoints vêm do Hugging Face sob demanda
//! ([`assets`]), na revisão fixa do [`catalog`], com SHA-256/SHA-1 conferidos. O modelo de fábrica é
//! `phonon-40m-pt` (CC BY-NC 4.0, uso NÃO comercial); `phonon-31m-en` é Apache-2.0 e público.
//! Modelos com gate exigem token em `apiKeys.huggingFace` e aceite dos termos na página do modelo.
//!
//! **Carga caríssima, cacheada.** [`cache`] mantém o `Synth` vivo por (modelo, idioma de
//! normalização); a síntese roda em `spawn_blocking` (o `ptts` bloqueia e usa threads próprias) e
//! respeita o [`CancelHandle`] entre chunks do stream, o mesmo contrato do Piper.

mod assets;
mod cache;
pub mod catalog;

use std::path::PathBuf;

use ptts::preprocess::Lang;
use ptts::synth::{SpeechOptions, Synth};

use crate::config::locale::app_locale;
use crate::config::store::ConfigStore;
use crate::modules::settings::tf;
use crate::modules::voice::tts::abort_registry::CancelHandle;

pub use catalog::DEFAULT_MODEL;

pub struct PhononOptions {
    pub model: String,
    pub voice: String,
    /// Idioma da interface, usado para escolher a normalização entre os idiomas do checkpoint.
    pub language: String,
    pub cache_dir: PathBuf,
}

fn msg(locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
    tf(key, locale, vars)
}

/// Primeiro idioma do checkpoint que casa com a interface; senão o primeiro do modelo. Não é
/// fallback silencioso de MODELO — o modelo é o que o usuário escolheu; só a normalização de
/// números/datas segue o idioma mais próximo que aquele checkpoint fala.
fn language_for(model: &catalog::PhononModel, ui_language: &str) -> &'static str {
    model.languages.iter().copied().find(|code| *code == ui_language).unwrap_or(model.languages[0])
}

fn lang_for(model: &catalog::PhononModel, ui_language: &str, locale: &str) -> Result<Lang, String> {
    language_for(model, ui_language)
        .parse()
        .map_err(|e: ptts::Error| msg(locale, "voice.tts.phonon.loadFailed", &[("detail", &e.to_string())]))
}

/// Síntese síncrona (chamada dentro de `spawn_blocking`): consome o stream em vez de `say` para
/// poder abortar entre chunks. `sample_rate` acompanha o PCM porque o modelo pode não ser 44,1 kHz.
fn synthesize(synth: &Synth, text: &str, voice: &str, cancel: &CancelHandle, locale: &str) -> Result<(Vec<f32>, u32), String> {
    let sample_rate = synth.sample_rate();
    let options = SpeechOptions::default().voice(voice);
    let stream = synth
        .stream_with(text, &options)
        .map_err(|e| msg(locale, "voice.tts.phonon.synthesisFailed", &[("detail", &e.to_string())]))?;
    let mut pcm = Vec::new();
    for chunk in stream {
        if cancel.is_cancelled() {
            return Err("TTS cancelled".to_string());
        }
        let chunk = chunk.map_err(|e| msg(locale, "voice.tts.phonon.synthesisFailed", &[("detail", &e.to_string())]))?;
        pcm.extend_from_slice(&chunk);
    }
    Ok((pcm, sample_rate))
}

/// Gera WAV PCM16 mono. Texto vazio, modelo/voz fora do catálogo e token ausente num modelo com
/// gate falham NOMEADO antes de qualquer áudio inventado.
pub async fn generate(text: &str, opts: &PhononOptions, store: &ConfigStore, cancel: &CancelHandle) -> Result<Vec<u8>, String> {
    let locale = app_locale(store);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(msg(&locale, "voice.tts.phonon.emptyText", &[]));
    }
    let model = catalog::model(opts.model.trim())
        .ok_or_else(|| msg(&locale, "voice.tts.phonon.modelNotInCatalog", &[("model", opts.model.trim())]))?;
    let voice = opts.voice.trim();
    if !model.voices.contains(&voice) {
        return Err(msg(&locale, "voice.tts.phonon.voiceNotInCatalog", &[("voice", voice)]));
    }
    let dir = assets::ensure_model(model, &opts.cache_dir, store).await?;
    let lang = lang_for(model, &opts.language, &locale)?;
    let synth = cache::synth(model.key, &dir, lang, &locale).await?;

    let text_owned = trimmed.to_string();
    let voice_owned = voice.to_string();
    let cancel = cancel.clone();
    let locale_for_task = locale.clone();
    let result = tokio::task::spawn_blocking(move || synthesize(&synth, &text_owned, &voice_owned, &cancel, &locale_for_task))
        .await
        .map_err(|e| msg(&locale, "voice.tts.phonon.synthesisFailed", &[("detail", &e.to_string())]))??;

    if result.0.is_empty() {
        return Err(msg(&locale, "voice.tts.phonon.emptyAudio", &[]));
    }
    crate::modules::voice::tts::providers::supertonic::encode_wav_pcm16(&result.0, result.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::voice::tts::abort_registry::AbortRegistry;

    fn cancel_handle() -> CancelHandle {
        let registry = Box::leak(Box::new(AbortRegistry::new()));
        registry.register_active().handle()
    }

    fn store() -> (ConfigStore, PathBuf) {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::boolean;
        let dir = std::env::temp_dir().join(format!("mika-phonon-facade-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "privacy",
            fields: vec![boolean("offlineMode", false), boolean("screenCapture", false)],
        }]);
        (ConfigStore::open(&dir, schema).unwrap(), dir)
    }

    fn opts(model: &str, voice: &str, dir: &std::path::Path) -> PhononOptions {
        PhononOptions { model: model.into(), voice: voice.into(), language: "pt".into(), cache_dir: dir.to_path_buf() }
    }

    #[tokio::test]
    async fn empty_text_is_rejected_before_disk_or_network() {
        let (store, dir) = store();
        let err = generate("   ", &opts(DEFAULT_MODEL, "default", &dir), &store, &cancel_handle()).await.unwrap_err();
        assert!(err.contains("vazio"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_model_outside_the_catalog_is_named_not_silently_defaulted() {
        let (store, dir) = store();
        let err = generate("oi", &opts("phonon-inventado", "default", &dir), &store, &cancel_handle()).await.unwrap_err();
        assert!(err.contains("phonon-inventado"), "{err}");
        assert!(err.contains("Phonon"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_voice_outside_the_checkpoint_is_rejected() {
        let (store, dir) = store();
        let err = generate("oi", &opts(DEFAULT_MODEL, "clone-inventado", &dir), &store, &cancel_handle()).await.unwrap_err();
        assert!(err.contains("clone-inventado"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Modo offline é o jeito determinístico de provar que o caminho tentado foi o download real do
    /// modelo (não um atalho): o chokepoint de rede recusa antes de baixar um byte.
    #[tokio::test]
    async fn offline_mode_blocks_the_model_download_by_name() {
        let (store, dir) = store();
        store.set("privacy.offlineMode", serde_json::json!(true)).unwrap();
        let err = generate("oi", &opts(DEFAULT_MODEL, "default", &dir), &store, &cancel_handle()).await.unwrap_err();
        assert!(err.to_lowercase().contains("offline"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_ui_language_picks_a_language_the_checkpoint_speaks() {
        let multilingual = catalog::model("phonon-40m-multilingual").unwrap();
        assert_eq!(language_for(multilingual, "fr"), "fr");
        assert_eq!(language_for(multilingual, "pt"), "pt");
        assert_eq!(language_for(multilingual, "ja"), "en");
        assert_eq!(language_for(catalog::model("phonon-31m-en").unwrap(), "pt"), "en");
    }

    /// Smoke REAL, manual: baixa o `phonon-31m-en` (Apache-2.0, público) e sintetiza uma frase.
    ///   MIKA_PHONON_DIR=/caminho cargo test --lib phonon -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "baixa o modelo phonon-31m-en do Hugging Face e sintetiza de verdade"]
    async fn real_model_synthesizes_audible_speech() {
        let (store, fallback_dir) = store();
        let cache_dir = std::env::var("MIKA_PHONON_DIR").map(PathBuf::from).unwrap_or_else(|_| fallback_dir.clone());
        let mut options = opts("phonon-31m-en", "default", &cache_dir);
        options.language = "en".into();
        let wav = generate("Hello! This is Phonon speaking on my machine.", &options, &store, &cancel_handle())
            .await
            .expect("síntese");
        let mut reader = hound::WavReader::new(std::io::Cursor::new(wav)).expect("WAV válido");
        let rate = reader.spec().sample_rate as f32;
        let samples: Vec<f32> = reader.samples::<i16>().map(|s| f32::from(s.expect("amostra")) / 32768.0).collect();
        let seconds = samples.len() as f32 / rate;
        let rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        assert!((1.0..12.0).contains(&seconds), "duração implausível: {seconds:.2} s");
        assert!(rms > 0.01, "áudio praticamente mudo: RMS {rms:.4}");
        let _ = std::fs::remove_dir_all(&fallback_dir);
    }
}
