//! Declaração de settings de `vision` para o `FeatureRegistry` — port de
//! `vision.schema.ts::VISION_SETTINGS` (`defineFeatureSchema`), não do módulo de tools de
//! `mod.rs` (isso é outro registro, `ToolRegistry`, já resolvido).
//!
//! **Por que é um literal JSON e não uma reimplementação de `defineFeatureSchema`.** O registro
//! guarda `Declaration` como `serde_json::Value` cru (ver `feature_registry::types::Declaration`)
//! — reimplementar a inferência de `zod-inference.ts` em Rust duplicaria uma segunda fonte da
//! verdade que nenhum teste Zod trava. [`declaration`] é o RESULTADO que `defineFeatureSchema`
//! produziria hoje para `VisionConfigSchema`, calculado à mão campo a campo e ancorado por teste
//! contra o fonte (`tests::field_count_and_paths_match_the_ts_schema`): se um campo for
//! adicionado/removido/renomeado do lado TS, o teste quebra em vez de a tela silenciosamente
//! mostrar um painel desatualizado.
//!
//! `pluginId`/`isCore` NÃO entram aqui — `FeatureRegistry::get_all_settings` os injeta via
//! `normalize_settings_declaration` a partir do id passado a `register()`.
//!
//! `register_feature()` não recebe `ConfigStore`: a declaração é 100% estática (defaults vêm do
//! próprio schema, não de config salva), ao contrário de `vision::register(store)` (que injeta o
//! `ConfigStore` real no `ToolHandler`). `enabled: true` espelha `registerFeatureWithTools` em
//! `bootstrap/tools.ts`, que chama `featureRegistry.register(pluginId, feature)` sem terceiro
//! argumento — o default do lado TS é `enabled = true`, sempre, independente do valor ao vivo de
//! `vision.enabled` (esse valor governa o TOGGLE dentro da aba via `enabledConfigPath`, não a
//! entrada no registro).

use serde_json::{json, Value};

use crate::feature_registry::{self, FeatureRegistration};

fn option(label: &str, value: &str) -> Value {
    json!({ "label": label, "value": value })
}

/// As 22 chaves de `VisionSettingsGroupSchema` (15) + `VisionAwarenessGroupSchema` (7), na mesma
/// ordem de `VisionConfigSchema = VisionSettingsGroupSchema.merge(VisionAwarenessGroupSchema)`.
fn fields() -> Vec<Value> {
    vec![
        json!({ "configPath": "vision.enabled", "type": "boolean", "label": "settings.vision.field.enabled", "description": "settings.vision.field.enabled" }),
        json!({ "configPath": "vision.provider", "type": "select", "label": "settings.vision.field.provider.label", "options": [
            option("settings.vision.field.provider.option.default", "default"),
            option("settings.vision.field.provider.option.openrouter", "openrouter"),
            option("settings.vision.field.provider.option.google", "google"),
            option("settings.vision.field.provider.option.ollama", "ollama"),
            option("settings.vision.field.provider.option.openaiCompatible", "openaiCompatible"),
            option("settings.vision.field.provider.option.unsloth", "unsloth"),
        ]}),
        json!({ "configPath": "vision.model", "type": "text", "label": "settings.vision.field.model.label", "placeholder": "ex.: llava:7b", "description": "settings.vision.field.model.description" }),
        json!({ "configPath": "vision.captureQuality", "type": "number", "label": "settings.vision.field.captureQuality.label", "min": 1, "max": 100 }),
        json!({ "configPath": "vision.captureIntervalMs", "type": "number", "label": "settings.vision.field.captureIntervalMs.label", "min": 60000, "max": 600000, "step": 5000 }),
        json!({ "configPath": "vision.analysisMode", "type": "select", "label": "settings.vision.field.analysisMode.label", "options": [
            option("settings.vision.field.analysisMode.option.focused", "focused"),
            option("settings.vision.field.analysisMode.option.balanced", "balanced"),
            option("settings.vision.field.analysisMode.option.full", "full"),
        ]}),
        json!({ "configPath": "vision.autoDescribeScreen", "type": "boolean", "label": "settings.vision.field.autoDescribeScreen", "description": "settings.vision.field.autoDescribeScreen" }),
        json!({ "configPath": "vision.adaptiveCompression", "type": "boolean", "label": "settings.vision.field.adaptiveCompression.label", "description": "settings.vision.field.adaptiveCompression.description" }),
        json!({ "configPath": "vision.maxImageSizeKB", "type": "number", "label": "settings.vision.field.maxImageSizeKB.label", "min": 50, "max": 2000, "step": 50, "description": "settings.vision.field.maxImageSizeKB.description" }),
        json!({ "configPath": "vision.compressionTargetSizeKB", "type": "number", "label": "settings.vision.field.compressionTargetSizeKB.label", "min": 50, "max": 1500, "step": 50, "description": "settings.vision.field.compressionTargetSizeKB.description" }),
        json!({ "configPath": "vision.llmQuality", "type": "number", "label": "settings.vision.field.llmQuality", "description": "settings.vision.field.llmQuality", "min": 1, "max": 100 }),
        json!({ "configPath": "vision.maxImageDimension", "type": "number", "label": "settings.vision.field.maxImageDimension", "description": "settings.vision.field.maxImageDimension", "min": 320, "max": 4096 }),
        json!({ "configPath": "vision.maxOutputTokens", "type": "number", "label": "settings.vision.panel.maxOutputTokensLabel", "min": 128, "max": 4096, "step": 64 }),
        json!({ "configPath": "vision.captureBlocklist", "type": "textarea", "label": "settings.vision.field.captureBlocklist.label", "description": "settings.vision.field.captureBlocklist.description", "placeholder": "bitwarden, 1password, banco, nubank", "maxLength": 2000 }),
        json!({ "configPath": "vision.visionAssistActive", "type": "boolean", "label": "settings.vision.field.visionAssistActive", "description": "settings.vision.field.visionAssistActive" }),
        json!({ "configPath": "vision.visionAssistAutoComment", "type": "boolean", "label": "settings.vision.field.visionAssistAutoComment", "description": "settings.vision.field.visionAssistAutoComment" }),
        json!({ "configPath": "vision.codeAwareness", "type": "boolean", "label": "settings.vision.field.codeAwareness", "description": "settings.vision.field.codeAwareness" }),
        json!({ "configPath": "vision.gameAwareness", "type": "boolean", "label": "settings.vision.field.gameAwareness", "description": "settings.vision.field.gameAwareness" }),
        json!({ "configPath": "vision.proactiveCommentaryChance", "type": "number", "label": "settings.vision.field.proactiveCommentaryChance.label", "min": 0, "max": 1, "step": 0.05, "description": "settings.vision.field.proactiveCommentaryChance.description" }),
        json!({ "configPath": "vision.textDensityThreshold", "type": "number", "label": "settings.vision.field.textDensityThreshold.label", "min": 0, "max": 1, "step": 0.01, "description": "settings.vision.field.textDensityThreshold.description" }),
        json!({ "configPath": "vision.readClipboardImages", "type": "boolean", "label": "settings.vision.field.readClipboardImages", "description": "settings.vision.field.readClipboardImages" }),
        json!({ "configPath": "vision.visualDiaryEnabled", "type": "boolean", "label": "settings.vision.field.visualDiaryEnabled", "description": "settings.vision.field.visualDiaryEnabled" }),
        json!({ "configPath": "vision.visualDiaryIntervalCycles", "type": "number", "label": "settings.vision.field.visualDiaryIntervalCycles.label", "min": 1, "max": 100, "description": "settings.vision.field.visualDiaryIntervalCycles.description" }),
    ]
}

