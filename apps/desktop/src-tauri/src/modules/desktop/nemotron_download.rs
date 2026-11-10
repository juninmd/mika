//! Wizard/settings asset `nemotron`: ~790 MB of pinned weights, streamed to disk with an incremental
//! SHA-256. `download_bytes` holds the whole body in memory, fine for the Whisper files but not for a
//! 690 MB encoder; the privacy chokepoint stays the same (`assert_network_allowed` before any GET).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::StreamExt;
use sha2::{Digest, Sha256};

use super::WizardResult;
use crate::config::locale::app_locale;
use crate::config::store::ConfigStore;
use crate::modules::inference::nemotron;
use crate::modules::privacy;
use crate::modules::settings::tf;

/// Texto no idioma da interface — chega à UI do wizard/settings como o resultado do download.
fn msg(locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
    tf(key, locale, vars)
}

/// Whole-file ceiling, sized for the encoder on a slow (~1 MB/s) link.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const PROVIDER_TAG: &str = "wizard:nemotron";

pub(super) async fn prepare_nemotron(store: &ConfigStore, data_dir: &Path) -> WizardResult {
    // The wizard and the settings card can both ask at once; two writers on one `.part` corrupt it.
    static IN_FLIGHT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _only_one = IN_FLIGHT.lock().await;
    let dir = nemotron::cache_dir(data_dir);
    let locale = app_locale(store);
    for (file, expected) in nemotron::ASSET_FILES {
        let path = dir.join(file);
        if sha256_of(path.clone()).await.as_deref() == Some(expected) {
            continue;
        }
        if let Err(e) = download_verified(store, &nemotron::asset_url(file), &path, expected).await {
            return WizardResult::fail(msg(&locale, "wizard.assetDownload.nemotronFailed", &[("file", file), ("detail", &e)]));
        }
    }
    WizardResult::ok(msg(&locale, "wizard.assetDownload.nemotronReady", &[]))
}

/// Hashing hundreds of MB is blocking IO: it runs on a blocking worker, never on the runtime.
pub(super) async fn sha256_of(path: PathBuf) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(path).ok()?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1 << 20];
        loop {
            let read = file.read(&mut buffer).ok()?;
            if read == 0 {
                return Some(hex::encode(hasher.finalize()));
            }
            hasher.update(&buffer[..read]);
        }
    })
    .await
    .ok()
    .flatten()
}

pub(super) async fn download_verified(store: &ConfigStore, url: &str, target: &Path, expected: &str) -> Result<(), String> {
    let locale = app_locale(store);
    privacy::assert_network_allowed(store, url, PROVIDER_TAG)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| msg(&locale, "wizard.assetDownload.createFailed", &[("path", &parent.display().to_string()), ("detail", &e.to_string())]))?;
    }
    let failed = |detail: String| msg(&locale, "wizard.assetDownload.failed", &[("detail", &detail)]);
    let response = privacy::fixed_endpoint_client().get(url).timeout(DOWNLOAD_TIMEOUT).send().await.map_err(|e| failed(e.to_string()))?;
    if !response.status().is_success() {
        return Err(failed(format!("HTTP {}", response.status().as_u16())));
    }
    let partial = PathBuf::from(format!("{}.part", target.to_string_lossy()));
    let result = match stream_to(response, &partial, &locale).await {
        Ok(digest) if digest == expected => rename_blocking(&partial, target, &locale).await,
        Ok(digest) => Err(msg(&locale, "wizard.assetDownload.digestMismatch", &[("digest", &digest)])),
        Err(e) => Err(e),
    };
    if result.is_err() {
        remove_blocking(partial).await;
    }
    result
}

/// Renaming/removing a ~690 MB file can stall on a slow disk or antivirus scan: blocking worker.
async fn rename_blocking(partial: &Path, target: &Path, locale: &str) -> Result<(), String> {
    let (from, to) = (partial.to_path_buf(), target.to_path_buf());
    let failed = |detail: String| msg(locale, "wizard.assetDownload.renameFailed", &[("detail", &detail)]);
    tokio::task::spawn_blocking(move || std::fs::rename(from, to))
        .await
        .map_err(|e| failed(e.to_string()))?
        .map_err(|e| failed(e.to_string()))
}

async fn remove_blocking(partial: PathBuf) {
    let _ = tokio::task::spawn_blocking(move || std::fs::remove_file(partial)).await;
}

async fn stream_to(response: reqwest::Response, partial: &Path, locale: &str) -> Result<String, String> {
    let write_failed = |detail: String| msg(locale, "wizard.assetDownload.writeFailed", &[("detail", &detail)]);
    let mut file = std::fs::File::create(partial)
        .map_err(|e| msg(locale, "wizard.assetDownload.createFailed", &[("path", &partial.display().to_string()), ("detail", &e.to_string())]))?;
    let mut hasher = Sha256::new();
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| msg(locale, "wizard.assetDownload.interrupted", &[("detail", &e.to_string())]))?;
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|e| write_failed(e.to_string()))?;
    }
    // Flushing ~690 MB can block for seconds: off the async runtime.
    tokio::task::spawn_blocking(move || file.sync_all())
        .await
        .map_err(|e| write_failed(e.to_string()))?
        .map_err(|e| write_failed(e.to_string()))?;
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_a_file_in_blocks_like_sha256sum() {
        let path = std::env::temp_dir().join(format!("mika-nemotron-hash-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        let digest = sha256_of(path.clone()).await;
        std::fs::remove_file(&path).unwrap();
        assert_eq!(digest.as_deref(), Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    }

    #[tokio::test]
    async fn a_missing_file_has_no_digest() {
        assert!(sha256_of(std::env::temp_dir().join("mika-nemotron-never-there")).await.is_none());
    }
}
