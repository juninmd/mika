//! `mika:plugin-rpc` de `mika.person-catalog` — porte de
//! `apps/desktop/src/features/person-catalog/handlers/person-catalog-rpc.ts`, para o painel de
//! Configurações e a janela de listagem (`renderer/`), NÃO para a tool do LLM ([`super::tool`], que
//! tem os próprios `handle_*` com forma de resposta diferente — trimada para o modelo, cheia aqui
//! para a UI).
//!
//! **Cada `Ok(v)` devolvido por um handler aqui É o objeto de negócio inteiro** (`{success, data}`
//! ou `{success, error}`), não só o `data`: `crate::modules::plugins::rpc::dispatch` embrulha isso
//! DE NOVO no envelope de transporte (`{success:true, data: <Ok(v) aqui>}`), e é esse envelope duplo
//! que `unwrapPluginRpcResponse` desembrulha UMA vez no renderer — o mesmo que
//! `main/infrastructure/bootstrap/plugin-rpc-registry.ts::dispatchPluginRpc` já fazia
//! (`data: await handler(args)`, onde `handler` é literalmente a função `ctx.rpc.handle(...)` que
//! devolve `{success,...}`). `Err(msg)` fica reservado para uma falha técnica de verdade (I/O do
//! `catalog.json`) — o equivalente Rust de um `throw` não capturado no handler TS, que vira
//! `handler-failed` no transporte (promise REJEITADA no renderer). Validação de negócio
//! ("confirm obrigatório", "não encontrado") é sempre `Ok({"success":false,"error":...})`, nunca
//! `Err` — senão a UI, que só lê `res.error` como string no caminho de sucesso, veria o objeto
//! `{code,message}` do transporte em vez do texto de negócio.
//!
//! ## Os 14 métodos — o que dispacha de verdade
//!
//! | Método | Backend | Status |
//! |---|---|---|
//! | `list`/`search`/`get`/`upsert`/`delete` | [`super::store::CatalogStore`] | dispacha de verdade |
//! | `block`/`unblock`/`blocked_list` | idem, via [`super::mutate::moderation`] (novo nesta rodada) | dispacha de verdade |
//! | `merge` | idem, via [`super::mutate::merge`] (novo nesta rodada) | dispacha de verdade |
//! | `clarifications`/`clarification_settle` | idem, `people_memory` pending questions | dispacha de verdade |
//! | `fact_edit`/`alias_remove` | idem, person facts and alias chips | dispacha de verdade |
//! | `image` | [`super::image`] (fetch guardado contra SSRF por hop + MIME pelos bytes + teto de 5 MiB) | dispacha de verdade |
//!
//! ## Diferenças de forma medidas contra o TS (nomeadas, não escondidas)
//! - `upsert`: TS devolve `{success, data: person, ambiguousWith}` — `ambiguousWith` é IRMÃO de
//!   `data`, não campo dentro dele. Aqui vira `data.ambiguousWith` (aninhado): nenhum consumidor no
//!   renderer lê `ambiguousWith` hoje (só `res.success`/`res.error`), então a diferença é inerte, mas
//!   fica registrada para quem for construir uma UI de ambiguidade depois.
//! - `delete`/`block`/`unblock`/`merge`: TS usa `ok()`/`fail()` do SDK (`ok()` sem argumento omite
//!   `data` do JSON); aqui `data` sempre aparece (`{}`/objeto vazio quando não há payload). O
//!   renderer só lê `success`/`error` nesses quatro métodos, nunca `data` — mesma conclusão.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::plugins::rpc::RpcHandler;

use super::pii::redact_pii;
use super::store::CatalogStore;
use super::types::{Confidence, PersonType, UpsertAttribute, UpsertInput};

const DEFAULT_MAX_MENTIONS: usize = 100;

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string).filter(|s| !s.is_empty())
}

/// Porte de `stringArray` — filtra por string não-vazia (após `trim`), mas guarda o valor ORIGINAL,
/// não o valor trimado (mesma pegadinha do TS: `item.trim().length > 0` filtra, não transforma).
fn opt_str_array(args: &Value, key: &str) -> Option<Vec<String>> {
    let arr = args.get(key)?.as_array()?;
    let values: Vec<String> = arr.iter().filter_map(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string).collect();
    (!values.is_empty()).then_some(values)
}

