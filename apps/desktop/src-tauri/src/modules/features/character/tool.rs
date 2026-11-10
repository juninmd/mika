//! Tool `avatar` (LLM) — port de `apps/desktop/src/features/character/tools/avatar.ts`.
//! Ver o cabeçalho de `mod.rs` para a tabela de paridade e as divergências documentadas.
//!
//! `action=speak` foi promovida de "degrada honesto" para PORTADA numa rodada anterior: a chave
//! `EventKey::SpeechRequest` (`"speech:request"`) passou a existir em `crate::events` e
//! `voice.ttsEnabled` já está no `SchemaRegistry` (`config/schemas/voice.rs`). Mesma checagem de
//! `CharacterService.speak`: `voice.ttsEnabled === false` recusa com `errorCode: "not_configured"`
//! ANTES de publicar; senão publica `EventKey::SpeechRequest` e devolve sucesso.
//!
//! `action=expression`/`action=inspect_model` fecham nesta rodada — ver o cabeçalho de `mod.rs`
//! para a medição completa de por que os dois bloqueios nomeados ali não se sustentavam mais.
//! `expression` despacha para `expression_action::run_expression` (arquivo próprio pelo teto de
//! 300 linhas), que publica `EventKey::Expression` (já existia, sem publicador) e agenda a
//! reversão para `neutral`, igual a `CharacterService.setExpression`/`scheduleRevert`;
//! `expression_bridge.rs` é quem encaminha isso à janela. `inspect_model` lê e parseia
//! `.model3.json`/`.vrm` do disco sem nenhuma crate nova — ver o cabeçalho de `inspect_model.rs`
//! para a medição.

use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::events::bus;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::affinity_action::run_affinity_delta;
use super::expression_action::run_expression;
use super::feature::{master_disabled_response, master_enabled};
use super::inspect_model::inspect_model;
use super::model_inventory::EXPRESSIONS;
use super::speak_action::run_speak;

const MODEL_NAME_CONFIG_PATH: &str = "character.modelName";
const MODEL_PATH_CONFIG_PATH: &str = "character.modelPath";

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "avatar".into(),
        description: Some(
            "Mika's avatar face and body. action=expression: play `expression` for `durationMs` — when asked to sit, read, dance, wave or emote, or for a big moment; for a line's ordinary mood the opening tag (<|NAME|>) is enough. action=inspect_model: what the loaded model can show. action=affinity_delta: nudge affinity toward the user (`delta`, positive = warmer, `reason`) after a meaningful moment. action=speak: rarely needed, your reply is already spoken."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["expression", "inspect_model", "speak", "affinity_delta"] },
                "expression": { "type": "string", "enum": EXPRESSIONS, "description": "Faces, poses (sitting, reading), gestures (wave, bow, clap, facepalm…), dances (floss, orange-justice, six-seven, groove) and stunts (senbonzakura = sword petal storm, throw-tomato)." },
                "durationMs": { "type": "number", "minimum": 500, "maximum": 30000, "description": "action=expression: ms before returning to neutral." },
                "text": { "type": "string", "minLength": 1, "maxLength": 500 },
                "delta": { "type": "number", "minimum": -20, "maximum": 20 },
                "reason": { "type": "string" },
            },
            "required": ["action"],
        }),
        plugin_id: "mika.character".into(),
        requires_window: false,
    }
}

struct AvatarToolHandler {
    store: Arc<ConfigStore>,
    /// Contador de geração do slot único de reversão de `expression` — ver `run_expression`.
    expression_revert_generation: Arc<AtomicU64>,
}

