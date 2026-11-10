//! Local text embedding: multilingual-e5-small (int8 ONNX, 384 dims, MIT). Memory search needed an
//! Ollama server on :11434 or a paid API to be semantic at all; without one it silently fell back
//! to keyword search. This runs in-process (~120 MB; CPU or GPU per `accel`), offline, and is also what the proactive
//! line guard uses to tell "the same thing in other words" apart from a new line.
//!
//! e5 expects a "query: " prefix; used on both sides because every caller here compares text
//! against text of the same kind (a memory against a memory, a line against a line).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::pipeline::PipelineTokenizer;

pub const MODEL_ID: &str = "Xenova/multilingual-e5-small";
pub const REVISION: &str = "761b726dd34fb83930e26aab4e9ac3899aa1fa78";
/// `(local file, path in the repo, SHA-256 measured with sha256sum on REVISION)`.
pub const FILES: [(&str, &str, &str); 2] = [
    ("model_quantized.onnx", "onnx/model_quantized.onnx", "f80102d3f2a1229f387d3c81909990d8945513e347b0eab049f7de3c6f98c193"),
    ("tokenizer.json", "tokenizer.json", "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39"),
];
pub const DIMENSIONS: usize = 384;
const MAX_TOKENS: usize = 512;
/// The chat LLM and the game compete for the same cores; an embedding is a few ms either way.
const INTRA_THREADS: usize = 2;

pub fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("e5-cache")
}

pub fn url(remote_path: &str) -> String {
    format!("https://huggingface.co/{MODEL_ID}/resolve/{REVISION}/{remote_path}")
}

/// Both files present. Digests are checked by the downloader, not per load.
pub fn is_downloaded(data_dir: &Path) -> bool {
    let dir = cache_dir(data_dir);
    FILES.iter().all(|(file, _, _)| dir.join(file).is_file())
}

/// Set once at boot (`desktop::ensure_local_embedding`): memory writes and the proactive guard
/// have no `AppHandle` to resolve it themselves.
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_data_dir(data_dir: &Path) {
    let _ = DATA_DIR.set(data_dir.to_path_buf());
}

/// [`embed_blocking`] on the boot data dir; `Err` before boot or before the download.
pub fn embed(text: &str) -> Result<Vec<f32>, String> {
    let data_dir = DATA_DIR.get().ok_or("embedding local: pasta de dados ainda não definida")?;
    embed_blocking(data_dir, text)
}