/// Porte de `personAttributes` — key E value (redigido) obrigatórios, confidence cai para `None`
/// (o store assume `Medium`) se não for exatamente `low`/`medium`/`high`.
fn parse_attributes(args: &Value) -> Option<Vec<UpsertAttribute>> {
    let arr = args.get("attributes")?.as_array()?;
    let attributes: Vec<UpsertAttribute> = arr
        .iter()
        .filter_map(|item| {
            let key = item.get("key")?.as_str()?;
            let raw_value = item.get("value")?.as_str()?;
            if key.is_empty() || raw_value.is_empty() {
                return None;
            }
            let value = redact_pii(raw_value);
            let confidence =
                item.get("confidence").and_then(Value::as_str).and_then(|c| serde_json::from_value::<Confidence>(json!(c)).ok());
            Some(UpsertAttribute { key: key.to_string(), value, confidence })
        })
        .collect();
    (!attributes.is_empty()).then_some(attributes)
}

/// Porte de `parseUpsertArgs` — só os campos que o formulário de edição do painel manda; note que
/// `bio` NÃO é redigido aqui (edição manual pelo dono do app), só `userNotes` e o valor de cada
/// atributo são (mesma assimetria do TS: `str(args.bio)` vs `redactedStr(args.userNotes)`).
fn build_upsert_input(args: &Value, name: String) -> UpsertInput {
    UpsertInput {
        id: opt_str(args, "id"),
        name,
        aliases: opt_str_array(args, "aliases"),
        kind: opt_str(args, "type").and_then(|t| PersonType::parse(&t)),
        bio: opt_str(args, "bio"),
        role: opt_str(args, "role"),
        origin: opt_str(args, "origin"),
        nationality: opt_str(args, "nationality"),
        image_url: opt_str(args, "imageUrl"),
        user_notes: opt_str(args, "userNotes").map(|n| redact_pii(&n)),
        user_rating: opt_str(args, "userRating"),
        tags: opt_str_array(args, "tags"),
        occupation: opt_str_array(args, "occupation"),
        known_for: opt_str_array(args, "knownFor"),
        attributes: parse_attributes(args),
        ..Default::default()
    }
}

/// Reclassifica um erro TÉCNICO de `CatalogStore` (`Err`, vira `handler-failed`) num erro de
/// NEGÓCIO (`Ok({success:false,...})`) quando a mensagem é um dos "não encontrado" nomeados que o
/// TS trata como retorno normal (`fail(...)`), nunca como `throw`. Qualquer outra mensagem (falha de
/// I/O) continua `Err`.
fn recoverable(error: String, prefix: &str) -> Result<Value, String> {
    if error.starts_with(prefix) {
        Ok(json!({ "success": false, "error": error }))
    } else {
        Err(error)
    }
}

fn handle_list(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let kind = args.get("type").and_then(Value::as_str).and_then(PersonType::parse);
    let sort_by = args.get("sortBy").and_then(Value::as_str).map(str::to_string);
    let limit = args.get("limit").and_then(Value::as_u64).map(|n| n as usize).unwrap_or(usize::MAX);
    Ok(json!({ "success": true, "data": store.list(kind, sort_by.as_deref(), limit) }))
}

/// TS não valida `query`: ausente vira `''`, e `''.includes` casa com QUALQUER pessoa (substring
/// vazia) — devolve o catálogo inteiro em vez de nada. Replicado, não "corrigido".
fn handle_search(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let query = args.get("query").and_then(Value::as_str).unwrap_or("");
    let limit = args.get("limit").and_then(Value::as_u64).map(|n| n as usize).unwrap_or(usize::MAX);
    Ok(json!({ "success": true, "data": store.search(query, limit) }))
}

fn handle_get(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let identifier = args.get("identifier").and_then(Value::as_str).unwrap_or("");
    match store.get(identifier).or_else(|| store.find_by_alias(identifier)) {
        Some(person) => Ok(json!({ "success": true, "data": person })),
        None => Ok(json!({ "success": false, "error": format!("Not found: {identifier}") })),
    }
}

fn handle_upsert(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let name = args.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if name.is_empty() {
        return Ok(json!({ "success": false, "error": "Name is required and cannot be empty." }));
    }
    if name.chars().count() > 255 {
        return Ok(json!({ "success": false, "error": "Name cannot exceed 255 characters." }));
    }

    let outcome = store.upsert(build_upsert_input(&args, name), now_ms(), DEFAULT_MAX_MENTIONS)?;
    let Some(person) = outcome.person else {
        let error = if outcome.blocked {
            "This person is blocked. Unblock them before adding them again."
        } else {
            "Name rejected: it does not look like a person name (max 48 chars, no sentence punctuation)."
        };
        return Ok(json!({ "success": false, "error": error }));
    };
    // Same signal as the tool and the watcher: the open list refreshes and a new person gets enriched.
    super::events::publish_updated(&person, outcome.created);

    let mut data = serde_json::to_value(&person).map_err(|e| format!("serializar pessoa: {e}"))?;
    // `ambiguousWith` é irmão de `data` no TS; aninhado aqui — ver o cabeçalho do arquivo.
    if let Some(ambiguous) = outcome.ambiguous_with {
        data["ambiguousWith"] = json!(ambiguous);
    }
    Ok(json!({ "success": true, "data": data }))
}

