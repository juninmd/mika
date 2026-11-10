//! Tool `desktop` (LLM) — port de `apps/desktop/src/features/desktop/tools/index.ts`, registrada
//! no `ToolRegistry` nativo (`crate::modules::tools`). Ver o cabeçalho de `mod.rs` para a tabela de
//! paridade e para o porquê de `launch` precisar de uma segunda passada de registro.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::launch::run_launch;
use super::process::list_running_processes;
use super::window::list_open_windows;

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "desktop".into(),
        description: Some(
            "Acts on the user's computer WITHOUT seeing it (window titles and process names only; for what's visible use vision_capture/vision_read). action=launch: open an app or website. action=window: list open windows. action=process: heaviest processes by memory. Check the listings before answering something that depends on what they're doing. To click/type inside an app: launch it, then use computer."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["launch", "window", "process"] },
                "target": { "type": "string", "description": "action=launch: app name (e.g. \"calculator\", \"steam\") or an http(s) URL. Ignored by window/process." },
            },
            "required": ["action"],
        }),
        plugin_id: "mika.desktop".into(),
        requires_window: false,
    }
}

/// `config: None` na primeira passada de registro ([`register_tools`], sem `AppHandle`):
/// `process`/`window` despacham de verdade desde o boot; `launch` degrada nomeado até
/// [`register`] (segunda passada) substituir este registro pelo mesmo padrão de
/// `person_catalog::register_stub` (`features::modules()` → `features::mod.rs`).
struct DesktopToolHandler {
    config: Option<Arc<ConfigStore>>,
}

fn launch_not_configured_yet() -> Value {
    json!({
        "success": false,
        "error": "action='launch' ainda não está disponível: o registro dependente de configuração ainda não rodou.",
        "errorCode": "unavailable",
    })
}

#[async_trait]
impl ToolHandler for DesktopToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        match action {
            "process" => Ok(list_running_processes()),
            "window" => Ok(list_open_windows()),
            "launch" => match &self.config {
                Some(config) => Ok(run_launch(args.get("target").and_then(Value::as_str), config).await),
                None => Ok(launch_not_configured_yet()),
            },
            other => Err(format!("desktop: ação desconhecida \"{other}\"")),
        }
    }
}

/// Primeira passada — efeito colateral de `features::modules()`, ANTES do `AppHandle` existir
/// (mesmo ponto de hoje). `process`/`window` já despacham de verdade aqui. `register_stub` (não
/// `register`): marca o nome como substituição pretendida para a segunda passada, em vez de uma
/// colisão de nome comum.
pub(crate) fn register_tools() {
    crate::modules::tools::registry().register_stub(tool_definition(), Arc::new(DesktopToolHandler { config: None }));
}

/// Segunda passada — chamada por `modules::register_config_bound_tools(app)` (`modules/mod.rs`,
/// fora do meu escopo editar; linha nomeada no relatório desta rodada). SUBSTITUI o registro da
/// primeira passada: mesmo nome de tool, `ToolRegistry::register` sobrescreve o stub (mesmo padrão
/// de `person_catalog::register_tools(app)` sobre `person_catalog::register_stub()`).
///
/// `#[allow(dead_code)]`: sem chamador em produção ATÉ a linha de boot (item 2 do cabeçalho de
/// `mod.rs`) ser adicionada por quem é dono de `modules/mod.rs` — mesmo estado transitório que
/// `launch_target::resolve_launch_target` tinha antes desta rodada.
#[allow(dead_code)]
pub(crate) fn register(store: Arc<ConfigStore>) {
    super::habits::register(Arc::clone(&store));
    crate::modules::tools::registry().register(tool_definition(), Arc::new(DesktopToolHandler { config: Some(store) }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tool_process_action_returns_a_real_snapshot() {
        let handler = DesktopToolHandler { config: None };
        let result = handler.call(json!({ "action": "process" })).await.unwrap();
        assert_eq!(result["success"], json!(true));
    }

    #[tokio::test]
    async fn tool_window_action_dispatches_for_real() {
        let handler = DesktopToolHandler { config: None };
        let result = handler.call(json!({ "action": "window" })).await.unwrap();
        // Ambiente sem sessão gráfica pode falhar a enumeração — o que este teste prova é que a
        // ação DESPACHA (nunca mais o `Err` fixo antigo), não que o ambiente de CI tem janelas.
        assert!(result["success"] == json!(true) || result["errorCode"] == json!("internal"));
    }

    /// NEGATIVO — sem a segunda passada de registro, `launch` degrada nomeado, nunca abre nada.
    #[tokio::test]
    async fn tool_launch_action_without_config_degrades_named_and_opens_nothing() {
        let handler = DesktopToolHandler { config: None };
        let result = handler.call(json!({ "action": "launch", "target": "calc" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("unavailable"));
    }

    #[tokio::test]
    async fn tool_unknown_action_is_a_named_error() {
        let handler = DesktopToolHandler { config: None };
        let err = handler.call(json!({ "action": "fly" })).await.unwrap_err();
        assert!(err.contains("ação desconhecida"), "got: {err}");
    }

    #[test]
    fn register_tools_adds_desktop_to_the_process_wide_registry() {
        register_tools();
        let definition = crate::modules::tools::registry().get("desktop").expect("desktop tool não registrada");
        assert_eq!(definition.plugin_id, "mika.desktop");
    }

    /// A segunda passada troca `config: None` por `config: Some(store)` no MESMO tipo de handler:
    /// `launch` passa a despachar de verdade (barrado por uma allowlist real, não mais pelo
    /// `errorCode: unavailable` fixo da primeira passada). Chama o handler direto — não pelo
    /// `ToolRegistry` de processo, compartilhado com outros testes deste arquivo em paralelo —
    /// para provar o comportamento sem correr risco de outro teste sobrescrever o registro no meio.
    #[tokio::test]
    async fn handler_with_config_unblocks_launch_validation() {
        let (store, _dir) = test_privacy_store();
        let handler = DesktopToolHandler { config: Some(Arc::new(store)) };

        let result = handler.call(json!({ "action": "launch", "target": "bash" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"), "esperava a allowlist real, não mais 'unavailable'");
    }

    /// `register(store)` de fato registra no `ToolRegistry` de processo (mesmo `plugin_id` da
    /// primeira passada — as duas passadas registram o MESMO nome de tool, então checar só o
    /// `plugin_id` é seguro mesmo com outro teste deste arquivo registrando em paralelo).
    #[test]
    fn register_adds_the_tool_to_the_process_wide_registry() {
        let (store, _dir) = test_privacy_store();
        register(Arc::new(store));
        let definition = crate::modules::tools::registry().get("desktop").expect("desktop tool não registrada");
        assert_eq!(definition.plugin_id, "mika.desktop");
    }

    fn test_privacy_store() -> (ConfigStore, TestDir) {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::boolean;
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mika-desktop-tool-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", false)] }]);
        let store = ConfigStore::open(&dir, schema).unwrap();
        (store, TestDir(dir))
    }

    struct TestDir(std::path::PathBuf);
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
