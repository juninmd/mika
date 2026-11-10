//! = `recording-allowed.ts`. Compartilhado por `episodic::write` e `social::service` — MESMA
//! chave de config (`memory.enabled`) que os dois lados do TS leem via `configPort()`.
//!
//! Porte direto, sem porta injetável: o TS usa `configPort()` (indireção substituível em teste);
//! aqui o chamador já recebe `&ConfigStore` de `crate::state::config_store(app)` (regra da tarefa:
//! nunca abrir uma segunda instância), então a função recebe a referência direto.

use crate::config::store::ConfigStore;

/// = `isMemoryRecordingAllowed`: `true` a menos que `memory.enabled` esteja explicitamente `false`
/// — ausência de config, ou config ilegível, nunca bloqueia a gravação (mesmo fallback do TS).
pub fn memory_recording_allowed(config: &ConfigStore) -> bool {
    config.get("memory.enabled").and_then(|v| v.as_bool()) != Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-memory-recording-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn allowed_by_default_and_blocked_only_when_explicitly_false() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        assert!(memory_recording_allowed(&store));

        store.set("memory.enabled", serde_json::json!(false)).unwrap();
        assert!(!memory_recording_allowed(&store));

        store.set("memory.enabled", serde_json::json!(true)).unwrap();
        assert!(memory_recording_allowed(&store));
    }
}
