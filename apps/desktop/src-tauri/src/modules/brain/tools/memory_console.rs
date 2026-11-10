//! `memory_console` — porte de `memoryContextConsoleTool`
//! (`apps/desktop/src/main/services/brain/tools/context-console.ts`).
//!
//! O que é REAL: a validação de `fact`/`id` obrigatórios (`add_fact`/`delete_episodic`), mesmas
//! mensagens do TS, `search_episodic`/`clear_episodic` (memória episódica, rodada anterior) e — a
//! partir desta rodada — `add_fact` (grafo social, `modules::memory::social`) e `delete_episodic`
//! (`EpisodicMemoryService::delete`, que já existia testado, só sem chamador de produção). `snapshot`
//! continua parcial, ver abaixo.
//!
//! **`search_episodic` nunca precisou de embedding.** `context-console.ts::searchEpisodic` chama
//! `mem.search(query, limit)` — que é `episodic-queries.ts::search`, busca por PALAVRA-CHAVE — não
//! `searchSemantic`. A suposição de que as duas dependiam de embedding (de uma rodada anterior) não
//! resistiu à leitura do TS: o gap real era só a visibilidade do submódulo `memory::episodic`
//! (privado), não um provedor de embedding — que, à parte, também não tem porte Rust hoje (ver
//! `episodic::embedding`, sempre `None`).
//!
//! **`snapshot` fica parcial, honesto.** No TS, `snapshot()` junta `promptTrace` (`brain` local,
//! `getLastPromptTrace()` — estado de turno em memória, não persistido; continua sem porte Rust) +
//! `episodic` (real) + `graph` de memória social (real, a partir desta rodada:
//! `SocialMemoryService.getGraph`). Devolver a resposta inteira como `unavailable` escondia a parte
//! que JÁ é real; devolver `ok` fingindo `promptTrace` inventaria dado que não existe (um trace de
//! prompt não tem como ser reconstruído fora do turno que o gerou). A saída: `success: true` com
//! `episodic`+`graph` de verdade e um `notPorted` nomeando só o que falta de verdade — mesmo padrão
//! que `clear_history` já usa para `cleared`/`skipped` (`features/privacy/clear.rs`).
//!
//! Funções de leitura recebem `Option<&EpisodicMemoryService>`/`Option<&SocialMemoryService>` em vez
//! de resolver os singletons internamente — só `run()` faz isso. É o que torna
//! `search_episodic`/`clear_episodic`/`delete_episodic`/`add_fact`/`snapshot` testáveis com um
//! `MemoryStore` em memória, sem depender do `OnceLock` de processo (que nenhum teste de unidade
//! consegue popular — precisa de `AppHandle`, ver `modules/memory/mod.rs::episodic_reader`).

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::memory::episodic::EpisodicMemoryService;
use crate::modules::memory::social::{AddFactInput, ForgetOutcome, ForgetUserProfileInput, SetUserProfileInput, SocialMemoryService};
use crate::modules::tools::ToolHandler;

/// `memoryContextConsoleTool.description` (`tools/context-console.ts`) mais as ações de apagar/editar
/// que o painel de memória chama (`use-memory-console.ts`).
pub(super) const DESCRIPTION: &str =
    "Memory console, only when the user asks to inspect or edit your memory. snapshot: overview; search_episodic {query}; facts: stored facts; add_fact {fact, subject?, predicate?}; delete_episodic {id}; delete_fact {id}; update_fact {id, fact}; set_user_name {name}; forget_user_name; forget_person {name}: erases the person everywhere; delete_relationship {id}; clear_episodic: wipes ALL episodes — explicit request only.";

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn fail(message: &str, code: &str) -> Value {
    json!({ "success": false, "error": message, "errorCode": code })
}

fn ok(data: Value) -> Value {
    json!({ "success": true, "data": data })
}

fn is_blank(value: &Value) -> bool {
    value.as_str().map(str::trim).unwrap_or("").is_empty()
}

