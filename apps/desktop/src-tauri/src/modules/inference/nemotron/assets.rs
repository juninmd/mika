//! Pinned export files. The revision and digests are the contract with `modules::desktop` (which
//! downloads them) and with the engine (which refuses to load a partial cache).

use std::path::{Path, PathBuf};

pub const MODEL_ID: &str = "onnx-community/nemotron-3.5-asr-streaming-0.6b-onnx-int4";
pub const REVISION: &str = "8364d9e2dd9da23789b480bdbba9e423717e42ee";

/// `(file, SHA-256)` measured with `sha256sum` on the pinned revision; the `.data` files hold the
/// external weights the `.onnx` graphs reference by name, so both halves must sit side by side.
pub const FILES: [(&str, &str); 7] = [
    ("encoder.onnx", "0b05217594ec0bda442e43a90a298ac2471a3bdcea9b169de34214e61a730e17"),
    ("encoder.onnx.data", "2f27295855aeb99ab1f8cd2254418d9ad7a087ea8dbe85f5596b4d887ea7d630"),
    ("decoder.onnx", "6a9f608dcbab71ebd81ffa4c198e82a5b6bb10f1c1830a94c752c5f543454df3"),
    ("decoder.onnx.data", "e5fd55cbeeb268f9d383e2ee72735b9fbbb13aea4bc7cd38cb73b8e16f1366c7"),
    ("joint.onnx", "e2c7d2fa40a243bf82eaca36c15698c52129de9361d2875d7f223f67fcd9482d"),
    ("joint.onnx.data", "2e0fb1c060f3777a1a76e78d5589dd54f01505a06dffbd2588e315508b402c12"),
    ("vocab.txt", "ca88922ac5a92c911b79985b69634d7a4c2ef604d61b71bbe2982210dd77cd43"),
];

pub fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("nemotron-cache")
}

pub fn url(file: &str) -> String {
    format!("https://huggingface.co/{MODEL_ID}/resolve/{REVISION}/{file}")
}

/// Missing files fail loud with what to do; digests are checked by the downloader, not per load
/// (hashing ~790 MB on every app start would cost seconds).
pub fn require(data_dir: &Path) -> Result<PathBuf, String> {
    let dir = cache_dir(data_dir);
    match FILES.iter().find(|(file, _)| !dir.join(file).is_file()) {
        Some((file, _)) => Err(format!(
            "modelo Nemotron não baixado ({file} ausente em {dir:?}) — baixe em Configurações > Voz ou refaça o passo de voz do wizard"
        )),
        None => Ok(dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_pinned_to_the_revision_not_main() {
        assert!(url("encoder.onnx").contains(REVISION));
        assert!(!url("encoder.onnx").contains("/main/"));
    }

    #[test]
    fn require_names_the_first_missing_file() {
        let dir = std::env::temp_dir().join(format!("mika-nemotron-assets-{}", std::process::id()));
        let error = require(&dir).unwrap_err();
        assert!(error.contains("encoder.onnx"), "{error}");
    }
}
