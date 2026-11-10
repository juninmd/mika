//! Suíte de `rpc.rs`, em arquivo separado só pela regra de 300 linhas do AGENTS.md.

use super::*;
use serde_json::json;

const CONTRACT_TS: &str = include_str!("../../../../src/shared/plugin-rpc-contract.ts");

struct Echo;

#[async_trait]
impl RpcHandler for Echo {
    async fn call(&self, args: Value) -> Result<Value, String> {
        Ok(json!({ "echo": args }))
    }
}

struct Boom;

#[async_trait]
impl RpcHandler for Boom {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        Err("handler exploded".into())
    }
}

fn request(plugin_id: &str, method: &str) -> Value {
    json!({ "pluginId": plugin_id, "method": method, "args": { "x": 1 } })
}

/// Os literais de código de erro são o contrato: o renderer despacha por `error.code`
/// (`unwrapPluginRpcResponse`). Um literal inventado aqui chegaria como erro genérico, e a UI
/// perderia a diferença entre "o método não existe" e "você não pode chamar" — que é toda a
/// informação que esse campo carrega.
#[test]
fn every_error_code_literal_matches_the_typescript_contract() {
    for code in [
        RpcErrorCode::InvalidRequest,
        RpcErrorCode::Unauthorized,
        RpcErrorCode::HandlerMissing,
        RpcErrorCode::HandlerFailed,
    ] {
        assert!(
            CONTRACT_TS.contains(&format!("= '{}'", code.as_str())),
            "'{}' não existe em plugin-rpc-contract.ts — o renderer não saberia despachar esse erro",
            code.as_str()
        );
    }
}

/// A forma do envelope, nos dois casos. `success: false` com `error.code`/`error.message`, e
/// `success: true` com `data` — é o que os dois schemas Zod do contrato aceitam, e nada além.
#[test]
fn the_response_envelope_has_the_shape_both_zod_schemas_accept() {
    let ok = success(json!({ "a": 1 }));
    assert_eq!(ok["success"], json!(true));
    assert_eq!(ok["data"], json!({ "a": 1 }));

    let bad = failure(RpcErrorCode::HandlerMissing, "nope");
    assert_eq!(bad["success"], json!(false));
    assert_eq!(bad["error"]["code"], json!("handler-missing"));
    assert_eq!(bad["error"]["message"], json!("nope"));
}

/// Espelha o regex `^mika\.[a-z0-9.-]+$`. Maiúscula, prefixo errado e `mika.` pelado são
/// recusados — um id que o TS rejeitaria e o Rust aceitasse seria um handler alcançável só de um
/// lado, e o teste que os compara nunca veria.
#[test]
fn plugin_ids_follow_the_same_pattern_the_zod_schema_enforces() {
    assert!(CONTRACT_TS.contains(r"/^mika\.[a-z0-9.-]+$/"), "o regex de pluginId mudou no contrato TS");

    for valid in ["mika.mood", "mika.person-catalog", "mika.gamer-companion", "mika.a.b"] {
        assert!(is_valid_plugin_id(valid), "'{valid}' deveria ser aceito");
    }
    for invalid in ["mood", "mika.", "mika.Mood", "outro.mood", "", "mika.espaço aqui"] {
        assert!(!is_valid_plugin_id(invalid), "'{invalid}' deveria ser recusado");
    }
}

#[test]
fn method_names_follow_the_same_pattern_the_zod_schema_enforces() {
    assert!(CONTRACT_TS.contains(r"/^[A-Za-z0-9._:-]+$/"), "o regex de method mudou no contrato TS");

    for valid in ["mood_get", "steam:top-games", "person_catalog.list", "A1"] {
        assert!(is_valid_method(valid), "'{valid}' deveria ser aceito");
    }
    for invalid in ["", "com espaço", "barra/aqui", "aspas\"aqui"] {
        assert!(!is_valid_method(invalid), "'{invalid}' deveria ser recusado");
    }
}

/// **A guarda que separa as janelas.** Todas compartilham o mesmo preload, então a identidade de
/// plugin é a única coisa que impede uma `plugin-view` qualquer de chamar
/// `person_catalog_delete` ou `brain_delete_social_fact`.
#[test]
fn a_plugin_view_can_only_call_its_own_plugin() {
    assert!(assert_caller_allowed(WindowType::PluginView, Some("mika.mood"), "mika.mood").is_ok());

    assert!(
        assert_caller_allowed(WindowType::PluginView, Some("mika.mood"), "mika.person-catalog").is_err(),
        "plugin-view chamando OUTRO plugin é a escalada que esta guarda existe para barrar"
    );
    assert!(assert_caller_allowed(WindowType::PluginView, None, "mika.mood").is_err());
    assert!(assert_caller_allowed(WindowType::PluginView, Some(""), "mika.mood").is_err());
}