#[async_trait]
impl ToolHandler for AvatarToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        // Toggle mestre da feature (`character.enabled`) — a UI já prometia isto ("Turns off the
        // avatar and character tools", `settings.character.field.enabled.description`) mas nada no
        // backend lia o campo (varredura de controles de UI de 2026-09-27). Gateia as 4 ações da
        // tool `avatar`; a renderização do modelo Live2D/VRM em si é do renderer, fora deste
        // módulo — ver o cabeçalho desta rodada no relatório para a lacuna documentada.
        if !master_enabled(&self.store) {
            return Ok(master_disabled_response());
        }
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        match action {
            "affinity_delta" => match args.get("delta").and_then(Value::as_f64) {
                Some(delta) => {
                    let reason = args.get("reason").and_then(Value::as_str);
                    Ok(run_affinity_delta(&self.store, delta, reason))
                }
                None => Ok(json!({ "success": false, "error": "action='affinity_delta' requires delta.", "errorCode": "invalid_args" })),
            },
            "speak" => match args.get("text").and_then(Value::as_str) {
                Some(text) if !text.is_empty() => Ok(run_speak(bus(), &self.store, text)),
                _ => Ok(json!({ "success": false, "error": "action='speak' requires text.", "errorCode": "invalid_args" })),
            },
            "expression" => match args.get("expression").and_then(Value::as_str) {
                Some(expression) => {
                    let duration_ms = args.get("durationMs").and_then(Value::as_f64).unwrap_or(5000.0);
                    Ok(run_expression(bus(), &self.expression_revert_generation, expression, duration_ms))
                }
                None => Ok(json!({ "success": false, "error": "action='expression' requires expression.", "errorCode": "invalid_args" })),
            },
            "inspect_model" => {
                let model_name = self
                    .store
                    .get(MODEL_NAME_CONFIG_PATH)
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_else(|| "default".to_string());
                let model_path = self.store.get(MODEL_PATH_CONFIG_PATH).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
                match inspect_model(&model_name, &model_path, None) {
                    Ok(data) => Ok(json!({ "success": true, "data": data })),
                    Err(error) => Ok(json!({ "success": false, "error": error, "errorCode": "internal" })),
                }
            }
            other => Err(format!("avatar: ação desconhecida \"{other}\"")),
        }
    }
}

