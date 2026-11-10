//! `mika:plugin-test` (ramo genérico, `modules/settings/plugin_test.rs`) — porte de
//! `apps/desktop/src/features/mood/index.ts::testConnection`.
//!
//! O TS checa `mood?.health().state === 'ready'`: a instância existe (ou não) dependendo de
//! `activate`/`deactivate` do plugin ter rodado. Rust não tem esse ciclo de vida ainda (item 1 do
//! cabeçalho de `mod.rs`) — o motor (`engine()`) e a afinidade persistida (`PersistentAffinity`) são
//! síncronos e sempre utilizáveis assim que [`register`] roda, sem etapa assíncrona de "ficar
//! pronto" para falhar no meio. [`super::persistence::register`] SEMPRE roda dentro de
//! `modules::register_config_bound_tools`, então alcançar este handler já prova o equivalente de
//! `health().state === 'ready'`. `ok:true` fixo é medido, não presumido.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::settings::plugin_test::{registry, TestConnection};

struct MoodTestConnection;

#[async_trait]
impl TestConnection for MoodTestConnection {
    async fn call(&self, _prompt: Option<&str>) -> Value {
        json!({ "ok": true, "message": "Serviços de humor e afinidade montados. Nada externo para testar." })
    }
}

/// Chamado por `super::persistence::register`, único chamador de produção.
pub(super) fn register() {
    registry().register("mika.mood", Arc::new(MoodTestConnection));
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
            let path = std::env::temp_dir().join(format!("mika-mood-plugin-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn dispatch_reaches_the_mood_handler_and_reports_it_mounted() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        register();

        let ctx = InvokeContext { window_type: WindowType::Main, window_label: "test".into(), plugin_id: None };
        let result = crate::modules::settings::plugin_test::dispatch(&store, &ctx, &dir.0, &json!({ "pluginId": "mika.mood" }))
            .await
            .unwrap();
        assert_eq!(result["ok"], json!(true));
        assert!(result["message"].as_str().unwrap().contains("humor e afinidade montados"));
    }
}