/// = `addFact`: `ok(await brain().getSocialMemory().addFact({ fact }))` — a resposta É o fato
/// gravado, sem envelope extra (mesma forma que o TS devolve).
fn add_fact(social: Option<&SocialMemoryService>, recording_allowed: bool, args: &Value) -> Value {
    if is_blank(&args["fact"]) {
        return fail("fact is required", "invalid_args");
    }
    let Some(social) = social else {
        return fail("social memory is unavailable", "unavailable");
    };
    let fact = args["fact"].as_str().unwrap_or_default().to_string();
    match social.add_fact(recording_allowed, AddFactInput { fact, ..Default::default() }, now_ms()) {
        Ok(Some(fact)) => ok(serde_json::to_value(&fact).unwrap_or_else(|_| json!({}))),
        Ok(None) => fail("memory recording is disabled (memory.enabled=false); nothing was saved", "denied"),
        Err(error) => fail(&format!("social memory write failed: {error}"), "internal"),
    }
}

/// = a leitura de grafo do task "facts as a graph beyond people" (defects.md "memory-relationship"
/// P2): `subject`/`predicate` opcionais, `limit` com o MESMO default de `search_episodic` (10) —
/// sem envelope de contagem extra, `data.facts` é a lista já filtrada e ativa.
fn facts(social: Option<&SocialMemoryService>, args: &Value) -> Value {
    let Some(social) = social else {
        return fail("social memory is unavailable", "unavailable");
    };
    let subject = args["subject"].as_str().map(str::trim).filter(|s| !s.is_empty());
    let predicate = args["predicate"].as_str().map(str::trim).filter(|s| !s.is_empty());
    let limit = args["limit"].as_u64().unwrap_or(10) as usize;
    match social.query_facts(subject, predicate, limit) {
        Ok(facts) => ok(json!({ "facts": facts })),
        Err(error) => fail(&format!("facts query failed: {error}"), "internal"),
    }
}

/// = o painel "esquecer fato". Apagar o fato que aposentou outro traz o antigo de volta
/// (`SocialMemoryService::delete_fact`).
fn delete_fact(social: Option<&SocialMemoryService>, args: &Value) -> Value {
    if is_blank(&args["id"]) {
        return fail("id is required", "invalid_args");
    }
    let Some(social) = social else {
        return fail("social memory is unavailable", "unavailable");
    };
    let id = args["id"].as_str().unwrap_or_default();
    match social.delete_fact(id) {
        Ok(true) => ok(json!({ "id": id, "deleted": true })),
        Ok(false) => fail("fact not found", "not_found"),
        Err(error) => fail(&format!("fact delete failed: {error}"), "internal"),
    }
}

/// Edição de um fato (texto e, opcionalmente, confiança) — antes só dava para apagar e recriar.
fn update_fact(social: Option<&SocialMemoryService>, recording_allowed: bool, args: &Value) -> Value {
    if is_blank(&args["id"]) || is_blank(&args["fact"]) {
        return fail("id and fact are required", "invalid_args");
    }
    if !recording_allowed {
        return fail("memory recording is disabled (memory.enabled=false); nothing was changed", "denied");
    }
    let Some(social) = social else {
        return fail("social memory is unavailable", "unavailable");
    };
    let id = args["id"].as_str().unwrap_or_default();
    let text = args["fact"].as_str().unwrap_or_default();
    match social.update_fact(recording_allowed, id, text, args["confidence"].as_f64(), now_ms()) {
        Ok(Some(fact)) => ok(serde_json::to_value(&fact).unwrap_or_else(|_| json!({}))),
        Ok(None) => fail("fact not found", "not_found"),
        Err(error) => fail(&format!("fact update failed: {error}"), "internal"),
    }
}

