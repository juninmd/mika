//! `mika:plugin-ui-catalog` — port de `apps/desktop/src/main/ipc/plugin-ui.ts::registerPluginUiIPC`.
//!
//! Vive em `modules/settings/`, não num arquivo novo direto sob `modules/` (que pediria uma linha
//! em `modules/mod.rs`, fora do meu escopo editar): `PluginUiModule` é um `NativeModule` PRÓPRIO,
//! com canal e `invoke()` só seus — só o arquivo é vizinho de `SettingsModule`, os dois nunca se
//! chamam.
//!
//! O handler TS inteiro é `{ views, settingsPanels } = featureRegistryPort().getUiCatalog()` —
//! `feature_registry::shared().get_ui_catalog()` já reproduz essa forma byte a byte, inclusive o
//! `camelCase` de `settingsPanels` (ver o cabeçalho de `UiCatalog` em `feature_registry/types.rs`).
//! Este módulo só serializa o que aquele getter devolve; não monta nada sozinho.
//!
//! **Sem `assertOwnerSurface`, de propósito — mesma escolha do TS.** O comentário de
//! `ipc/plugin-ui.ts` é explícito: uma janela `plugin-view` (`PluginViewApp`) precisa deste
//! catálogo para achar a PRÓPRIA view declarada antes de existir qualquer outra forma de saber
//! qual componente montar — e só devolve metadado (id/título/componente), nunca um valor de
//! config. Aplicar a guarda aqui, ao contrário do TS, quebraria toda `plugin-view` na primeira
//! renderização: regressão, não enrijecimento. Ao contrário de `mika:settings-get-all`/
//! `mika:settings-set` (`handlers.rs`), este canal não lê `ConfigStore` nenhum.
//!
//! **Sem equivalente a `await waitForPlugins()`, e não é lacuna.** O TS espera o `PluginLoader`
//! terminar de ativar cada plugin porque só ISSO popula o catálogo, em runtime, por requisição.
//! O Rust não tem esse atraso: `lib.rs::setup` chama `modules::register_config_bound_tools(&handle)`
//! — que registra toda aba/view/painel que hoje existe em Rust — ANTES de `windows::create_main`
//! (comentário no próprio `setup`: "o backend sobe antes da janela"), e uma `plugin-view` só pode
//! existir depois que a janela principal já montou. Por ordem de boot, o catálogo já está cheio na
//! primeira vez que qualquer janela poderia perguntar por ele.
//!
//! **Escopo do catálogo hoje: 2 dos 14 `FeatureDefinition` declaram `views`/`settingsPanels`.**
//! Medido lendo TODO `*.ts` sob `apps/desktop/src`/`packages` por `views:`/`settingsPanels:` fora
//! de teste — só `gamer-companion.feature.ts` (3 views) e `person-catalog.feature.ts` (1 view + 1
//! painel) declaram algo; as outras 12 abas (inclusive `voice`, que nem tab de settings tem em Rust
//! ainda — ver a atualização mais recente no cabeçalho de `mod.rs`) não têm view nem painel a
//! perder. Reivindicar este canal NÃO depende de `voice` estar portado: `get_ui_catalog()` nunca lê
//! `settings`, só `views`/`settings_panels`, e os dois únicos donos de algo aí (`gamer::feature`,
//! `features::person_catalog::feature`) já registram as próprias views/painéis desde esta rodada.
//! `modules/plugins/mod.rs` ainda documenta os quatro canais candidatos do diretório dele (inclusive
//! este) como "nenhum reivindicável hoje" — desatualizado especificamente para
//! `mika:plugin-ui-catalog` depois desta rodada; não editado aqui por não ser meu arquivo (mesma
//! régua já aplicada à nota velha de `features/mod.rs`), só reportado.

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::feature_registry;
use crate::native::NativeModule;
use crate::protocol::InvokeContext;

const CH_PLUGIN_UI_CATALOG: &str = "mika:plugin-ui-catalog";
const CHANNELS: &[&str] = &[CH_PLUGIN_UI_CATALOG];

/// O corpo da rota, sem `AppHandle`/`InvokeContext` — `get_ui_catalog()` não lê config nem janela,
/// então não há motivo para a assinatura carregar nenhum dos dois. Separado de `invoke` para que o
/// teste não precise montar um `AppHandle` de mentira (o resto do crate testa assim: ver
/// `handlers.rs::build_get_all`/`build_ui_prefs`).
fn build_catalog() -> Value {
    serde_json::to_value(feature_registry::shared().get_ui_catalog())
        .expect("UiCatalog só carrega Vec<Value> — sempre serializa")
}

pub struct PluginUiModule;

#[async_trait]
impl NativeModule for PluginUiModule {
    fn name(&self) -> &'static str {
        "plugin_ui"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(
        &self,
        _app: &AppHandle,
        _ctx: &InvokeContext,
        channel: &str,
        _payload: Value,
    ) -> Result<Value, String> {
        match channel {
            CH_PLUGIN_UI_CATALOG => Ok(build_catalog()),
            other => Err(format!("plugin_ui: canal '{other}' declarado mas não tratado")),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn channels_are_the_exact_literal() {
        let module = PluginUiModule;
        assert_eq!(module.channels(), &["mika:plugin-ui-catalog"]);
    }

    /// Prova a forma exata (`views`/`settingsPanels` camelCase, `pluginId` injetado) contra os dois
    /// únicos donos de conteúdo hoje — não contra o tamanho do registro inteiro, que outros testes
    /// do processo também populam em paralelo (`feature_registry::shared()` é `OnceLock` de
    /// processo; ver o cabeçalho de `feature_registry/mod.rs`).
    #[test]
    fn gamer_companion_and_person_catalog_views_and_panels_carry_their_plugin_id() {
        crate::modules::gamer::feature::register_feature();
        crate::modules::features::person_catalog::feature::register_feature();

        let response = build_catalog();

        let views = response["views"].as_array().unwrap();
        let hud = views.iter().find(|v| v["id"] == json!("gamer-companion.hud")).expect("view do HUD presente");
        assert_eq!(hud["pluginId"], json!("mika.gamer-companion"));
        let listing = views
            .iter()
            .find(|v| v["id"] == json!("person-catalog.listing"))
            .expect("view de listagem presente");
        assert_eq!(listing["pluginId"], json!("mika.person-catalog"));

        let panels = response["settingsPanels"].as_array().unwrap();
        let crud = panels.iter().find(|p| p["id"] == json!("person-catalog.crud")).expect("painel CRUD presente");
        assert_eq!(crud["pluginId"], json!("mika.person-catalog"));
        assert_eq!(crud["placement"], json!("after-schema"));
    }

    /// A chave é `settingsPanels`, camelCase — se a `Declaration` de `feature_registry/types.rs`
    /// perder o `#[serde(rename_all = "camelCase")]`, o schema Zod do lado TS
    /// (`desktopPluginUiCatalogResponseSchema`) apagaria o campo em silêncio (a chave
    /// `settings_panels` não bate com nenhuma do schema).
    #[test]
    fn the_response_uses_camel_case_settings_panels() {
        let response = build_catalog();
        let obj = response.as_object().unwrap();
        assert!(obj.contains_key("views"));
        assert!(obj.contains_key("settingsPanels"));
        assert!(!obj.contains_key("settings_panels"));
    }
}
