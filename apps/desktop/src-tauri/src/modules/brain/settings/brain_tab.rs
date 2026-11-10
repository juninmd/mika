//! Declaração da aba `brain` — porte de `BRAIN_SETTINGS`
//! (`apps/desktop/src/main/services/brain/brain.schema.ts`), campo a campo, no formato que
//! `defineFeatureSchema`/`inferFieldMeta` produzem hoje (`packages/sdk/src/features/`).

use serde_json::{json, Value};

/// `autoApproveTools` é um `z.array(z.string())` — `inferFieldMeta` não tem um ramo para
/// `ZodArray` (só boolean/number/enum/string/object), então cai no fallback genérico: `{configPath,
/// type: 'text', label}`, SEM `description` mesmo o campo tendo uma — confirmado lendo
/// `zod-inference.ts` linha a linha. Reproduzido aqui de propósito, não é um campo esquecido.
pub(super) fn declaration() -> Value {
    json!({
        "tabId": "brain",
        "tabLabel": "settings.brain.tab.label",
        "tabIcon": "Brain",
        "tabGroup": "settings.brain.tab.group",
        "tabDescription": "settings.brain.tab.description",
        "component": "MemoryPanel",
        "enabledConfigPath": "brain.enabled",
        "groups": [
            {
                "title": "settings.brain.group.title",
                "component": "MemoryPanel",
                "groupPrefix": "brain",
                "fields": [
                    { "configPath": "brain.enabled", "type": "boolean", "label": "settings.brain.field.enabled", "description": "settings.brain.field.enabled" },
                    { "configPath": "brain.embeddingEnabled", "type": "boolean", "label": "settings.brain.field.embeddingEnabled", "description": "settings.brain.field.embeddingEnabled" },
                    { "configPath": "brain.recencyBias", "type": "number", "label": "settings.brain.field.recencyBias.label", "min": 0.0, "max": 1.0, "inputMode": "decimal" },
                    { "configPath": "brain.rerankingEnabled", "type": "boolean", "label": "settings.brain.field.rerankingEnabled", "description": "settings.brain.field.rerankingEnabled" },
                    { "configPath": "brain.emotionalTagging", "type": "boolean", "label": "settings.brain.field.emotionalTagging", "description": "settings.brain.field.emotionalTagging" },
                    { "configPath": "brain.ragTopK", "type": "number", "label": "settings.brain.field.ragTopK.label", "min": 1.0, "max": 20.0, "inputMode": "decimal" },
                    { "configPath": "brain.minSimilarity", "type": "number", "label": "settings.brain.field.minSimilarity.label", "min": 0.0, "max": 1.0, "inputMode": "decimal" },
                    { "configPath": "brain.lexicalRerankWeight", "type": "number", "label": "settings.brain.field.lexicalRerankWeight.label", "min": 0.0, "max": 0.5, "inputMode": "decimal" },
                    { "configPath": "brain.approveRiskyTools", "type": "boolean", "label": "settings.brain.field.approveRiskyTools", "description": "settings.brain.field.approveRiskyTools" },
                    { "configPath": "brain.autoApproveTools", "type": "text", "label": "settings.brain.field.autoApproveTools" },
                ],
            },
            {
                "title": "settings.memoryConfig.group.title",
                "component": "MemoryPanel",
                "groupPrefix": "memory",
                "fields": [
                    { "configPath": "memory.shortTermTtlHours", "type": "number", "label": "settings.memory.field.shortTermTtlHours.label", "min": 1.0, "max": 720.0, "inputMode": "decimal" },
                    { "configPath": "memory.longTermTtlDays", "type": "number", "label": "settings.memory.field.longTermTtlDays.label", "min": 1.0, "max": 1095.0, "inputMode": "decimal" },
                ],
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_approve_tools_field_has_no_description_matching_the_zod_inference_fallback_gap() {
        let declaration = declaration();
        let fields = declaration["groups"][0]["fields"].as_array().unwrap();
        let field = fields.iter().find(|f| f["configPath"] == json!("brain.autoApproveTools")).unwrap();
        assert_eq!(field["type"], json!("text"));
        assert!(field.get("description").is_none());
    }

    #[test]
    fn enabled_config_path_is_inferred_from_the_first_group_with_an_enabled_field() {
        assert_eq!(declaration()["enabledConfigPath"], json!("brain.enabled"));
    }
}
