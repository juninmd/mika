//! `run_affinity_delta` — o que a ação `affinity_delta` da tool `avatar` despacha
//! (`tool.rs::AvatarToolHandler::call`), extraído para arquivo próprio pelo teto de 300 linhas do
//! AGENTS.md. Espelha `CharacterService.setAffinityDelta`
//! (`apps/desktop/src/features/character/services/character.service.ts`) — MESMA escala que o
//! `ConfigStore` já guarda (-100..100), sem a reescala que `features/mood/persistence.rs` faz para
//! `mood`/`affinity`.

use serde_json::{json, Value};

use crate::config::store::ConfigStore;

const AFFINITY_CONFIG_PATH: &str = "character.affinityScore";

/// `clampAffinity` (`packages/sdk/src/utils/clamp.ts`) — os números que chegam por `serde_json`
/// são sempre finitos (JSON não representa NaN/Infinity), então só o clamp de faixa importa aqui.
fn clamp_affinity(value: f64) -> f64 {
    value.clamp(-100.0, 100.0)
}

pub fn run_affinity_delta(store: &ConfigStore, delta: f64, reason: Option<&str>) -> Value {
    let current = store.get(AFFINITY_CONFIG_PATH).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let next = clamp_affinity(current + delta);

    if let Err(error) = store.set(AFFINITY_CONFIG_PATH, json!(next)) {
        return json!({ "success": false, "error": error, "errorCode": "unavailable" });
    }

    json!({
        "success": true,
        "data": { "previous": current, "delta": delta, "next": next, "reason": reason },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::float_range;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-affinity-action-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (Arc<ConfigStore>, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "character",
            fields: vec![float_range("affinityScore", 0.0, -100.0, 100.0)],
        }]);
        (Arc::new(ConfigStore::open(&dir.0, schema).unwrap()), dir)
    }

    #[test]
    fn adds_to_the_current_score_and_persists() {
        let (store, _dir) = store();

        let result = run_affinity_delta(&store, 15.0, None);
        assert_eq!(result["data"]["next"], json!(15.0));
        assert_eq!(store.get("character.affinityScore"), Some(json!(15.0)));

        let second = run_affinity_delta(&store, -5.0, Some("ganhou uma piada"));
        assert_eq!(second["data"]["previous"], json!(15.0));
        assert_eq!(second["data"]["next"], json!(10.0));
        assert_eq!(second["data"]["reason"], json!("ganhou uma piada"));
    }

    /// NEGATIVO — nunca ultrapassa a faixa -100..100, mesmo com delta agressivo.
    #[test]
    fn is_clamped_to_the_declared_range() {
        let (store, _dir) = store();

        run_affinity_delta(&store, 20.0, None);
        let result = run_affinity_delta(&store, 20.0, None);
        assert_eq!(result["data"]["next"], json!(40.0));

        for _ in 0..10 {
            run_affinity_delta(&store, 20.0, None);
        }
        assert_eq!(store.get("character.affinityScore"), Some(json!(100.0)));
    }
}
