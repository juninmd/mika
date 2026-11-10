//! Lógica das três rotas que `mod.rs` despacha. Os literais de canal e o `channels()` do trait
//! ficam em `mod.rs`, não aqui — ver o comentário no topo daquele arquivo.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::protocol::{InvokeContext, WindowType};

/// `ConfigKey.AppLocale` / `ConfigKey.CharacterTheme` em `packages/sdk/src/contracts/schemas/config-keys.enum.ts`.
/// Copiados como literal porque o enum é TypeScript; não há como importar o símbolo daqui.
const APP_LOCALE_PATH: &str = "app.locale";
const CHARACTER_THEME_PATH: &str = "character.theme";

/// Espelha `assertOwnerSurface` de `apps/desktop/src/main/ipc/surface-policy.ts`: só janela dona
/// (main/settings/wizard) lê ou escreve config. Sem isto uma plugin-view leria e gravaria a
/// configuração do usuário por trás dela.
pub(super) fn assert_owner_surface(ctx: &InvokeContext, route: &str) -> Result<(), String> {
    match ctx.window_type {
        WindowType::Main | WindowType::Settings | WindowType::Wizard => Ok(()),
        other => {
            log::warn!("rota negada para esta janela ({other:?}): {route}");
            Err(format!("Settings access denied by policy ({route})"))
        }
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

pub(super) fn build_get_all(store: &ConfigStore) -> Value {
    json!({ "config": store.get_all() })
}

/// Espelha `registerSettingsIPC`'s `settingsDeclarations` route (`ipc/settings.ts`): sem
/// `assertOwnerSurface` (nenhuma janela é recusada — o TS também não guarda esta rota, é só
/// metadado de tela), locale resolvido da config, e tudo passado por `localize_feature_settings`
/// antes de devolver — sem isso a tela mostra a chave i18n crua.
///
/// **Ligada em 2026-09-21**, depois de `modules/voice/feature.rs` publicar `voice` e `stt` — as
/// duas últimas abas que faltavam. Ficou escrita e testada sem chamador por três rodadas de
/// propósito: até o catálogo fechar, servir esta rota apagaria da tela do usuário toda aba ainda
/// não registrada em Rust, em silêncio e sem erro.
///
/// O que garante que ela não volte a ser servida com catálogo furado NÃO é este comentário: é
/// `modules/mod.rs::the_catalog_covers_every_tab_the_ts_declares`, que lê os 13
/// `defineFeatureSchema` do fonte TypeScript e exige cada `tabId` no registro vivo.
pub(super) fn build_declarations(store: &ConfigStore) -> Value {
    let locale = crate::config::locale::app_locale(store);

    let raw = crate::feature_registry::shared().get_all_settings();
    let declarations = super::localize_feature_settings(&raw, &locale);
    json!({ "declarations": declarations })
}

pub(super) fn build_ui_prefs(store: &ConfigStore) -> Value {
    json!({
        "locale": store.get(APP_LOCALE_PATH),
        "theme": store.get(CHARACTER_THEME_PATH),
    })
}

/// Espelha `desktopSettingsSetResponseSchema`/`desktopConfigChangeSchema` em
/// `packages/sdk/src/contracts/desktop-ipc-core-schemas.ts`. `accepted`/`rejectedReason` existem
/// para o controle da UI saber que o valor foi recusado e por quê — sem eles é um controle que
/// simplesmente não obedece.
pub(super) fn handle_set(store: &ConfigStore, payload: &Value) -> Value {
    let path = payload
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let value = payload.get("value").cloned().unwrap_or(Value::Null);
    let timestamp = now_millis();

    if path.is_empty() {
        return json!({
            "success": false,
            "change": {
                "path": "",
                "value": value,
                "previousValue": Value::Null,
                "source": "system",
                "timestamp": timestamp,
            },
        });
    }

    let previous_value = store.get(&path).unwrap_or(Value::Null);

    match store.set(&path, value.clone()) {
        Ok(()) => json!({
            "success": true,
            "change": {
                "path": path,
                "value": value,
                "previousValue": previous_value,
                "source": "runtime",
                "timestamp": timestamp,
                "accepted": true,
            },
        }),
        Err(error) => json!({
            "success": false,
            "change": {
                "path": path,
                "value": value,
                "previousValue": previous_value,
                "source": "runtime",
                "timestamp": timestamp,
                "accepted": false,
                "rejectedReason": error,
            },
            "error": error,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Diretório temporário mínimo, mesmo padrão de `config/store.rs::tests::tempdir`. Não vale
    /// depender de um crate só para isto.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mika-settings-module-test-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        (store, dir)
    }

    fn ctx(window_type: WindowType) -> InvokeContext {
        InvokeContext {
            window_type,
            window_label: "test".into(),
            plugin_id: None,
        }
    }

    #[test]
    fn owner_surfaces_pass_the_guard() {
        assert!(assert_owner_surface(&ctx(WindowType::Main), "r").is_ok());
        assert!(assert_owner_surface(&ctx(WindowType::Settings), "r").is_ok());
        assert!(assert_owner_surface(&ctx(WindowType::Wizard), "r").is_ok());
    }

    /// A guarda é o que impede uma plugin-view de ler ou escrever a config do usuário por trás
    /// dela. Sem este teste, uma regressão aqui vira acesso silencioso.
    #[test]
    fn non_owner_surfaces_are_rejected() {
        assert!(assert_owner_surface(&ctx(WindowType::PluginView), "r").is_err());
        assert!(assert_owner_surface(&ctx(WindowType::Unknown), "r").is_err());
    }

    #[test]
    fn get_all_wraps_the_snapshot_under_config() {
        let (store, _dir) = store();
        let response = build_get_all(&store);
        assert!(response["config"]["memory"]["enabled"].is_boolean());
    }

    #[test]
    fn ui_prefs_exposes_only_locale_and_theme() {
        let (store, _dir) = store();
        let response = build_ui_prefs(&store);
        assert_eq!(
            response.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["locale", "theme"]
        );
    }

    #[test]
    fn set_accepts_a_declared_path_and_reports_the_previous_value() {
        let (store, _dir) = store();

        let response = handle_set(&store, &json!({ "path": "memory.enabled", "value": false }));

        assert_eq!(response["success"], json!(true));
        assert_eq!(response["change"]["path"], json!("memory.enabled"));
        assert_eq!(response["change"]["value"], json!(false));
        assert_eq!(response["change"]["previousValue"], json!(true));
        assert_eq!(response["change"]["accepted"], json!(true));
        assert_eq!(store.get("memory.enabled"), Some(json!(false)));
    }

    /// A mensagem de erro sobe até o `error` de topo E até `change.rejectedReason` — é o que o
    /// renderer mostra quando um controle recusa o valor.
    #[test]
    fn set_rejects_an_undeclared_path_with_a_readable_reason() {
        let (store, _dir) = store();

        let response = handle_set(&store, &json!({ "path": "memory.naoExiste", "value": 1 }));

        assert_eq!(response["success"], json!(false));
        assert_eq!(response["change"]["accepted"], json!(false));
        assert!(response["error"].as_str().unwrap().contains("memory.naoExiste"));
        assert!(response["change"]["rejectedReason"]
            .as_str()
            .unwrap()
            .contains("memory.naoExiste"));
    }

    #[test]
    fn set_with_empty_path_is_refused_without_touching_the_store() {
        let (store, _dir) = store();

        let response = handle_set(&store, &json!({ "path": "  ", "value": 1 }));

        assert_eq!(response["success"], json!(false));
        assert_eq!(response["change"]["source"], json!("system"));
    }

    /// Prova as duas metades do que a rota promete: nenhuma aba fica pra trás (o `len()` bate com
    /// o que `get_all_settings()` já tinha, não um subconjunto), e `localize_feature_settings`
    /// está de fato no caminho (a chave crua nunca escapa para o `tabLabel`).
    #[test]
    fn declarations_return_every_currently_registered_tab_translated_not_raw_keys() {
        let (store, _dir) = store();
        crate::modules::vision::register_feature();

        let expected_count = crate::feature_registry::shared().get_all_settings().len();
        let response = build_declarations(&store);
        let declarations = response["declarations"].as_array().unwrap();

        assert_eq!(declarations.len(), expected_count);

        let vision = declarations
            .iter()
            .find(|d| d["tabId"] == json!("vision"))
            .expect("aba vision presente na resposta");
        assert_ne!(vision["tabLabel"], json!("settings.vision.tab.label"), "tabLabel cru — localize_feature_settings não rodou");
        assert_eq!(vision["tabLabel"], json!("Visão"));
    }

    #[test]
    fn declarations_resolve_against_the_locale_stored_in_config() {
        let (store, _dir) = store();
        store.set("app.locale", json!("en-US")).unwrap();
        crate::modules::vision::register_feature();

        let response = build_declarations(&store);
        let vision = response["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["tabId"] == json!("vision"))
            .expect("aba vision presente");
        assert_eq!(vision["tabLabel"], json!("Vision"));
    }
}
