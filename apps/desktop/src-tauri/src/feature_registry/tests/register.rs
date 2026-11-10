use super::super::{FeatureId, FeatureRegistration, FeatureRegistry};
use super::{registration, settings};

#[test]
fn feature_id_defaults_to_plugin_id() {
    let registry = FeatureRegistry::new();
    registry.register("mika.voice", registration(true, vec![settings("voice")]));

    assert!(registry.get(&FeatureId::from("mika.voice")).is_some());
}

#[test]
fn explicit_id_is_used_over_plugin_id() {
    let registry = FeatureRegistry::new();
    registry.register(
        "gamer-companion",
        FeatureRegistration {
            id: Some(FeatureId::from("gamer-companion.hud")),
            enabled: true,
            settings: vec![settings("hud")],
            ..Default::default()
        },
    );

    assert!(registry.get(&FeatureId::from("gamer-companion.hud")).is_some());
    assert!(registry.get(&FeatureId::from("gamer-companion")).is_none());
}

/// `mergedSettings` do lado TS: o `tabId` que já existia vence, o registro mais recente só
/// ACRESCENTA abas novas.
#[test]
fn re_registering_merges_settings_by_tab_id_first_wins() {
    let registry = FeatureRegistry::new();
    let id = FeatureId::from("mika.voice");

    registry.register("mika.voice", registration(true, vec![settings("voice")]));
    registry.register(
        "mika.voice",
        FeatureRegistration {
            id: Some(id.clone()),
            enabled: true,
            settings: vec![settings("voice"), settings("stt")],
            ..Default::default()
        },
    );

    let tabs: Vec<String> = registry
        .get(&id)
        .unwrap()
        .settings
        .iter()
        .map(|s| s["tabId"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(tabs, vec!["voice", "stt"]);
}

/// `enabled = enabled || previous.enabled`: uma vez habilitada, um registro seguinte com
/// `enabled: false` não desliga a feature.
#[test]
fn re_registering_disabled_does_not_turn_off_a_previously_enabled_feature() {
    let registry = FeatureRegistry::new();
    let id = FeatureId::from("mika.voice");

    registry.register("mika.voice", registration(true, vec![settings("voice")]));
    registry.register(
        "mika.voice",
        FeatureRegistration {
            id: Some(id.clone()),
            enabled: false,
            settings: vec![],
            ..Default::default()
        },
    );

    assert!(registry.get(&id).unwrap().enabled);
}

/// `{...feature, tools: mergedTools, settings: mergedSettings}`: só `tools` e `settings` entram
/// na mescla. `views`/`settingsPanels` vêm inteiros da chamada mais recente.
#[test]
fn re_registering_replaces_views_and_panels_instead_of_merging() {
    let registry = FeatureRegistry::new();
    let id = FeatureId::from("mika.voice");
    let first_view = serde_json::json!({ "id": "voice-hud" });
    let second_view = serde_json::json!({ "id": "voice-hud-v2" });

    registry.register(
        "mika.voice",
        FeatureRegistration {
            enabled: true,
            views: vec![first_view],
            ..Default::default()
        },
    );
    registry.register(
        "mika.voice",
        FeatureRegistration {
            id: Some(id.clone()),
            enabled: true,
            views: vec![second_view],
            ..Default::default()
        },
    );

    let views = registry.get(&id).unwrap().views;
    assert_eq!(views.len(), 1);
    assert_eq!(views[0]["id"], "voice-hud-v2");
}

/// `removeDisabledDuplicates`: reativar uma feature que reivindica um `tabId` já ocupado por uma
/// entrada DESABILITADA do mesmo plugin apaga a entrada velha, em vez de deixar as duas
/// competindo pela mesma aba.
#[test]
fn enabling_a_tab_removes_the_disabled_entry_that_held_it() {
    let registry = FeatureRegistry::new();

    registry.register(
        "mika.gamer-companion",
        FeatureRegistration {
            id: Some(FeatureId::from("gamer-companion.old")),
            enabled: false,
            settings: vec![settings("hud")],
            ..Default::default()
        },
    );
    registry.register(
        "mika.gamer-companion",
        FeatureRegistration {
            id: Some(FeatureId::from("gamer-companion.new")),
            enabled: true,
            settings: vec![settings("hud")],
            ..Default::default()
        },
    );

    assert!(registry.get(&FeatureId::from("gamer-companion.old")).is_none());
    assert!(registry.get(&FeatureId::from("gamer-companion.new")).is_some());
}

/// Uma entrada desabilitada de OUTRO plugin com o mesmo `tabId` não é tocada — a dedupe é
/// escopada por plugin, igual ao `registered.pluginId !== pluginId` do lado TS.
#[test]
fn disabled_entry_from_another_plugin_survives() {
    let registry = FeatureRegistry::new();

    registry.register(
        "mika.plugin-a",
        FeatureRegistration {
            id: Some(FeatureId::from("plugin-a.panel")),
            enabled: false,
            settings: vec![settings("shared-tab")],
            ..Default::default()
        },
    );
    registry.register(
        "mika.plugin-b",
        FeatureRegistration {
            id: Some(FeatureId::from("plugin-b.panel")),
            enabled: true,
            settings: vec![settings("shared-tab")],
            ..Default::default()
        },
    );

    assert!(registry.get(&FeatureId::from("plugin-a.panel")).is_some());
}
