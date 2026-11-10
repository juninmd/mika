//! `mika.mood` — os 4 métodos de `mika:plugin-rpc` que este plugin serve, porte de
//! `apps/desktop/src/features/mood/index.ts::activate` (`ctx.rpc.handle(...)`).
//!
//! `mood_get`/`affinity_get`/`affinity_add_interaction` leem/escrevem [`PersistentAffinity`] — a
//! MESMA afinidade persistida que a tool `mood` (`super::persistence`) usa desde
//! `register_config_bound_tools`, não o `AffinityInner` em memória de `super::engine` (que só existe
//! antes desse segundo registro). É o que `ports.score.read/write` faz no TS: os três escritores
//! (`mood` RPC, tool `mood`, tool `avatar`) disputam o MESMO `character.affinityScore`, então ler
//! fresco é a única leitura correta — reaproveita [`PersistentAffinity`] em vez de duplicar a conta.
//!
//! `mood_update` continua sobre o motor de humor em memória (`super::engine`) — TS nunca persistiu
//! valência/excitação (item 1 do cabeçalho de `mod.rs`), então não há divergência nova aqui.
//!
//! Por isso [`register`] só pode acontecer no MESMO ponto que já tem o `ConfigStore`
//! (`super::persistence::register`), não em `super::tool::register_tools` — sem ele não existe
//! afinidade persistida para `mood_get`/`affinity_get`/`affinity_add_interaction` lerem.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};
use crate::modules::plugins::rpc::{registry, RpcHandler};

use super::engine::engine;
use super::persistence::PersistentAffinity;

const PLUGIN_ID: &str = "mika.mood";

struct MoodGet;

#[async_trait]
impl RpcHandler for MoodGet {
    /// Porte de `ctx.rpc.handle('mood_get', async () => ({ success: true, data: instance.current() }))`
    /// — `instance.current()` é só o `MoodState` (valência/excitação/rótulo), NUNCA a afinidade
    /// (que é `affinity_get`, método separado no TS).
    async fn call(&self, _args: Value) -> Result<Value, String> {
        Ok(engine().mood_snapshot())
    }
}

struct MoodUpdate;

#[async_trait]
impl RpcHandler for MoodUpdate {
    /// Porte de `ctx.rpc.handle('mood_update', ...)`: valida os dois campos ANTES de tocar o motor
    /// (mesmo erro nomeado do TS), publica `MoodChanged` — mesmo `ctx.events.emit` do TS.
    async fn call(&self, args: Value) -> Result<Value, String> {
        let valence = args.get("valence").and_then(Value::as_f64);
        let arousal = args.get("arousal").and_then(Value::as_f64);
        match (valence, arousal) {
            (Some(v), Some(a)) => {
                let result = engine().update(v, a);
                bus().publish(EventKey::MoodChanged, result["data"].clone());
                Ok(result["data"].clone())
            }
            _ => Err("valence and arousal must be finite numbers.".into()),
        }
    }
}

struct AffinityGet {
    affinity: PersistentAffinity,
}

#[async_trait]
impl RpcHandler for AffinityGet {
    /// Porte de `ctx.rpc.handle('affinity_get', async () => ({ success: true, data: instance.affinity() }))`.
    async fn call(&self, _args: Value) -> Result<Value, String> {
        Ok(self.affinity.snapshot())
    }
}

struct AffinityAddInteraction {
    affinity: PersistentAffinity,
}

#[async_trait]
impl RpcHandler for AffinityAddInteraction {
    /// Porte de `ctx.rpc.handle('affinity_add_interaction', ...)` -> `instance.registerInteraction(weight)`
    /// -> `registerInteractionAndNotify`: devolve só `{level,label,score}` (SEM `levelUp`/`reason`,
    /// que são campos exclusivos da tool `mood`, não desta RPC) e publica `MoodAffinityLevelUp` só
    /// quando o nível sobe.
    async fn call(&self, args: Value) -> Result<Value, String> {
        let weight = args.get("weight").and_then(Value::as_f64).unwrap_or(1.0);
        let (data, level_up) = self.affinity.add_interaction(weight)?;
        if level_up {
            bus().publish(EventKey::MoodAffinityLevelUp, data.clone());
        }
        Ok(data)
    }
}

