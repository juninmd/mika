//! Declaração de settings de `privacy` para o `FeatureRegistry` — port de
//! `privacy.schema.ts::PRIVACY_SETTINGS` (`defineFeatureSchema`), não da lógica de chokepoint em
//! `super` (captura de tela / modo offline). Ver o cabeçalho de `../vision/feature.rs` para por
//! que é um literal JSON calculado à mão em vez de reimplementar `zod-inference.ts`.
//!
//! **Sem `enabledConfigPath`, de propósito.** Nenhum dos dois grupos de `PrivacySchema` declara um
//! campo `enabled` — `inferEnabledConfigPath` do lado TS não acha candidato e devolve `undefined`;
//! mesmo caso de `llm/feature.rs` (5 grupos, nenhum com `enabled`).
//!
//! **Opções de `conversationRetention` vêm do fallback de `inferEnumField`,
//! não de `options` explícito.** `privacy.schema.ts` só passa `m({ label })` — sem `options` — para
//! o enum, então `zod-inference.ts` gera `options` a partir dos VALORES do enum via
//! `capitalize()` (`v.charAt(0).toUpperCase() + v.slice(1)`). Para um valor que começa com dígito
//! (`'1day'`, `'7days'`, `'30days'`, `'90days'`) `charAt(0).toUpperCase()` não muda nada — só
//! `'forever'` vira `'Forever'`. Reproduzido aqui de propósito, não é capitalização esquecida.
//!
//! `register_feature()` não recebe `ConfigStore`: mesma razão de `vision::register_feature` — é
//! dado estático, e `enabled: true` espelha `definePlugin`/`registerFeatureWithTools`, que nunca
//! passam um terceiro argumento ao registro.

use serde_json::{json, Value};

use crate::feature_registry::{self, FeatureRegistration};

/// Mesmo `manifest.id` de `apps/desktop/src/features/privacy/index.ts`.
pub const PRIVACY_PLUGIN_ID: &str = "mika.privacy";

fn retention_option(label: &str, value: &str) -> Value {
    json!({ "label": label, "value": value })
}

/// As 5 opções de `conversationRetention`, na ordem do enum — fallback de
/// `capitalize()` explicado no cabeçalho deste arquivo.
fn retention_options() -> Vec<Value> {
    vec![
        retention_option("1day", "1day"),
        retention_option("7days", "7days"),
        retention_option("30days", "30days"),
        retention_option("90days", "90days"),
        retention_option("Forever", "forever"),
    ]
}

/// Os 3 campos do grupo `settings.privacy.group.permissions` (`PrivacySchema`, primeiro
/// `z.object`), na ordem declarada.
fn permissions_fields() -> Vec<Value> {
    vec![
        json!({
            "configPath": "privacy.conversationRetention",
            "type": "select",
            "label": "settings.privacy.field.conversationRetention.label",
            "options": retention_options(),
        }),
        json!({
            "configPath": "privacy.screenCapture",
            "type": "boolean",
            "label": "settings.privacy.field.screenCapture.label",
        }),
        json!({
            "configPath": "privacy.clipboardLogging",
            "type": "boolean",
            "label": "settings.privacy.field.clipboardLogging.label",
        }),
    ]
}

/// O único campo do grupo `settings.privacy.group.sharing` (segundo `z.object`).
fn sharing_fields() -> Vec<Value> {
    vec![json!({
        "configPath": "privacy.offlineMode",
        "type": "boolean",
        "label": "settings.privacy.field.offlineMode.label",
        "description": "settings.privacy.field.offlineMode.description",
    })]
}

/// `PRIVACY_SETTINGS` (`privacy.schema.ts`), sem `pluginId`/`isCore` — o registro injeta os dois.
pub fn declaration() -> Value {
    json!({
        "tabId": "privacy",
        "tabLabel": "settings.privacy.tab.label",
        "tabIcon": "Shield",
        "tabGroup": "settings.privacy.tab.group",
        "tabDescription": "settings.privacy.tab.description",
        "accentColor": "emerald",
        "component": "PrivacyPanel",
        "groups": [
            {
                "title": "settings.privacy.group.permissions",
                "component": "PrivacyPanel",
                "groupPrefix": "privacy",
                "badge": "Local first",
                "columns": 2,
                "fields": permissions_fields(),
            },
            {
                "title": "settings.privacy.group.sharing",
                "component": "PrivacyPanel",
                "groupPrefix": "privacy",
                "badge": "Sharing",
                "columns": 2,
                "fields": sharing_fields(),
            },
        ],
    })
}

/// Registra a aba `privacy` no `FeatureRegistry` compartilhado sob `mika.privacy` — mesmo
/// `pluginId` de `manifest.id`. Chame UMA VEZ no boot.
pub fn register_feature() {
    feature_registry::shared().register(
        PRIVACY_PLUGIN_ID,
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
        assert_eq!(d["tabId"], json!("privacy"));
        assert_eq!(d["accentColor"], json!("emerald"));
        assert!(
            d.get("enabledConfigPath").is_none(),
            "nenhum grupo tem campo 'enabled' — sem enabledConfigPath, igual ao lado TS"
        );
        assert_eq!(d["groups"].as_array().unwrap().len(), 2);
    }

    /// Toda `configPath` é prefixada por `privacy.` — um campo sem o prefixo grava fora do
    /// namespace da feature, silenciosamente.
    #[test]
    fn every_field_config_path_is_under_the_privacy_namespace() {
        for group in declaration()["groups"].as_array().unwrap() {
            for field in group["fields"].as_array().unwrap() {
                let path = field["configPath"].as_str().unwrap();
                assert!(path.starts_with("privacy."), "configPath fora do namespace: {path}");
            }
        }
    }

    #[test]
    fn retention_fields_fall_back_to_capitalized_enum_values_for_options() {
        let d = declaration();
        let field = d["groups"][0]["fields"][0].clone();
        assert_eq!(field["configPath"], json!("privacy.conversationRetention"));
        let options = field["options"].as_array().unwrap();
        assert_eq!(options.len(), 5);
        assert_eq!(options[0], json!({ "label": "1day", "value": "1day" }));
        assert_eq!(options[4], json!({ "label": "Forever", "value": "forever" }));
    }

    #[test]
    fn register_feature_publishes_the_privacy_tab_under_the_plugin_manifest_id() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&PRIVACY_PLUGIN_ID.into())
            .expect("mika.privacy registrado");
        assert!(registered.enabled);
        assert_eq!(registered.settings[0]["tabId"], json!("privacy"));
    }
}
