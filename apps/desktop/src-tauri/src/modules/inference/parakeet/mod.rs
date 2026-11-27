//! Local STT with NVIDIA Parakeet TDT 0.6B v3 (multilingual, int8 ONNX, pinned in [`assets`]) over
//! `ort`. Opt-in: Nemotron stays the default, Whisper stays an option, and the ~670 MB export is
//! only downloaded when the user picks this card.
//!
//! The export is a NeMo TDT transducer: `nemo128` produces log-mel features, a Conformer encoder
//! subsamples them, and `decoder_joint` emits a token plus a duration per step. The model detects
//! the language itself, so `voice.asrLanguage` is accepted for the shared STT contract but ignored
//! here (documented in [`transcribe`]).
//!
//! The CPU decision mirrors Nemotron's int8 model: the graphs are quantized CPU kernels and this
//! export has no GPU measurement, so Auto keeps it on CPU and the explicit "GPU" preference still
//! tries it (see [`super::accel`]).

mod assets;
mod pipeline;
mod tdt;
mod vocab;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use ort::session::Session;

pub use assets::{cache_dir, url as asset_url, FILES as ASSET_FILES};
use pipeline::Graphs;
use vocab::Vocab;

pub const SAMPLE_RATE: u32 = 16_000;
/// Below 100 ms there is nothing to transcribe — same floor as Whisper and Nemotron.
const MIN_SAMPLES: usize = SAMPLE_RATE as usize / 10;
/// Same cap as Nemotron: four threads keep the other cores for the game and the local LLM.
const INTRA_THREADS: usize = 4;

#[derive(Debug, serde::Serialize)]
pub struct ParakeetResult {
    pub text: String,
    pub duration_ms: u64,
}

struct Engine {
    data_dir: PathBuf,
    /// Device the plan asked for (see [`super::accel`]); a different plan reloads.
    device: super::session::Device,
    graphs: Graphs,
    vocab: Vocab,
}

fn engine_cache() -> &'static Mutex<Option<Engine>> {
    static CACHE: OnceLock<Mutex<Option<Engine>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

static LEASE: super::Lease = super::Lease::new();

/// See [`super::evict_idle`].
pub(super) fn evict_if_idle(max_idle: std::time::Duration, now_ms: u64) -> bool {
    super::evict_if_idle(engine_cache(), &LEASE, max_idle, now_ms)
}

fn session(load: &mut super::session::Load, path: &Path) -> Result<Session, String> {
    load.session(path).map_err(|e| format!("ort não carregou {path:?}: {e}"))
}

fn load(data_dir: &Path) -> Result<Engine, String> {
    let dir = assets::require(data_dir)?;
    let vocab = Vocab::parse(&std::fs::read_to_string(dir.join("vocab.txt")).map_err(|e| e.to_string())?);
    if vocab.len() <= vocab.blank() || !vocab.has_blank() {
        return Err(format!("vocab.txt do Parakeet truncado: {} peças", vocab.len()));
    }
    let threads = Some(INTRA_THREADS.min(crate::modules::system::governor::onnx_threads()));
    let mut load = super::session::Load::new(super::accel::Model::Parakeet, threads);
    let preprocessor = session(&mut load, &dir.join("nemo128.onnx"))?;
    let encoder = session(&mut load, &dir.join("encoder-model.int8.onnx"))?;
    let decoder = session(&mut load, &dir.join("decoder_joint-model.int8.onnx"))?;
    let (state_shape, state_len) = pipeline::state_shape(&decoder)?;
    Ok(Engine {
        data_dir: data_dir.to_path_buf(),
        device: load.plan().device,
        graphs: Graphs { preprocessor, encoder, decoder, state_shape, state_len, vocab_size: vocab.len(), blank: vocab.blank() },
        vocab,
    })
}

fn pcm16_to_f32(pcm: &[u8], sample_rate: u32) -> Result<Vec<f32>, String> {
    if sample_rate != SAMPLE_RATE {
        return Err(format!("Parakeet espera PCM16 a {SAMPLE_RATE} Hz, recebeu {sample_rate} Hz"));
    }
    if pcm.len() % 2 != 0 {
        return Err("PCM16 com número ímpar de bytes".to_string());
    }
    Ok(pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0).collect())
}

fn loaded_engine(data_dir: &Path) -> Result<std::sync::MutexGuard<'static, Option<Engine>>, String> {
    let mut guard = engine_cache().lock().map_err(|_| "cache do engine Parakeet envenenado".to_string())?;
    LEASE.touch();
    let device = super::accel::plan(super::accel::Model::Parakeet).device;
    if guard.as_ref().is_none_or(|e| e.data_dir != data_dir || e.device != device) {
        *guard = Some(load(data_dir)?);
    }
    Ok(guard)
}