/// `VISION_SETTINGS` (`vision.schema.ts`), sem `pluginId`/`isCore` — o registro injeta os dois.
pub fn declaration() -> Value {
    json!({
        "tabId": "vision",
        "tabLabel": "settings.vision.tab.label",
        "tabDescription": "settings.vision.tab.description",
        "tabIcon": "Eye",
        "tabGroup": "settings.vision.tab.group",
        "component": "VisionSettingsPanel",
        "enabledConfigPath": "vision.enabled",
        "groups": [{
            "title": "settings.vision.tab.label",
            "component": "VisionSettingsPanel",
            "groupPrefix": "vision",
            "fields": fields(),
        }],
    })
}

/// Registra `mika.vision` no `FeatureRegistry` compartilhado — mesmo `pluginId` de
/// `registerFeatureWithTools('mika.vision', VisionFeature)` em `bootstrap/tools.ts`. Chame UMA
/// VEZ no boot (ver o cabeçalho deste arquivo para por que não precisa de `ConfigStore`).
pub fn register_feature() {
    feature_registry::shared().register(
        "mika.vision",
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
        assert_eq!(d["tabId"], json!("vision"));
        assert_eq!(d["enabledConfigPath"], json!("vision.enabled"));
        assert_eq!(d["groups"].as_array().unwrap().len(), 1);
        assert_eq!(d["groups"][0]["fields"].as_array().unwrap().len(), 23);
    }

    /// Toda `configPath` é prefixada por `vision.` — um campo sem o prefixo grava fora do
    /// namespace da feature, silenciosamente.
    #[test]
    fn every_field_config_path_is_under_the_vision_namespace() {
        for field in declaration()["groups"][0]["fields"].as_array().unwrap() {
            let path = field["configPath"].as_str().unwrap();
            assert!(
                path.starts_with("vision."),
                "configPath fora do namespace: {path}"
            );
        }
    }

    /// `vision.schema.ts` foi removido (backend Node deletado na migração para Tauri); os 23
    /// campos que ele fixava viram pin direto sobre `fields()`, única fonte de verdade agora.
    #[test]
    fn field_count_and_paths_match_the_expected_keys() {
        let keys = [
            "enabled",
            "provider",
            "model",
            "captureQuality",
            "captureIntervalMs",
            "analysisMode",
            "autoDescribeScreen",
            "adaptiveCompression",
            "maxImageSizeKB",
            "compressionTargetSizeKB",
            "llmQuality",
            "maxImageDimension",
            "maxOutputTokens",
            "captureBlocklist",
            "visionAssistActive",
            "visionAssistAutoComment",
            "codeAwareness",
            "gameAwareness",
            "proactiveCommentaryChance",
            "textDensityThreshold",
            "readClipboardImages",
            "visualDiaryEnabled",
            "visualDiaryIntervalCycles",
        ];
        let declared = fields();
        assert_eq!(keys.len(), declared.len(), "contagem de campos mudou — releia antes de ajustar o pin");
        for key in keys {
            let path = format!("vision.{key}");
            assert!(
                declared.iter().any(|f| f["configPath"] == json!(path)),
                "'{path}' sumiu de fields() — releia antes de ajustar o pin"
            );
        }
    }

    #[test]
    fn register_feature_publishes_the_vision_tab() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&"mika.vision".into())
            .expect("mika.vision registrado");
        assert!(registered.enabled);
        assert_eq!(registered.settings[0]["tabId"], json!("vision"));
    }
}
