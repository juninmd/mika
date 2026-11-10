//! Declaração de settings de `scheduler` para o `FeatureRegistry` — port de
//! `scheduler.schema.ts::SCHEDULER_SETTINGS` (`defineFeatureSchema`, outrora `packages/scheduler/
//! src` — pacote deletado em 2026-09-22, backend Node morto sem chamador; a lógica de lembretes
//! ([`super::store`]/[`super::tool`]) já era só Rust). Ver o
//! cabeçalho de `../../vision/feature.rs` para por que é um literal JSON calculado à mão em vez de
//! reimplementar `zod-inference.ts`.
//!
//! **O namespace de config `scheduler` já existe** (`config/schemas/misc.rs::scheduler`, campo
//! `enabled`) — esta declaração é só a aba de settings (UI) que lê/escreve nesse namespace; não
//! duplica nem declara namespace novo. O cabeçalho de `../mod.rs` ("nenhum namespace `scheduler`
//! existe em `config/schemas/`") ficou desatualizado depois que aquele namespace entrou em
//! `misc.rs` — divergência reportada, não corrigida aqui (fora do escopo desta declaração).
//!
//! **`pluginId` é `mika.scheduler`, o `manifest.id` de
//! `apps/desktop/src/features/scheduler/index.ts`** — o pacote `@mika/scheduler`
//! (`scheduler.feature.ts`/`scheduler.module.ts`) usa `id: 'scheduler'` para o próprio
//! `FeatureDefinition`/`ModuleManifest`, mas quem registra no `FeatureRegistry` de verdade é o
//! plugin do app (`definePlugin({ manifest, feature: SchedulerFeature, ... })`), sob o
//! `manifest.id` prefixado — mesma convenção de `mcp::MCP_PLUGIN_ID`/`privacy::PRIVACY_PLUGIN_ID`.
//!
//! `register_feature()` não recebe `ConfigStore`: mesma razão de `vision::register_feature` — é
//! dado estático, e `enabled: true` espelha `definePlugin`/`registerFeatureWithTools`, que nunca
//! passam um terceiro argumento ao registro.

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::feature_registry::{self, FeatureRegistration};

/// Mesmo `manifest.id` de `apps/desktop/src/features/scheduler/index.ts`.
pub const SCHEDULER_PLUGIN_ID: &str = "mika.scheduler";

/// Toggle mestre da feature (`scheduler.enabled`), lido do `ConfigStore` REAL a cada chamada —
/// nunca cacheado, mesma técnica de `person_catalog::tool::catalog_enabled`. Antes da varredura de
/// controles de UI de 2026-09-27 o campo era desenhado (`SettingsTabHero`) mas nada no backend o
/// lia: [`super::firing`] disparava lembretes e [`super::tool`] agendava novos com a aba desligada.
pub fn master_enabled(store: &ConfigStore) -> bool {
    store.get("scheduler.enabled").and_then(|v| v.as_bool()) != Some(false)
}

/// Resposta padrão de tool quando `scheduler.enabled=false` — mesma forma de
/// `person_catalog::tool::catalog_disabled`/`gamer::feature::master_disabled_response`.
pub fn master_disabled_response() -> Value {
    json!({
        "success": false,
        "error": "The scheduler feature is disabled (scheduler.enabled=false); no action was taken.",
        "errorCode": "denied",
    })
}

/// O único campo de `SchedulerSchema` (`scheduler.schema.ts`).
fn fields() -> Vec<Value> {
    vec![json!({
        "configPath": "scheduler.enabled",
        "type": "boolean",
        "label": "settings.scheduler.field.enabled",
        "description": "settings.scheduler.field.enabled",
    })]
}

/// `SCHEDULER_SETTINGS` (`scheduler.schema.ts`), sem `pluginId`/`isCore` — o registro injeta os
/// dois. `enabledConfigPath` é inferido de `scheduler.enabled` (único grupo, com campo `enabled`),
/// mesma regra de `inferEnabledConfigPath` do lado TS.
pub fn declaration() -> Value {
    json!({
        "tabId": "scheduler",
        "tabLabel": "settings.scheduler.tab.label",
        "tabIcon": "AlarmClock",
        "tabGroup": "settings.scheduler.tab.group",
        "tabDescription": "settings.scheduler.tab.description",
        "accentColor": "violet",
        "component": "SchedulerPanel",
        "enabledConfigPath": "scheduler.enabled",
        "groups": [{
            // O TS reusa a chave `tab.label` como `title` do único grupo (não `group.title`) —
            // copiado literal, não é engano de porte.
            "title": "settings.scheduler.tab.label",
            "component": "SchedulerPanel",
            "groupPrefix": "scheduler",
            "columns": 2,
            "fields": fields(),
        }],
    })
}

/// Registra a aba `scheduler` no `FeatureRegistry` compartilhado sob `mika.scheduler` — mesmo
/// `pluginId` de `manifest.id`. Chame UMA VEZ no boot.
pub fn register_feature() {
    feature_registry::shared().register(
        SCHEDULER_PLUGIN_ID,
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
        assert_eq!(d["tabId"], json!("scheduler"));
        assert_eq!(d["enabledConfigPath"], json!("scheduler.enabled"));
        assert_eq!(d["accentColor"], json!("violet"));
        assert_eq!(d["groups"].as_array().unwrap().len(), 1);
        assert_eq!(d["groups"][0]["fields"].as_array().unwrap().len(), 1);
    }

    /// Toda `configPath` é prefixada por `scheduler.` — um campo sem o prefixo grava fora do
    /// namespace da feature, silenciosamente.
    #[test]
    fn every_field_config_path_is_under_the_scheduler_namespace() {
        for field in declaration()["groups"][0]["fields"].as_array().unwrap() {
            let path = field["configPath"].as_str().unwrap();
            assert!(path.starts_with("scheduler."), "configPath fora do namespace: {path}");
        }
    }

    // `field_count_and_paths_match_the_ts_schema` (pin contra `packages/scheduler/src/
    // scheduler.schema.ts`) foi removido em 2026-09-22: `packages/scheduler` era backend Node
    // morto (zero import fora do próprio pacote, `@mika/scheduler` nem listado nas deps do app) —
    // deletado no mesmo commit do audit de `check:events`. Sem TS para divergir, este pin não tinha
    // mais o que provar; a fonte da verdade do campo `enabled` agora é só este arquivo.

    fn scratch_dir(label: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("mika-scheduler-feature-test-{label}-{}-{unique}", std::process::id()))
    }

    #[test]
    fn master_enabled_defaults_to_true_when_config_is_absent() {
        let store = ConfigStore::open(&scratch_dir("default"), crate::config::registry::SchemaRegistry::new(vec![])).unwrap();
        assert!(master_enabled(&store));
    }

    #[test]
    fn master_enabled_is_false_only_when_explicitly_disabled() {
        let store = ConfigStore::open(&scratch_dir("explicit"), crate::config::schemas::registry()).unwrap();
        assert!(master_enabled(&store));
        store.set("scheduler.enabled", json!(false)).unwrap();
        assert!(!master_enabled(&store));
    }

    #[test]
    fn register_feature_publishes_the_scheduler_tab_under_the_plugin_manifest_id() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&SCHEDULER_PLUGIN_ID.into())
            .expect("mika.scheduler registrado");
        assert!(registered.enabled);
        assert_eq!(registered.settings[0]["tabId"], json!("scheduler"));
    }
}