/// O nome com que a Mika chama o usuário (`preferredName` do perfil social).
fn set_user_name(social: Option<&SocialMemoryService>, recording_allowed: bool, args: &Value) -> Value {
    if is_blank(&args["name"]) {
        return fail("name is required", "invalid_args");
    }
    let Some(social) = social else {
        return fail("social memory is unavailable", "unavailable");
    };
    let name = args["name"].as_str().unwrap_or_default().trim().to_string();
    match social.set_user_profile(recording_allowed, SetUserProfileInput { preferred_name: Some(name), ..Default::default() }, now_ms()) {
        Ok(Some(profile)) => ok(serde_json::to_value(&profile).unwrap_or_else(|_| json!({}))),
        Ok(None) => fail("memory recording is disabled (memory.enabled=false); nothing was saved", "denied"),
        Err(error) => fail(&format!("profile write failed: {error}"), "internal"),
    }
}

fn forget_user_name(social: Option<&SocialMemoryService>, recording_allowed: bool) -> Value {
    let Some(social) = social else {
        return fail("social memory is unavailable", "unavailable");
    };
    let input = ForgetUserProfileInput { fields: vec!["preferredName".to_string()], ..Default::default() };
    match social.forget_user_profile(recording_allowed, input, now_ms()) {
        Ok(ForgetOutcome::Blocked) => fail("memory recording is disabled (memory.enabled=false); nothing was changed", "denied"),
        Ok(_) => ok(json!({ "forgotten": true })),
        Err(error) => fail(&format!("profile write failed: {error}"), "internal"),
    }
}

fn delete_relationship(social: Option<&SocialMemoryService>, args: &Value) -> Value {
    if is_blank(&args["id"]) {
        return fail("id is required", "invalid_args");
    }
    let Some(social) = social else {
        return fail("social memory is unavailable", "unavailable");
    };
    let id = args["id"].as_str().unwrap_or_default();
    match social.delete_relationship(id) {
        Ok(true) => ok(json!({ "id": id, "deleted": true })),
        Ok(false) => fail("relationship not found", "not_found"),
        Err(error) => fail(&format!("relationship delete failed: {error}"), "internal"),
    }
}

/// "Esquecer" uma pessoa de verdade: ficha do catálogo, grafo social (inclusive arestas que
/// apontam para ela) e as lembranças episódicas que a citam — o mesmo alcance de "apagar" na aba
/// Pessoas. Só o nó do grafo deixaria a pessoa viva nas conversas guardadas.
fn forget_person(social: Option<&SocialMemoryService>, catalog: Option<&crate::modules::features::person_catalog::CatalogStore>, args: &Value) -> Value {
    if is_blank(&args["name"]) {
        return fail("name is required", "invalid_args");
    }
    let Some(social) = social else {
        return fail("social memory is unavailable", "unavailable");
    };
    let name = args["name"].as_str().unwrap_or_default().trim();
    match crate::modules::features::person_catalog::forget_person_everywhere(catalog, social.memory(), name) {
        Ok(true) => ok(json!({ "name": name, "forgotten": true })),
        Ok(false) => fail("person not found", "not_found"),
        Err(error) => fail(&format!("forget person failed: {error}"), "internal"),
    }
}

/// = `deleteEpisodic`.
fn delete_episodic(mem: Option<&EpisodicMemoryService>, args: &Value) -> Value {
    if is_blank(&args["id"]) {
        return fail("id is required", "invalid_args");
    }
    let Some(mem) = mem else {
        return fail("episodic delete is unavailable", "unavailable");
    };
    let id = args["id"].as_str().unwrap_or_default();
    match mem.delete(id) {
        Ok(true) => json!({ "success": true, "data": { "id": id, "deleted": true } }),
        Ok(false) => json!({ "success": false, "data": { "id": id, "deleted": false }, "error": "episode not found", "errorCode": "not_found" }),
        Err(error) => fail(&format!("episodic delete failed: {error}"), "internal"),
    }
}

