//! `mika:plugin-test` (ramo genérico, `modules/settings/plugin_test.rs`) — porte de
//! `apps/desktop/src/features/gamer-companion/index.ts::testConnection` (`testSteamConnection`).
//!
//! ## Medido, não presumido
//! - **Braço da API Web** chama `GetOwnedGames` de verdade (`steam_owned_games.rs`), com o
//!   transporte injetado para o teste não abrir socket.
//! - **Braço local (`listSteamGames`).** [`super::steam_library::list_steam_games`] é INFALÍVEL —
//!   devolve `Vec` vazio para uma pasta inexistente, nunca `Err` (diferente do
//!   `try/catch` do TS, que distingue "erro ao listar" de "pasta não configurada"). Sem uma forma de
//!   provocar erro aqui, este braço sempre reporta o MESMO texto de sucesso do TS
//!   (`"Caminho local da Steam testado: N jogos instalados detectados."`), configurado ou não —
//!   nunca o ramo "erro ao listar" nem o ramo "pasta não configurada", porque nenhum dos dois é
//!   alcançável neste build.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::settings::plugin_test::{registry, TestConnection};

use super::steam_library::list_steam_games;
use super::steam_owned_games::{owned_games_count, OwnedGamesFetcher, ReqwestFetcher};

fn config_text(config: &ConfigStore, path: &str) -> String {
    config.get(path).and_then(|v| v.as_str().map(str::trim).map(str::to_string)).unwrap_or_default()
}

struct GamerCompanionTestConnection {
    config: Arc<ConfigStore>,
    fetcher: Arc<dyn OwnedGamesFetcher>,
}

#[async_trait]
impl TestConnection for GamerCompanionTestConnection {
    async fn call(&self, _prompt: Option<&str>) -> Value {
        let api_key = config_text(&self.config, "gamer-companion.steamApiKey");
        let steam_id = config_text(&self.config, "gamer-companion.steamId64");
        let steam_path = config_text(&self.config, "gamer-companion.steamPath");

        let mut results = Vec::new();
        let mut ok = true;

        if !api_key.is_empty() && !steam_id.is_empty() {
            match owned_games_count(self.fetcher.as_ref(), &self.config, &api_key, &steam_id).await {
                Ok(count) => results.push(format!("Conectado à API da Steam! {count} jogos encontrados na biblioteca.")),
                Err(error) => {
                    ok = false;
                    results.push(format!("Erro na API da Steam: {error}"));
                }
            }
        } else {
            results.push("API Key da Steam ou SteamID64 não configurados (pulando teste da API).".to_string());
        }

        let path = if steam_path.is_empty() { None } else { Some(steam_path.as_str()) };
        let games = list_steam_games(path, true);
        results.push(format!("Caminho local da Steam testado: {} jogos instalados detectados.", games.len()));

        json!({ "ok": ok, "message": results.join("\n") })
    }
}

/// Chamado por `super::register_session_tools`, único chamador de produção.
pub(super) fn register(config: Arc<ConfigStore>) {
    registry().register("mika.gamer-companion", Arc::new(GamerCompanionTestConnection { config, fetcher: Arc::new(ReqwestFetcher) }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{InvokeContext, WindowType};
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-gamer-plugin-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Sem credencial nenhuma — o estado de uma instalação nova. Nunca toca rede: o braço de API
    /// nem chega a existir neste build.
    #[tokio::test]
    async fn dispatch_reports_the_local_scan_when_no_steam_credentials_are_configured() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        register(Arc::new(ConfigStore::open(&dir.0.join("cfg2"), crate::config::schemas::registry()).unwrap()));

        let ctx = InvokeContext { window_type: WindowType::Main, window_label: "test".into(), plugin_id: None };
        let result =
            crate::modules::settings::plugin_test::dispatch(&store, &ctx, &dir.0, &json!({ "pluginId": "mika.gamer-companion" }))
                .await
                .unwrap();
        assert_eq!(result["ok"], json!(true));
        let message = result["message"].as_str().unwrap();
        assert!(message.contains("pulando teste da API"));
        assert!(message.contains("jogos instalados detectados"));
    }

    struct FakeSteam(u16, &'static str);

    #[async_trait]
    impl OwnedGamesFetcher for FakeSteam {
        async fn fetch(&self, _api_key: &str, _steam_id: &str) -> Result<(u16, Vec<u8>), String> {
            Ok((self.0, self.1.as_bytes().to_vec()))
        }
    }

    fn with_credentials(dir: &TempDir, steam: FakeSteam) -> GamerCompanionTestConnection {
        let config = Arc::new(ConfigStore::open(&dir.0.join("cfg2"), crate::config::schemas::registry()).unwrap());
        config.set("gamer-companion.steamApiKey", json!("chave-secreta-steam")).unwrap();
        config.set("gamer-companion.steamId64", json!("76500000000000000")).unwrap();
        GamerCompanionTestConnection { config, fetcher: Arc::new(steam) }
    }

    /// "Validar" com credencial falhava SEMPRE: o braço da API nem existia em Rust.
    #[tokio::test]
    async fn validate_reaches_the_steam_web_api_when_credentials_are_configured() {
        let dir = TempDir::new();
        let body = r#"{"response":{"game_count":2,"games":[{"appid":1},{"appid":2}]}}"#;
        let result = with_credentials(&dir, FakeSteam(200, body)).call(None).await;
        assert_eq!(result["ok"], json!(true));
        assert!(result["message"].as_str().unwrap().contains("2 jogos encontrados"), "{result}");
    }

    #[tokio::test]
    async fn validate_reports_a_rejected_key_without_leaking_it() {
        let dir = TempDir::new();
        let result = with_credentials(&dir, FakeSteam(401, "")).call(None).await;
        assert_eq!(result["ok"], json!(false));
        let message = result["message"].as_str().unwrap();
        assert!(message.contains("Chave inválida (401)"), "{message}");
        assert!(!message.contains("chave-secreta-steam"));
    }

    #[tokio::test]
    async fn validate_respects_offline_mode_before_calling_steam() {
        let dir = TempDir::new();
        let connection = with_credentials(&dir, FakeSteam(200, r#"{"response":{}}"#));
        connection.config.set("privacy.offlineMode", json!(true)).unwrap();
        let result = connection.call(None).await;
        assert_eq!(result["ok"], json!(false));
        assert!(result["message"].as_str().unwrap().contains("offline"), "{result}");
    }
}
