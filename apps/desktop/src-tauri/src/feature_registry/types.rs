//! Os tipos de dados do registro. Separado de `mod.rs` para caber no teto de 300 linhas por
//! arquivo sem espremer a documentação de uso, que é o que faz este módulo utilizável por quem só
//! leu este arquivo uma vez.

use std::fmt;

use serde::Serialize;

/// Um valor JSON exatamente na forma que o contrato TS já define
/// (`FeatureSettingsDeclaration` / `PluginViewDeclaration` / `SettingsPanelDeclaration` em
/// `packages/sdk/src/contracts`). O registro NÃO duplica esse contrato em structs Rust: ele
/// guarda e reserve o JSON como o caller montou, do mesmo jeito que `ConfigStore` guarda config
/// como `serde_json::Value` sem um tipo Rust por namespace. Duplicar o contrato aqui criaria uma
/// segunda fonte da verdade que nenhum teste TS consegue travar — o de lá já muda por Zod.
pub type Declaration = serde_json::Value;

/// Identificador do plugin que registrou a feature (`pluginId` do lado TS).
///
/// Newtype em vez de `String` solta: o registro tem DOIS identificadores parecidos
/// (`PluginId`, `FeatureId`) e uma chamada com os dois trocados de posição compila sem aviso se
/// ambos forem `String`. O compilador recusa a troca com os tipos separados.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(pub String);

/// Identificador da feature dentro do registro (`feature.id ?? pluginId` do lado TS). Ver
/// `PluginId` para o porquê de não ser `String`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FeatureId(pub String);

macro_rules! impl_id_conversions {
    ($ty:ident) => {
        impl From<&str> for $ty {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl From<String> for $ty {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

impl_id_conversions!(PluginId);
impl_id_conversions!(FeatureId);

/// O que um módulo passa para `FeatureRegistry::register`.
///
/// Espelha `FeatureDefinition` (`packages/sdk/src/features/define-feature.ts`), exceto por
/// `tools`: quem executa tools é um registro separado (`ToolRegistryService` do lado TS, o
/// equivalente Rust é outra frente — ver o cabeçalho de `mod.rs`). Nenhum dos dois canais que
/// este módulo desbloqueia (`mika:settings-declarations`, `mika:plugin-ui-catalog`) lê `tools`.
#[derive(Debug, Clone, Default)]
pub struct FeatureRegistration {
    /// `None` usa o `pluginId` do registro como id da feature — mesmo default de
    /// `feature.id ?? pluginId` em `feature-registry.ts`.
    pub id: Option<FeatureId>,
    pub enabled: bool,
    /// Um `Declaration` por aba de settings (`FeatureSettingsDeclaration`, uma por `tabId`).
    pub settings: Vec<Declaration>,
    /// `PluginViewDeclaration` sem o campo `pluginId` — o registro injeta.
    pub views: Vec<Declaration>,
    /// `SettingsPanelDeclaration` sem o campo `pluginId` — o registro injeta.
    pub settings_panels: Vec<Declaration>,
}

/// O que o registro guarda por feature, e o que `get`/`get_all` devolvem.
#[derive(Debug, Clone)]
pub struct RegisteredFeature {
    pub feature_id: FeatureId,
    pub plugin_id: PluginId,
    pub enabled: bool,
    pub settings: Vec<Declaration>,
    pub views: Vec<Declaration>,
    pub settings_panels: Vec<Declaration>,
}

/// Espelha `PluginUiCatalog` (`packages/sdk/src/contracts`) campo a campo, inclusive o
/// `camelCase` de `settingsPanels` — é a forma que `mika:plugin-ui-catalog` devolve hoje pelo
/// sidecar, e o contrato IPC não muda com a migração.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiCatalog {
    pub views: Vec<Declaration>,
    pub settings_panels: Vec<Declaration>,
}