/// Loads the graphs ahead of the first utterance. A missing model is not an error here: the
/// download card is the place that reports it.
pub fn prewarm(data_dir: &Path) {
    if assets::require(data_dir).is_err() {
        return;
    }
    let started = Instant::now();
    match loaded_engine(data_dir) {
        Ok(_) => log::info!("Parakeet STT carregado em {} ms", started.elapsed().as_millis()),
        Err(e) => log::warn!("Parakeet STT não pré-carregou: {e}"),
    }
}

fn transcribe_blocking(data_dir: &Path, pcm: &[u8], sample_rate: u32, _language: Option<&str>) -> Result<Option<ParakeetResult>, String> {
    let samples = pcm16_to_f32(pcm, sample_rate)?;
    if samples.len() < MIN_SAMPLES {
        return Ok(None);
    }
    let started = Instant::now();
    let mut guard = loaded_engine(data_dir)?;
    let engine = guard.as_mut().expect("loaded_engine só devolve o cache preenchido");
    let tokens = pipeline::transcribe(&mut engine.graphs, &samples)?;
    let text = engine.vocab.decode(&tokens);
    Ok(Some(ParakeetResult { text, duration_ms: started.elapsed().as_millis() as u64 }))
}

/// ONNX runs are synchronous: they occupy one blocking worker, never the async runtime.
///
/// `language` is intentionally ignored: Parakeet v3 detects the spoken language on its own, unlike
/// Whisper (forced language) and Nemotron (a `lang_id` prompt).
pub async fn transcribe(data_dir: PathBuf, pcm: Vec<u8>, sample_rate: u32, language: Option<String>) -> Result<Option<ParakeetResult>, String> {
    tokio::task::spawn_blocking(move || transcribe_blocking(&data_dir, &pcm, sample_rate, language.as_deref()))
        .await
        .map_err(|e| format!("worker de transcrição Parakeet não completou: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm16_rejects_the_wrong_rate_and_odd_lengths() {
        assert!(pcm16_to_f32(&[0, 0], 8_000).is_err());
        assert!(pcm16_to_f32(&[0, 0, 0], SAMPLE_RATE).is_err());
        assert_eq!(pcm16_to_f32(&[0, 0x80], SAMPLE_RATE).unwrap(), vec![-1.0]);
    }

    #[test]
    fn audio_shorter_than_the_floor_is_no_speech_without_loading_the_model() {
        let missing = std::env::temp_dir().join("mika-parakeet-never-downloaded");
        assert!(transcribe_blocking(&missing, &[0u8; 100], SAMPLE_RATE, None).unwrap().is_none());
    }

    #[test]
    fn prewarm_without_the_model_loads_nothing() {
        prewarm(&std::env::temp_dir().join("mika-parakeet-never-downloaded"));
        assert!(engine_cache().lock().unwrap().is_none());
    }

    /// Needs the real model (~670 MB): `MIKA_PARAKEET_DIR=<dir with parakeet-cache/> cargo test
    /// --lib parakeet -- --ignored`. The pinned HF `nemo128.onnx` differs slightly from onnx-asr's
    /// bundled preprocessor: this export produces "Ah, ela voltou!" where onnx-asr returns "?".
    /// Everything else matches the reference, and noise returns nothing.
    #[test]
    #[ignore = "precisa do modelo Parakeet (~670 MB) em MIKA_PARAKEET_DIR e da fixture de voz pt-BR"]
    fn transcribes_the_real_pt_br_fixture_and_stays_silent_on_noise() {
        let data_dir = PathBuf::from(std::env::var("MIKA_PARAKEET_DIR").expect("MIKA_PARAKEET_DIR"));
        let wav = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/voice/fala-usuario-pt-br.wav")).unwrap();
        // The fixture carries a LIST chunk before `data`, so the samples do not start at byte 44.
        let data = wav.windows(4).position(|w| w == b"data").expect("chunk data") + 8;
        let text = transcribe_blocking(&data_dir, &wav[data..], SAMPLE_RATE, None).unwrap().unwrap().text;
        assert_eq!(text, "Ah, ela voltou! Ela tá diferente, ela. Eu tô mudando ela. Ah.");
        let noise: Vec<u8> = (0..SAMPLE_RATE as usize * 3).flat_map(|i| (((i * 7919) % 6000) as i16 - 3000).to_le_bytes()).collect();
        assert_eq!(transcribe_blocking(&data_dir, &noise, SAMPLE_RATE, None).unwrap().unwrap().text, "");
    }
}
