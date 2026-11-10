//! Testes de `capture_tool.rs` — extraídos para caber no teto de 300 linhas por arquivo (mesmo
//! padrão de `llm_client.rs`/`llm_client_tests.rs`).

use super::*;
use std::path::PathBuf;

/// Mesmo padrão de `privacy.rs`/`telemetry.rs`: diretório isolado por teste, sem trazer
/// dependência só para isso.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-vision-capture-tool-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store_with(enabled: bool, screen_capture: bool, blocklist: Option<&str>) -> (ConfigStore, TempDir) {
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::{boolean, text};

    let dir = TempDir::new();
    let schema = SchemaRegistry::new(vec![
        Namespace { name: "vision", fields: vec![boolean("enabled", false), text("captureBlocklist", "")] },
        Namespace { name: "privacy", fields: vec![boolean("screenCapture", false)] },
    ]);
    let store = ConfigStore::open(&dir.0, schema).unwrap();
    store.set("vision.enabled", json!(enabled)).unwrap();
    store.set("privacy.screenCapture", json!(screen_capture)).unwrap();
    if let Some(list) = blocklist {
        store.set("vision.captureBlocklist", json!(list)).unwrap();
    }
    (store, dir)
}

// -- frame: os 3 fail-closed que não tocam xcap (config decide antes da captura) -------------

#[test]
fn frame_is_disabled_result_when_vision_toggle_is_off() {
    let (store, _dir) = store_with(false, true, None);
    assert_eq!(frame(&store, &json!({})), disabled_result());
}

/// NEGATIVO — o caminho que o chokepoint existe para negar: `vision.enabled=true` não basta,
/// `privacy.screenCapture` continua sendo quem decide.
#[test]
fn frame_refuses_capture_when_privacy_screen_capture_is_off() {
    let (store, _dir) = store_with(true, false, None);
    assert_eq!(frame(&store, &json!({})), capture_unavailable_result());
}

#[test]
fn frame_disabled_check_runs_before_the_privacy_check() {
    // Os dois desligados: a mensagem é a de "plugin desligado", não a de "captura vazia" —
    // prova a ORDEM, não só que os dois, isolados, recusam.
    let (store, _dir) = store_with(false, false, None);
    assert_eq!(frame(&store, &json!({})), disabled_result());
}

// -- canonical_action (smoke item 7: Gemma sent an action outside the enum) ---------------------

#[test]
fn canonical_action_maps_known_synonyms_to_the_enum_value() {
    for alias in ["capture", "look", "see", "photo", "picture", "screen"] {
        assert_eq!(canonical_action(alias), "screenshot", "alias: {alias}");
    }
    for alias in ["grab", "snapshot"] {
        assert_eq!(canonical_action(alias), "frame", "alias: {alias}");
    }
    for alias in ["react", "comment", "narrate"] {
        assert_eq!(canonical_action(alias), "watch_now", "alias: {alias}");
    }
}

#[test]
fn canonical_action_passes_through_already_canonical_values_and_unknown_ones() {
    for canonical in ["screenshot", "frame", "watch_now"] {
        assert_eq!(canonical_action(canonical), canonical);
    }
    assert_eq!(canonical_action("levitate"), "levitate", "não mapeado, sobra pro match de despacho recusar");
}

/// PROVA end-to-end: um alias chega no handler igual chegaria da LLM, e resulta no MESMO
/// desfecho do valor canônico — não só que a função pura mapeia certo.
#[tokio::test]
async fn handler_dispatches_an_alias_action_the_same_as_its_canonical_value() {
    let (store, _dir) = store_with(false, true, None);
    let handler = VisionCaptureHandler::new(Arc::new(store), None);

    let aliased = handler.call(json!({ "action": "look" })).await.unwrap();
    let canonical = handler.call(json!({ "action": "screenshot" })).await.unwrap();
    assert_eq!(aliased, canonical);
    assert_eq!(aliased, disabled_result());
}

// -- quality_from -------------------------------------------------------------------------------