/// = `searchEpisodic`: com `query` não-vazia usa busca por palavra-chave (`mem.search`); sem
/// `query`, devolve o `getAll()` — MESMA ramificação do TS (`query?.trim() ? mem.search(...) :
/// mem.getAll()`), inclusive o default `limit ?? 10` só do lado da busca.
fn search_episodic(mem: Option<&EpisodicMemoryService>, args: &Value) -> Value {
    let Some(mem) = mem else {
        return fail("episodic memory search is unavailable", "unavailable");
    };
    let query = args["query"].as_str().map(str::trim).filter(|q| !q.is_empty());
    let result = match query {
        Some(q) => {
            let limit = args["limit"].as_u64().unwrap_or(10) as usize;
            mem.search(q, limit)
        }
        None => mem.get_all(),
    };
    match result {
        Ok(memories) => ok(json!({ "memories": memories })),
        Err(error) => fail(&format!("episodic memory search failed: {error}"), "internal"),
    }
}

/// = `clearEpisodic`. `episodic::queries::clear` só devolve `Ok(false)` se um `DELETE` não afetar
/// nenhuma linha apesar de existir chave — na prática sempre `Ok(true)`; o ramo `cleared: false`
/// fica pela paridade de FORMATO com o TS (`success`/`error`/`errorCode` todos condicionais a
/// `cleared`), não porque seja alcançável hoje.
fn clear_episodic(mem: Option<&EpisodicMemoryService>) -> Value {
    let Some(mem) = mem else {
        return fail("episodic memory is unavailable", "unavailable");
    };
    match mem.clear() {
        Ok(true) => json!({ "success": true, "data": { "cleared": true } }),
        Ok(false) => json!({
            "success": false,
            "data": { "cleared": false },
            "error": "episodic memory could not be cleared",
            "errorCode": "internal",
        }),
        Err(error) => fail(&format!("episodic memory could not be cleared: {error}"), "internal"),
    }
}

/// = `snapshot`, parcial e nomeado — ver "snapshot fica parcial" no cabeçalho do arquivo.
/// `graph` falhar em ler (erro de I/O do `MemoryStore`, não falta de dado) degrada para o MESMO
/// `notPorted` da ausência de grafo — mesma política do resto do arquivo: nunca finge, nunca crasha
/// o resto do snapshot por causa de UMA peça.
fn snapshot(mem: Option<&EpisodicMemoryService>, social: Option<&SocialMemoryService>) -> Value {
    let Some(mem) = mem else {
        return fail("memory/context console is unavailable", "unavailable");
    };
    let episodic = match mem.get_all() {
        Ok(episodic) => episodic,
        Err(error) => return fail(&format!("snapshot failed: {error}"), "internal"),
    };

    let graph = social.and_then(|s| s.get_graph().ok());
    let mut not_ported: Vec<&str> = Vec::new();
    let mut data = json!({ "episodic": episodic });
    // `promptTrace` ANTES de `socialGraph` no vetor — mesma ordem que o `not_ported` original
    // sempre teve (`vec!["promptTrace"]` seguido de `.push("socialGraph")`), preservada aqui para
    // não quebrar quem lê `notPorted[0]` posicionalmente.
    //
    // `promptTrace` só existe depois do primeiro turno real do processo (`turn::record`, no
    // chokepoint de `run_turn_ollama`/`run_turn_llm`) — nenhum turno ainda rodou é o único caso em
    // que continua faltando de verdade, não mais "sem porte".
    match crate::modules::brain::turn::last_prompt_trace() {
        Some(trace) => data["promptTrace"] = trace,
        None => not_ported.push("promptTrace"),
    }
    match graph {
        Some(graph) => data["graph"] = serde_json::to_value(&graph).unwrap_or_else(|_| json!({})),
        None => not_ported.push("socialGraph"),
    }
    data["notPorted"] = json!(not_ported);
    data["shortTerm"] = short_term();
    data["peopleMemory"] = people_memory();
    ok(data)
}

/// Short-term layer: the raw recent window + rolling summary (chat) and the facts established in
/// this conversation (session accumulator) — what is in the prompt now, before consolidation.
fn short_term() -> Value {
    let (messages, rolling_summary) = crate::modules::chat::short_term_view();
    let session_facts = crate::modules::brain::post_turn::session_accumulator::accumulator().lines();
    json!({ "messages": messages, "rollingSummary": rolling_summary, "sessionFacts": session_facts })
}

