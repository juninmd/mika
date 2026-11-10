//! Local STT with NVIDIA Nemotron 3.5 ASR Streaming 0.6B (int4 ONNX, pinned in [`assets`]) over
//! `ort`. Chosen over Whisper because Whisper's free-running decoder turns noise into caption
//! text ("[Música]", "[SOM DE GOL]"); this RNNT transducer anchors every token to audio and, on
//! the same noise set (silence, white noise, music, crowd, keys), returned nothing at all while
//! transcribing pt-BR speech correctly — measured against the numpy reference before this port.

mod assets;
mod features;
mod lang;
mod stream;
mod vocab;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use ort::session::Session;

pub use assets::{cache_dir, url as asset_url, FILES as ASSET_FILES};
use features::{LogMel, SAMPLE_RATE};
use stream::Graphs;
use vocab::Vocab;

/// Below 100 ms there is nothing to transcribe — same floor as the Whisper path.
const MIN_SAMPLES: usize = SAMPLE_RATE as usize / 10;
/// Measured on the pt-BR fixture (6 s, 16 logical cores): idle, RTF 0.244 with 4 threads, the same
/// as ORT's all-cores default (2 threads: 0.320); beside a parallel build, 0.455 against 0.403.
/// Capping at 4 leaves the other cores to the game and the local LLM for ~10% of a sub-real-time pass.
const INTRA_THREADS: usize = 4;

#[derive(Debug, serde::Serialize)]
pub struct NemotronResult {
    pub text: String,
    pub duration_ms: u64,
}

struct Engine {
    data_dir: PathBuf,
    /// Dispositivo que o plano pediu (`accel`); plano diferente recarrega.
    device: super::session::Device,
    graphs: Graphs,
    vocab: Vocab,
    mel: LogMel,
}

fn engine_cache() -> &'static Mutex<Option<Engine>> {
    static CACHE: OnceLock<Mutex<Option<Engine>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

static LEASE: super::Lease = super::Lease::new();

/// Ver [`super::evict_idle`].
pub(super) fn evict_if_idle(max_idle: std::time::Duration, now_ms: u64) -> bool {
    super::evict_if_idle(engine_cache(), &LEASE, max_idle, now_ms)
}

/// CPU in Auto (`accel::Workload::CpuMeasured`), measured on an RTX 4060 Ti with DirectML: the int4
/// encoder fails to initialise (MatMulNBits, E_INVALIDARG); decoder/joiner run but slower (RTF 0.332
/// vs 0.257, a copy per token); the fp16 export runs 4x faster and returns empty text for clear
/// speech. "GPU" forced in Settings tries it anyway and falls back per graph.
fn session(load: &mut super::session::Load, path: &Path) -> Result<Session, String> {
    load.session(path).map_err(|e| format!("ort não carregou {path:?}: {e}"))
}

fn load(data_dir: &Path) -> Result<Engine, String> {
    let dir = assets::require(data_dir)?;
    let vocab = Vocab::parse(&std::fs::read_to_string(dir.join("vocab.txt")).map_err(|e| e.to_string())?);
    if vocab.len() <= stream::BLANK {
        return Err(format!("vocab.txt do Nemotron truncado: {} peças", vocab.len()));
    }
    let threads = Some(INTRA_THREADS.min(crate::modules::system::governor::onnx_threads()));
    let mut load = super::session::Load::new(super::accel::Model::Nemotron, threads);
    let graphs = Graphs {
        encoder: session(&mut load, &dir.join("encoder.onnx"))?,
        decoder: session(&mut load, &dir.join("decoder.onnx"))?,
        joiner: session(&mut load, &dir.join("joint.onnx"))?,
    };
    Ok(Engine { data_dir: data_dir.to_path_buf(), device: load.plan().device, graphs, vocab, mel: LogMel::new() })
}

fn pcm16_to_f32(pcm: &[u8], sample_rate: u32) -> Result<Vec<f32>, String> {
    if sample_rate != SAMPLE_RATE {
        return Err(format!("Nemotron espera PCM16 a {SAMPLE_RATE} Hz, recebeu {sample_rate} Hz"));
    }
    if pcm.len() % 2 != 0 {
        return Err("PCM16 com número ímpar de bytes".to_string());
    }
    Ok(pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0).collect())
}

