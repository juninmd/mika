//! `mika:plugin-test` (ramo genérico, `modules/settings/plugin_test.rs`) — porte de
//! `apps/desktop/src/features/character/index.ts::testConnection`.
//!
//! O TS responde `ok:false` quando `activeService` é `null` (plugin desativado ou falhou ao
//! ativar). Aqui não existe esse estado para reproduzir: `character` é `tier: 'core'` (sem
//! `enabledConfigPath` — nunca desativável pelo usuário) e [`register`] SEMPRE roda dentro de
//! `modules::register_config_bound_tools`, então alcançar este handler já prova o equivalente de
//! `activeService` não-nulo. `ok:true` fixo é medido a partir do comportamento real do boot em
//! Rust, não presumido por conveniência.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::settings::plugin_test::{registry, TestConnection};

struct CharacterTestConnection;

#[async_trait]
impl TestConnection for CharacterTestConnection {
    async fn call(&self, _prompt: Option<&str>) -> Value {
        json!({ "ok": true, "message": "Serviço de personagem montado. Nada externo para testar." })
    }
}

/// Chamado por `super::tool::register`, único chamador de produção.
pub(super) fn register() {
    registry().register("mika.character", Arc::new(CharacterTestConnection));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::store::ConfigStore;
    use crate::protocol::{InvokeContext, WindowType};
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-character-plugin-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Dispacha via `plugin_test::dispatch`, não o handler direto — prova o caminho real que a
    /// tela de Settings percorre (guarda de superfície + resolução de `pluginId` inclusas).
    #[tokio::test]
    async fn dispatch_reaches_the_character_handler_and_reports_it_mounted() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        register();

        let ctx = InvokeContext { window_type: WindowType::Main, window_label: "test".into(), plugin_id: None };
        let result = crate::modules::settings::plugin_test::dispatch(&store, &ctx, &dir.0, &json!({ "pluginId": "mika.character" }))
            .await
            .unwrap();
        assert_eq!(result["ok"], json!(true));
        assert!(result["message"].as_str().unwrap().contains("Serviço de personagem montado"));
    }
}
