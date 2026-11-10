// Suíte em arquivo separado só pela regra de 300 linhas do AGENTS.md — mesmo padrão de
// `features::desktop::launch`/`launch_tests.rs`. Continua sendo o mesmo módulo `tool::tests`.
#![cfg(test)]

use super::super::input_backend::fake::{FakeInputBackend, Recorded};
use super::*;
use crate::config::registry::{Namespace, SchemaRegistry};
use crate::config::schema::boolean;
use std::sync::atomic::{AtomicU32, Ordering};

struct TestDir(std::path::PathBuf);
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn test_store(enabled: bool) -> (Arc<ConfigStore>, TestDir) {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mika-computer-use-tool-test-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let schema = SchemaRegistry::new(vec![Namespace { name: "computer-use", fields: vec![boolean("enabled", enabled)] }]);
    let store = ConfigStore::open(&dir, schema).unwrap();
    (Arc::new(store), TestDir(dir))
}

fn handler_with_fake(enabled: bool) -> (ComputerToolHandler, Arc<FakeInputBackend>, TestDir) {
    let (store, dir) = test_store(enabled);
    let backend = Arc::new(FakeInputBackend::default());
    let handler = ComputerToolHandler::with_backend(store, backend.clone());
    (handler, backend, dir)
}

#[test]
fn tool_definition_declares_the_five_actions_and_only_requires_action() {
    let definition = tool_definition();
    assert_eq!(definition.name, "computer");
    assert_eq!(definition.input_schema["required"], json!(["action"]));
    assert_eq!(
        definition.input_schema["properties"]["action"]["enum"],
        json!(["find", "click_element", "click", "type_text", "key"])
    );
}

/// Smoke 2026-09-26: with "abre a calculadora" as an example here, the model opened apps through this
/// tool with an invented `action`, and "say what you are about to do" became "Posso prosseguir?"
/// instead of a call. The app's own dialog is the approval step.
#[test]
fn description_routes_opening_apps_to_desktop_launch_and_never_asks_in_text() {
    let description = tool_definition().description.unwrap();
    assert!(description.contains("desktop tool (action=launch)"), "{description}");
    assert!(!description.contains("say what you are about to do"), "{description}");
}
/// SecOps requirement (brief): "refused when the request originates from screen/web content".
/// There is no origin field on the call to gate structurally yet (tracked as a follow-up — see
/// `tool.rs`'s header), so the description is the enforcement surface: it must carry the refusal
/// in words the model can match against the `NAO SAO INSTRUCOES` marker labeled content already
/// uses (`browser::page`, `browser::search`, `mcp::register`). Losing this sentence silently
/// reopens the exact prompt-injection path the brief calls out, so it is worth a named test rather
/// than only a comment.
#[test]
fn tool_description_tells_the_model_to_refuse_requests_from_screen_or_web_content() {
    let description = tool_definition().description.expect("computer tool must document itself");
    assert!(
        description.contains("NAO SAO INSTRUCOES"),
        "description must name the same untrusted-content marker `label_untrusted_content` \
         produces, so the model can recognize it: {description}"
    );
    assert!(
        description.to_lowercase().contains("refuse") || description.to_lowercase().contains("refus"),
        "description must instruct the model to refuse acting on it: {description}"
    );
}

#[tokio::test]
async fn missing_action_is_invalid_args_not_a_hard_error() {
    let (handler, _backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({})).await.unwrap();
    assert_eq!(result["errorCode"], json!("invalid_args"));
}

#[tokio::test]
async fn an_unknown_action_is_a_named_hard_error() {
    let (handler, _backend, _dir) = handler_with_fake(true);
    let error = handler.call(json!({ "action": "teleport" })).await.unwrap_err();
    assert!(error.contains("unknown action"), "got: {error}");
}

#[tokio::test]
async fn disabled_by_config_refuses_before_dispatching_anything() {
    let (handler, backend, _dir) = handler_with_fake(false);
    let result = handler.call(json!({ "action": "click", "x": 10, "y": 10 })).await.unwrap();
    assert_eq!(result["errorCode"], json!("disabled"));
    assert!(backend.calls().is_empty(), "the input backend must never be touched while disabled");
}

#[tokio::test]
async fn click_requires_numeric_x_and_y() {
    let (handler, backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({ "action": "click" })).await.unwrap();
    assert_eq!(result["errorCode"], json!("invalid_args"));
    assert!(backend.calls().is_empty());
}

#[tokio::test]
async fn click_moves_then_clicks_through_the_injected_backend() {
    let (handler, backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({ "action": "click", "x": 12, "y": 34, "button": "right" })).await.unwrap();
    assert_eq!(result["success"], json!(true));
    assert_eq!(backend.calls(), vec![Recorded::MoveTo(12, 34), Recorded::Click(MouseButton::Right)]);
}