#[test]
fn quality_from_defaults_to_eighty() {
    assert_eq!(quality_from(&json!({})), 80);
}

#[test]
fn quality_from_clamps_to_the_valid_range() {
    assert_eq!(quality_from(&json!({ "quality": 500 })), 100);
    assert_eq!(quality_from(&json!({ "quality": 0 })), 1);
    assert_eq!(quality_from(&json!({ "quality": 42 })), 42);
}

// -- any_visible_window_blocked (sem depender de `xcap` capturar de verdade) ------------------

#[test]
fn no_blocklist_configured_never_blocks() {
    let (store, _dir) = store_with(true, true, Some(""));
    assert!(!any_visible_window_blocked(&store));
}

// -- ToolHandler::call dispatch -----------------------------------------------------------------

#[tokio::test]
async fn handler_refuses_screenshot_and_watch_now_with_the_disabled_result_when_vision_is_off() {
    let (store, _dir) = store_with(false, true, None);
    let handler = VisionCaptureHandler::new(Arc::new(store), None);

    for action in ["screenshot", "watch_now"] {
        let result = handler.call(json!({ "action": action })).await.unwrap();
        assert_eq!(result, disabled_result());
    }
}

/// PROVA DE DESPACHO REAL: gate liberado, captura de tela de verdade (`xcap`, ambiente do
/// runner decide se acha monitor) e — se achar — uma tentativa real de chamar o Ollama contra a
/// PORTA MORTA (nunca o Ollama real da máquina, mesmo padrão de `modules/gamer/llm_dead_port.rs`
/// e de `read_tool.rs`). De qualquer forma o resultado nunca é mais o stub antigo
/// "esta ação precisa de um modelo... que o backend Rust ainda não tem".
#[tokio::test]
async fn handler_no_longer_returns_the_old_not_yet_supported_stub_for_screenshot_and_watch_now() {
    let dir = TempDir::new();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    store.set("vision.enabled", json!(true)).unwrap();
    store.set("privacy.screenCapture", json!(true)).unwrap();
    store.set("apiKeys.ollamaBaseUrl", json!("http://127.0.0.1:11500")).unwrap();
    let handler = VisionCaptureHandler::new(Arc::new(store), None);

    for action in ["screenshot", "watch_now"] {
        let result = handler.call(json!({ "action": action })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        let error = result["error"].as_str().unwrap_or_default();
        assert!(!error.contains("ainda não tem"), "action={action} ainda devolve o stub antigo: {error}");
        // Sem monitor no runner -> "Screen capture returned empty result" (gate); com monitor ->
        // "erro do provedor de visão" (porta morta) — as duas são despacho real, nenhuma é mock.
        assert!(
            error.contains("Screen capture returned empty result") || error.contains("erro do provedor de visão"),
            "action={action} error inesperado: {error}"
        );
    }
}

// -- untrusted-content labeling (SecOps parity with `BUSCA_WEB`) --------------------------------

/// `screenshot`/`watch_now` não são testáveis sem rede real no caminho de sucesso (mesma
/// limitação de `read_tool.rs`) — pinamos que o texto do modelo passa por `label_screen_output`
/// lendo o próprio fonte de produção.
#[test]
fn screenshot_and_watch_now_label_their_output_as_untrusted_screen_content() {
    let source = include_str!("capture_tool.rs");
    let production = source.split("#[cfg(test)]").next().unwrap_or_default();
    assert!(production.contains("label_screen_output(&outcome.text),"), "screenshot não rotula a análise como não-confiável");
    assert!(production.contains("response\": label_screen_output(&outcome.text)"), "watch_now não rotula a resposta como não-confiável");
}

#[tokio::test]
async fn handler_rejects_an_unknown_action_as_a_named_error() {
    let (store, _dir) = store_with(true, true, None);
    let handler = VisionCaptureHandler::new(Arc::new(store), None);

    let err = handler.call(json!({ "action": "levitate" })).await.unwrap_err();
    assert!(err.contains("ação desconhecida"), "got: {err}");
}
