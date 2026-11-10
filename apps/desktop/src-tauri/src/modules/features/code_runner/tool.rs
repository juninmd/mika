//! Tool `code` (LLM), action `run_js` — registrada no `ToolRegistry` nativo
//! (`crate::modules::tools`). Ver o cabeçalho de `mod.rs` para por que ela roda sem aprovação e
//! `sandbox.rs` para os limites que a tornam segura mesmo assim.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::sandbox::{run_js, RunJsOutput};

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "code".into(),
        description: Some(
            "Runs JavaScript for real in a sandbox (no filesystem, network, `require`, `fetch`, `process` or shell; runaway loops stop at ~3 s). Use when the user asks to run, test or calculate with code — not to just describe it. Console output and the last expression come back as data. Show the exact code you ran in a fenced block next to the result (speech skips it). The app shows its own approval dialog, so call directly — never ask for permission in text."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["run_js"] },
                "code": { "type": "string", "description": "JavaScript source; the last expression is returned." },
            },
            "required": ["action", "code"],
        }),
        plugin_id: "mika.code".into(),
        requires_window: false,
    }
}

struct CodeToolHandler;

impl RunJsOutput {
    fn into_json(self) -> Value {
        json!({
            "success": true,
            "data": {
                "stdout": self.stdout,
                "result": self.result,
                "error": self.error,
                "durationMs": self.duration_ms,
            }
        })
    }
}

#[async_trait]
impl ToolHandler for CodeToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        match action {
            "run_js" => {
                let code = args.get("code").and_then(Value::as_str).unwrap_or_default().to_string();
                // `run_js` bloqueia (thread dedicada + `recv_timeout` de até ~3s) — despachada via
                // `spawn_blocking` para não travar o worker async que também resolve `mika_invoke`
                // (mesma razão de `tokio::task::spawn_blocking` em `inference/vad.rs`).
                let outcome = tokio::task::spawn_blocking(move || run_js(&code))
                    .await
                    .map_err(|join_err| format!("code: execução cancelada: {join_err}"))?;
                Ok(outcome.into_json())
            }
            other => Err(format!("code: ação desconhecida \"{other}\"")),
        }
    }
}

/// Config-livre — efeito colateral de `features::modules()` (`features/mod.rs`), mesma primeira
/// passada de `mood`/`desktop` (`process`/`window`). Sem `AppHandle`/`ConfigStore` nenhum: não há
/// segunda passada para este módulo.
///
/// `register_destructive`, não `register`: ver o cabeçalho de `mod.rs` — `boa_engine` 0.22 não tem
/// teto de memória (só de iteração/recursão/pilha da VM): pânico do motor morre na task (release é
/// `unwind`), mas estouro de memória aborta o processo, então um script hostil ou só quebrado pode
/// custar o processo inteiro da Mika, não só a thread da sandbox. Até isso ter uma correção de
/// arquitetura (processo filho com limite de memória do SO), o gate de aprovação é a única defesa
/// entre o usuário e esse risco.
pub(crate) fn register_tools() {
    crate::modules::tools::registry().register_destructive(tool_definition(), Arc::new(CodeToolHandler));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_js_action_dispatches_for_real() {
        let handler = CodeToolHandler;
        let result = handler.call(json!({ "action": "run_js", "code": "1 + 1" })).await.unwrap();
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["result"], json!("2"));
        assert_eq!(result["data"]["error"], Value::Null);
    }

    #[tokio::test]
    async fn unknown_action_is_a_named_error() {
        let handler = CodeToolHandler;
        let result = handler.call(json!({ "action": "eval_python" })).await;
        assert!(result.is_err());
    }

    /// `register_tools` tem de usar `register_destructive`, não `register`: sem teto de memória no
    /// `boa_engine` (ver o cabeçalho de `mod.rs`) e com OOM abortando o processo mesmo em `unwind`,
    /// um script hostil pode custar o processo inteiro da Mika — o gate de aprovação é a única
    /// defesa até isso ganhar uma correção de arquitetura (processo filho com limite de memória).
    #[test]
    fn register_tools_marks_code_as_destructive() {
        register_tools();
        assert!(crate::modules::tools::registry().is_destructive_tool("code"));
    }

    #[tokio::test]
    async fn tool_definition_declares_run_js_only() {
        let def = tool_definition();
        assert_eq!(def.input_schema["properties"]["action"]["enum"], json!(["run_js"]));
        assert!(!def.requires_window);
    }

    /// Smoke 2026-09-26: "the user approves each run" became a "Posso executar?" reply with no call.
    #[test]
    fn description_says_the_app_dialog_is_the_approval_so_the_model_calls_directly() {
        let description = tool_definition().description.unwrap();
        assert!(description.contains("never ask for permission in text"), "{description}");
    }
}
