//! `mika:plugin-test` (ramo genérico, `modules/settings/plugin_test.rs`) — porte de
//! `apps/desktop/src/features/mcp/index.ts::testConnection`.
//!
//! Mesmas três condições do TS, lidas frescas a cada chamada (nunca cacheadas no construtor): modo
//! offline bloqueia mesmo com servidor vivo (MCP é processo externo com rede própria — ver o
//! cabeçalho de `mod.rs`), catálogo vazio é falha nomeada, senão soma servidores/tools do MESMO
//! [`super::connections`] que [`super::connect_and_register`] já preenche em produção.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::settings::plugin_test::{registry, TestConnection};

struct McpTestConnection {
    config: Arc<ConfigStore>,
}

#[async_trait]
impl TestConnection for McpTestConnection {
    async fn call(&self, _prompt: Option<&str>) -> Value {
        if crate::modules::privacy::is_offline_mode_enabled(&self.config) {
            return json!({ "ok": false, "message": "Modo offline ligado: nenhum servidor MCP sobe" });
        }

        let servers = super::connections().lock().unwrap_or_else(|e| e.into_inner());
        if servers.is_empty() {
            return json!({ "ok": false, "message": "No MCP server connected" });
        }
        let tools: usize = servers.iter().map(|s| s.tools.len()).sum();
        json!({ "ok": true, "message": format!("{} server(s), {} tool(s)", servers.len(), tools) })
    }
}

fn register(config: Arc<ConfigStore>) {
    registry().register("mika.mcp", Arc::new(McpTestConnection { config }));
}

/// Chamado por `super::boot`, único chamador de produção — resolve o `ConfigStore` aqui (não em
/// `mod.rs`) só para manter aquele arquivo sob o teto de 300 linhas do AGENTS.md.
pub(super) fn register_from_boot(app: &tauri::AppHandle) {
    match crate::state::config_store(app) {
        Ok(store) => register(store),
        Err(error) => log::error!("mcp: sem ConfigStore — teste de conexão de mika:plugin-test não registrado: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{InvokeContext, WindowType};
    use std::path::PathBuf;

    /// Os dois testes registram `mika.mcp` no MESMO registro global de `plugin_test` — rodando em
    /// paralelo, um lia o `ConfigStore` (offline ligado) que o outro acabou de registrar.
    fn serial() -> &'static tokio::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-mcp-plugin-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Sem servidor conectado e sem modo offline — o caminho que qualquer instalação nova bate
    /// (nenhum `mcp.json` declarado ainda).
    #[tokio::test]
    async fn dispatch_names_no_server_connected_when_nothing_is_up() {
        let _serial = serial().lock().await;
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        register(Arc::new(ConfigStore::open(&dir.0.join("cfg2"), crate::config::schemas::registry()).unwrap()));

        let ctx = InvokeContext { window_type: WindowType::Main, window_label: "test".into(), plugin_id: None };
        let result = crate::modules::settings::plugin_test::dispatch(&store, &ctx, &dir.0, &json!({ "pluginId": "mika.mcp" }))
            .await
            .unwrap();
        assert_eq!(result["ok"], json!(false));
        assert_eq!(result["message"], json!("No MCP server connected"));
    }

    /// NEGATIVO — modo offline vence mesmo antes de olhar `connections()`.
    #[tokio::test]
    async fn dispatch_blocks_on_offline_mode_before_checking_connections() {
        let _serial = serial().lock().await;
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        let config = Arc::new(ConfigStore::open(&dir.0.join("cfg2"), crate::config::schemas::registry()).unwrap());
        config.set("privacy.offlineMode", json!(true)).unwrap();
        register(config);

        let ctx = InvokeContext { window_type: WindowType::Main, window_label: "test".into(), plugin_id: None };
        let result = crate::modules::settings::plugin_test::dispatch(&store, &ctx, &dir.0, &json!({ "pluginId": "mika.mcp" }))
            .await
            .unwrap();
        assert_eq!(result["ok"], json!(false));
        assert!(result["message"].as_str().unwrap().to_lowercase().contains("offline"));
    }
}
