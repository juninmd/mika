//! Tool `mood` (LLM) — port de `packages/mood/src/tools/mood.tool.ts`, registrada de verdade no
//! `ToolRegistry` nativo (`crate::modules::tools`). Ver o cabeçalho de `mod.rs` para as
//! divergências documentadas frente ao TS (afinidade não persistida nesta versão — ver
//! `persistence.rs` para a que persiste — e `tick()` sem timer).
//!
//! `action=update`/`action=register_interaction` publicam `EventKey::MoodChanged`/
//! `MoodAffinityLevelUp` no barramento de processo (`crate::events`, chave disponível desde
//! 2026-09-21c) — mesmo comportamento de `mood.tool.ts`/`registerInteractionAndNotify`
//! (`affinity.service.ts`): `update` emite sempre, `register_interaction` só quando o nível sobe.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::events::{bus, EventKey};
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::engine::engine;

/// Reexposta a `super::persistence` — a versão com config registra a MESMA definição de tool,
/// só o handler muda (afinidade lida/gravada em `character.affinityScore`).
pub(super) fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "mood".into(),
        description: Some(
            "Mika's own mood/affinity engine. Your current mood is ALREADY in your context — don't call this in normal chat. get: only when asked about your mood/affinity stats. update: shift mood after a real emotional turn (`valence` and `arousal` required). register_interaction: log a milestone moment (`weight` 1 = normal, `reason`). decay: relax toward baseline by `factor`."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["get", "update", "register_interaction", "decay"] },
                "valence": { "type": "number", "minimum": -1, "maximum": 1, "description": "-1 sad/upset … 1 happy." },
                "arousal": { "type": "number", "minimum": -1, "maximum": 1, "description": "-1 calm/sleepy … 1 excited." },
                "weight": { "type": "number", "minimum": 0.5, "maximum": 5 },
                "reason": { "type": "string" },
                "factor": { "type": "number", "minimum": 0.1, "maximum": 1 },
            },
            "required": ["action"],
        }),
        plugin_id: "mika.mood".into(),
        requires_window: false,
    }
}

struct MoodToolHandler;

#[async_trait]
impl ToolHandler for MoodToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        match action {
            "get" => Ok(engine().get()),
            "update" => {
                let valence = args.get("valence").and_then(Value::as_f64);
                let arousal = args.get("arousal").and_then(Value::as_f64);
                match (valence, arousal) {
                    (Some(v), Some(a)) => {
                        let result = engine().update(v, a);
                        bus().publish(EventKey::MoodChanged, result["data"].clone());
                        Ok(result)
                    }
                    _ => Err("action='update' requires both valence and arousal.".into()),
                }
            }
            "register_interaction" => {
                let weight = args.get("weight").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.5, 5.0);
                let reason = args.get("reason").and_then(Value::as_str);
                let result = engine().register_interaction(weight, reason);
                if result["data"]["levelUp"] == json!(true) {
                    bus().publish(
                        EventKey::MoodAffinityLevelUp,
                        json!({
                            "level": result["data"]["level"],
                            "label": result["data"]["label"],
                            "score": result["data"]["score"],
                        }),
                    );
                }
                Ok(result)
            }
            "decay" => {
                let factor = args.get("factor").and_then(Value::as_f64).unwrap_or(0.5).clamp(0.1, 1.0);
                Ok(engine().decay(factor))
            }
            other => Err(format!("mood: ação desconhecida \"{other}\"")),
        }
    }
}

