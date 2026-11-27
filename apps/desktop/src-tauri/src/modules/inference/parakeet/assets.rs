//! Pinned export files. The revision and digests are the contract with `modules::desktop` (which
//! downloads them) and with the engine (which refuses to load a partial cache).

use std::path::{Path, PathBuf};

pub const MODEL_ID: &str = "istupakov/parakeet-tdt-0.6b-v3-onnx";
pub const REVISION: &str = "8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce";

/// `(file, SHA-256)` measured with `sha256sum` on the pinned revision. The int8 export keeps the
/// three graphs plus `vocab.txt` side by side; ~670 MB in total.
pub const FILES: [(&str, &str); 4] = [
    ("encoder-model.int8.onnx", "6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09"),
    ("decoder_joint-model.int8.onnx", "eea7483ee3d1a30375daedc8ed83e3960c91b098812127a0d99d1c8977667a70"),
    ("nemo128.onnx", "a9fde1486ebfcc08f328d75ad4610c67835fea58c73ba57e3209a6f6cf019e9f"),
    ("vocab.txt", "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d"),
];

pub fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("parakeet-cache")
}

pub fn url(file: &str) -> String {
    format!("https://huggingface.co/{MODEL_ID}/resolve/{REVISION}/{file}")
}

/// Missing files fail loud with what to do; digests are checked by the downloader, not per load
/// (hashing ~670 MB on every app start would cost seconds).
pub fn require(data_dir: &Path) -> Result<PathBuf, String> {
    let dir = cache_dir(data_dir);
    match FILES.iter().find(|(file, _)| !dir.join(file).is_file()) {
        Some((file, _)) => Err(format!(
            "modelo Parakeet não baixado ({file} ausente em {dir:?}) — baixe em Configurações > Voz ou refaça o passo de voz do wizard"
        )),
        None => Ok(dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_pinned_to_the_revision_not_main() {
        assert!(url("vocab.txt").contains(REVISION));
        assert!(!url("vocab.txt").contains("/main/"));
    }

    #[test]
    fn require_names_the_first_missing_file() {
        let dir = std::env::temp_dir().join(format!("mika-parakeet-assets-{}", std::process::id()));
        let error = require(&dir).unwrap_err();
        assert!(error.contains("encoder-model.int8.onnx"), "{error}");
    }
}
