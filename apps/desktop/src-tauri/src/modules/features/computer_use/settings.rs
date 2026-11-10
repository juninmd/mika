//! Settings tab for `computer-use.enabled` — one boolean, one card, same minimal shape as
//! `features::mood`'s `mood_settings`/`register` (`persistence.rs`): the declaration is derived
//! from the config schema itself (`settings_declaration::declare_namespace`) instead of a
//! hand-duplicated field list, so the tab can never drift from what `ConfigStore` actually
//! validates.

use std::sync::Arc;

use serde_json::Value;

use crate::config::store::ConfigStore;

pub(crate) const PLUGIN_ID: &str = "mika.computer-use";

fn declare_settings(schema: &crate::config::registry::SchemaRegistry) -> Option<Value> {
    let mut settings = super::super::settings_declaration::declare_namespace(
        schema,
        "computer-use",
        "computer-use",
        "settings.computerUse.tab.label",
        "MousePointerClick",
        "settings.computerUse.tab.label",
    )?;
    settings["groups"][0]["component"] = Value::from("ComputerUsePanel");
    settings["tabDescription"] = Value::from("settings.computerUse.panel.description");
    settings["enabledConfigPath"] = Value::from("computer-use.enabled");
    Some(settings)
}

/// Registers the `computer` tool (destructive — see `tool.rs`) and the settings tab in one call,
/// the same shape as `features::mood::persistence::register`. Called once, from
/// `modules::register_config_bound_tools`.
pub(crate) fn register(store: Arc<ConfigStore>) {
    super::tool::register(Arc::clone(&store));

    if let Some(settings) = declare_settings(store.schema()) {
        crate::feature_registry::shared().register(
            PLUGIN_ID,
            crate::feature_registry::FeatureRegistration { enabled: true, settings: vec![settings], ..Default::default() },
        );
    } else {
        log::warn!("computer_use: namespace 'computer-use' ausente do SchemaRegistry — settings não declaradas");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_renders_a_panel_the_renderer_registers() {
        let registry = include_str!("../../../../../src/renderer/src/components/settings/feature-components.tsx");
        assert!(registry.contains("ComputerUsePanel: (props)"));
        let settings = declare_settings(&crate::config::schemas::registry()).expect("computer-use namespace declared");
        assert_eq!(settings["groups"][0]["component"], "ComputerUsePanel");
        assert_eq!(settings["tabId"], "computer-use");
    }

    // Without these the UI guessed `<tabId>.enabled` and showed a generic description.
    #[test]
    fn the_tab_declares_its_master_switch_and_a_description_the_locales_carry() {
        let settings = declare_settings(&crate::config::schemas::registry()).expect("computer-use namespace declared");
        assert_eq!(settings["enabledConfigPath"], "computer-use.enabled");
        assert!(crate::config::schemas::registry().validate("computer-use.enabled", &serde_json::json!(false)).is_ok());
        let description = settings["tabDescription"].as_str().expect("tabDescription");
        assert_ne!(crate::modules::settings::t(description, "pt-BR"), description, "{description} missing from pt-BR");
    }

    #[test]
    fn register_publishes_both_the_tool_and_the_tab() {
        let dir = std::env::temp_dir().join(format!("mika-computer-use-settings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Arc::new(ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap());

        register(Arc::clone(&store));

        assert!(crate::modules::tools::registry().get("computer").is_some());
        let registered = crate::feature_registry::shared().get(&PLUGIN_ID.into()).expect("mika.computer-use registrado");
        assert_eq!(registered.settings[0]["tabId"], serde_json::json!("computer-use"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
