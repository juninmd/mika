//! Suíte de `rpc.rs`, em arquivo separado só pela regra de 300 linhas do AGENTS.md (mesmo truque de
//! `modules::plugins::rpc.rs`/`rpc_tests.rs` e de `store.rs`/`store_tests.rs`).
//!
//! O teste de ponta a ponta usa `crate::modules::plugins::rpc::dispatch` (não o handler direto) —
//! é a única forma de provar que a guarda + o registro + o handler despacham juntos, do jeito que o
//! renderer realmente chama. Os demais testes chamam os `handle_*` privados direto: cada um abre o
//! PRÓPRIO `CatalogStore` temporário e nunca toca `registry()` (`OnceLock` de processo, compartilhado
//! entre testes paralelos — só o teste de ponta a ponta usa o registro real, uma vez).

use super::*;
use crate::modules::plugins::rpc::dispatch;
use crate::protocol::WindowType;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-person-catalog-rpc-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn open_store(dir: &TempDir) -> CatalogStore {
    CatalogStore::open(&dir.0.join("catalog.json")).unwrap()
}

fn request(method: &str, args: Value) -> Value {
    json!({ "pluginId": "mika.person-catalog", "method": method, "args": args })
}

/// Chama pelo transporte de verdade e devolve o objeto de NEGÓCIO (`{success,data|error,...}`) que
/// o handler produziu — o mesmo desembrulho que `unwrapPluginRpcResponse` faz no renderer, só que
/// aqui vira `assert` em vez de `throw` quando o TRANSPORTE (não o negócio) falha. Ver o cabeçalho
/// de `rpc.rs` para por que os dois níveis existem.
async fn call(method: &str, args: Value) -> Value {
    let response = dispatch(WindowType::Main, None, &request(method, args)).await;
    assert_eq!(response["success"], json!(true), "transporte falhou para {method}: {response}");
    response["data"].clone()
}

/// A prova de despacho real: registra o store no registro GLOBAL do processo e passa por
/// `dispatch(...)`, não por `handle_*` direto — cobre guarda + lookup + handler juntos, na mesma
/// combinação que `bridge.rs` usa em produção. Inclui um round-trip pelo DISCO (reabre o mesmo
/// `catalog.json` por fora do `store` registrado) para provar que a escrita disparada pela mensagem
/// RPC foi persistida, não só devolvida na resposta.
#[tokio::test]
async fn dispatches_every_real_method_end_to_end_through_the_registry() {
    let dir = TempDir::new();
    let store: &'static CatalogStore = Box::leak(Box::new(open_store(&dir)));
    register_with_store(store);

    let created = call(
        "person_catalog_upsert",
        json!({ "name": "Ada Lovelace", "userNotes": "liga pra 11 98765-4321", "attributes": [{ "key": "email", "value": "ada@example.com" }] }),
    )
    .await;
    assert_eq!(created["success"], json!(true), "{created}");
    let id = created["data"]["id"].as_str().unwrap().to_string();
    assert!(created["data"]["userNotes"].as_str().unwrap().contains("[PHONE]"), "PII vazou na resposta: {created}");

    // Disco, não memória: reabre por fora do `store` registrado.
    let reopened = open_store(&dir);
    let on_disk = reopened.get(&id).unwrap();
    assert!(on_disk.user_notes.unwrap().contains("[PHONE]"), "PII vazou pro catalog.json");
    assert!(
        on_disk.attributes.iter().any(|a| a.value == "[EMAIL]"),
        "valor de atributo com PII vazou pro catalog.json"
    );

    let fetched = call("person_catalog_get", json!({ "identifier": id })).await;
    assert_eq!(fetched["data"]["primaryName"], json!("Ada Lovelace"));

    let listed = call("person_catalog_list", json!({})).await;
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);
    let searched = call("person_catalog_search", json!({ "query": "ada" })).await;
    assert_eq!(searched["data"].as_array().unwrap().len(), 1);

    let blocked = call("person_catalog_block", json!({ "identifier": id, "confirm": true })).await;
    assert_eq!(blocked["success"], json!(true), "{blocked}");
    let refused = call("person_catalog_upsert", json!({ "name": "Ada Lovelace" })).await;
    assert_eq!(refused["success"], json!(false), "pessoa bloqueada não podia recatalogar: {refused}");
    let blocked_list = call("person_catalog_blocked_list", json!({})).await;
    assert!(!blocked_list["data"].as_array().unwrap().is_empty());
    // `block` guarda a chave do NOME (`catalogKey(primaryName)`), não do id — mesmo TS: bloquear
    // por id acha a pessoa, mas o que fica na lista é o nome dela. `unblock` precisa da mesma chave.
    let unblocked = call("person_catalog_unblock", json!({ "identifier": "Ada Lovelace" })).await;
    assert_eq!(unblocked["success"], json!(true), "{unblocked}");

    let target = call("person_catalog_upsert", json!({ "name": "Ada Lovelace" })).await;
    let target_id = target["data"]["id"].as_str().unwrap().to_string();
    let source = call("person_catalog_upsert", json!({ "name": "Ada L" })).await;
    let source_id = source["data"]["id"].as_str().unwrap().to_string();
    let merged = call("person_catalog_merge", json!({ "targetId": target_id, "sourceId": source_id, "confirm": true })).await;
    assert_eq!(merged["success"], json!(true), "{merged}");
    assert_eq!(merged["data"]["person"]["id"], json!(target_id));

    let refused_delete = call("person_catalog_delete", json!({ "id": target_id })).await;
    assert_eq!(refused_delete["success"], json!(false), "delete sem confirm tinha de ser recusado");
    // `dryRun` é a prévia da confirmação da UI: responde sem apagar nem exigir `confirm`.
    let preview = call("person_catalog_delete", json!({ "id": target_id, "dryRun": true })).await;
    assert_eq!(preview["success"], json!(true), "{preview}");
    assert_eq!(preview["data"]["dryRun"], json!(true), "{preview}");
    let deleted = call("person_catalog_delete", json!({ "id": target_id, "confirm": true })).await;
    assert_eq!(deleted["success"], json!(true), "{deleted}");

}

