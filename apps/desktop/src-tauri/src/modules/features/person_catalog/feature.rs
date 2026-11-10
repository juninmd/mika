//! Declaração de settings de `person-catalog` para o `FeatureRegistry` — port de
//! `person-catalog.schema.ts::PERSON_CATALOG_SETTINGS` (`defineFeatureSchema`), não do modelo de
//! dados ([`super::types`]) nem da redação de PII ([`super::pii`]) do resto deste diretório. Ver o
//! cabeçalho de `../../vision/feature.rs` para por que é um literal JSON calculado à mão em vez de
//! reimplementar `zod-inference.ts`.
//!
//! **`enabled`/`autoExtractOnChat` usam `.describe(string)` puro no TypeScript, não
//! `m({...})`.** `parseDescribeMeta` (`zod-inference.ts`) trata uma string que não começa com `{`
//! como `label` E `description` ao mesmo tempo — os dois campos abaixo repetem a MESMA chave i18n
//! duas vezes de propósito, não é duplicação por engano (mesmo caso de
//! `misc.rs::person_catalog` no lado de config).
//!
//! `register_feature()` não recebe `ConfigStore`: mesma razão de `vision::register_feature` — é
//! dado estático, e `enabled: true` espelha `definePlugin`/`registerFeatureWithTools`, que nunca
//! passam um terceiro argumento ao registro.

use serde_json::{json, Value};

use crate::feature_registry::{self, FeatureRegistration};

/// Mesmo `manifest.id` de `apps/desktop/src/features/person-catalog/index.ts`.
pub const PERSON_CATALOG_PLUGIN_ID: &str = "mika.person-catalog";

/// Os campos de `PersonCatalogSchema` (`person-catalog.schema.ts`) mais `webEnrichment` (opt-in
/// do enriquecimento pela web, só existe no Rust).
fn fields() -> Vec<Value> {
    vec![
        json!({
            "configPath": "person-catalog.enabled",
            "type": "boolean",
            "label": "settings.personCatalog.field.enabled",
            "description": "settings.personCatalog.field.enabled",
        }),
        json!({
            "configPath": "person-catalog.autoExtractOnChat",
            "type": "boolean",
            "label": "settings.personCatalog.field.autoExtractOnChat",
            "description": "settings.personCatalog.field.autoExtractOnChat",
        }),
        json!({
            "configPath": "person-catalog.webEnrichment",
            "type": "boolean",
            "label": "settings.personCatalog.field.webEnrichment.label",
            "description": "settings.personCatalog.field.webEnrichment.description",
        }),
        json!({
            "configPath": "person-catalog.maxMentionsPerPerson",
            "type": "number",
            "label": "settings.personCatalog.field.maxMentionsPerPerson.label",
            "helperText": "settings.personCatalog.field.maxMentionsPerPerson.helperText",
            "min": 1,
            "max": 500,
            "inputMode": "decimal",
        }),
    ]
}

/// `PERSON_CATALOG_SETTINGS` (`person-catalog.schema.ts`), sem `pluginId`/`isCore` — o registro
/// injeta os dois. `enabledConfigPath` é inferido de `person-catalog.enabled` (primeiro grupo com
/// campo `enabled`), mesma regra de `inferEnabledConfigPath` do lado TS.
pub fn declaration() -> Value {
    json!({
        "tabId": "person-catalog",
        "tabLabel": "settings.personCatalog.tab.label",
        "tabIcon": "Users",
        "tabGroup": "settings.personCatalog.tab.group",
        "tabDescription": "settings.personCatalog.tab.description",
        "heroNote": "settings.personCatalog.tab.heroNote",
        "accentColor": "rose",
        "component": "PersonCatalogPanel",
        "enabledConfigPath": "person-catalog.enabled",
        "groups": [{
            // O TS reusa a chave `tab.label` como `title` do único grupo (não `group.title`) —
            // copiado literal, não é engano de porte.
            "title": "settings.personCatalog.tab.label",
            "component": "PersonCatalogPanel",
            "groupPrefix": "person-catalog",
            "badge": "Social Graph",
            "columns": 2,
            "fields": fields(),
        }],
    })
}

