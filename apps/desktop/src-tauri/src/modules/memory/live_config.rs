//! Config da memória lida A CADA USO do `ConfigStore` único do processo.
//!
//! Antes, `memory_store(app)` copiava `memory.enabled`, `memory.episodicListLimit`, o scoring de
//! `brain.*` e as configurações de embedding para `OnceLock`s no boot: a aba Memória/Cérebro
//! gravava, e o runtime seguia com o valor antigo até reiniciar o app (trocar o provedor de
//! embedding não trocava nada). Guardar o `Arc<ConfigStore>` e ler na hora fecha isso; `get` é
//! leitura de mapa em memória, não disco.

use crate::config::schemas::llm_defaults;
use std::sync::{Arc, OnceLock};

use super::{EmbeddingSettings, EpisodicScoringConfig};
use crate::config::store::ConfigStore;

/// = `GET_ALL_LIMIT_DEFAULT` (`episodic-types.ts`).
pub(super) const EPISODIC_LIST_LIMIT_DEFAULT: i64 = 500;

static CONFIG: OnceLock<Arc<ConfigStore>> = OnceLock::new();

pub(super) fn attach(config: Arc<ConfigStore>) {
    let _ = CONFIG.set(config);
}

fn config() -> Option<&'static ConfigStore> {
    CONFIG.get().map(Arc::as_ref)
}

fn text(config: &ConfigStore, path: &str) -> String {
    config.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn text_or(config: &ConfigStore, path: &str, fallback: &str) -> String {
    Some(text(config, path)).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
}

fn bool_or(config: &ConfigStore, path: &str, fallback: bool) -> bool {
    config.get(path).and_then(|v| v.as_bool()).unwrap_or(fallback)
}

fn f64_or(config: &ConfigStore, path: &str, fallback: f64) -> f64 {
    config.get(path).and_then(|v| v.as_f64()).unwrap_or(fallback)
}

pub(super) fn list_limit_from(config: &ConfigStore) -> i64 {
    config.get("memory.episodicListLimit").and_then(|v| v.as_i64()).filter(|&n| n != 0).unwrap_or(EPISODIC_LIST_LIMIT_DEFAULT)
}

pub(super) fn scoring_from(config: &ConfigStore) -> EpisodicScoringConfig {
    EpisodicScoringConfig {
        reranking_enabled: bool_or(config, "brain.rerankingEnabled", true),
        emotional_tagging: bool_or(config, "brain.emotionalTagging", true),
        embedding_enabled: bool_or(config, "brain.embeddingEnabled", true),
        recency_bias: f64_or(config, "brain.recencyBias", 0.25),
        rag_top_k: config.get("brain.ragTopK").and_then(|v| v.as_u64()).map(|n| n as usize).filter(|&n| n != 0).unwrap_or(5),
        min_similarity: f64_or(config, "brain.minSimilarity", 0.25),
        lexical_rerank_weight: f64_or(config, "brain.lexicalRerankWeight", 0.15),
        short_term_ttl_hours: f64_or(config, "memory.shortTermTtlHours", 168.0),
        long_term_ttl_days: f64_or(config, "memory.longTermTtlDays", 365.0),
    }
}

pub(super) fn embedding_from(config: &ConfigStore) -> EmbeddingSettings {
    EmbeddingSettings {
        provider: text_or(config, "llm.embeddingProvider", "local"),
        model: text(config, "llm.embeddingModel"),
        base_url: text_or(config, "apiKeys.ollamaBaseUrl", llm_defaults::OLLAMA_BASE_URL),
        api_key: text(config, "apiKeys.ollamaApiKey"),
        google_api_key: text(config, "apiKeys.googleAI"),
        openrouter_api_key: text(config, "apiKeys.openrouter"),
        openai_compat_base_url: text(config, "apiKeys.openaiCompatibleBaseUrl"),
        openai_compat_api_key: text(config, "apiKeys.openaiCompatibleApiKey"),
        unsloth_base_url: text_or(config, "apiKeys.unslothBaseUrl", "http://localhost:8888/v1"),
        unsloth_api_key: text(config, "apiKeys.unslothApiKey"),
        task_type: text(config, "llm.embeddingTaskType"),
        dimensions: config.get("llm.embeddingOutputDimensionality").and_then(|v| v.as_i64()).unwrap_or(0),
        offline_mode: crate::modules::privacy::is_offline_mode_enabled(config),
    }
}

/// `None` só antes do primeiro `memory_store(app)` do processo (e sempre em teste de unidade).
pub(super) fn list_limit() -> i64 {
    config().map_or(EPISODIC_LIST_LIMIT_DEFAULT, list_limit_from)
}

pub(super) fn recording_allowed() -> bool {
    config().is_none_or(super::memory_recording_allowed)
}

pub(super) fn scoring() -> Option<EpisodicScoringConfig> {
    config().map(scoring_from)
}

pub(super) fn embedding() -> Option<EmbeddingSettings> {
    config().map(embedding_from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store() -> (ConfigStore, std::path::PathBuf) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("mika-memory-live-config-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap();
        (ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap(), dir)
    }

    /// O defeito: a leitura acontecia uma vez; aqui cada leitura depois de um `set` vê o valor novo.
    #[test]
    fn a_setting_changed_after_boot_is_seen_by_the_next_read() {
        let (config, dir) = store();
        assert_eq!(embedding_from(&config).provider, "local");
        config.set("llm.embeddingProvider", json!("ollama")).unwrap();
        assert_eq!(embedding_from(&config).provider, "ollama");

        let top_k_before = scoring_from(&config).rag_top_k;
        config.set("brain.ragTopK", json!(top_k_before + 3)).unwrap();
        assert_eq!(scoring_from(&config).rag_top_k, top_k_before + 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn defaults_hold_when_a_value_is_missing_or_zero() {
        let (config, dir) = store();
        let embedding = embedding_from(&config);
        assert_eq!((embedding.base_url.as_str(), embedding.dimensions), ("http://localhost:11434", 0));
        assert_eq!(list_limit_from(&config), config.get("memory.episodicListLimit").and_then(|v| v.as_i64()).filter(|&n| n != 0).unwrap_or(500));
        let _ = std::fs::remove_dir_all(dir);
    }
}
