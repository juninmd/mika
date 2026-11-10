//! Declaração de settings de `mcp` para o `FeatureRegistry` — port de `mcp.schema.ts::MCP_SETTINGS`
//! (`defineFeatureSchema`). Ver o cabeçalho de `../vision/feature.rs` para por que é um literal JSON
//! calculado à mão em vez de reimplementar `zod-inference.ts`.
//!
//! **Só a declaração de settings mora aqui, de propósito.** `register.rs` já cuida do catálogo de
//! TOOLS descobertas (`ToolRegistry` nativo, `MCP_PLUGIN_ID`); este arquivo NUNCA registra tool —
//! reutiliza a mesma constante [`super::register::MCP_PLUGIN_ID`] só para a aba de settings entrar
//! sob o mesmo `pluginId` do plugin TS (`mika/index.ts::manifest.id`).
//!
//! `register_feature()` não recebe `ConfigStore`: a declaração é 100% estática (o toggle real de
//! `mcp.enabled` mora no valor salvo, lido por quem resolver `mika:settings-get-all`, não aqui) —
//! mesma razão de `vision::register_feature`/`llm::register_feature`. `enabled: true` espelha
//! `registerFeatureWithTools`/`definePlugin`, que nunca passam um terceiro argumento pro registro.

use serde_json::{json, Value};

use crate::feature_registry::{self, FeatureRegistration};

use super::register::MCP_PLUGIN_ID;

/// Os 2 campos de `McpSchema` (`mcp.schema.ts`), na ordem declarada.
fn fields() -> Vec<Value> {
    vec![
        json!({
            "configPath": "mcp.enabled",
            "type": "boolean",
            "label": "settings.mcp.field.enabled.label",
            "description": "settings.mcp.field.enabled.description",
        }),
        json!({
            "configPath": "mcp.timeoutSeconds",
            "type": "number",
            "label": "settings.mcp.field.timeoutSeconds.label",
            "description": "settings.mcp.field.timeoutSeconds.description",
            "min": 2,
            "max": 120,
            "inputMode": "decimal",
        }),
    ]
}

/// `MCP_SETTINGS` (`mcp.schema.ts`), sem `pluginId`/`isCore` — o registro injeta os dois.
/// `enabledConfigPath` é inferido de `mcp.enabled` (primeiro grupo com campo `enabled`), mesma
/// regra de `inferEnabledConfigPath` do lado TS.
pub fn declaration() -> Value {
    json!({
        "tabId": "mcp",
        "tabLabel": "settings.mcp.tab.label",
        "tabIcon": "Plug",
        "tabGroup": "settings.mcp.tab.group",
        "tabDescription": "settings.mcp.tab.description",
        "accentColor": "cyan",
        "component": "McpPanel",
        "enabledConfigPath": "mcp.enabled",
        "groups": [{
            "title": "settings.mcp.group.title",
            "component": "McpPanel",
            "groupPrefix": "mcp",
            "columns": 2,
            "fields": fields(),
        }],
    })
}

/// Registra a aba `mcp` no `FeatureRegistry` compartilhado sob `mika.mcp` — mesmo `pluginId` de
/// `manifest.id` em `apps/desktop/src/features/mcp/index.ts`. Chame UMA VEZ no boot.
pub fn register_feature() {
    feature_registry::shared().register(
        MCP_PLUGIN_ID,
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
        assert_eq!(d["tabId"], json!("mcp"));
        assert_eq!(d["enabledConfigPath"], json!("mcp.enabled"));
        assert_eq!(d["accentColor"], json!("cyan"));
        assert_eq!(d["groups"].as_array().unwrap().len(), 1);
        assert_eq!(d["groups"][0]["fields"].as_array().unwrap().len(), 2);
    }

    /// Toda `configPath` é prefixada por `mcp.` — um campo sem o prefixo grava fora do namespace
    /// da feature, silenciosamente.
    #[test]
    fn every_field_config_path_is_under_the_mcp_namespace() {
        for field in declaration()["groups"][0]["fields"].as_array().unwrap() {
            let path = field["configPath"].as_str().unwrap();
            assert!(path.starts_with("mcp."), "configPath fora do namespace: {path}");
        }
    }

    #[test]
    fn register_feature_publishes_the_mcp_tab_under_the_plugin_manifest_id() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&MCP_PLUGIN_ID.into())
            .expect("mika.mcp registrado");
        assert!(registered.enabled);
        assert_eq!(registered.settings[0]["tabId"], json!("mcp"));
    }
}
