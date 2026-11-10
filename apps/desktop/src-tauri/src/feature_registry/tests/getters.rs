use super::super::{FeatureRegistration, FeatureRegistry};
use super::{registration, settings};

/// `getAllSettings()` do lado TS NUNCA olha `enabled` — é a própria tela de Settings quem deixa
/// o usuário reativar a partir dali. Só `getAllViews`/`getAllSettingsPanels` filtram.
#[test]
fn get_all_settings_includes_disabled_features() {
    let registry = FeatureRegistry::new();
    registry.register("mika.voice", registration(false, vec![settings("voice")]));

    let all = registry.get_all_settings();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["tabId"], "voice");
}

/// `pluginId` por padrão, `isCore` inferido do prefixo `"mika."` — só quando o caller não deu o
/// valor. Espelha `settingsDeclaration()` (`packages/sdk/src/features/feature-catalog.ts`).
#[test]
fn get_all_settings_defaults_plugin_id_and_infers_is_core() {
    let registry = FeatureRegistry::new();
    registry.register("mika.voice", registration(true, vec![settings("voice")]));
    registry.register("gamer-companion", registration(true, vec![settings("hud")]));

    let all = registry.get_all_settings();
    let core = all.iter().find(|d| d["tabId"] == "voice").unwrap();
    let late = all.iter().find(|d| d["tabId"] == "hud").unwrap();

    assert_eq!(core["pluginId"], "mika.voice");
    assert_eq!(core["isCore"], true);
    assert_eq!(late["pluginId"], "gamer-companion");
    assert_eq!(late["isCore"], false);
}

/// Um `pluginId` explícito no `Declaration` não é sobrescrito — só o `tabId` ausente cairia no
/// default do registrador.
#[test]
fn get_all_settings_respects_an_explicit_plugin_id() {
    let registry = FeatureRegistry::new();
    let declaration = serde_json::json!({ "tabId": "voice", "pluginId": "mika.voice.sub", "isCore": false });
    registry.register("mika.voice", registration(true, vec![declaration]));

    let all = registry.get_all_settings();
    assert_eq!(all[0]["pluginId"], "mika.voice.sub");
    assert_eq!(all[0]["isCore"], false);
}

/// `tabId` duplicado entre DUAS features: o primeiro registrado vence, o segundo some da lista
/// (com aviso, não pânico).
#[test]
fn get_all_settings_keeps_the_first_registered_tab_id_on_collision() {
    let registry = FeatureRegistry::new();
    registry.register("mika.plugin-a", registration(true, vec![settings("shared")]));
    registry.register("mika.plugin-b", registration(true, vec![settings("shared")]));

    let all = registry.get_all_settings();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["pluginId"], "mika.plugin-a");
}

#[test]
fn get_all_views_and_panels_only_include_enabled_features() {
    let registry = FeatureRegistry::new();
    registry.register(
        "mika.vision",
        FeatureRegistration {
            enabled: true,
            views: vec![serde_json::json!({ "id": "vision-view" })],
            settings_panels: vec![serde_json::json!({ "id": "vision-panel" })],
            ..Default::default()
        },
    );
    registry.register(
        "mika.browser",
        FeatureRegistration {
            enabled: false,
            views: vec![serde_json::json!({ "id": "browser-view" })],
            ..Default::default()
        },
    );

    let catalog = registry.get_ui_catalog();
    assert_eq!(catalog.views.len(), 1);
    assert_eq!(catalog.views[0]["id"], "vision-view");
    assert_eq!(catalog.views[0]["pluginId"], "mika.vision");
    assert_eq!(catalog.settings_panels.len(), 1);
}

/// `viewDeclaration`/`settingsPanelDeclaration`: `pluginId` é sempre sobrescrito pelo
/// registrador, mesmo se o `Declaration` já trouxer um (ao contrário de settings).
#[test]
fn get_all_views_overwrites_a_plugin_id_already_present_in_the_declaration() {
    let registry = FeatureRegistry::new();
    registry.register(
        "mika.vision",
        FeatureRegistration {
            enabled: true,
            views: vec![serde_json::json!({ "id": "v", "pluginId": "someone-else" })],
            ..Default::default()
        },
    );

    assert_eq!(registry.get_all_views()[0]["pluginId"], "mika.vision");
}

/// `getUiCatalog()` devolve `{ views, settingsPanels }` em `camelCase` — a forma que
/// `mika:plugin-ui-catalog` já devolve hoje pelo sidecar.
#[test]
fn ui_catalog_serializes_with_camel_case_keys() {
    let registry = FeatureRegistry::new();
    let json = serde_json::to_value(registry.get_ui_catalog()).unwrap();

    assert!(json.get("views").is_some());
    assert!(json.get("settingsPanels").is_some());
}

/// `getAllSettings()` itera um `Map` do lado TS, que preserva ORDEM DE INSERÇÃO. Um `HashMap`
/// Rust não garantiria isso entre rodadas — é por isso que o registro guarda um `Vec`.
#[test]
fn get_all_settings_preserves_registration_order() {
    let registry = FeatureRegistry::new();
    for tab in ["a", "b", "c", "d", "e"] {
        registry.register(format!("plugin-{tab}"), registration(true, vec![settings(tab)]));
    }

    let order: Vec<String> = registry
        .get_all_settings()
        .iter()
        .map(|d| d["tabId"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(order, vec!["a", "b", "c", "d", "e"]);
}