fn handle_merge(store: &CatalogStore, args: Value) -> Result<Value, String> {
    if args.get("confirm").and_then(Value::as_bool) != Some(true) {
        return Ok(json!({ "success": false, "error": "Explicit confirmation is required to merge two people." }));
    }
    let target_id = opt_str(&args, "targetId");
    let source_id = opt_str(&args, "sourceId");
    let (Some(target_id), Some(source_id)) = (target_id, source_id) else {
        return Ok(json!({ "success": false, "error": "targetId and sourceId are required." }));
    };

    match store.merge(&target_id, &source_id, now_ms(), DEFAULT_MAX_MENTIONS) {
        Ok(outcome) => {
            let mut data = json!({ "alreadyMerged": outcome.already_merged });
            if let Some(person) = outcome.person {
                data["person"] = serde_json::to_value(&person).map_err(|e| format!("serializar pessoa: {e}"))?;
            }
            if let Some(from) = outcome.merged_from {
                data["mergedFrom"] = json!(from);
            }
            Ok(json!({ "success": true, "data": data }))
        }
        Err(error) => recoverable(error, "Person not found"),
    }
}

fn handle_block(store: &CatalogStore, args: Value) -> Result<Value, String> {
    if args.get("confirm").and_then(Value::as_bool) != Some(true) {
        return Ok(json!({ "success": false, "error": "Explicit confirmation is required to block a person." }));
    }
    let Some(identifier) = opt_str(&args, "identifier") else {
        return Ok(json!({ "success": false, "error": "identifier is required." }));
    };
    let outcome = store.block(&identifier)?;
    Ok(json!({ "success": true, "data": { "blockedKeys": outcome.blocked_keys, "removed": outcome.removed } }))
}

fn handle_unblock(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let Some(identifier) = opt_str(&args, "identifier") else {
        return Ok(json!({ "success": false, "error": "identifier is required." }));
    };
    match store.unblock(&identifier) {
        Ok(blocked) => Ok(json!({ "success": true, "data": { "blocked": blocked } })),
        Err(error) => recoverable(error, "Not blocked"),
    }
}

fn handle_blocked_list(store: &CatalogStore, _args: Value) -> Result<Value, String> {
    Ok(json!({ "success": true, "data": store.list_blocked() }))
}

fn handle_delete(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let id = args.get("id").and_then(Value::as_str).unwrap_or("");
    // `dryRun`: só conta o que seria apagado (a UI mostra antes de pedir a confirmação) — não apaga nada.
    if args.get("dryRun").and_then(Value::as_bool) == Some(true) {
        return match super::forget::preview_delete(store, crate::modules::memory::memory_store_reader(), id) {
            Ok(response) => Ok(response),
            Err(error) => recoverable(error, "Person not found"),
        };
    }
    if args.get("confirm").and_then(Value::as_bool) != Some(true) {
        return Ok(json!({ "success": false, "error": "Explicit confirmation is required to delete a person." }));
    }
    // Ficha + grafo social + memória episódica: apagar só a ficha deixava a pessoa lembrada.
    match super::forget::delete_everywhere(store, crate::modules::memory::memory_store_reader(), id) {
        Ok(response) => Ok(response),
        Err(error) => recoverable(error, "Person not found"),
    }
}

/// Open "qual irmão?" questions, with each candidate's id and display name for the picker.
fn handle_clarifications(store: &CatalogStore, _args: Value) -> Result<Value, String> {
    let people = store.list(None, None, usize::MAX);
    let data: Vec<Value> = store
        .pending_clarifications()
        .into_iter()
        .map(|p| {
            let candidates: Vec<Value> =
                people.iter().filter(|person| p.candidate_ids.contains(&person.id)).map(|person| json!({ "id": person.id, "name": person.primary_name })).collect();
            json!({ "id": p.id, "label": p.label, "facts": p.facts, "createdAt": p.created_at, "candidates": candidates })
        })
        .collect();
    Ok(json!({ "success": true, "data": data }))
}

