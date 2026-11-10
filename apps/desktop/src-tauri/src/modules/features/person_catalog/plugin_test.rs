//! `mika:plugin-test` (ramo genérico, `modules/settings/plugin_test.rs`) — porte de
//! `apps/desktop/src/features/person-catalog/index.ts::testConnection`.
//!
//! O TS responde `ok:false` quando `activeCatalog` é `null` (plugin desativado ou falhou ao
//! ativar). [`super::tool::register_tools`] só chega a chamar [`register`] DEPOIS que
//! `catalog_store(app)` abriu com sucesso (o `?` acima propaga e sai cedo em qualquer falha) — então
//! alcançar este handler já prova o equivalente de `activeCatalog` não-nulo. `ok:true` fixo é
//! medido, não presumido.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::settings::plugin_test::{registry, TestConnection};

struct PersonCatalogTestConnection;

#[async_trait]
impl TestConnection for PersonCatalogTestConnection {
    async fn call(&self, _prompt: Option<&str>) -> Value {
        json!({ "ok": true, "message": "Catálogo de pessoas montado. Nada externo para testar." })
    }
}

/// Chamado por `super::tool::register_tools`, único chamador de produção.
pub(super) fn register() {
    registry().register("mika.person-catalog", Arc::new(PersonCatalogTestConnection));
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
            let path = std::env::temp_dir().join(format!("mika-person-catalog-plugin-test-{}-{unique}", std::process::id()));
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
    async fn dispatch_reaches_the_person_catalog_handler_and_reports_it_mounted() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        register();

        let ctx = InvokeContext { window_type: WindowType::Main, window_label: "test".into(), plugin_id: None };
        let result =
            crate::modules::settings::plugin_test::dispatch(&store, &ctx, &dir.0, &json!({ "pluginId": "mika.person-catalog" }))
                .await
                .unwrap();
        assert_eq!(result["ok"], json!(true));
        assert!(result["message"].as_str().unwrap().contains("Catálogo de pessoas montado"));
    }
}
