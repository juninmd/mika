//! `mika:plugin-test` (ramo genérico, `modules/settings/plugin_test.rs`) — porte de
//! `apps/desktop/src/features/privacy/index.ts::testConnection` (`testPrivacyReach`).
//!
//! O TS soma 18 dependências opcionais do `PrivacyRuntime` e reporta `ok:false` nomeando as que
//! faltam — na prática sempre 0 ou 18, porque `activate()` seta as 18 de uma vez, nunca parcial.
//! **Aqui a resposta é medida, não simétrica ao TS**: [`super::clear::SKIPPED_STORES`] é a MESMA
//! lista que `clear_history` usa para o que este build de Rust ainda não alcança (12 de 18 stores
//! hoje) — reusada em vez de duplicada, para as duas nunca divergirem em silêncio. Enquanto a lista
//! não estiver vazia, `ok:false` nomeia exatamente o que um "apaga tudo" deixaria no disco; fica
//! `ok:true` sozinho quando `clear.rs` fechar as últimas 6.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::settings::plugin_test::{registry, TestConnection};

/// = `required.length` no TS (`testPrivacyReach`) — as 18 dependências do `PrivacyRuntime`.
const TOTAL_STORES: usize = 18;

/// Espelha os 3 `gates` de `testPrivacyReach` — só entram na mensagem quando a cobertura já é
/// total, mesma condição do TS (`ok:true` é o único ramo que os lê).
fn gates_summary(config: &ConfigStore) -> String {
    let offline = config.get("privacy.offlineMode").and_then(|v| v.as_bool()).unwrap_or(false);
    let screen_capture = config.get("privacy.screenCapture").and_then(|v| v.as_bool()).unwrap_or(false);
    let clipboard = config.get("privacy.clipboardLogging").and_then(|v| v.as_bool()).unwrap_or(false);
    format!(
        "{}, {}, {}",
        if offline { "rede bloqueada" } else { "rede liberada" },
        if screen_capture { "captura de tela ligada" } else { "captura de tela desligada" },
        if clipboard { "área de transferência no contexto" } else { "área de transferência protegida" },
    )
}

struct PrivacyTestConnection {
    config: Arc<ConfigStore>,
}

#[async_trait]
impl TestConnection for PrivacyTestConnection {
    async fn call(&self, _prompt: Option<&str>) -> Value {
        if super::clear::SKIPPED_STORES.is_empty() {
            return json!({
                "ok": true,
                "message": format!("Apagamento alcança as {TOTAL_STORES} stores. Estado atual: {}.", gates_summary(&self.config)),
            });
        }
        json!({
            "ok": false,
            "message": format!(
                "O apagamento não alcança: {}. Um pedido de \"apaga tudo\" deixaria esses dados no disco.",
                super::clear::SKIPPED_STORES.join(", "),
            ),
        })
    }
}

/// Chamado por `super::register`, único chamador de produção.
pub(super) fn register(config: Arc<ConfigStore>) {
    registry().register("mika.privacy", Arc::new(PrivacyTestConnection { config }));
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
            let path = std::env::temp_dir().join(format!("mika-privacy-plugin-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// NEGATIVO — reflete o estado REAL de cobertura hoje (12 de 18): nunca `ok:true` enquanto
    /// `clear.rs::SKIPPED_STORES` não estiver vazia, senão a tela afirmaria "apaga tudo" com 6
    /// stores intocadas.
    #[tokio::test]
    async fn dispatch_names_every_store_this_build_does_not_reach_yet() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        register(Arc::new(ConfigStore::open(&dir.0.join("cfg2"), crate::config::schemas::registry()).unwrap()));

        let ctx = InvokeContext { window_type: WindowType::Main, window_label: "test".into(), plugin_id: None };
        let result = crate::modules::settings::plugin_test::dispatch(&store, &ctx, &dir.0, &json!({ "pluginId": "mika.privacy" }))
            .await
            .unwrap();
        assert_eq!(result["ok"], json!(false));
        for skipped in super::super::clear::SKIPPED_STORES {
            assert!(result["message"].as_str().unwrap().contains(skipped), "mensagem não nomeia '{skipped}'");
        }
    }
}