/// `personId` present → its facts go to that person; absent → dismissed.
fn handle_clarification_settle(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let Some(id) = opt_str(&args, "id") else {
        return Ok(json!({ "success": false, "error": "id is required." }));
    };
    let person_id = opt_str(&args, "personId");
    if store.settle_clarification(&id, person_id.as_deref(), now_ms())? {
        if let Some(person) = person_id.as_deref().and_then(|pid| store.get(pid)) {
            super::events::publish_updated(&person, false);
        }
        return Ok(json!({ "success": true }));
    }
    Ok(json!({ "success": false, "error": format!("Clarification not found: {id}") }))
}

/// Edit one fact (`text`) or delete it (no `text`).
fn handle_fact_edit(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let (Some(person_id), Some(fact_id)) = (opt_str(&args, "personId"), opt_str(&args, "factId")) else {
        return Ok(json!({ "success": false, "error": "personId and factId are required." }));
    };
    let text = opt_str(&args, "text").map(|t| redact_pii(&t));
    match store.edit_fact(&person_id, &fact_id, text.as_deref(), now_ms()) {
        Ok(person) => {
            super::events::publish_updated(&person, false);
            Ok(json!({ "success": true, "data": person }))
        }
        Err(error) if error.starts_with("Fact not found") => Ok(json!({ "success": false, "error": error })),
        Err(error) => recoverable(error, "Person not found"),
    }
}

fn handle_alias_remove(store: &CatalogStore, args: Value) -> Result<Value, String> {
    let (Some(person_id), Some(alias)) = (opt_str(&args, "personId"), opt_str(&args, "alias")) else {
        return Ok(json!({ "success": false, "error": "personId and alias are required." }));
    };
    match store.remove_alias(&person_id, &alias, now_ms()) {
        Ok(person) => {
            super::events::publish_updated(&person, false);
            Ok(json!({ "success": true, "data": person }))
        }
        Err(error) => recoverable(error, "Person not found"),
    }
}

type MethodFn = fn(&CatalogStore, Value) -> Result<Value, String>;

/// Ponteiro de função em vez de 9 structs quase idênticas — o único estado por método é QUAL função
/// chamar; o `&'static CatalogStore` é sempre o mesmo singleton de processo.
struct Method {
    store: &'static CatalogStore,
    f: MethodFn,
}

#[async_trait]
impl RpcHandler for Method {
    async fn call(&self, args: Value) -> Result<Value, String> {
        (self.f)(self.store, args)
    }
}

const METHODS: &[(&str, MethodFn)] = &[
    ("person_catalog_list", handle_list),
    ("person_catalog_search", handle_search),
    ("person_catalog_get", handle_get),
    ("person_catalog_upsert", handle_upsert),
    ("person_catalog_merge", handle_merge),
    ("person_catalog_block", handle_block),
    ("person_catalog_unblock", handle_unblock),
    ("person_catalog_blocked_list", handle_blocked_list),
    ("person_catalog_delete", handle_delete),
    ("person_catalog_clarifications", handle_clarifications),
    ("person_catalog_clarification_settle", handle_clarification_settle),
    ("person_catalog_fact_edit", handle_fact_edit),
    ("person_catalog_alias_remove", handle_alias_remove),
];

/// Chamado por `modules::register_config_bound_tools(app)` — mesma segunda passada de
/// `super::tool::register_tools`, pelo mesmo motivo (precisa de `AppHandle` para abrir
/// `catalog.json`). Reusa o MESMO singleton de processo (`super::catalog_store`), nunca uma segunda
/// instância — ver o cabeçalho de `store_singleton.rs` para por que duas instâncias sobre o mesmo
/// arquivo se pisam.
pub(crate) fn register(app: &tauri::AppHandle) -> Result<(), String> {
    let store = super::catalog_store(app)?;
    register_with_store(store);
    let config = crate::state::config_store(app)?;
    let image = Arc::new(super::image::ImageHandler { store: config });
    crate::modules::plugins::rpc::registry().register("mika.person-catalog", "person_catalog_image", image);
    Ok(())
}

/// Separado de [`register`] só para o teste alcançar sem `AppHandle` real — mesma necessidade que
/// `tool.rs` documenta para o próprio `register_tools` (comentário de
/// `register_stub_answers_unavailable_until_the_real_store_is_wired`).
fn register_with_store(store: &'static CatalogStore) {
    let registry = crate::modules::plugins::rpc::registry();
    for (method, f) in METHODS {
        registry.register("mika.person-catalog", method, Arc::new(Method { store, f: *f }));
    }
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
