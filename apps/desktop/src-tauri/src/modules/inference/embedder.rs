//! Local text embedding: granite-embedding-97m-multilingual-r2 (IBM, Apache-2.0, ModernBERT, 384
//! dims, int8 ONNX). Memory search needed an Ollama server on :11434 or a paid API to be semantic at
//! all; without one it silently fell back to keyword search. This runs in-process (~125 MB; CPU or
//! GPU per `accel`), offline, and is also what the proactive line guard uses to tell "the same thing
//! in other words" apart from a new line.
//!
//! **Why granite over multilingual-e5-small (the previous model).** Same size class and the same 384
//! dimensions, but multilingual MTEB retrieval 60.3 vs 50.9 (IBM's card, 18 languages), 8k context
//! instead of 512, and no "query: " prefix. Switching changes the vector space: the memory index
//! signature names this model (`memory::episodic::embedding::signature`), the old vectors are stashed,
//! and `reembed_missing` recomputes them in the background at boot (`desktop::ensure_local_embedding`).
//! The proactive guard's threshold was re-measured for this model (`brain::semantic_repeat`).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::pipeline::PipelineTokenizer;

pub const MODEL_ID: &str = "onnx-community/granite-embedding-97m-multilingual-r2-ONNX";
pub const REVISION: &str = "536a9f241cb3f02a9c5995a1e708c784bd274859";
/// `(local file, path in the repo, SHA-256)` — the Hugging Face LFS digests on REVISION (2026-11-15).
pub const FILES: [(&str, &str, &str); 2] = [
    (
        "model_quantized.onnx",
        "onnx/model_quantized.onnx",
        "704c1ebca5fbb7cd83ced41827658ac4c9990c64f7f2874d22b78044e5022e22",
    ),
    (
        "tokenizer.json",
        "tokenizer.json",
        "51947676cae1f991fa51c6b9a24e14ee5460e5f0b9f692f13bb3159829d1592a",
    ),
];
pub const DIMENSIONS: usize = 384;
const MAX_TOKENS: usize = 512;
/// The chat LLM and the game compete for the same cores; an embedding is a few ms either way.
const INTRA_THREADS: usize = 2;

pub fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("granite-cache")
}

/// The e5 files (~120 MB) are read by nothing once granite is in place.
pub fn remove_legacy_cache(data_dir: &Path) {
    let legacy = data_dir.join("e5-cache");
    if legacy.is_dir() && is_downloaded(data_dir) {
        if let Err(e) = std::fs::remove_dir_all(&legacy) {
            log::warn!("cache do e5 não saiu ({}): {e}", legacy.display());
        }
    }
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
    let data_dir = DATA_DIR
        .get()
        .ok_or("embedding local: pasta de dados ainda não definida")?;
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
        return Err(format!(
            "modelo de embedding local não baixado ({:?})",
            cache_dir(data_dir)
        ));
    }
    let dir = cache_dir(data_dir);
    let threads = Some(INTRA_THREADS.min(crate::modules::system::governor::onnx_threads()));
    let (session, plan) =
        super::session::load(super::accel::Model::E5, &dir.join(FILES[0].0), threads)
            .map_err(|e| format!("ort não carregou o embedder: {e}"))?;
    // `canonicalize_file` accepts both the legacy "version 1.0" layout and the current one.
    let canonical = tokenizers::canonicalize_file(dir.join(FILES[1].0))
        .map_err(|e| format!("tokenizer do embedder não converteu: {e}"))?;
    let tokenizer = tokenizers::from_json(&canonical)
        .map_err(|e| format!("tokenizer do embedder não carregou: {e}"))?;
    Ok(Engine {
        data_dir: data_dir.to_path_buf(),
        device: plan.device,
        session,
        tokenizer,
    })
}