/// Chamado por `super::persistence::register` — único ponto do módulo `mood` que já tem o
/// `ConfigStore` que a afinidade persistida exige. Ver o cabeçalho deste arquivo.
pub(super) fn register(store: Arc<ConfigStore>) {
    let registry = registry();
    registry.register(PLUGIN_ID, "mood_get", Arc::new(MoodGet));
    registry.register(PLUGIN_ID, "mood_update", Arc::new(MoodUpdate));
    registry.register(
        PLUGIN_ID,
        "affinity_get",
        Arc::new(AffinityGet { affinity: PersistentAffinity::new(Arc::clone(&store)) }),
    );
    registry.register(
        PLUGIN_ID,
        "affinity_add_interaction",
        Arc::new(AffinityAddInteraction { affinity: PersistentAffinity::new(store) }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::float_range;
    use crate::protocol::WindowType;
    use serde_json::json;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-mood-rpc-test-{}-{unique}", std::process::id()));
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
        let schema = SchemaRegistry::new(vec![
            Namespace { name: "character", fields: vec![float_range("affinityScore", 0.0, -100.0, 100.0)] },
            Namespace { name: "mood", fields: vec![float_range("affinityGainRate", 1.0, 0.0, 10.0)] },
        ]);
        (Arc::new(ConfigStore::open(&dir.0, schema).unwrap()), dir)
    }

    /// Prova despacho REAL via `rpc::dispatch`, não o handler direto — os 4 métodos, um payload
    /// que exercita `mood_update` (motor em memória) e `affinity_add_interaction` (persistido).
    #[tokio::test]
    async fn all_four_methods_dispatch_through_the_registry() {
        let _order = crate::modules::features::mood::TOOL_REGISTRATION_ORDER
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (config, _dir) = store();
        register(config);

        // `mood_update` primeiro, não `mood_get`: `engine()` é um `OnceLock` de PROCESSO
        // compartilhado por `tool.rs`/`persistence.rs`/aqui — o rótulo inicial depende de que
        // outro teste rodou antes no mesmo binário. `update()` SOBRESCREVE (não incrementa), então
        // o valor pós-`update` é determinístico independente do que veio antes.
        let update = crate::modules::plugins::rpc::dispatch(
            WindowType::Main,
            None,
            &json!({ "pluginId": "mika.mood", "method": "mood_update", "args": { "valence": 0.5, "arousal": 0.2 } }),
        )
        .await;
        assert_eq!(update["data"]["valence"], json!(0.5));

        let get = crate::modules::plugins::rpc::dispatch(
            WindowType::Main,
            None,
            &json!({ "pluginId": "mika.mood", "method": "mood_get" }),
        )
        .await;
        assert_eq!(get["success"], json!(true));
        assert_eq!(get["data"]["valence"], json!(0.5));
        assert_eq!(get["data"]["arousal"], json!(0.2));
        assert_eq!(get["data"]["label"], json!("happy"), "valence>0.3 e arousal não < -0.3 é 'happy'");

        let affinity = crate::modules::plugins::rpc::dispatch(
            WindowType::Main,
            None,
            &json!({ "pluginId": "mika.mood", "method": "affinity_get" }),
        )
        .await;
        assert_eq!(affinity["data"]["score"], json!(50.0));

        let interaction = crate::modules::plugins::rpc::dispatch(
            WindowType::Main,
            None,
            &json!({ "pluginId": "mika.mood", "method": "affinity_add_interaction", "args": { "weight": 2.0 } }),
        )
        .await;
        assert_eq!(interaction["data"]["score"], json!(52.0));
        // Nem `levelUp` nem `reason` — só a tool `mood` (não esta RPC) adiciona esses campos.
        assert!(interaction["data"].get("levelUp").is_none());
    }

    /// NEGATIVO — `mood_update` sem os dois campos é `handler-failed` nomeado, nunca sucesso parcial.
    #[tokio::test]
    async fn mood_update_without_both_fields_is_a_named_failure() {
        let _order = crate::modules::features::mood::TOOL_REGISTRATION_ORDER
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (config, _dir) = store();
        register(config);

        let result = crate::modules::plugins::rpc::dispatch(
            WindowType::Main,
            None,
            &json!({ "pluginId": "mika.mood", "method": "mood_update", "args": { "valence": 0.5 } }),
        )
        .await;
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["error"]["code"], json!("handler-failed"));
    }

    /// `affinity_add_interaction` publica `MoodAffinityLevelUp` só quando cruza nível — mesma
    /// condição de `registerInteractionAndNotify`.
    #[tokio::test]
    async fn affinity_add_interaction_publishes_level_up_only_when_crossing_a_boundary() {
        let _order = crate::modules::features::mood::TOOL_REGISTRATION_ORDER
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (config, _dir) = store();
        register(Arc::clone(&config));
        let mut sub = bus().subscribe();

        // Score parte de 50 ('friend') — ganho de 15 cruza para 60 ('fan').
        crate::modules::plugins::rpc::dispatch(
            WindowType::Main,
            None,
            &json!({ "pluginId": "mika.mood", "method": "affinity_add_interaction", "args": { "weight": 15.0 } }),
        )
        .await;

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        let envelope = loop {
            let envelope =
                tokio::time::timeout_at(deadline, sub.recv()).await.expect("MoodAffinityLevelUp não publicado").unwrap();
            if envelope.key == EventKey::MoodAffinityLevelUp {
                break envelope;
            }
        };
        assert_eq!(envelope.payload["label"], json!("fan"));
    }
}