#[test]
fn upsert_rejects_an_empty_or_overlong_name_without_touching_the_store() {
    let dir = TempDir::new();
    let store = open_store(&dir);
    let empty = handle_upsert(&store, json!({ "name": "   " })).unwrap();
    assert_eq!(empty, json!({ "success": false, "error": "Name is required and cannot be empty." }));

    let too_long = handle_upsert(&store, json!({ "name": "a".repeat(256) })).unwrap();
    assert_eq!(too_long, json!({ "success": false, "error": "Name cannot exceed 255 characters." }));
    assert!(store.list(None, None, 10).is_empty());
}

#[test]
fn merge_requires_confirm_and_both_ids_before_touching_the_store() {
    let dir = TempDir::new();
    let store = open_store(&dir);
    let no_confirm = handle_merge(&store, json!({ "targetId": "a", "sourceId": "b" })).unwrap();
    assert_eq!(no_confirm["error"], json!("Explicit confirmation is required to merge two people."));

    let missing_ids = handle_merge(&store, json!({ "confirm": true, "targetId": "a" })).unwrap();
    assert_eq!(missing_ids["error"], json!("targetId and sourceId are required."));
}

#[test]
fn block_requires_confirm_and_identifier() {
    let dir = TempDir::new();
    let store = open_store(&dir);
    let no_confirm = handle_block(&store, json!({ "identifier": "p1" })).unwrap();
    assert_eq!(no_confirm["error"], json!("Explicit confirmation is required to block a person."));

    let missing_id = handle_block(&store, json!({ "confirm": true })).unwrap();
    assert_eq!(missing_id["error"], json!("identifier is required."));
}

#[test]
fn unblock_requires_identifier_and_names_an_identifier_that_was_never_blocked() {
    let dir = TempDir::new();
    let store = open_store(&dir);
    let missing_id = handle_unblock(&store, json!({})).unwrap();
    assert_eq!(missing_id["error"], json!("identifier is required."));

    let never_blocked = handle_unblock(&store, json!({ "identifier": "ninguem" })).unwrap();
    assert_eq!(never_blocked["success"], json!(false));
    assert!(never_blocked["error"].as_str().unwrap().contains("Not blocked"));
}

