//! Declaração de settings `mika.embeddings` no `FeatureRegistry` — porte de `EMBEDDING_SETTINGS`
//! (`apps/desktop/src/main/services/llm/embedding.schema.ts`). Mora em `modules/memory/` porque é
//! o mapeamento que a coordenação desta rodada definiu: no TS o par `EmbeddingFeature`/tools de
//! embedding vive em `services/llm/`, mas o back-end de busca por embedding em Rust é dever de
//! `memory` (ver o cabeçalho de `mod.rs`) — não existe `modules/llm/` dono desta aba.
//!
//! **Achado, não regressão**: `EmbeddingFeature.enabledConfigPath` (`'brain.embeddingEnabled'`,
//! `embedding.feature.ts`) só existe no `FeatureDefinition` — `FeatureRegistryService.register`
//! nunca lê esse campo (só `feature.settings`), então ele nunca chega ao
//! `FeatureSettingsDeclaration` de verdade. `defineFeatureSchema` também não infere
//! `enabledConfigPath` sozinho aqui porque nenhum campo do grupo se chama `enabled`. O resultado —
//! confirmado lendo os dois arquivos — é que a aba `embeddings` HOJE não carrega
//! `enabledConfigPath` nenhum no TS. Esta declaração reproduz esse comportamento (campo ausente),
//! não o que `embedding.feature.ts` parecia prometer.

use serde_json::{json, Value};

use crate::feature_registry::FeatureRegistration;

/// = `EMBEDDING_TASK_TYPE_OPTIONS` (`packages/sdk/src/contracts/types/runtime-types.ts`) — rótulos
/// em inglês, literais no próprio catálogo (não são chave de i18n, ao contrário do resto da aba).
fn task_type_options() -> Value {
    json!([
        { "label": "Semantic Similarity", "value": "SEMANTIC_SIMILARITY" },
        { "label": "Classification", "value": "CLASSIFICATION" },
        { "label": "Clustering", "value": "CLUSTERING" },
        { "label": "Retrieval Document", "value": "RETRIEVAL_DOCUMENT" },
        { "label": "Retrieval Query", "value": "RETRIEVAL_QUERY" },
        { "label": "Question Answering", "value": "QUESTION_ANSWERING" },
        { "label": "Fact Verification", "value": "FACT_VERIFICATION" },
        { "label": "Code Retrieval Query", "value": "CODE_RETRIEVAL_QUERY" },
    ])
}

fn declaration() -> Value {
    json!({
        "tabId": "embeddings",
        "tabLabel": "settings.embeddings.tab.label",
        "tabIcon": "Brain",
        "tabGroup": "settings.embeddings.tab.group",
        "tabDescription": "settings.embeddings.tab.description",
        "component": "EmbeddingProviderPanel",
        "heroNote": "settings.embeddings.tab.heroNote",
        "groups": [{
            "title": "settings.embeddings.group.title",
            "component": "EmbeddingProviderPanel",
            "columns": 2,
            "groupPrefix": "llm",
            "fields": [
                {
                    "configPath": "llm.embeddingProvider",
                    "type": "select",
                    "label": "settings.embeddings.field.embeddingProvider.label",
                    "helperText": "settings.embeddings.field.embeddingProvider.helperText",
                    "options": [
                        { "label": "settings.embeddings.field.embeddingProvider.option.google", "value": "google" },
                        { "label": "settings.embeddings.field.embeddingProvider.option.openrouter", "value": "openrouter" },
                        { "label": "settings.embeddings.field.embeddingProvider.option.ollama", "value": "ollama" },
                        { "label": "settings.embeddings.field.embeddingProvider.option.openaiCompatible", "value": "openaiCompatible" },
                        { "label": "settings.embeddings.field.embeddingProvider.option.unsloth", "value": "unsloth" },
                    ],
                },
                {
                    "configPath": "llm.embeddingModel",
                    "type": "text",
                    "label": "settings.embeddings.field.embeddingModel.label",
                    "placeholder": "gemini-embedding-001, embeddinggemma...",
                    "helperText": "settings.embeddings.field.embeddingModel.helperText",
                },
                {
                    "configPath": "llm.embeddingTaskType",
                    "type": "select",
                    "label": "settings.embeddings.field.embeddingTaskType.label",
                    "helperText": "settings.embeddings.field.embeddingTaskType.helperText",
                    "options": task_type_options(),
                },
                {
                    "configPath": "llm.embeddingOutputDimensionality",
                    "type": "number",
                    "label": "settings.embeddings.field.embeddingOutputDimensionality.label",
                    "placeholder": "0 = padrão do modelo",
                    "helperText": "settings.embeddings.field.embeddingOutputDimensionality.helperText",
                    "min": 0,
                    "inputMode": "decimal",
                },
            ],
        }],
    })
}

/// Registra a aba `embeddings` uma vez — chame de `modules()` (efeito colateral, sem
/// `ConfigStore` nenhum).
pub(super) fn register() {
    crate::feature_registry::shared().register(
        "mika.embeddings",
        FeatureRegistration { enabled: true, settings: vec![declaration()], ..Default::default() },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `embedding.schema.ts` foi removido (backend Node deletado na migração para Tauri); o pin de
    /// contagem de 4 campos passa a ser coberto direto por `register_publishes_the_embeddings_tab_with_no_enabled_config_path`
    /// abaixo, que já assere `fields().len() == 4` sobre a própria `declaration()`.
    #[test]
    fn register_publishes_the_embeddings_tab_with_no_enabled_config_path() {
        register();
        let all = crate::feature_registry::shared().get_all_settings();
        let tab = all.iter().find(|d| d["tabId"] == json!("embeddings")).expect("aba embeddings registrada");
        assert_eq!(tab["pluginId"], json!("mika.embeddings"));
        assert!(tab.get("enabledConfigPath").is_none(), "achado do cabeçalho: TS também não seta este campo");
        assert_eq!(tab["groups"][0]["fields"].as_array().unwrap().len(), 4);
    }
}