/// Long-term people layer at a glance: catalogued people, facts on their cards, open questions.
fn people_memory() -> Value {
    let Some(catalog) = crate::modules::features::person_catalog::catalog_reader() else { return Value::Null };
    let people = catalog.list(None, None, usize::MAX);
    let facts: usize = people.iter().map(|p| p.facts.len()).sum();
    json!({ "people": people.len(), "facts": facts, "pendingClarifications": catalog.pending_clarifications().len() })
}

fn run(args: &Value) -> Value {
    // Único ponto que toca os singletons de processo — as funções acima recebem a referência já
    // resolvida, o que as torna testáveis sem `AppHandle` (ver cabeçalho do arquivo).
    let mem = crate::modules::memory::episodic_reader();
    let social = crate::modules::memory::social_reader();
    match args["action"].as_str().unwrap_or("snapshot") {
        "add_fact" => add_fact(social.as_ref(), crate::modules::memory::recording_allowed_cached(), args),
        "delete_episodic" => delete_episodic(mem.as_ref(), args),
        "clear_episodic" => clear_episodic(mem.as_ref()),
        "search_episodic" => search_episodic(mem.as_ref(), args),
        "facts" => facts(social.as_ref(), args),
        "delete_fact" => delete_fact(social.as_ref(), args),
        "update_fact" => update_fact(social.as_ref(), crate::modules::memory::recording_allowed_cached(), args),
        "set_user_name" => set_user_name(social.as_ref(), crate::modules::memory::recording_allowed_cached(), args),
        "forget_user_name" => forget_user_name(social.as_ref(), crate::modules::memory::recording_allowed_cached()),
        "delete_relationship" => delete_relationship(social.as_ref(), args),
        "forget_person" => forget_person(social.as_ref(), crate::modules::features::person_catalog::catalog_reader(), args),
        "snapshot" => snapshot(mem.as_ref(), social.as_ref()),
        other => fail(&format!("memory_console: ação desconhecida \"{other}\""), "invalid_args"),
    }
}

pub(super) struct MemoryConsoleHandler;

#[async_trait]
impl ToolHandler for MemoryConsoleHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        Ok(run(&args))
    }
}

