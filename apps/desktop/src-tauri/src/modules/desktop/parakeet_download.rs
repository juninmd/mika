//! Wizard/settings asset `parakeet`: ~670 MB of pinned int8 graphs, streamed to disk with the same
//! incremental SHA-256 as Nemotron. The privacy chokepoint stays in `nemotron_download`
//! (`assert_network_allowed` before any GET); this module only carries the Parakeet file list.

use std::path::Path;

use super::nemotron_download::{download_verified, sha256_of};
use super::WizardResult;
use crate::config::locale::app_locale;
use crate::config::store::ConfigStore;
use crate::modules::inference::parakeet;
use crate::modules::settings::tf;

fn msg(locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
    tf(key, locale, vars)
}

const PROVIDER_TAG: &str = "wizard:parakeet";

pub(super) async fn prepare_parakeet(store: &ConfigStore, data_dir: &Path) -> WizardResult {
    // The wizard and the settings card can both ask at once; two writers on one `.part` corrupt it.
    static IN_FLIGHT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _only_one = IN_FLIGHT.lock().await;
    let dir = parakeet::cache_dir(data_dir);
    let locale = app_locale(store);
    for (file, expected) in parakeet::ASSET_FILES {
        let path = dir.join(file);
        if sha256_of(path.clone()).await.as_deref() == Some(expected) {
            continue;
        }
        if let Err(e) = download_verified(store, &parakeet::asset_url(file), &path, expected, PROVIDER_TAG).await {
            return WizardResult::fail(msg(&locale, "wizard.assetDownload.parakeetFailed", &[("file", file), ("detail", &e)]));
        }
    }
    WizardResult::ok(msg(&locale, "wizard.assetDownload.parakeetReady", &[]))
}