/// Unit-length embedding of `text`. Blocking (CPU inference): call it off the async runtime.
pub fn embed_blocking(data_dir: &Path, text: &str) -> Result<Vec<f32>, String> {
    let mut cache = engine_cache().lock().unwrap_or_else(|e| e.into_inner());
    LEASE.touch();
    let device = super::accel::plan(super::accel::Model::E5).device;
    if cache
        .as_ref()
        .is_none_or(|engine| engine.data_dir != data_dir || engine.device != device)
    {
        *cache = Some(load(data_dir)?);
    }
    let engine = cache.as_mut().expect("engine loaded above");
    let encoding = engine
        .tokenizer
        .encode(text, &Default::default())
        .wait()
        .map_err(|e| format!("tokenizer do embedder falhou: {e}"))?
        .into_iter()
        .next()
        .ok_or("tokenizer do embedder não devolveu nada")?;
    let ids: Vec<i64> = encoding
        .ids()
        .iter()
        .take(MAX_TOKENS)
        .map(|token| i64::from(token.id()))
        .collect();
    let len = ids.len() as i64;
    let tensor =
        |values: Vec<i64>| Tensor::from_array(([1i64, len], values)).map_err(|e| e.to_string());
    let outputs = engine
        .session
        .run(ort::inputs!["input_ids" => tensor(ids)?, "attention_mask" => tensor(vec![1; len as usize])?])
        .map_err(|e| format!("embedder falhou: {e}"))?;
    let (_, hidden) = outputs["last_hidden_state"]
        .try_extract_tensor::<f32>()
        .map_err(|e| e.to_string())?;
    Ok(cls_normalized(hidden, DIMENSIONS))
}

/// The first token's vector ([CLS]) scaled to unit length — granite's pooling
/// (`1_Pooling/config.json`: `pooling_mode_cls_token`).
pub fn cls_normalized(hidden: &[f32], dim: usize) -> Vec<f32> {
    let cls = &hidden[..dim.min(hidden.len())];
    let norm = cls.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm == 0.0 {
        return cls.to_vec();
    }
    cls.iter().map(|v| v / norm).collect()
}

/// Cosine of two unit vectors is their dot product.
pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pooling_takes_the_first_token_and_normalizes() {
        let hidden = [3.0, 4.0, /* other tokens */ 100.0, 100.0];
        assert_eq!(cls_normalized(&hidden, 2), vec![0.6, 0.8]);
    }

    #[test]
    fn a_zero_cls_vector_stays_zero_not_nan() {
        assert_eq!(cls_normalized(&[0.0, 0.0, 1.0, 1.0], 2), vec![0.0, 0.0]);
    }

    #[test]
    fn urls_are_pinned_to_the_revision() {
        assert!(url(FILES[0].1).contains(REVISION) && !url(FILES[0].1).contains("/main/"));
    }

    // Real model — not in the default gate (needs the ~125 MB download). Reproduce:
    //   MIKA_E5_TEST_DATA_DIR=<dir with granite-cache/{model_quantized.onnx,tokenizer.json}>
    //   cargo test --lib inference::embedder -- --ignored
    #[test]
    #[ignore = "precisa do modelo de embedding baixado — ver comentário acima"]
    fn paraphrases_are_closer_than_unrelated_lines() {
        let dir = PathBuf::from(
            std::env::var("MIKA_E5_TEST_DATA_DIR").expect("defina MIKA_E5_TEST_DATA_DIR"),
        );
        let a = embed_blocking(&dir, "Terminal aberto de novo? Parece hacker de filme.").unwrap();
        let b = embed_blocking(
            &dir,
            "Olha você no terminal outra vez, todo hacker de cinema.",
        )
        .unwrap();
        let c = embed_blocking(&dir, "Que tal uma pausa pra beber água?").unwrap();
        assert_eq!(a.len(), DIMENSIONS);
        assert!(
            similarity(&a, &b) > similarity(&a, &c) + 0.05,
            "{} vs {}",
            similarity(&a, &b),
            similarity(&a, &c)
        );
    }
}