/// A única `PluginViewDeclaration` de `PersonCatalogFeature.views` (`person-catalog.feature.ts`),
/// sem `pluginId` — o registro injeta. Título/descrição são texto pt-BR literal no TS, copiados
/// como estão — mesma nota de `gamer::feature::views`.
fn views() -> Vec<Value> {
    vec![json!({
        "id": "person-catalog.listing",
        "title": "Catálogo de Pessoas",
        "component": "catalogListing",
        "description": "Listagem completa de todas as pessoas catalogadas com relações, atributos e avistamentos.",
        "defaultWindow": { "width": 1200, "height": 800, "minWidth": 900, "minHeight": 600 },
    })]
}

/// A única `SettingsPanelDeclaration` de `PersonCatalogFeature.settingsPanels`, sem `pluginId` — o
/// registro injeta.
fn settings_panels() -> Vec<Value> {
    vec![json!({
        "id": "person-catalog.crud",
        "title": "Pessoas",
        "component": "catalogCRUD",
        "placement": "after-schema",
    })]
}

/// Registra a aba `person-catalog` no `FeatureRegistry` compartilhado sob `mika.person-catalog` —
/// mesmo `pluginId` de `manifest.id`. Chame UMA VEZ no boot.
pub fn register_feature() {
    feature_registry::shared().register(
        PERSON_CATALOG_PLUGIN_ID,
        FeatureRegistration {
            enabled: true,
            settings: vec![declaration()],
            views: views(),
            settings_panels: settings_panels(),
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
        assert_eq!(d["tabId"], json!("person-catalog"));
        assert_eq!(d["enabledConfigPath"], json!("person-catalog.enabled"));
        assert_eq!(d["accentColor"], json!("rose"));
        assert_eq!(d["groups"].as_array().unwrap().len(), 1);
        assert_eq!(d["groups"][0]["fields"].as_array().unwrap().len(), 4);
    }

    /// Toda `configPath` é prefixada por `person-catalog.` — um campo sem o prefixo grava fora do
    /// namespace da feature, silenciosamente.
    #[test]
    fn every_field_config_path_is_under_the_person_catalog_namespace() {
        for field in declaration()["groups"][0]["fields"].as_array().unwrap() {
            let path = field["configPath"].as_str().unwrap();
            assert!(path.starts_with("person-catalog."), "configPath fora do namespace: {path}");
        }
    }

    #[test]
    fn register_feature_publishes_the_person_catalog_tab_under_the_plugin_manifest_id() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&PERSON_CATALOG_PLUGIN_ID.into())
            .expect("mika.person-catalog registrado");
        assert!(registered.enabled);
        assert_eq!(registered.settings[0]["tabId"], json!("person-catalog"));
    }

    /// `mika:plugin-ui-catalog` lê `views`/`settings_panels`, não `settings` — sem esta asserção a
    /// view de listagem e o painel CRUD somem do catálogo em silêncio (nenhum dos dois tem `tabId`,
    /// então a dedupe de `get_all_settings` nunca acusaria a ausência).
    #[test]
    fn register_feature_publishes_the_view_and_the_settings_panel_too() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&PERSON_CATALOG_PLUGIN_ID.into())
            .expect("mika.person-catalog registrado");
        assert_eq!(registered.views.len(), 1);
        assert_eq!(registered.views[0]["id"], json!("person-catalog.listing"));
        assert_eq!(registered.settings_panels.len(), 1);
        assert_eq!(registered.settings_panels[0]["id"], json!("person-catalog.crud"));
        assert_eq!(registered.settings_panels[0]["placement"], json!("after-schema"));
    }

    /// `person-catalog.feature.ts` foi removido (backend Node deletado na migração para Tauri); o
    /// pin de forma (`views`/`settingsPanels`) já é coberto por [`register_feature_publishes_the_view_and_the_settings_panel_too`]
    /// acima, que assere os mesmos ids/placement direto sobre a declaração registrada.
    #[test]
    fn views_and_settings_panels_match_the_expected_shape() {
        assert_eq!(1, views().len());
        assert_eq!(1, settings_panels().len());
    }
}
