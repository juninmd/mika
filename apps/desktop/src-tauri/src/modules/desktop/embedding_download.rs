//! Wizard/settings asset `local-embedding`: the pinned e5 files (`inference::embedder`), same
//! verified streaming download as Nemotron (privacy chokepoint, SHA-256, `.part` then rename).

use std::path::Path;

use super::nemotron_download::{download_verified, sha256_of};
use super::WizardResult;
use crate::config::locale::app_locale;
use crate::config::store::ConfigStore;
use crate::modules::inference::embedder;
use crate::modules::settings::tf;

pub(super) async fn prepare_local_embedding(store: &ConfigStore, data_dir: &Path) -> WizardResult {
    // Boot, the wizard and the settings card can all ask at once; two writers on one `.part` corrupt it.
    static IN_FLIGHT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _only_one = IN_FLIGHT.lock().await;
    let dir = embedder::cache_dir(data_dir);
    let locale = app_locale(store);
    for (file, remote, expected) in embedder::FILES {
        let path = dir.join(file);
        if sha256_of(path.clone()).await.as_deref() == Some(expected) {
            continue;
        }
        if let Err(e) = download_verified(store, &embedder::url(remote), &path, expected).await {
            return WizardResult::fail(tf("wizard.assetDownload.embeddingFailed", &locale, &[("file", file), ("detail", &e)]));
        }
    }
    WizardResult::ok(tf("wizard.assetDownload.embeddingReady", &locale, &[]))
}