/// Espelha `memoryContextConsoleTool.inputSchema`. `action` tem default `'snapshot'` no Zod
/// (`.default('snapshot')`), então NÃO entra em `required` — mesma leitura de
/// `zod-inference.ts::unwrapOptionalDefault` para os outros catálogos deste crate.
pub(super) fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": [
                    "snapshot", "search_episodic", "add_fact", "clear_episodic", "delete_episodic", "facts",
                    "delete_fact", "update_fact", "set_user_name", "forget_user_name", "forget_person", "delete_relationship",
                ],
            },
            "query": { "type": "string" },
            "id": { "type": "string" },
            "fact": { "type": "string" },
            "name": { "type": "string" },
            "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
            "subject": { "type": "string" },
            "predicate": { "type": "string" },
            "limit": { "type": "number", "minimum": 1, "maximum": 50 },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_fact_without_fact_is_a_named_invalid_args_error() {
        let result = run(&json!({ "action": "add_fact" }));
        assert_eq!(result, json!({ "success": false, "error": "fact is required", "errorCode": "invalid_args" }));
    }

    #[test]
    fn add_fact_with_a_blank_fact_is_also_invalid_args() {
        let result = run(&json!({ "action": "add_fact", "fact": "   " }));
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    #[test]
    fn add_fact_with_a_real_fact_degrades_honestly_never_claims_it_was_stored() {
        let result = run(&json!({ "action": "add_fact", "fact": "gosta de rust" }));
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("unavailable"));
    }

    #[test]
    fn delete_episodic_without_id_is_a_named_invalid_args_error() {
        let result = run(&json!({ "action": "delete_episodic" }));
        assert_eq!(result, json!({ "success": false, "error": "id is required", "errorCode": "invalid_args" }));
    }

    /// `run()` resolve `episodic_reader()` internamente, e nenhum teste de unidade consegue popular
    /// aquele `OnceLock` (precisa de `AppHandle` — ver `modules/memory/mod.rs`). O que este teste
    /// prova é o FALLBACK honesto quando o store de processo ainda não abriu — não mais "sempre
    /// degrada": os testes abaixo (`search_episodic`/`clear_episodic`/`snapshot` chamados direto,
    /// com um `MemoryStore` em memória injetado) provam o caminho real.
    #[test]
    fn every_data_dependent_action_degrades_honestly_when_the_process_store_never_opened() {
        for action in ["clear_episodic", "search_episodic", "snapshot", "facts"] {
            let result = run(&json!({ "action": action }));
            assert_eq!(result["success"], json!(false), "action {action}");
            assert_eq!(result["errorCode"], json!("unavailable"), "action {action}");
        }
    }

    #[test]
    fn missing_action_defaults_to_snapshot_like_the_zod_default() {
        let result = run(&json!({}));
        assert_eq!(result["error"], json!("memory/context console is unavailable"));
    }

    #[tokio::test]
    async fn handler_never_panics_on_an_empty_payload() {
        let handler = MemoryConsoleHandler;
        let result = handler.call(json!({})).await.unwrap();
        assert_eq!(result["success"], json!(false));
    }

    // --- caminho real, com um MemoryStore injetado (sem tocar o singleton de processo) --------

    fn store() -> crate::modules::memory::MemoryStore {
        crate::modules::memory::MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn search_episodic_without_query_falls_back_to_get_all_newest_first() {
        let kv = store();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "primeiro fato", None, None).unwrap();
        svc.store(true, "segundo fato", None, None).unwrap();

        let result = search_episodic(Some(&svc), &json!({}));
        assert_eq!(result["success"], json!(true));
        let memories = result["data"]["memories"].as_array().unwrap();
        assert_eq!(memories.len(), 2);
        assert_eq!(memories[0]["content"], json!("segundo fato"), "mais recente primeiro");
    }

    #[test]
    fn search_episodic_with_query_uses_keyword_search_not_semantic() {
        let kv = store();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "Mika gosta de Rust", None, None).unwrap();
        svc.store(true, "assunto totalmente diferente", None, None).unwrap();

        let result = search_episodic(Some(&svc), &json!({ "query": "rust" }));
        let memories = result["data"]["memories"].as_array().unwrap();
        assert_eq!(memories.len(), 1, "só o que casa a palavra-chave");
        assert_eq!(memories[0]["content"], json!("Mika gosta de Rust"));
    }

    #[test]
    fn clear_episodic_real_wipes_the_store_and_reports_cleared_true() {
        let kv = store();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "vai sumir", None, None).unwrap();

        let result = clear_episodic(Some(&svc));
        assert_eq!(result, json!({ "success": true, "data": { "cleared": true } }));
        assert!(svc.get_all().unwrap().is_empty());
    }

    #[test]
    fn snapshot_without_social_memory_names_both_pieces_still_not_ported() {
        let kv = store();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "fato real", None, None).unwrap();

        let result = snapshot(Some(&svc), None);
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["episodic"][0]["content"], json!("fato real"));
        assert_eq!(result["data"]["notPorted"], json!(["promptTrace", "socialGraph"]), "nunca finge que promptTrace/graph vieram de verdade");
        assert!(result["data"].get("graph").is_none());
    }

    #[test]
    fn snapshot_with_social_memory_includes_the_real_graph_and_only_names_prompt_trace() {
        let kv = store();
        let episodic = EpisodicMemoryService::new(&kv);
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);
        social.add_relationship(true, crate::modules::memory::social::AddRelationshipInput { name: "Ada".into(), label: "amiga".into(), confidence: None }, 1).unwrap();

        let result = snapshot(Some(&episodic), Some(&social));
        assert_eq!(result["data"]["notPorted"], json!(["promptTrace"]), "grafo social real não pode mais aparecer como faltando");
        assert_eq!(result["data"]["graph"]["people"][0]["primaryName"], json!("Ada"));
    }

    // --- add_fact / delete_episodic, caminho real ----------------------------------------------

    #[test]
    fn add_fact_writes_through_the_social_memory_service_and_returns_the_stored_fact() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);

        let result = add_fact(Some(&social), true, &json!({ "fact": "gosta de rust" }));
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["fact"], json!("gosta de rust"));
        // Documentava o defeito (subject default em inglês, "user") antes da task "facts as a
        // graph" unificar o subject canônico — ver `memory::social::rules::USER_SUBJECT`.
        assert_eq!(result["data"]["subject"], json!(crate::modules::memory::social::USER_SUBJECT));
    }

    #[test]
    fn add_fact_blocked_by_recording_disabled_reports_denied_not_success() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);

        let result = add_fact(Some(&social), false, &json!({ "fact": "nao deveria gravar" }));
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("denied"));
    }

    // --- apagar/editar pelo painel: as ações que a UI chama (antes: "ação desconhecida") ----------

    #[test]
    fn every_action_the_memory_panel_sends_is_in_the_schema_and_handled() {
        let listed = schema()["properties"]["action"]["enum"].as_array().unwrap().clone();
        for action in ["delete_fact", "update_fact", "set_user_name", "forget_user_name", "forget_person", "delete_relationship"] {
            assert!(listed.contains(&json!(action)), "{action} fora do schema");
            let result = run(&json!({ "action": action }));
            assert_ne!(result["error"].as_str().unwrap_or("").contains("ação desconhecida"), true, "{action} sem handler");
        }
    }

    #[test]
    fn delete_fact_removes_the_fact_and_reports_missing_ids() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);
        let stored = social.add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1).unwrap().unwrap();

        assert_eq!(delete_fact(Some(&social), &json!({}))["errorCode"], json!("invalid_args"));
        let gone = delete_fact(Some(&social), &json!({ "id": stored.id }));
        assert_eq!(gone["success"], json!(true));
        assert!(social.query_facts(None, None, 10).unwrap().is_empty());
        assert_eq!(delete_fact(Some(&social), &json!({ "id": stored.id }))["errorCode"], json!("not_found"));
    }

    #[test]
    fn update_fact_rewrites_the_text_through_the_redacting_path() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);
        let stored = social.add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1).unwrap().unwrap();

        let result = update_fact(Some(&social), true, &json!({ "id": stored.id, "fact": "gosta de go" }));
        assert_eq!(result["success"], json!(true), "{result}");
        let facts = social.query_facts(None, None, 10).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].fact, "gosta de go");
        assert_eq!(update_fact(Some(&social), true, &json!({ "id": stored.id, "fact": "  " }))["errorCode"], json!("invalid_args"));
        assert_eq!(update_fact(Some(&social), false, &json!({ "id": stored.id, "fact": "x y z" }))["errorCode"], json!("denied"));
    }

    #[test]
    fn user_name_can_be_set_and_forgotten_from_the_panel() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);

        assert_eq!(set_user_name(Some(&social), true, &json!({ "name": " " }))["errorCode"], json!("invalid_args"));
        assert_eq!(set_user_name(Some(&social), false, &json!({ "name": "Ana" }))["errorCode"], json!("denied"));
        assert_eq!(set_user_name(Some(&social), true, &json!({ "name": "Ana" }))["success"], json!(true));
        assert_eq!(social.get_user_profile().unwrap().unwrap().preferred_name.as_deref(), Some("Ana"));

        assert_eq!(forget_user_name(Some(&social), true)["success"], json!(true));
        assert_eq!(social.get_user_profile().unwrap().unwrap().preferred_name, None);
    }

    #[test]
    fn delete_relationship_removes_the_edge() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);
        let edge = social
            .add_relationship(true, crate::modules::memory::social::AddRelationshipInput { name: "Ada".into(), label: "amiga".into(), confidence: None }, 1)
            .unwrap()
            .unwrap();

        assert_eq!(delete_relationship(Some(&social), &json!({}))["errorCode"], json!("invalid_args"));
        assert_eq!(delete_relationship(Some(&social), &json!({ "id": edge.id }))["success"], json!(true));
        assert_eq!(delete_relationship(Some(&social), &json!({ "id": edge.id }))["errorCode"], json!("not_found"));
    }

    #[test]
    fn forget_person_erases_the_person_everywhere_not_just_the_graph_node() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);
        social
            .add_relationship(true, crate::modules::memory::social::AddRelationshipInput { name: "Ada".into(), label: "amiga".into(), confidence: None }, 1)
            .unwrap();
        EpisodicMemoryService::new(&kv).store(true, "Ada veio jogar", None, None).unwrap();
        EpisodicMemoryService::new(&kv).store(true, "gosta de rust", None, None).unwrap();

        assert_eq!(forget_person(Some(&social), None, &json!({}))["errorCode"], json!("invalid_args"));
        let result = forget_person(Some(&social), None, &json!({ "name": "Ada" }));
        assert_eq!(result["success"], json!(true), "{result}");
        assert!(social.get_graph().unwrap().people.is_empty());
        let left: Vec<String> = EpisodicMemoryService::new(&kv).get_all().unwrap().into_iter().map(|e| e.content).collect();
        assert_eq!(left, ["gosta de rust"]);
        assert_eq!(forget_person(Some(&social), None, &json!({ "name": "Ada" }))["errorCode"], json!("not_found"));
    }

    // --- facts, caminho real ---------------------------------------------------------------------

    #[test]
    fn facts_answers_do_que_eu_gosto_by_subject_alias_without_a_predicate() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);
        social.add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1).unwrap();

        // A tool pede pelo alias "eu" — tem de bater com o subject canônico que add_fact usou.
        let result = facts(Some(&social), &json!({ "subject": "eu" }));
        assert_eq!(result["success"], json!(true));
        let list = result["data"]["facts"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["fact"], json!("gosta de rust"));
    }

    #[test]
    fn facts_answers_qual_jogo_eu_jogo_by_predicate() {
        let kv = store();
        let social = crate::modules::memory::social::SocialMemoryService::new(&kv);
        social.add_fact(true, AddFactInput { fact: "joga elden ring".into(), predicate: Some("joga".into()), object: Some("elden ring".into()), ..Default::default() }, 1).unwrap();
        social.add_fact(true, AddFactInput { fact: "gosta de rust".into(), predicate: Some("gosta de".into()), ..Default::default() }, 2).unwrap();

        let result = facts(Some(&social), &json!({ "predicate": "joga" }));
        let list = result["data"]["facts"].as_array().unwrap();
        assert_eq!(list.len(), 1, "predicate filtra fora o fato que não é sobre jogar");
        assert_eq!(list[0]["object"], json!("elden ring"));
    }

    #[test]
    fn facts_without_social_memory_degrades_honestly() {
        let result = facts(None, &json!({}));
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("unavailable"));
    }

    #[test]
    fn delete_episodic_real_deletes_an_existing_entry_and_names_a_missing_one() {
        let kv = store();
        let svc = EpisodicMemoryService::new(&kv);
        let id = svc.store(true, "vai sumir", None, None).unwrap().unwrap();

        let deleted = delete_episodic(Some(&svc), &json!({ "id": id }));
        assert_eq!(deleted, json!({ "success": true, "data": { "id": id, "deleted": true } }));

        let missing = delete_episodic(Some(&svc), &json!({ "id": "ghost" }));
        assert_eq!(missing["success"], json!(false));
        assert_eq!(missing["errorCode"], json!("not_found"));
    }
}
