//! Prova de despacho REAL via `rpc::dispatch` (não o handler direto) para os 4 métodos de
//! `mika.brain`, incluindo os negativos (`id` inexistente, args faltando/mal formados). `TEST_LOCK`
//! serializa: `register_with_store` sobrescreve as mesmas 4 chaves globais de
//! `plugins::rpc::registry()` a cada chamada, e `cargo test` roda em paralelo (mesmo padrão de
//! `gamer::rpc::TEST_LOCK`).
//!
//! `MemoryStore::open_in_memory()` — nenhum destes testes toca disco, rede ou provedor pago.

use super::*;
use crate::modules::plugins::rpc::dispatch;
use crate::protocol::WindowType;
use serde_json::json;

fn mem() -> &'static MemoryStore {
    Box::leak(Box::new(MemoryStore::open_in_memory().unwrap()))
}

async fn call(method: &str, args: Value) -> Value {
    dispatch(WindowType::Main, None, &json!({ "pluginId": PLUGIN_ID, "method": method, "args": args })).await
}

/// Positiva — os 4 métodos, um fluxo completo: soma (vazio) -> adiciona -> lista -> atualiza -> apaga.
#[tokio::test]
async fn all_four_methods_dispatch_through_the_registry() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_with_store(mem());

    let empty = call("brain_get_social_facts", Value::Null).await;
    assert_eq!(empty["success"], json!(true));
    assert_eq!(empty["data"], json!([]), "banco recém-aberto não tem fato nenhum");

    let added = call("brain_add_social_fact", json!({ "subject": "user", "fact": "gosta de rust" })).await;
    assert_eq!(added["success"], json!(true));
    assert_eq!(added["data"]["fact"], json!("gosta de rust"));
    // "user"/"eu" são apelidos do próprio usuário e caem no subject canônico que a persona lê.
    assert_eq!(added["data"]["subject"], json!(crate::modules::memory::social::USER_SUBJECT));
    let id = added["data"]["id"].as_str().unwrap().to_string();

    let listed = call("brain_get_social_facts", Value::Null).await;
    assert_eq!(listed["data"].as_array().unwrap().len(), 1, ">= o fato recém-gravado aparece na leitura");

    let updated = call("brain_update_social_fact", json!({ "id": id, "fact": "  gosta muito de rust  ", "confidence": 0.4 })).await;
    assert_eq!(updated["success"], json!(true));
    assert_eq!(updated["data"]["fact"], json!("gosta muito de rust"), "texto é aparado (trim)");
    assert_eq!(updated["data"]["confidence"], json!(0.4));

    // O id acompanha o texto: o fato editado tem id novo e o antigo deixou de existir.
    let edited_id = updated["data"]["id"].as_str().unwrap().to_string();
    assert_ne!(edited_id, id);
    let deleted = call("brain_delete_social_fact", json!({ "id": edited_id })).await;
    assert_eq!(deleted["success"], json!(true));
    assert_eq!(deleted["data"], json!({ "success": true }));

    let after_delete = call("brain_get_social_facts", Value::Null).await;
    assert_eq!(after_delete["data"], json!([]), "apagado não sobra na leitura seguinte");
}

/// NEGATIVO — `brain_update_social_fact` mantém a `confidence` existente quando o payload não a envia
/// (mesma regra de `confidence ?? item.confidence` do TS).
#[tokio::test]
async fn update_keeps_existing_confidence_when_payload_omits_it() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_with_store(mem());

    let added = call("brain_add_social_fact", json!({ "fact": "prefere café", "confidence": 0.9 })).await;
    let id = added["data"]["id"].as_str().unwrap().to_string();

    let updated = call("brain_update_social_fact", json!({ "id": id, "fact": "prefere muito café" })).await;
    assert_eq!(updated["data"]["confidence"], json!(0.9), "sem confidence no payload, mantém a do fato existente");
}

/// NEGATIVO — `brain_update_social_fact` com `id` inexistente é `handler-failed` nomeado, nunca
/// sucesso parcial (mesmo destino do `throw new Error('Fato não encontrado')` do TS — ver o
/// cabeçalho de `rpc.rs`).
#[tokio::test]
async fn update_with_unknown_id_is_a_named_failure() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_with_store(mem());

    let result = call("brain_update_social_fact", json!({ "id": "nope", "fact": "x" })).await;
    assert_eq!(result["success"], json!(false));
    assert_eq!(result["error"]["code"], json!("handler-failed"));
    assert_eq!(result["error"]["message"], json!("Fato não encontrado"));
}

/// NEGATIVO — `brain_update_social_fact` sem `id` no payload cai no MESMO caminho de "não
/// encontrado" (id ausente vira `""`, que nunca casa com um registro real).
#[tokio::test]
async fn update_without_an_id_field_is_also_not_found() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_with_store(mem());

    let result = call("brain_update_social_fact", json!({ "fact": "x" })).await;
    assert_eq!(result["error"]["code"], json!("handler-failed"));
    assert_eq!(result["error"]["message"], json!("Fato não encontrado"));
}

/// NEGATIVO — `brain_update_social_fact` com `id` válido mas SEM `fact` no payload (args mal
/// formados) é `handler-failed`, nunca grava com texto ausente.
#[tokio::test]
async fn update_without_a_fact_field_is_a_named_failure_not_a_silent_write() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_with_store(mem());

    let added = call("brain_add_social_fact", json!({ "fact": "gosta de xadrez" })).await;
    let id = added["data"]["id"].as_str().unwrap().to_string();

    let result = call("brain_update_social_fact", json!({ "id": id })).await;
    assert_eq!(result["success"], json!(false));
    assert_eq!(result["error"]["code"], json!("handler-failed"));
    assert_eq!(result["error"]["message"], json!("fact is required"));
}

/// NEGATIVO — `brain_add_social_fact` sem `fact` (args faltando) nunca grava nada; mesmo código de
/// falha que um `TypeError` de `undefined.trim()` produziria no TS (`handler-failed`).
#[tokio::test]
async fn add_without_a_fact_field_is_a_named_failure() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let store = mem();
    register_with_store(store);

    let result = call("brain_add_social_fact", json!({ "subject": "user" })).await;
    assert_eq!(result["success"], json!(false));
    assert_eq!(result["error"]["code"], json!("handler-failed"));
    assert_eq!(result["error"]["message"], json!("fact is required"));
    assert_eq!(SocialMemoryService::new(store).get_graph().unwrap().facts.len(), 0, "nada foi persistido");
}

/// NEGATIVO — `brain_delete_social_fact` de um `id` que nunca existiu devolve `{success:false}`, não
/// um erro de transporte (mesma forma que o teste TS `devolve { success: false }` já documenta).
#[tokio::test]
async fn delete_of_an_unknown_id_reports_success_false_not_a_transport_error() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_with_store(mem());

    let result = call("brain_delete_social_fact", json!({ "id": "ghost" })).await;
    assert_eq!(result["success"], json!(true), "envelope de transporte: o HANDLER não falhou");
    assert_eq!(result["data"], json!({ "success": false }), "mas o negócio diz que não achou nada pra apagar");
}

/// NEGATIVO — `brain_delete_social_fact` sem `id` no payload cai no mesmo `{success:false}` (id
/// ausente vira `""`, nunca casa com um registro real).
#[tokio::test]
async fn delete_without_an_id_field_reports_success_false() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_with_store(mem());

    let result = call("brain_delete_social_fact", Value::Null).await;
    assert_eq!(result["data"], json!({ "success": false }));
}
