//! Carga e cache do `ptts::synth::Synth` por (modelo, idioma de normalização).
//!
//! Carregar um checkpoint Phonon custa o mesmo que carregar um modelo ONNX: ler dezenas de MB,
//! montar a rede e registrar as vozes. Por isso a instância fica viva no processo e é reusada entre
//! falas — o mesmo desenho de `providers/supertonic/model.rs`, adaptado: `Synth` já é `Send + Sync`,
//! então o cache guarda `Arc<Synth>` e a síntese concorrente não precisa de mutex.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use ptts::checkpoint::{Checkpoint, ResolveOptions};
use ptts::preprocess::Lang;
use ptts::synth::{DeviceKind, Quant, Synth};
use tokio::sync::Mutex;

use crate::modules::settings::tf;

type Cache = HashMap<String, Arc<Synth>>;

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(model_key: &str, lang: Lang) -> String {
    format!("{model_key}:{}", lang.as_str())
}

fn load_error(locale: &str, detail: &str) -> String {
    tf("voice.tts.phonon.loadFailed", locale, &[("detail", detail)])
}

/// O `ptts` quer um `Lang` na CARGA (a normalização de números/datas faz parte do builder), então
/// o idioma entra na chave do cache: o mesmo checkpoint em `fr` e `de` são duas instâncias.
fn load_synth(dir: &Path, lang: Lang, locale: &str) -> Result<Synth, String> {
    let checkpoint = Checkpoint::resolve(dir, ResolveOptions { quant: Quant::Q80, weights: None })
        .map_err(|e| load_error(locale, &e.to_string()))?;
    let mut synth = checkpoint.builder(lang).device(DeviceKind::Cpu).build().map_err(|e| load_error(locale, &e.to_string()))?;
    checkpoint.register_voices(&mut synth);
    Ok(synth)
}

/// Devolve a instância carregada (ou carrega na primeira vez) para este modelo+idioma.
pub async fn synth(model_key: &'static str, dir: &Path, lang: Lang, locale: &str) -> Result<Arc<Synth>, String> {
    let key = cache_key(model_key, lang);
    let mut loaded = cache().lock().await;
    if let Some(existing) = loaded.get(&key) {
        return Ok(Arc::clone(existing));
    }
    let dir = dir.to_path_buf();
    let locale = locale.to_string();
    let locale_for_task = locale.clone();
    let synth = tokio::task::spawn_blocking(move || load_synth(&dir, lang, &locale_for_task))
        .await
        .map_err(|e| load_error(&locale, &e.to_string()))??;
    let synth = Arc::new(synth);
    loaded.insert(key, Arc::clone(&synth));
    Ok(synth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_key_separates_the_same_checkpoint_by_normalization_language() {
        assert_eq!(cache_key("phonon-40m-multilingual", Lang::Pt), "phonon-40m-multilingual:pt");
        assert_ne!(cache_key("phonon-40m-multilingual", Lang::Pt), cache_key("phonon-40m-multilingual", Lang::Fr));
    }
}
