//! Suíte de `FeatureRegistry`, dividida por área — um arquivo só passaria do teto de 300 linhas
//! muito antes de cobrir merge, dedupe e leitura.

mod concurrency;
mod getters;
mod register;
mod shared;

use super::{Declaration, FeatureRegistration};

/// Uma `FeatureSettingsDeclaration` mínima, só com o que os testes de merge/dedupe precisam
/// (`tabId`). Os outros campos (`groups`, `tabLabel`, ...) são texto de UI que o registro nunca
/// inspeciona — adicioná-los aqui só encobriria o que cada teste realmente verifica.
pub(super) fn settings(tab_id: &str) -> Declaration {
    serde_json::json!({ "tabId": tab_id })
}

pub(super) fn registration(enabled: bool, settings: Vec<Declaration>) -> FeatureRegistration {
    FeatureRegistration {
        enabled,
        settings,
        ..Default::default()
    }
}
