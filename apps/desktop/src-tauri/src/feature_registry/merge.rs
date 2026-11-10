//! Funções puras que reproduzem o que `FeatureRegistryService` faz ao registrar e ao ler —
//! separadas de `mod.rs` para que a lógica de merge/normalização tenha teste próprio, sem
//! precisar montar um `FeatureRegistry` inteiro para cada caso de borda.

use std::collections::HashSet;

use serde_json::Value;

use super::types::{Declaration, PluginId, RegisteredFeature};

/// `d.tabId` do lado TS. `None` quando o `Declaration` não é um objeto ou não tem `tabId` — o
/// contrato exige o campo, mas o registro não confia num invariante que não pode checar (o valor
/// chega como JSON solto), então trata ausência como "nunca colide com nada" em vez de entrar em
/// pânico.
pub(super) fn tab_id_of(declaration: &Declaration) -> Option<&str> {
    declaration.get("tabId").and_then(Value::as_str)
}

/// `mergedSettings` de `feature-registry.ts::register`: começa do que já existia e só acrescenta
/// o que chega com um `tabId` novo. Um `tabId` repetido perde — o registro ANTERIOR fica, igual
/// ao `.some(...)` do lado TS, que testa contra o que já foi empurrado, não contra o que chega.
pub(super) fn merge_settings(previous: &[Declaration], incoming: &[Declaration]) -> Vec<Declaration> {
    let mut merged = previous.to_vec();

    for setting in incoming {
        let already_present = match tab_id_of(setting) {
            Some(tab_id) => merged.iter().any(|existing| tab_id_of(existing) == Some(tab_id)),
            // Sem tabId não há como saber se colide — o TS teria o mesmo problema com um
            // `FeatureSettingsDeclaration` fora do contrato; aqui o registro simplesmente aceita.
            None => false,
        };

        if !already_present {
            merged.push(setting.clone());
        }
    }

    merged
}

/// `removeDisabledDuplicates` de `feature-registry.ts`: quando uma feature HABILITADA reivindica
/// um `tabId` que uma entrada DESABILITADA do MESMO plugin já ocupava, a desabilitada sai. Sem
/// isso, alternar uma feature entre versões (ou reativar depois de um registro parcial) deixaria
/// o painel antigo morto no registro, competindo por aba com o novo.
pub(super) fn remove_disabled_duplicates(
    entries: &mut Vec<RegisteredFeature>,
    plugin_id: &PluginId,
    incoming_feature_id: &super::types::FeatureId,
    incoming_settings: &[Declaration],
) {
    let incoming_tabs: HashSet<&str> = incoming_settings.iter().filter_map(tab_id_of).collect();

    entries.retain(|entry| {
        let same_feature = &entry.feature_id == incoming_feature_id;
        let other_plugin = &entry.plugin_id != plugin_id;
        if same_feature || other_plugin || entry.enabled {
            return true;
        }

        !entry.settings.iter().filter_map(tab_id_of).any(|tab| incoming_tabs.contains(tab))
    });
}

/// `settingsDeclaration(pluginId, declaration)` do lado TS, na parte que este registro pode
/// reproduzir sobre um `Value` solto: `pluginId` por padrão (nunca sobrescreve um explícito) e
/// `isCore` inferido de `pluginId` começar com `"mika."` quando o caller não decidiu.
/// **i18n fica de fora de propósito** — ver o cabeçalho de `mod.rs`.
pub(super) fn normalize_settings_declaration(plugin_id: &PluginId, declaration: &Declaration) -> Declaration {
    let mut normalized = declaration.clone();
    let Some(obj) = normalized.as_object_mut() else {
        return normalized;
    };

    let resolved_plugin_id = obj
        .get("pluginId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| plugin_id.0.clone());

    if obj.get("isCore").is_none_or(Value::is_null) {
        let is_core = resolved_plugin_id.starts_with("mika.") || plugin_id.0.starts_with("mika.");
        obj.insert("isCore".to_string(), Value::Bool(is_core));
    }
    obj.insert("pluginId".to_string(), Value::String(resolved_plugin_id));

    normalized
}

/// `viewDeclaration`/`settingsPanelDeclaration` do lado TS: `pluginId` é sempre o de quem
/// registrou, sem checar se o `Declaration` já trazia um (ao contrário dos settings, que
/// respeitam um `pluginId` explícito — ver `normalize_settings_declaration`).
pub(super) fn inject_plugin_id(plugin_id: &PluginId, declaration: &Declaration) -> Declaration {
    let mut injected = declaration.clone();
    if let Some(obj) = injected.as_object_mut() {
        obj.insert("pluginId".to_string(), Value::String(plugin_id.0.clone()));
    }
    injected
}
