//! Declaração de settings de `llm` para o `FeatureRegistry` — port de `llm.schema.ts::LLM_SETTINGS`
//! (`defineFeatureSchema`). Ver o cabeçalho de `vision/feature.rs` para por que é um literal JSON
//! calculado à mão em vez de reimplementar `zod-inference.ts`; os campos moram em
//! `feature_fields.rs` só para caber no teto de 300 linhas.
//!
//! **`pluginId` divergente de propósito, e por quê.** No TS, `LLM_SETTINGS` não tem registro
//! próprio: `brain.feature.ts` importa `LLM_SETTINGS` de `../llm` e o embute dentro de
//! `BrainFeature.settings = [BRAIN_SETTINGS, LLM_SETTINGS]`, registrado sob `mika.brain`
//! (`bootstrap/tools.ts::registerFeatureWithTools('mika.brain', BrainFeature)`) — outra frente,
//! fora do meu escopo. Como `mika:settings-declarations` (`getAllSettings`) NÃO filtra por
//! `enabled` nem por quem é dono do `pluginId` (só dedupe por `tabId` — ver
//! `feature_registry::get_all_settings`), registrar esta aba sob um id PRÓPRIO (`mika.llm`) em vez
//! de sob `mika.brain` produz o MESMO conjunto de abas na tela — só o `pluginId`/`isCore` anotado
//! na declaração da aba `llm` muda (de `mika.brain` para `mika.llm`). Documentado aqui e no
//! relatório da rodada para quem for dono de `mika.brain` não reabrir a investigação achando um
//! `LLM_SETTINGS` ausente — ele está registrado, só que por outro id.
//!
//! `register_feature()` não recebe `ConfigStore`: mesma razão de `vision::register_feature` — é
//! dado estático, e `enabled: true` espelha o default de `FeatureRegistryService::register`
//! (`enabled = true`) que `registerFeatureWithTools` nunca sobrescreve.

use serde_json::{json, Value};

use super::feature_fields::{api_key_fields, model_and_behavior_fields, provider_field};
use crate::feature_registry::{self, FeatureRegistration};

fn groups() -> Vec<Value> {
    vec![
        json!({
            "title": "settings.llm.group.apiKeys",
            "component": "LLMConnectionPanel",
            "groupPrefix": "apiKeys",
            "description": "settings.llm.group.apiKeys.description",
            "fields": api_key_fields(),
        }),
        json!({
            "title": "settings.llm.group.connection",
            "component": "LLMConnectionPanel",
            "groupPrefix": "llm",
            "columns": 2,
            "fields": [provider_field()],
        }),
        json!({
            "title": "settings.llm.group.openrouterCatalog",
            "badge": "Modelos",
            "component": "OpenRouterModelPanel",
            "visibleWhen": { "configPath": "llm.provider", "equals": "openrouter" },
            "fields": [],
        }),
        json!({
            "title": "settings.llm.group.model",
            "component": "LLMConnectionPanel",
            "description": "settings.llm.group.model.description",
            "groupPrefix": "llm",
            "columns": 2,
            "fields": model_and_behavior_fields(),
        }),
        json!({
            "title": "settings.llm.group.usageStats",
            "component": "TokenUsagePanel",
            "fields": [],
        }),
    ]
}

/// `LLM_SETTINGS` (`llm.schema.ts`), sem `pluginId`/`isCore` — o registro injeta os dois. Sem
/// `component`/`enabledConfigPath` no nível da aba: `inferTabComponent` não acha um componente
/// comum aos 5 grupos, e nenhum grupo declara campo `enabled` — mesmo resultado do lado TS.
pub fn declaration() -> Value {
    json!({
        "tabId": "llm",
        "tabLabel": "settings.llm.tab.label",
        "tabIcon": "Cpu",
        "tabGroup": "settings.llm.tab.group",
        "tabDescription": "settings.llm.tab.description",
        "heroNote": "settings.llm.tab.heroNote",
        "groups": groups(),
    })
}

/// Registra a aba `llm` no `FeatureRegistry` compartilhado sob `mika.llm` — ver o cabeçalho deste
/// arquivo para por que não é `mika.brain`. Chame UMA VEZ no boot.
pub fn register_feature() {
    feature_registry::shared().register(
        "mika.llm",
        FeatureRegistration {
            enabled: true,
            settings: vec![declaration()],
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_has_the_expected_tab_shape() {
        let d = declaration();
        assert_eq!(d["tabId"], json!("llm"));
        assert!(
            d.get("component").is_none(),
            "5 grupos usam componentes diferentes — sem component no tab"
        );
        assert!(
            d.get("enabledConfigPath").is_none(),
            "nenhum grupo tem campo 'enabled' — sem enabledConfigPath"
        );
        assert_eq!(d["groups"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn empty_schema_groups_carry_metadata_without_fields() {
        let d = declaration();
        let catalog_group = &d["groups"][2];
        assert_eq!(catalog_group["fields"].as_array().unwrap().len(), 0);
        assert_eq!(catalog_group["visibleWhen"]["equals"], json!("openrouter"));

        let usage_group = &d["groups"][4];
        assert_eq!(usage_group["fields"].as_array().unwrap().len(), 0);
        assert_eq!(usage_group["component"], json!("TokenUsagePanel"));
    }

    /// Pin de ordem: os 5 títulos de grupo, na ordem que `llm.schema.ts` declarava antes de o
    /// backend Node ser deletado (migração para Tauri). `groups()` agora é a única fonte de
    /// verdade — o invariante que importa (a UI não reordena os grupos em silêncio) continua
    /// coberto sobre a própria declaração Rust.
    #[test]
    fn group_titles_are_in_the_expected_order() {
        let titles_in_order = [
            "settings.llm.group.apiKeys",
            "settings.llm.group.connection",
            "settings.llm.group.openrouterCatalog",
            "settings.llm.group.model",
            "settings.llm.group.usageStats",
        ];

        let declared = groups();
        assert_eq!(declared.len(), titles_in_order.len());
        for (group, title) in declared.iter().zip(titles_in_order) {
            assert_eq!(group["title"], json!(title));
        }
    }

    #[test]
    fn register_feature_publishes_the_llm_tab_under_its_own_plugin_id() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&"mika.llm".into())
            .expect("mika.llm registrado");
        assert!(registered.enabled);
        assert_eq!(registered.settings[0]["tabId"], json!("llm"));
    }
}