#[test]
fn delete_requires_explicit_confirm() {
    let dir = TempDir::new();
    let store = open_store(&dir);
    let refused = handle_delete(&store, json!({ "id": "p1" })).unwrap();
    assert_eq!(refused["error"], json!("Explicit confirmation is required to delete a person."));
}

/// NEGATIVO — `search`/`list` sem `query`/`limit` nunca crasham: o TS trata ausência como `''`/sem
/// teto, e a réplica Rust tem de aceitar o mesmo `args` vazio sem pânico (regressão do
/// `it.each(['person_catalog_search', ...])` do TS que prova exatamente isto).
#[test]
fn search_and_list_handle_missing_args_without_panicking() {
    let dir = TempDir::new();
    let store = open_store(&dir);
    assert_eq!(handle_search(&store, json!({})).unwrap()["success"], json!(true));
    assert_eq!(handle_list(&store, json!({})).unwrap()["success"], json!(true));
    assert_eq!(handle_get(&store, json!({})).unwrap()["success"], json!(false));
}

/// The people-memory UI: a pending "qual irmão?" is listed with its candidates' names, settling it
/// hands the waiting fact to the chosen brother, and that fact can then be edited and deleted.
#[test]
fn clarification_settle_then_fact_edit_and_delete() {
    use super::super::people_memory::parse_ops;
    let dir = TempDir::new();
    let store = open_store(&dir);
    let setup = r#"{"ops":[{"op":"upsert_person","ref":{"name":"Pedro","relation":"irmão"}},{"op":"upsert_person","ref":{"name":"João","relation":"irmão"}}]}"#;
    store.apply_people_ops(&parse_ops(setup), "meus irmãos", 1, 100).unwrap();
    let ambiguous = r#"{"ops":[{"op":"add_fact","ref":{"relation":"irmão"},"fact":"passou na faculdade"}]}"#;
    store.apply_people_ops(&parse_ops(ambiguous), "meu irmão passou na faculdade", 2, 100).unwrap();

    let listed = handle_clarifications(&store, json!({})).unwrap();
    let pending = &listed["data"][0];
    assert_eq!(pending["label"], json!("irmão"));
    assert_eq!(pending["facts"], json!(["passou na faculdade"]));
    let mut names: Vec<String> = pending["candidates"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["João", "Pedro"]);

    let pedro = store.find_by_alias("Pedro").unwrap();
    let settled = handle_clarification_settle(&store, json!({ "id": pending["id"], "personId": pedro.id })).unwrap();
    assert_eq!(settled["success"], json!(true));
    let pedro = store.find_by_alias("Pedro").unwrap();
    assert_eq!(pedro.facts.len(), 1);
    assert!(handle_clarifications(&store, json!({})).unwrap()["data"].as_array().unwrap().is_empty());

    let edited = handle_fact_edit(&store, json!({ "personId": pedro.id, "factId": pedro.facts[0].id, "text": "passou em medicina" })).unwrap();
    assert_eq!(edited["data"]["facts"][0]["text"], json!("passou em medicina"));
    let deleted = handle_fact_edit(&store, json!({ "personId": pedro.id, "factId": pedro.facts[0].id })).unwrap();
    assert_eq!(deleted["data"]["facts"], Value::Null, "an empty fact list is omitted from the JSON");
    let missing = handle_fact_edit(&store, json!({ "personId": pedro.id, "factId": "nope" })).unwrap();
    assert_eq!(missing["success"], json!(false));
}

#[test]
fn an_alias_chip_can_be_removed() {
    let dir = TempDir::new();
    let store = open_store(&dir);
    let input = UpsertInput { name: "Dani".into(), aliases: Some(vec!["Esposa".into(), "Dan".into()]), ..Default::default() };
    let person = store.upsert(input, 1, 100).unwrap().person.unwrap();
    let result = handle_alias_remove(&store, json!({ "personId": person.id, "alias": "Dan" })).unwrap();
    assert_eq!(result["data"]["aliases"], json!(["Esposa"]));
    assert!(store.find_by_alias("Dan").is_none(), "the removed alias no longer resolves");
}
