//! `mika:plugin-test` (ramo genérico, `modules/settings/plugin_test.rs`) — porte de
//! `apps/desktop/src/features/scheduler/index.ts::testConnection`.
//!
//! O TS devolve `ok:true` incondicional, sem checar estado nenhum — lembretes só falam com o
//! `SqliteReminderRepository` local, nunca com rede. Mesma condição aqui: fixa, sem depender do
//! store, porque a tool `reminder` já registra este handler dentro de [`super::tool::register`],
//! que só é chamado quando `SchedulerStore::open` deu certo.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::settings::plugin_test::{registry, TestConnection};

struct SchedulerTestConnection;

#[async_trait]
impl TestConnection for SchedulerTestConnection {
    async fn call(&self, _prompt: Option<&str>) -> Value {
        json!({ "ok": true, "message": "Lembretes usam apenas o banco local — nada externo para testar." })
    }
}

/// Chamado por `super::tool::register`, único chamador de produção.
pub(super) fn register() {
    registry().register("mika.scheduler", Arc::new(SchedulerTestConnection));
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
            let path = std::env::temp_dir().join(format!("mika-scheduler-plugin-test-{}-{unique}", std::process::id()));
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
    async fn dispatch_reaches_the_scheduler_handler_and_reports_it_local_only() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        register();

        let ctx = InvokeContext { window_type: WindowType::Main, window_label: "test".into(), plugin_id: None };
        let result = crate::modules::settings::plugin_test::dispatch(&store, &ctx, &dir.0, &json!({ "pluginId": "mika.scheduler" }))
            .await
            .unwrap();
        assert_eq!(result["ok"], json!(true));
        assert!(result["message"].as_str().unwrap().contains("banco local"));
    }
}