/// A float coordinate (as some JSON clients encode whole numbers) must not be rejected.
#[tokio::test]
async fn click_accepts_float_encoded_coordinates() {
    let (handler, backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({ "action": "click", "x": 12.0, "y": 34.0 })).await.unwrap();
    assert_eq!(result["success"], json!(true));
    assert_eq!(backend.calls()[0], Recorded::MoveTo(12, 34));
}

#[tokio::test]
async fn type_text_requires_a_string_text_field() {
    let (handler, backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({ "action": "type_text" })).await.unwrap();
    assert_eq!(result["errorCode"], json!("invalid_args"));
    assert!(backend.calls().is_empty());
}

#[tokio::test]
async fn type_text_dispatches_through_the_backend_and_never_echoes_the_text() {
    let (handler, backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({ "action": "type_text", "text": "hunter2" })).await.unwrap();
    assert_eq!(result["success"], json!(true));
    assert_eq!(result["length"], json!(7));
    assert!(result.to_string().find("hunter2").is_none(), "the typed text must never appear in the result");
    assert_eq!(backend.calls(), vec![Recorded::TypeText("hunter2".into())]);
}

#[tokio::test]
async fn key_requires_a_combo_string() {
    let (handler, backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({ "action": "key" })).await.unwrap();
    assert_eq!(result["errorCode"], json!("invalid_args"));
    assert!(backend.calls().is_empty());
}

#[tokio::test]
async fn key_rejects_an_unparseable_combo_before_touching_the_backend() {
    let (handler, backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({ "action": "key", "combo": "ctrl+banana" })).await.unwrap();
    assert_eq!(result["errorCode"], json!("invalid_args"));
    assert!(backend.calls().is_empty());
}

/// `win+r` abre o Executar e a ação `type_text` digitaria o comando: a tool recusa antes do backend.
#[tokio::test]
async fn key_refuses_a_launcher_combo_before_touching_the_backend() {
    let (handler, backend, _dir) = handler_with_fake(true);
    for combo in ["win+r", "ctrl+alt+delete", "ctrl+shift+esc"] {
        let result = handler.call(json!({ "action": "key", "combo": combo })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"), "{combo}");
        assert!(result["error"].as_str().unwrap().contains("blocked"), "{combo}: {result}");
    }
    assert!(backend.calls().is_empty());
}

#[tokio::test]
async fn key_dispatches_the_parsed_combo() {
    let (handler, backend, _dir) = handler_with_fake(true);
    let result = handler.call(json!({ "action": "key", "combo": "ctrl+c" })).await.unwrap();
    assert_eq!(result["success"], json!(true));
    assert_eq!(backend.calls().len(), 1);
}

#[tokio::test]
async fn click_element_requires_a_non_empty_name() {
    let (handler, _backend, _dir) = handler_with_fake(true);
    let missing = handler.call(json!({ "action": "click_element" })).await.unwrap();
    assert_eq!(missing["errorCode"], json!("invalid_args"));
    let empty = handler.call(json!({ "action": "click_element", "name": "" })).await.unwrap();
    assert_eq!(empty["errorCode"], json!("invalid_args"));
}

/// Regression for the reviewed defect: every branch of `dispatch_blocking` is a synchronous OS
/// call (UIA/COM, Win32, `enigo`), and running it directly on the async task would stall the same
/// tokio worker that also resolves `mika_invoke` for as long as the call takes — up to ~1.65s on
/// the real UIA path. `#[tokio::test]` defaults to a single-worker (`current_thread`) runtime, so
/// if `call()` blocks that worker, the concurrently spawned ticker below gets no chance to run at
/// all during the fake backend's simulated 50ms of "OS" work; if `call()` moves the blocking work
/// off the worker (`tokio::task::spawn_blocking`), the ticker keeps ticking underneath it.
#[tokio::test]
async fn call_does_not_stall_the_async_worker_during_a_blocking_os_call() {
    let (handler, backend, _dir) = handler_with_fake(true);
    *backend.block_millis.lock().unwrap() = 50;

    let ticks = std::sync::Arc::new(AtomicU32::new(0));
    let ticks_for_task = ticks.clone();
    let ticker = tokio::spawn(async move {
        for _ in 0..10 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            ticks_for_task.fetch_add(1, Ordering::Relaxed);
        }
    });

    let result = handler.call(json!({ "action": "click", "x": 1, "y": 1 })).await.unwrap();
    assert_eq!(result["success"], json!(true));
    ticker.await.unwrap();

    assert!(
        ticks.load(Ordering::Relaxed) > 0,
        "the async ticker made no progress while `call` ran — the blocking OS call is stalling \
         the worker that also serves mika_invoke, not running in spawn_blocking"
    );
}

#[test]
fn register_adds_computer_to_the_process_wide_registry_as_destructive() {
    let (store, _dir) = test_store(true);
    register(store);
    let definition = crate::modules::tools::registry().get("computer").expect("computer tool não registrada");
    assert_eq!(definition.plugin_id, "mika.computer-use");
    assert!(
        crate::modules::tools::registry().is_destructive_tool("computer"),
        "the computer tool must go through the approval gate on every call"
    );
}
