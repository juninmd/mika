//! Suíte de `registry.rs`, separada só pela regra de 300 linhas — ver o comentário no ponto de
//! declaração (`super::tests`). Mesmo módulo lógico, `use super::*` continua enxergando tudo que
//! `mod.rs` deste diretório declara como privado.

use super::*;
use super::super::approval::{ApprovalDecision, ApprovalScope};
use serde_json::json;

struct Echo;

#[async_trait]
impl ToolHandler for Echo {
    async fn call(&self, args: Value) -> Result<Value, String> {
        Ok(json!({ "success": true, "echoed": args }))
    }
}

/// Prova que a validação roda ANTES do handler: se este handler chegar a executar em um teste
/// de rejeição, o teste falha por outro motivo (resultado errado), nunca por acaso passando.
struct PanicsIfCalled;

#[async_trait]
impl ToolHandler for PanicsIfCalled {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        panic!("handler não deveria ter sido chamado — validação deveria ter barrado antes")
    }
}

fn def(name: &str, plugin_id: &str, schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: None,
        input_schema: schema,
        plugin_id: plugin_id.into(),
        requires_window: false,
    }
}

#[tokio::test]
async fn dispatches_to_the_registered_handler() {
    let registry = ToolRegistry::new();
    let schema = json!({ "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] });
    registry.register(def("say_hi", "test", schema), Arc::new(Echo));

    let result = registry.execute("say_hi", json!({ "text": "oi" })).await.unwrap();
    assert_eq!(result, json!({ "success": true, "echoed": { "text": "oi" } }));
}

#[tokio::test]
async fn unknown_tool_is_a_named_error_not_a_panic() {
    let registry = ToolRegistry::new();
    let err = registry.execute("does_not_exist", json!({})).await.unwrap_err();
    assert!(err.contains("Tool not found: does_not_exist"), "got: {err}");
}

/// The error is the model's only map of the catalog: a hallucinated name gets the real ones back.
#[tokio::test]
async fn unknown_tool_error_lists_the_available_tools() {
    let registry = ToolRegistry::new();
    registry.register(def("say_hi", "test", json!({})), Arc::new(Echo));
    let err = registry.execute("sai_hi", json!({})).await.unwrap_err();
    assert!(err.contains("Available tools: say_hi"), "got: {err}");
}

/// A no-arg call sent as `null` and a number sent as a string reach the handler repaired.
#[tokio::test]
async fn repairs_sloppy_model_arguments_before_validation_and_dispatch() {
    let registry = ToolRegistry::new();
    let schema = json!({ "type": "object", "properties": { "n": { "type": "integer" } }, "required": ["n"] });
    registry.register(def("count", "test", schema), Arc::new(Echo));

    let result = registry.execute("count", json!({ "n": "3" })).await.unwrap();
    assert_eq!(result["echoed"], json!({ "n": 3 }));
    let no_args = registry.execute("count", json!(null)).await.unwrap_err();
    assert!(no_args.contains("missing required field"), "null vira {{}} e cai na validação normal: {no_args}");
}

/// Prova a integração com `validation.rs`: rejeição de schema acontece antes de o handler ser
/// chamado, não só que `validate_args` isoladamente rejeita (isso já está em `validation.rs`).
#[tokio::test]
async fn rejects_invalid_args_before_dispatch() {
    let registry = ToolRegistry::new();
    let schema = json!({ "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] });
    registry.register(def("needs_text", "test", schema), Arc::new(PanicsIfCalled));

    let err = registry.execute("needs_text", json!({})).await.unwrap_err();
    assert!(err.contains("missing required field"), "got: {err}");
}

#[test]
fn normalizes_names_like_the_ts_registry() {
    assert_eq!(ToolRegistry::normalize_name("My Tool!"), "My_Tool");
    assert_eq!(ToolRegistry::normalize_name("___"), "tool");
    assert_eq!(ToolRegistry::normalize_name(""), "tool");
    assert_eq!(ToolRegistry::normalize_name("already_ok"), "already_ok");
}

#[tokio::test]
async fn get_and_get_all_reflect_registered_tools() {
    let registry = ToolRegistry::new();
    registry.register(
        ToolDefinition {
            name: "cat_tool".into(),
            description: Some("d".into()),
            input_schema: json!({ "type": "object" }),
            plugin_id: "p".into(),
            requires_window: true,
        },
        Arc::new(Echo),
    );

    // Nome normalizado bate mesmo consultando com uma forma "crua".
    assert!(registry.get("cat tool").is_some());

    let all = registry.get_all();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "cat_tool");
    assert_eq!(all[0].plugin_id, "p");
    assert!(all[0].requires_window);
    assert!(!all[0].destructive, "tool registrada via register() comum não é destructive");
}

/// Mesma política de `tool-registry.ts`: colisão de nome normalizado não é erro, é
/// substituição — o último `register` ganha.
#[tokio::test]
async fn re_registering_the_same_normalized_name_overwrites() {
    let registry = ToolRegistry::new();
    registry.register(def("dup", "a", json!({})), Arc::new(PanicsIfCalled));
    registry.register(def("dup", "b", json!({})), Arc::new(Echo));

    assert_eq!(registry.get_all().len(), 1);
    assert_eq!(registry.get("dup").unwrap().plugin_id, "b");

    let result = registry.execute("dup", json!({ "x": 1 })).await.unwrap();
    assert_eq!(result["success"], json!(true));
}

/// O par `register_stub` (primeira passada, sem `AppHandle`) → `register` (segunda passada, com
/// `AppHandle`/`ConfigStore`) que `mood`/`desktop`/`person_catalog` usam para trocar um handler
/// config-livre pelo de verdade assim que o boot chega em `register_config_bound_tools` — ver o
/// cabeçalho de `ToolRegistry::stub_names`. Isso é uma substituição PRETENDIDA: não deve aparecer
/// em `unexpected_collisions()`.
#[tokio::test]
async fn stub_then_real_registration_is_not_an_unexpected_collision() {
    let registry = ToolRegistry::new();
    registry.register_stub(def("mood", "features.mood", json!({})), Arc::new(PanicsIfCalled));
    registry.register(def("mood", "features.mood", json!({})), Arc::new(Echo));

    assert_eq!(registry.get("mood").unwrap().plugin_id, "features.mood", "o handler de verdade tem de vencer");
    assert!(registry.unexpected_collisions().is_empty(), "stub→real não é colisão inesperada");

    // O handler ativo é mesmo o segundo (`Echo`), não o stub que panicaria se chamado.
    let result = registry.execute("mood", json!({})).await.unwrap();
    assert_eq!(result["success"], json!(true));
}

/// A condição que a suíte precisa PEGAR de verdade: dois módulos DISTINTOS registrando o mesmo
/// nome por `register()` comum (nenhum dos dois passou por `register_stub`) é uma colisão real —
/// tem de aparecer em `unexpected_collisions()`, não só sumir num `log::warn!` que ninguém lê.
#[tokio::test]
async fn two_unrelated_registrations_of_the_same_name_is_an_unexpected_collision() {
    let registry = ToolRegistry::new();
    registry.register(def("dup", "plugin.a", json!({})), Arc::new(Echo));
    registry.register(def("dup", "plugin.b", json!({})), Arc::new(Echo));

    assert_eq!(registry.unexpected_collisions(), vec!["dup".to_string()]);
}

/// Mesma prova para `register_stub` colidindo com um nome já usado por uma tool real — o outro
/// lado do par, também precisa contar.
#[tokio::test]
async fn register_stub_colliding_with_a_real_tool_is_also_an_unexpected_collision() {
    let registry = ToolRegistry::new();
    registry.register(def("dup", "plugin.a", json!({})), Arc::new(Echo));
    registry.register_stub(def("dup", "plugin.b", json!({})), Arc::new(Echo));

    assert_eq!(registry.unexpected_collisions(), vec!["dup".to_string()]);
}

/// `unexpected_collisions()` já provava a colisão internamente, mas nada fora de `ToolRegistry`
/// conseguia ver — `modules::all()` (primeira passada, sem `AppHandle`) chama `register()` direto
/// nos módulos já portados, e nenhum deles retorna `Result`. `feature_health` é o mesmo mecanismo
/// que `register_config_bound_tools` (segunda passada) já usa para virar `bootstrap.degraded`; uma
/// colisão real de nome de tool tem de chegar lá também, não só ficar presa neste `Vec` privado.
///
/// Roda em thread isolada como o resto da suíte de `feature_health` (`OnceLock` de processo) — ver
/// o cabeçalho de `feature_health.rs`.
#[tokio::test]
async fn unexpected_collision_reaches_feature_health() {
    let registry = ToolRegistry::new();
    // Counts only entries naming this tool: the list is process-wide and other tests running in
    // parallel append to it, so a total-length delta (and `last()`) raced and flaked.
    let mine = || crate::feature_health::registration_failures().iter().filter(|f| f.contains("dup_health")).count();
    let before = mine();

    registry.register(def("dup_health", "plugin.a", json!({})), Arc::new(Echo));
    registry.register(def("dup_health", "plugin.b", json!({})), Arc::new(Echo));

    assert_eq!(mine(), before + 1, "a colisão da primeira passada tem de virar 1 falha em feature_health, nomeando a tool");
}

/// `registry()` é o singleton por processo que os módulos já portados usam para se cadastrar
/// — prova só que ele existe e é estável entre chamadas, sem reivindicar canal nenhum.
#[test]
fn process_wide_registry_is_a_stable_singleton() {
    assert!(std::ptr::eq(registry(), registry()));
}

struct AlwaysApproves;
#[async_trait]
impl ApprovalHandler for AlwaysApproves {
    async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
        Ok(ApprovalDecision { approved: true, scope: ApprovalScope::Once })
    }
}

/// O caso que mais importa para o veto do SecOps: tool registrada como `destructive` sem
/// handler de aprovação configurado NUNCA chega no handler real — nega como resultado de
/// negócio, mesma forma que `ToolRegistryService.execute` no TS.
#[tokio::test]
async fn destructive_tool_denies_without_an_approval_handler_and_never_calls_the_real_handler() {
    let registry = ToolRegistry::new();
    registry.register_destructive(def("delete_everything", "test", json!({})), Arc::new(PanicsIfCalled));

    let result = registry.execute("delete_everything", json!({})).await.unwrap();
    assert_eq!(result, json!({ "success": false, "error": "user_denied" }));
}

/// Com um handler de aprovação real instalado (`set_approval_handler`) que aprova, a tool
/// destrutiva executa de verdade — prova o caminho feliz do mesmo gate.
#[tokio::test]
async fn destructive_tool_executes_once_the_installed_handler_approves() {
    let registry = ToolRegistry::new();
    registry.register_destructive(def("delete_everything", "test", json!({})), Arc::new(Echo));
    registry.set_approval_handler(Arc::new(AlwaysApproves));

    let result = registry.execute("delete_everything", json!({ "x": 1 })).await.unwrap();
    assert_eq!(result, json!({ "success": true, "echoed": { "x": 1 } }));
}

struct AlwaysDenies;
#[async_trait]
impl ApprovalHandler for AlwaysDenies {
    async fn decide(&self, _request: ApprovalRequest) -> Result<ApprovalDecision, String> {
        Ok(ApprovalDecision { approved: false, scope: ApprovalScope::Once })
    }
}

/// O outro caso que o veto do SecOps exige, com um handler REAL instalado (não a ausência de
/// um): decisão explícita de negação também nunca chega no handler da tool. Não basta provar
/// "sem handler nega" — um handler instalado que nega tem de ter a MESMA garantia.
#[tokio::test]
async fn destructive_tool_denies_when_the_installed_handler_denies_and_never_calls_the_real_handler() {
    let registry = ToolRegistry::new();
    registry.register_destructive(def("delete_everything", "test", json!({})), Arc::new(PanicsIfCalled));
    registry.set_approval_handler(Arc::new(AlwaysDenies));

    let result = registry.execute("delete_everything", json!({})).await.unwrap();
    assert_eq!(result, json!({ "success": false, "error": "user_denied" }));
}

/// Tool não marcada `destructive` continua não gateada — o gate é opt-in por chamador de
/// `register_destructive`, nunca padrão para toda tool registrada.
#[tokio::test]
async fn non_destructive_tool_never_asks_for_approval() {
    let registry = ToolRegistry::new();
    registry.register(def("harmless", "test", json!({})), Arc::new(Echo));
    // Sem handler nenhum instalado — se isto passasse pelo gate, negaria por padrão.
    let result = registry.execute("harmless", json!({})).await.unwrap();
    assert_eq!(result["success"], json!(true));
}

/// Servidor MCP removido/reiniciado pela UI tira as próprias tools do catálogo: sem isto a tool
/// continuava em `mika:tools-list` apontando para um processo morto, e re-registrar o mesmo nome
/// depois de reiniciar contaria como colisão inesperada.
#[tokio::test]
async fn unregister_removes_the_tool_and_its_destructive_mark() {
    let registry = ToolRegistry::new();
    registry.register_destructive(def("mcp_fs_read", "mika.mcp", json!({ "type": "object" })), Arc::new(Echo));

    assert!(registry.unregister("mcp_fs_read"));

    assert!(registry.get("mcp_fs_read").is_none());
    assert!(!registry.is_destructive_tool("mcp_fs_read"));
    assert!(!registry.unregister("mcp_fs_read"), "segunda remoção não acha nada");
    registry.register_destructive(def("mcp_fs_read", "mika.mcp", json!({ "type": "object" })), Arc::new(Echo));
    assert!(registry.unexpected_collisions().is_empty());
}

struct Panics;

#[async_trait]
impl ToolHandler for Panics {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        panic!("boom {}", 42)
    }
}

/// O contrato de `execute` é "nunca panic": um handler que dá panic volta como `Err` nomeado e o
/// registro segue servindo as outras chamadas.
#[tokio::test]
async fn a_panicking_handler_becomes_a_named_error_and_the_registry_keeps_working() {
    let registry = ToolRegistry::new();
    registry.register(def("explodes", "test", json!({ "type": "object" })), Arc::new(Panics));
    registry.register(def("fine", "test", json!({ "type": "object" })), Arc::new(Echo));

    let err = registry.execute("explodes", json!({})).await.unwrap_err();
    assert!(err.starts_with("tool panicked:") && err.contains("boom 42"), "got: {err}");
    assert!(registry.execute("fine", json!({})).await.is_ok());
}