/// `main`/`settings` chamam qualquer plugin (são superfícies do próprio app); `wizard` não chama
/// nada — o onboarding roda antes de o usuário ter configurado qualquer política.
///
/// `plugin-rpc-registry.ts` foi removido (backend Node deletado na migração para Tauri); as 3
/// superfícies permitidas que ele declarava (`PLUGIN_RPC_ALLOWED_WINDOW_TYPES`) viram snapshot
/// congelado, extraído do fonte no momento da deleção.
#[test]
fn the_allowed_surfaces_match_the_frozen_list() {
    for entry in ["main", "settings", "plugin-view"] {
        let window = WindowType::from_label(entry);
        assert!(
            assert_caller_allowed(window, Some("mika.mood"), "mika.mood").is_ok(),
            "'{entry}' era permitido antes da deleção do TS e está negado aqui"
        );
    }

    assert!(assert_caller_allowed(WindowType::Wizard, None, "mika.mood").is_err());
}

#[tokio::test]
async fn a_registered_method_dispatches_and_wraps_the_result() {
    registry().register("mika.rpc-test", "echo_method", Arc::new(Echo));

    let response = dispatch(WindowType::Main, None, &request("mika.rpc-test", "echo_method")).await;
    assert_eq!(response["success"], json!(true));
    assert_eq!(response["data"]["echo"], json!({ "x": 1 }));
}

/// Handler que falha vira `handler-failed` — RESPOSTA, não rejeição de IPC. O renderer espera
/// desembrulhar o envelope; um `Err` de comando chegaria como exceção sem `code`.
#[tokio::test]
async fn a_failing_handler_becomes_handler_failed_not_an_ipc_rejection() {
    registry().register("mika.rpc-test", "boom_method", Arc::new(Boom));

    let response = dispatch(WindowType::Main, None, &request("mika.rpc-test", "boom_method")).await;
    assert_eq!(response["success"], json!(false));
    assert_eq!(response["error"]["code"], json!("handler-failed"));
}

#[tokio::test]
async fn an_unregistered_method_is_handler_missing() {
    let response = dispatch(WindowType::Main, None, &request("mika.rpc-test", "never-registered")).await;
    assert_eq!(response["error"]["code"], json!("handler-missing"));
}

#[tokio::test]
async fn a_malformed_request_is_refused_before_the_guard_even_runs() {
    for payload in [json!({}), json!({ "pluginId": "nope", "method": "x" }), json!({ "pluginId": "mika.x" })] {
        let response = dispatch(WindowType::Main, None, &payload).await;
        assert_eq!(response["error"]["code"], json!("invalid-request"), "payload {payload} deveria ser recusado");
    }
}

/// A guarda roda ANTES de procurar o handler: uma `plugin-view` alheia não pode descobrir, pela
/// diferença entre `unauthorized` e `handler-missing`, quais métodos outro plugin registrou.
#[tokio::test]
async fn the_guard_runs_before_the_lookup_so_it_leaks_no_method_names() {
    registry().register("mika.rpc-test", "secret_method", Arc::new(Echo));

    let response = dispatch(
        WindowType::PluginView,
        Some("mika.outro"),
        &request("mika.rpc-test", "secret_method"),
    )
    .await;
    assert_eq!(response["error"]["code"], json!("unauthorized"));

    let unknown = dispatch(WindowType::PluginView, Some("mika.outro"), &request("mika.rpc-test", "nao-existe")).await;
    assert_eq!(
        unknown["error"]["code"],
        json!("unauthorized"),
        "método existente e inexistente têm de responder IGUAL para quem não pode chamar"
    );
}

// Os 4 arquivos TS que davam o numerador (25 métodos) foram removidos (backend Node deletado na
// migração para Tauri). O denominador congelado já vive em
// `native_tests::FROZEN_TS_METHODS` (25 entradas) e é cravado ali contra o registro Rust em
// `every_frozen_ts_method_is_registered_in_rust` — duplicar a contagem aqui não prova nada a mais.
