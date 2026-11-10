//! Declaração de settings `mika.browser` no `FeatureRegistry` — porte de `BROWSER_SETTINGS`
//! (`apps/desktop/src/main/services/browser/browser.schema.ts`), byte a byte no que o registro
//! nativo consegue representar (ver a nota de i18n em `feature_registry/mod.rs`: os textos ficam
//! como chave crua, a localização é de quem for ligar `mika:settings-declarations`).
//!
//! `BrowserFeature.tools` é `{}` no TS — nenhuma tool a registrar no `ToolRegistry` aqui, só a aba
//! de Settings. `enabledConfigPath` sai como `"browser.enabled"` porque o grupo único tem um campo
//! `enabled` (mesma inferência de `inferEnabledConfigPath` em `define-feature-schema.ts`).

use serde_json::{json, Value};

use crate::feature_registry::FeatureRegistration;

fn declaration() -> Value {
    json!({
        "tabId": "browser",
        "tabLabel": "settings.browser.tab.label",
        "tabIcon": "Globe",
        "tabGroup": "settings.browser.tab.group",
        "tabDescription": "settings.browser.tab.description",
        "accentColor": "blue",
        "enabledConfigPath": "browser.enabled",
        "groups": [{
            "title": "settings.browser.group.title",
            "component": "BrowserPanel",
            "columns": 2,
            "groupPrefix": "browser",
            "fields": [
                {
                    "configPath": "browser.enabled",
                    "type": "boolean",
                    "label": "settings.browser.field.enabled.label",
                    "description": "settings.browser.field.enabled.description",
                },
                {
                    "configPath": "browser.searchEngine",
                    "type": "select",
                    "label": "settings.browser.field.searchEngine.label",
                    "options": [
                        { "label": "settings.browser.field.searchEngine.option.auto", "value": "auto" },
                        { "label": "settings.browser.field.searchEngine.option.bing", "value": "bing" },
                        { "label": "settings.browser.field.searchEngine.option.duckduckgo", "value": "duckduckgo" },
                        { "label": "settings.browser.field.searchEngine.option.yahoo", "value": "yahoo" },
                    ],
                },
            ],
        }],
    })
}

/// Registra a aba `browser` uma vez — chame de `modules()` (efeito colateral, sem `ConfigStore`
/// nenhum, mesmo padrão de `gamer::register_tools`).
pub(super) fn register() {
    crate::feature_registry::shared().register(
        "mika.browser",
        FeatureRegistration { enabled: true, settings: vec![declaration()], ..Default::default() },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_publishes_the_browser_tab() {
        register();
        let all = crate::feature_registry::shared().get_all_settings();
        let tab = all.iter().find(|d| d["tabId"] == json!("browser")).expect("aba browser registrada");
        assert_eq!(tab["pluginId"], json!("mika.browser"));
        assert_eq!(tab["enabledConfigPath"], json!("browser.enabled"));
    }
}