/// Chamado por `modules::register_config_bound_tools(app)`. Registra a tool `avatar` no
/// `ToolRegistry` nativo e a declaração de settings da aba `character` (3 grupos, ver
/// `super::feature`) em `crate::feature_registry::shared()`. Um registro só por `tabId`: isto
/// SUBSTITUI o `declare_namespace` de cartão único que vivia aqui — os dois nunca coexistem, ou o
/// segundo perderia em silêncio para o primeiro (mesma classe de defeito documentada no
/// cabeçalho de `feature_registry/mod.rs`). Também registra o teste de conexão de `mika:plugin-test`
/// (`super::plugin_test`, arquivo próprio pelo teto de 300 linhas).
pub fn register(store: Arc<ConfigStore>) {
    let handler = AvatarToolHandler { store: Arc::clone(&store), expression_revert_generation: Arc::new(AtomicU64::new(0)) };
    crate::modules::tools::registry().register(tool_definition(), Arc::new(handler));
    super::plugin_test::register();

    if let Some(settings) = super::feature::declaration(store.schema()) {
        crate::feature_registry::shared().register(
            "mika.character",
            crate::feature_registry::FeatureRegistration { enabled: true, settings: vec![settings], ..Default::default() },
        );
    } else {
        log::warn!("character: namespace 'character' ausente do SchemaRegistry — settings não declaradas");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::float_range;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-character-tool-test-{}-{unique}", std::process::id()));
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
            fields: vec![
                crate::config::schema::boolean("enabled", true),
                float_range("affinityScore", 0.0, -100.0, 100.0),
                crate::config::schema::text("modelName", "default"),
                crate::config::schema::text("modelPath", ""),
            ],
        }]);
        (Arc::new(ConfigStore::open(&dir.0, schema).unwrap()), dir)
    }

    fn handler(store: Arc<ConfigStore>) -> AvatarToolHandler {
        AvatarToolHandler { store, expression_revert_generation: Arc::new(AtomicU64::new(0)) }
    }

    /// Master toggle (`character.enabled=false`) — "Turns off the avatar and character tools"
    /// (i18n) had no backend reader before this round; every `avatar` action must now refuse.
    #[tokio::test]
    async fn call_is_denied_when_the_master_toggle_is_off() {
        let (store, _dir) = store();
        store.set("character.enabled", json!(false)).unwrap();
        let handler = handler(store);

        let result = handler.call(json!({ "action": "affinity_delta", "delta": 5.0 })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("denied"));
    }

    // `affinity_delta`'s scoring/clamping is covered in `affinity_action.rs`; the dispatch itself
    // is exercised end-to-end by `register_adds_the_tool_and_a_settings_declaration` and by the
    // shared `handler()`/`store()` helpers used across this file's other actions.
    #[tokio::test]
    async fn affinity_delta_without_delta_is_a_named_invalid_args_error() {
        let (store, _dir) = store();
        let handler = handler(store);

        let result = handler.call(json!({ "action": "affinity_delta" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    // `speak`'s tts-enabled gate and bus publish are covered in `speak_action.rs`; only the
    // dispatch-level "missing text" guard is exercised here.
    #[tokio::test]
    async fn speak_without_text_is_a_named_invalid_args_error() {
        let (store, _dir) = store();
        let handler = handler(store);

        let result = handler.call(json!({ "action": "speak" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    // -- expression --------------------------------------------------------------------------------
    // A cobertura de publicação/reversão/cancelamento em si mora em
    // `expression_action.rs` (bus isolado, sem depender do singleton de processo). Aqui só a
    // fiação do dispatch: leitura de argumento e o envelope que `run_expression` devolve.

    #[tokio::test]
    async fn expression_dispatches_to_run_expression_and_returns_its_shape() {
        let (store, _dir) = store();
        let handler = handler(store);

        let result = handler
            .call(json!({ "action": "expression", "expression": "sitting", "durationMs": 30_000 }))
            .await
            .unwrap();
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["expression"], json!("sitting"));
        assert_eq!(result["data"]["durationMs"], json!(30000.0));
    }

    #[tokio::test]
    async fn expression_without_expression_is_a_named_invalid_args_error() {
        let (store, _dir) = store();
        let handler = handler(store);

        let result = handler.call(json!({ "action": "expression" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    // -- inspect_model ------------------------------------------------------------------------------

    /// `inspect_model` real (sem `AppHandle`, sem fixture sintética) — resolve contra os ativos
    /// verdadeiros do avatar via `CARGO_MANIFEST_DIR` (ver `inspect_model.rs`). A cobertura de
    /// parsing/traversal em si mora em `inspect_model.rs`/`model_inventory.rs`; aqui só a fiação do
    /// dispatch da tool (leitura de config + envelope `success`/`data`/`errorCode`).
    #[tokio::test]
    async fn inspect_model_dispatches_for_real_against_the_configured_model() {
        let (store, _dir) = store();
        let handler = handler(store);

        let result = handler.call(json!({ "action": "inspect_model" })).await.unwrap();
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["kind"], json!("live2d"), "sem character.modelName/modelPath configurados, cai no demo Live2D");
        assert_eq!(result["data"]["supportedExpressions"].as_array().unwrap().len(), EXPRESSIONS.len());
    }

    /// NEGATIVO — um `modelPath` que não existe em disco nomeia o erro (`errorCode: "internal"`),
    /// igual ao `catch` de `avatar.ts::execute` em volta de `CharacterService.inspectLive2DModel`,
    /// nunca finge sucesso.
    #[tokio::test]
    async fn inspect_model_fails_named_when_the_configured_model_path_does_not_exist() {
        let (store, _dir) = store();
        store.set("character.modelPath", json!("live2d/does-not-exist/missing.model3.json")).unwrap();
        let handler = handler(store);

        let result = handler.call(json!({ "action": "inspect_model" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("internal"));
    }

    #[tokio::test]
    async fn register_adds_the_tool_and_a_settings_declaration() {
        let (store, _dir) = store();
        register(store);

        let definition = crate::modules::tools::registry().get("avatar").expect("avatar não registrada");
        assert_eq!(definition.plugin_id, "mika.character");

        let feature = crate::feature_registry::shared().get(&crate::feature_registry::FeatureId::from("mika.character"));
        let feature = feature.expect("settings de 'character' não registradas em feature_registry::shared()");
        // Prova que o registro usa `super::feature::declaration` (3 grupos), não mais o cartão
        // único de `declare_namespace` (1 grupo) — a troca do item 3 realmente tomou efeito.
        assert_eq!(feature.settings[0]["groups"].as_array().unwrap().len(), 3);
    }
}