/// Chamado por `features::modules()` — registra a tool `mood` no `ToolRegistry` de processo como
/// handler-tampão da PRIMEIRA passada (sem `ConfigStore`, sem persistência de afinidade).
/// `persistence::register(store)` (segunda passada) SUBSTITUI este handler pelo
/// `PersistentMoodToolHandler` de verdade — via `register_stub`, essa substituição pretendida não
/// conta como colisão (ver `ToolRegistry::stub_names`).
pub(crate) fn register_tools() {
    crate::modules::tools::registry().register_stub(definition(), Arc::new(MoodToolHandler));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tool_get_action_round_trips_through_the_handler() {
        let handler = MoodToolHandler;
        let result = handler.call(json!({ "action": "get" })).await.unwrap();
        assert_eq!(result["success"], json!(true));
    }

    /// NEGATIVO — `update` sem os dois campos é erro nomeado, nunca um valor parcial silencioso.
    #[tokio::test]
    async fn tool_update_without_both_fields_is_a_named_error() {
        let handler = MoodToolHandler;
        let err = handler.call(json!({ "action": "update", "valence": 0.5 })).await.unwrap_err();
        assert!(err.contains("requires both valence and arousal"), "got: {err}");
    }

    /// NEGATIVO — ação desconhecida nunca cai num braço silencioso.
    #[tokio::test]
    async fn tool_unknown_action_is_a_named_error() {
        let handler = MoodToolHandler;
        let err = handler.call(json!({ "action": "fly" })).await.unwrap_err();
        assert!(err.contains("ação desconhecida"), "got: {err}");
    }

    #[tokio::test]
    async fn tool_register_interaction_clamps_weight_outside_the_declared_range() {
        let handler = MoodToolHandler;
        let result = handler.call(json!({ "action": "register_interaction", "weight": 999.0 })).await.unwrap();
        assert_eq!(result["success"], json!(true));
    }

    /// `action=update` publica `MoodChanged` — mesmo comportamento de `mood.tool.ts`. Toma
    /// `TOOL_REGISTRATION_ORDER`: `super::rpc`'s tests também publicam neste MESMO barramento
    /// global (`mood_update`), e sem a trava um `MoodChanged` de outro teste concorrente poderia
    /// ser lido como se fosse este.
    #[tokio::test]
    async fn tool_update_publishes_mood_changed() {
        let _order = super::super::TOOL_REGISTRATION_ORDER.lock().unwrap_or_else(|e| e.into_inner());
        let handler = MoodToolHandler;
        let mut sub = bus().subscribe();

        handler.call(json!({ "action": "update", "valence": 0.5, "arousal": 0.2 })).await.unwrap();

        // `bus()` é global ao processo (ver o comentário do teste abaixo) — pula eventos de OUTROS
        // testes concorrentes até achar o `MoodChanged` desta chamada, sem esperar além de 1s.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        let envelope = loop {
            let envelope = tokio::time::timeout_at(deadline, sub.recv())
                .await
                .expect("MoodChanged não foi publicado")
                .expect("bus fechou antes de publicar (Closed)");
            if envelope.key == EventKey::MoodChanged {
                break envelope;
            }
        };
        assert_eq!(envelope.payload["valence"], json!(0.5));
    }

    /// NEGATIVO — `register_interaction` que NÃO cruza um nível não publica `MoodAffinityLevelUp`.
    /// `bus()` é um singleton de PROCESSO (`events/bus.rs`) compartilhado por TODOS os testes do
    /// binário, que o `cargo test` roda em paralelo por padrão — um `recv()` cru contaria eventos
    /// de OUTROS testes concorrentes (ex.: `tool_update_publishes_mood_changed`) como falso
    /// positivo. Drena a janela de 50ms e só falha se a chave MoodAffinityLevelUp específica
    /// aparecer, nunca por ruído alheio no mesmo barramento. Toma `TOOL_REGISTRATION_ORDER` pelo
    /// mesmo motivo: `super::rpc`'s `affinity_add_interaction` PUBLICA essa chave de propósito num
    /// dos próprios testes — sem a trava, aquela publicação real cairia nesta janela de 50ms.
    #[tokio::test]
    async fn tool_register_interaction_only_publishes_level_up_when_it_crosses_a_boundary() {
        let _order = super::super::TOOL_REGISTRATION_ORDER.lock().unwrap_or_else(|e| e.into_inner());
        let handler = MoodToolHandler;
        let mut sub = bus().subscribe();

        // Ganho pequeno o bastante para não cruzar nível nenhum a partir de zero.
        handler.call(json!({ "action": "register_interaction", "weight": 0.5 })).await.unwrap();

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(50);
        loop {
            match tokio::time::timeout_at(deadline, sub.recv()).await {
                Ok(Some(envelope)) => {
                    assert_ne!(envelope.key, EventKey::MoodAffinityLevelUp, "publicou MoodAffinityLevelUp sem cruzar nível");
                }
                _ => break,
            }
        }
    }

    #[test]
    fn register_tools_adds_mood_to_the_process_wide_registry() {
        let _order = super::super::TOOL_REGISTRATION_ORDER.lock().unwrap_or_else(|e| e.into_inner());
        register_tools();
        let definition = crate::modules::tools::registry().get("mood").expect("mood tool não registrada");
        assert_eq!(definition.plugin_id, "mika.mood");
        assert!(!definition.requires_window);
    }
}