fn loaded_engine(data_dir: &Path) -> Result<std::sync::MutexGuard<'static, Option<Engine>>, String> {
    let mut guard = engine_cache().lock().map_err(|_| "cache do engine Nemotron envenenado".to_string())?;
    LEASE.touch();
    let device = super::accel::plan(super::accel::Model::Nemotron).device;
    if guard.as_ref().is_none_or(|e| e.data_dir != data_dir || e.device != device) {
        *guard = Some(load(data_dir)?);
    }
    Ok(guard)
}

/// Loads the graphs ahead of the first utterance, which otherwise pays 1.3 s (warm disk) to
/// 4.7 s (cold) before a word is decoded. A missing model is not an error here: the download card
/// is the place that reports it.
pub fn prewarm(data_dir: &Path) {
    if assets::require(data_dir).is_err() {
        return;
    }
    let started = Instant::now();
    match loaded_engine(data_dir) {
        Ok(_) => log::info!("Nemotron STT carregado em {} ms", started.elapsed().as_millis()),
        Err(e) => log::warn!("Nemotron STT não pré-carregou: {e}"),
    }
}

fn transcribe_blocking(data_dir: &Path, pcm: &[u8], sample_rate: u32, language: Option<&str>) -> Result<Option<NemotronResult>, String> {
    let samples = pcm16_to_f32(pcm, sample_rate)?;
    if samples.len() < MIN_SAMPLES {
        return Ok(None);
    }
    let started = Instant::now();
    let mut guard = loaded_engine(data_dir)?;
    let engine = guard.as_mut().expect("loaded_engine só devolve o cache preenchido");
    let mel = engine.mel.compute(&samples);
    let tokens = stream::transcribe(&mut engine.graphs, &mel, lang::prompt_id(language))?;
    let text = engine.vocab.decode(&tokens);
    Ok(Some(NemotronResult { text, duration_ms: started.elapsed().as_millis() as u64 }))
}

/// ONNX runs are synchronous: they occupy one blocking worker, never the async runtime.
pub async fn transcribe(data_dir: PathBuf, pcm: Vec<u8>, sample_rate: u32, language: Option<String>) -> Result<Option<NemotronResult>, String> {
    tokio::task::spawn_blocking(move || transcribe_blocking(&data_dir, &pcm, sample_rate, language.as_deref()))
        .await
        .map_err(|e| format!("worker de transcrição Nemotron não completou: {e}"))?
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
        let missing = std::env::temp_dir().join("mika-nemotron-never-downloaded");
        assert!(transcribe_blocking(&missing, &[0u8; 100], SAMPLE_RATE, None).unwrap().is_none());
    }

    #[test]
    fn prewarm_without_the_model_loads_nothing() {
        prewarm(&std::env::temp_dir().join("mika-nemotron-never-downloaded"));
        assert!(engine_cache().lock().unwrap().is_none());
    }

    /// Needs the real model (≈790 MB): `MIKA_NEMOTRON_DIR=<dir with nemotron-cache/> cargo test
    /// --lib nemotron -- --ignored`. Reference from the numpy engine on the same fixture.
    #[test]
    #[ignore = "precisa do modelo Nemotron (~790 MB) em MIKA_NEMOTRON_DIR e da fixture de voz pt-BR"]
    fn transcribes_the_real_pt_br_fixture_and_stays_silent_on_noise() {
        let data_dir = PathBuf::from(std::env::var("MIKA_NEMOTRON_DIR").expect("MIKA_NEMOTRON_DIR"));
        let wav = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/voice/fala-usuario-pt-br.wav")).unwrap();
        // The fixture carries a LIST chunk before `data`, so the samples do not start at byte 44.
        let data = wav.windows(4).position(|w| w == b"data").expect("chunk data") + 8;
        let text = transcribe_blocking(&data_dir, &wav[data..], SAMPLE_RATE, Some("pt")).unwrap().unwrap().text;
        assert_eq!(text, "Ela voltou ela tá diferente ela tô mudando ela");
        let noise: Vec<u8> = (0..SAMPLE_RATE as usize * 3).flat_map(|i| (((i * 7919) % 6000) as i16 - 3000).to_le_bytes()).collect();
        assert_eq!(transcribe_blocking(&data_dir, &noise, SAMPLE_RATE, Some("pt")).unwrap().unwrap().text, "");
    }
}