struct Engine {
    data_dir: PathBuf,
    /// Dispositivo que o plano pediu (`accel`, carga de fundo: cede a GPU ao jogo); muda → recarrega.
    device: super::session::Device,
    session: Session,
    tokenizer: PipelineTokenizer,
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

fn load(data_dir: &Path) -> Result<Engine, String> {
    if !is_downloaded(data_dir) {
        return Err(format!("modelo de embedding local não baixado ({:?})", cache_dir(data_dir)));
    }
    let dir = cache_dir(data_dir);
    let threads = Some(INTRA_THREADS.min(crate::modules::system::governor::onnx_threads()));
    let (session, plan) = super::session::load(super::accel::Model::E5, &dir.join(FILES[0].0), threads).map_err(|e| format!("ort não carregou o e5: {e}"))?;
    // Same legacy "version 1.0" tokenizer.json as Whisper's: it has to be canonicalized first.
    let canonical = tokenizers::canonicalize_file(dir.join(FILES[1].0)).map_err(|e| format!("tokenizer do e5 não converteu: {e}"))?;
    let tokenizer = tokenizers::from_json(&canonical).map_err(|e| format!("tokenizer do e5 não carregou: {e}"))?;
    Ok(Engine { data_dir: data_dir.to_path_buf(), device: plan.device, session, tokenizer })
}

/// Unit-length embedding of `text`. Blocking (CPU inference): call it off the async runtime.
pub fn embed_blocking(data_dir: &Path, text: &str) -> Result<Vec<f32>, String> {
    let mut cache = engine_cache().lock().unwrap_or_else(|e| e.into_inner());
    LEASE.touch();
    let device = super::accel::plan(super::accel::Model::E5).device;
    if cache.as_ref().is_none_or(|engine| engine.data_dir != data_dir || engine.device != device) {
        *cache = Some(load(data_dir)?);
    }
    let engine = cache.as_mut().expect("engine loaded above");
    let encoding = engine
        .tokenizer
        .encode(format!("query: {text}").as_str(), &Default::default())
        .wait()
        .map_err(|e| format!("tokenizer do e5 falhou: {e}"))?
        .into_iter()
        .next()
        .ok_or("tokenizer do e5 não devolveu nada")?;
    let ids: Vec<i64> = encoding.ids().iter().take(MAX_TOKENS).map(|token| i64::from(token.id())).collect();
    // `None` means unpadded: every token counts.
    let mask: Vec<i64> = match encoding.attention_mask() {
        Some(mask) => mask.iter().take(ids.len()).map(|&m| i64::from(m)).collect(),
        None => vec![1; ids.len()],
    };
    let len = ids.len() as i64;
    let tensor = |values: Vec<i64>| Tensor::from_array(([1i64, len], values)).map_err(|e| e.to_string());
    let outputs = engine
        .session
        .run(ort::inputs![
            "input_ids" => tensor(ids)?,
            "attention_mask" => tensor(mask.clone())?,
            "token_type_ids" => tensor(vec![0; len as usize])?,
        ])
        .map_err(|e| format!("e5 falhou: {e}"))?;
    let (_, hidden) = outputs["last_hidden_state"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
    Ok(mean_pool_normalized(hidden, &mask, DIMENSIONS))
}

/// Mean of the token vectors the mask keeps, scaled to unit length (e5's pooling).
pub fn mean_pool_normalized(hidden: &[f32], mask: &[i64], dim: usize) -> Vec<f32> {
    let mut sum = vec![0f32; dim];
    let mut kept = 0f32;
    for (token, &m) in hidden.chunks_exact(dim).zip(mask) {
        if m == 0 {
            continue;
        }
        kept += 1.0;
        sum.iter_mut().zip(token).for_each(|(s, v)| *s += v);
    }
    let norm = sum.iter().map(|v| (v / kept.max(1.0)).powi(2)).sum::<f32>().sqrt();
    if norm == 0.0 {
        return sum;
    }
    sum.iter().map(|v| v / kept.max(1.0) / norm).collect()
}

/// Cosine of two unit vectors is their dot product.
pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pooling_averages_only_the_kept_tokens_and_normalizes() {
        let hidden = [3.0, 0.0, /* padding */ 100.0, 100.0, 0.0, 4.0];
        let pooled = mean_pool_normalized(&hidden, &[1, 0, 1], 2);
        assert!((pooled[0] - 0.6).abs() < 1e-6 && (pooled[1] - 0.8).abs() < 1e-6, "{pooled:?}");
    }

    #[test]
    fn a_fully_masked_input_is_a_zero_vector_not_nan() {
        assert_eq!(mean_pool_normalized(&[1.0, 2.0], &[0], 2), vec![0.0, 0.0]);
    }

    #[test]
    fn urls_are_pinned_to_the_revision() {
        assert!(url(FILES[0].1).contains(REVISION) && !url(FILES[0].1).contains("/main/"));
    }

    // Real model — not in the default gate (needs the ~135 MB download). Reproduce:
    //   MIKA_E5_TEST_DATA_DIR=<dir with e5-cache/{model_quantized.onnx,tokenizer.json}>
    //   cargo test --lib inference::embedder -- --ignored
    #[test]
    #[ignore = "precisa do modelo e5 baixado — ver comentário acima"]
    fn paraphrases_are_closer_than_unrelated_lines() {
        let dir = PathBuf::from(std::env::var("MIKA_E5_TEST_DATA_DIR").expect("defina MIKA_E5_TEST_DATA_DIR"));
        let a = embed_blocking(&dir, "Terminal aberto de novo? Parece hacker de filme.").unwrap();
        let b = embed_blocking(&dir, "Olha você no terminal outra vez, todo hacker de cinema.").unwrap();
        let c = embed_blocking(&dir, "Que tal uma pausa pra beber água?").unwrap();
        assert_eq!(a.len(), DIMENSIONS);
        assert!(similarity(&a, &b) > similarity(&a, &c) + 0.05, "{} vs {}", similarity(&a, &b), similarity(&a, &c));
    }
}
