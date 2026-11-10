//! Testes de `read_tool.rs` — extraídos para caber no teto de 300 linhas por arquivo (mesmo
//! padrão de `llm_client.rs`/`llm_client_tests.rs`).

use super::*;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-vision-read-tool-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Mesmo padrão de `modules/gamer/llm_dead_port.rs`: porta sem listener, nunca o Ollama real da
/// máquina — o teste prova o CAMINHO de despacho, não a resposta de um modelo específico.
const DEAD_PORT_URL: &str = "http://127.0.0.1:11500";

fn store_with_vision_enabled() -> (ConfigStore, TempDir) {
    let dir = TempDir::new();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    store.set("vision.enabled", json!(true)).unwrap();
    store.set("apiKeys.ollamaBaseUrl", json!(DEAD_PORT_URL)).unwrap();
    (store, dir)
}

// -- describe: não depende de captura de tela — prova de despacho determinística -------------

#[tokio::test]
async fn describe_is_disabled_result_when_vision_toggle_is_off() {
    let dir = TempDir::new();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    let handler = VisionReadHandler::new(Arc::new(store), None);

    let result = handler.call(json!({ "action": "describe", "imageBase64": "ZmFrZQ==" })).await.unwrap();
    assert_eq!(result["errorCode"], json!("not_configured"));
}

#[tokio::test]
async fn describe_rejects_missing_image_as_invalid_args() {
    let (store, _dir) = store_with_vision_enabled();
    let handler = VisionReadHandler::new(Arc::new(store), None);

    let result = handler.call(json!({ "action": "describe" })).await.unwrap();
    assert_eq!(result["errorCode"], json!("invalid_args"));
}

/// PROVA DE DESPACHO REAL: com o gate liberado e uma imagem de verdade, a tool tenta a LLM —
/// a porta morta faz a chamada falhar com um erro de PROVEDOR nomeado, nunca mais o antigo
/// stub "esta ação precisa de um modelo... que o backend Rust ainda não tem".
#[tokio::test]
async fn describe_reaches_the_llm_client_and_reports_a_named_provider_error_on_a_dead_port() {
    let (store, _dir) = store_with_vision_enabled();
    let handler = VisionReadHandler::new(Arc::new(store), None);
    // Raw base64 1x1 PNG: the size cap now decodes the image, so it has to be a real one.
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
    let result = handler.call(json!({ "action": "describe", "imageBase64": png })).await.unwrap();
    assert_eq!(result["success"], json!(false));
    assert_eq!(result["errorCode"], json!("internal"));
    let error = result["error"].as_str().unwrap_or_default();
    assert!(error.contains("erro do provedor de visão"), "got: {error}");
    assert!(!error.contains("ainda não tem"), "resposta ainda é o stub antigo: {error}");
}

#[tokio::test]
async fn handler_rejects_an_unknown_action_as_a_named_error() {
    let (store, _dir) = store_with_vision_enabled();
    let handler = VisionReadHandler::new(Arc::new(store), None);

    let err = handler.call(json!({ "action": "teleport" })).await.unwrap_err();
    assert!(err.contains("ação desconhecida"), "got: {err}");
}

// -- canonical_action (smoke item 7: Gemma sent an action outside the enum) ---------------------

#[test]
fn canonical_action_maps_known_synonyms_to_the_enum_value() {
    for alias in ["read", "text", "transcribe"] {
        assert_eq!(canonical_action(alias), "ocr", "alias: {alias}");
    }
    for alias in ["look", "see", "image", "analyze", "analyse"] {
        assert_eq!(canonical_action(alias), "describe", "alias: {alias}");
    }
    for alias in ["who", "identify", "person", "character"] {
        assert_eq!(canonical_action(alias), "people", "alias: {alias}");
    }
}

#[test]
fn canonical_action_passes_through_already_canonical_values_and_unknown_ones() {
    for canonical in ["describe", "ocr", "people"] {
        assert_eq!(canonical_action(canonical), canonical);
    }
    assert_eq!(canonical_action("teleport"), "teleport", "não mapeado, sobra pro match de despacho recusar");
}

/// PROVA end-to-end: um alias chega no handler igual chegaria da LLM, e resulta no MESMO
/// desfecho do valor canônico.
#[tokio::test]
async fn handler_dispatches_an_alias_action_the_same_as_its_canonical_value() {
    let dir = TempDir::new();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    let handler = VisionReadHandler::new(Arc::new(store), None);

    let aliased = handler.call(json!({ "action": "look", "imageBase64": "ZmFrZQ==" })).await.unwrap();
    let canonical = handler.call(json!({ "action": "describe", "imageBase64": "ZmFrZQ==" })).await.unwrap();
    assert_eq!(aliased, canonical);
    assert_eq!(aliased["errorCode"], json!("not_configured"));
}

// -- untrusted-content labeling (SecOps parity with `BUSCA_WEB`) --------------------------------

/// `describe`/`ocr` não são testáveis sem rede real no caminho de sucesso (mesma limitação
/// documentada em `search/tests.rs::snippet_is_labeled_as_untrusted_web_content`) — pinamos que
/// o texto do modelo passa por `label_screen_output` lendo o próprio fonte de produção.
#[test]
fn describe_and_ocr_label_their_output_as_untrusted_screen_content() {
    let source = include_str!("read_tool.rs");
    let production = source.split("#[cfg(test)]").next().unwrap_or_default();
    assert!(production.contains("\"analysis\": label_screen_output(&outcome.text)"), "describe não rotula a análise como não-confiável");
    assert!(production.contains("\"text\": label_screen_output(&outcome.text)"), "ocr não rotula o texto como não-confiável");
}

// -- images_from_args ---------------------------------------------------------------------------

#[test]
fn images_from_args_prefers_the_images_array_over_a_single_image_base64() {
    let args = json!({ "imageBase64": "single", "images": [{ "imageBase64": "a" }, { "imageBase64": "b" }] });
    assert_eq!(images_from_args(&args), vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn images_from_args_falls_back_to_a_single_image_base64() {
    let args = json!({ "imageBase64": " single " });
    assert_eq!(images_from_args(&args), vec!["single".to_string()]);
}

#[test]
fn images_from_args_is_empty_without_any_image() {
    assert!(images_from_args(&json!({})).is_empty());
}
