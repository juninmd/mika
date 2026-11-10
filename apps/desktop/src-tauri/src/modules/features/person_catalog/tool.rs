//! Tool `person_catalog` (LLM) — porte de
//! `apps/desktop/src/features/person-catalog/tools/index.ts` + `handlers/index.ts::runAction` +
//! `handlers/person-catalog-queries.ts` + `handlers/person-catalog-delete.ts` +
//! `handlers/person-catalog-social-actions.ts`.
//!
//! ## As 11 ações de `tools/schema.ts::ACTIONS` — o que dispacha de verdade agora
//!
//! | Ação | Backend TS | Rust hoje |
//! |---|---|---|
//! | `upsert`/`get`/`search`/`list`/`add_mention`/`update_social` | `PersonCatalogService` → `catalog.json` | **dispacha de verdade** ([`super::store::CatalogStore`]) |
//! | `add_fact`/`delete_fact`/`add_relationship`/`delete_relationship` | `ports.ts::socialMemoryOrNull()` → `TOKENS.BrainService.getSocialMemory()` | **dispacha de verdade**, a partir desta rodada — [`crate::modules::memory::social::SocialMemoryService`], mesmo `memory.db` que a memória episódica já usa (ver o cabeçalho de `modules::memory::social`) |
//! | `delete` | catálogo + `forgetPersonEverywhere` (social + episódica) | **dispacha de verdade nas três pernas** — catálogo ([`super::store::CatalogStore::delete`]), social ([`SocialMemoryService::forget_person`]) e episódica ([`EpisodicMemoryService::forget_mentions`]); `data.warning` só aparece quando o grafo social não abriu OU a varredura episódica falhou (ver [`handle_delete`]) |
//!
//! `maxMentionsPerPerson` é configurável no TS (`person-catalog.maxMentionsPerPerson`, schema
//! `1..=500`, default 100). Aqui é a constante [`DEFAULT_MAX_MENTIONS`] — só `AppHandle` foi
//! adicionado à assinatura de registro (para resolver o caminho de `catalog.json`), não
//! `ConfigStore`; fiar a mais uma dependência de DI por um valor cujo default já cobre o caso comum
//! não valia o acoplamento extra nesta rodada. Gap nomeado, não fingido.
//!
//! ## `ConfigStore`/`MemoryStore`, a partir desta rodada
//! `register_tools(app)` passa a resolver `crate::state::config_store(app)` (gate `memory.enabled`
//! para `add_fact`/`add_relationship`, MESMA leitura que `modules::memory::memory_recording_allowed`
//! faz para a memória episódica) e `crate::modules::memory::memory_store(app)` (o `MemoryStore` de
//! processo por trás do grafo social). Nenhum dos dois é uma segunda instância: `config_store` é
//! cacheado em `ShellState` (chamar de novo devolve o mesmo `Arc`, ver `state.rs`) e `memory_store` é
//! um `OnceLock` próprio já aberto por `features::privacy::clear::register` antes deste ponto. Se
//! qualquer um falhar ao resolver, o catálogo continua registrando igual — só as 4 ações sociais e a
//! varredura de `delete` degradam, nomeadas, em vez de derrubar a tool inteira.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::memory::social::{AddFactInput, AddRelationshipInput, SocialMemoryService};
use crate::modules::memory::MemoryStore;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::pii::redact_pii;
use super::store::CatalogStore;
use super::types::{Confidence, MentionSource, NewMention, PersonRelationship, PersonType, RelationshipType, UpsertAttribute, UpsertInput};

const DESCRIPTION: &str = "People memory (real people, characters, VTubers, organizations). upsert: create/update a profile (`name`, `type`, `origin`, short `bio`) silently — never for a passing mention. get/search/list: read profiles. add_mention: log that someone came up. update_social: affinity/trust toward them. add_fact/delete_fact, add_relationship/delete_relationship: edit facts and links. relationships: a person's links; path: how two people connect (`from`/`to`, \"user\" = the user). delete: forget a person everywhere — only on an explicit request, with confirm=true.";

/// As 11 ações originais de `ACTIONS` (`tools/schema.ts`) mais `relationships`/`path` — as 2 ações
/// de LEITURA do grafo social que o pacote "people catalog + relationship graph" acrescentou (item
/// 4: "quem é o Pedro?"/"como o João conhece a Maria?" não eram respondíveis por nenhuma ação antes
/// desta rodada — ver o cabeçalho de `memory::social::mod` e o teste
/// `actions_match_the_frozen_ts_schema_snapshot` abaixo, que agora também trava ESTAS 2 como parte
/// do contrato, não só o snapshot congelado do TS).
const ACTIONS: [&str; 13] = [
    "upsert",
    "get",
    "search",
    "list",
    "delete",
    "add_mention",
    "update_social",
    "add_fact",
    "delete_fact",
    "add_relationship",
    "delete_relationship",
    "relationships",
    "path",
];

/// Porte do default de `person-catalog.schema.ts::maxMentionsPerPerson` — ver o cabeçalho do arquivo.
const DEFAULT_MAX_MENTIONS: usize = 100;
const DEFAULT_LIMIT: u64 = 20;
const MAX_LIMIT: u64 = 50;

/// Um único `json!` com as ~60 propriedades estourava `recursion_limit` da macro (erro de
/// compilação, não de lógica). Construído campo a campo — cada `json!` interno fica pequeno — para
/// não mexer no `recursion_limit` do crate inteiro por causa de uma tool.
fn input_schema() -> Value {
    let mut properties = serde_json::Map::new();
    let mut prop = |name: &str, schema: Value| {
        properties.insert(name.to_string(), schema);
    };

    prop("action", json!({ "type": "string", "enum": ACTIONS.to_vec() }));
    prop("name", json!({ "type": "string" }));
    prop("identifier", json!({ "type": "string", "description": "Person id, name or alias." }));
    prop("id", json!({ "type": "string" }));
    prop("fact", json!({ "type": "string" }));
    prop("subject", json!({ "type": "string" }));
    prop("label", json!({ "type": "string" }));
    prop("confidence", json!({ "type": "number", "minimum": 0, "maximum": 1 }));
    prop("query", json!({ "type": "string" }));
    prop("confirm", json!({ "type": "boolean", "description": "action=delete: must be true." }));
    prop("limit", json!({ "type": "integer", "minimum": 1, "maximum": 200 }));
    prop("sortBy", json!({ "type": "string", "enum": ["mentionCount", "lastSeenAt", "createdAt", "name"] }));
    prop("aliases", json!({ "type": "array", "items": { "type": "string" } }));
    prop("type", json!({ "type": "string", "description": "e.g. real_person, fictional_character, organization, vtuber." }));
    prop("bio", json!({ "type": "string", "maxLength": 800 }));
    prop("gender", json!({ "type": "string" }));
    prop("birthDate", json!({ "type": "string" }));
    prop("birthPlace", json!({ "type": "string" }));
    prop("deathDate", json!({ "type": "string" }));
    prop("nationality", json!({ "type": "string" }));
    prop("languages", json!({ "type": "array", "items": { "type": "string" } }));
    prop("occupation", json!({ "type": "array", "items": { "type": "string" } }));
    prop("knownFor", json!({ "type": "array", "items": { "type": "string" } }));
    prop("activeYears", json!({ "type": "string" }));
    prop("awards", json!({ "type": "array", "items": { "type": "string" } }));
    prop("socialLinks", json!({ "type": "object", "additionalProperties": { "type": "string" } }));
    // Antes desta rodada este `object` genérico não dizia ao modelo QUAIS campos preencher (ver o
    // pacote "people catalog + relationship graph", item 4: "the relationships array currently has
    // zero documented shape") — `name`/`type` são obrigatórios, o resto opcional, mesmo shape de
    // `PersonRelationship` (`types.rs`).
    prop(
        "relationships",
        json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "type": { "type": "string", "description": "Free-text relationship label, e.g. \"works with\", \"colleague\"." },
                    "affinity": { "type": "number" },
                    "trust": { "type": "number" },
                    "description": { "type": "string" },
                },
                "required": ["name", "type"],
            },
        }),
    );
    prop(
        "relationshipType",
        json!({
            "type": "string",
            "enum": ["stranger", "acquaintance", "friend", "close_friend", "best_friend", "rival", "enemy", "vvip", "creator"],
        }),
    );
    prop("relationToUser", json!({ "type": "string", "description": "Free-text kinship/role with the USER, e.g. \"brother\", \"boss\" — distinct from relationshipType (the affinity ladder)." }));
    prop("from", json!({ "type": "string", "description": "action='path': a person's name/identifier, or \"user\" for the user themselves." }));
    prop("to", json!({ "type": "string", "description": "action='path': same as 'from', the other end of the search." }));
    prop("maxDepth", json!({ "type": "integer", "minimum": 1, "maximum": 3, "description": "action='path': how many hops to search (capped at 3)." }));
    prop("affinity", json!({ "type": "number", "minimum": -100, "maximum": 100 }));
    prop("trustLevel", json!({ "type": "number", "minimum": -100, "maximum": 100 }));
    prop("affinityChange", json!({ "type": "number", "minimum": -100, "maximum": 100 }));
    prop("trustChange", json!({ "type": "number", "minimum": -100, "maximum": 100 }));
    prop("origin", json!({ "type": "string" }));
    prop("role", json!({ "type": "string" }));
    prop("physicalDescription", json!({ "type": "string" }));
    prop("abilities", json!({ "type": "array", "items": { "type": "string" } }));
    prop("affiliation", json!({ "type": "string" }));
    prop("species", json!({ "type": "string" }));
    prop("firstAppearance", json!({ "type": "string" }));
    prop("voiceActor", json!({ "type": "string" }));
    prop("userRating", json!({ "type": "string" }));
    prop("userNotes", json!({ "type": "string" }));
    prop("mentionText", json!({ "type": "string" }));
    prop("mentionSource", json!({ "type": "string", "enum": ["chat", "voice", "vision", "manual", "import"] }));
    prop("mentionUrl", json!({ "type": "string" }));
    prop("mentionAppName", json!({ "type": "string" }));
    prop("mentionWindowTitle", json!({ "type": "string" }));
    prop("text", json!({ "type": "string" }));
    prop("source", json!({ "type": "string", "enum": ["chat", "voice", "vision", "manual", "import"] }));
    prop("url", json!({ "type": "string" }));
    prop("appName", json!({ "type": "string" }));
    prop("windowTitle", json!({ "type": "string" }));
    prop("context", json!({ "type": "string" }));
    prop("tags", json!({ "type": "array", "items": { "type": "string" } }));
    prop("attributes", json!({ "type": "array", "items": { "type": "object" } }));
    prop("notes", json!({ "type": "string" }));

    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": ["action"],
    })
}

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "person_catalog".into(),
        description: Some(DESCRIPTION.into()),
        input_schema: input_schema(),
        plugin_id: "mika.person-catalog".into(),
        requires_window: false,
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn non_empty(args: &Value, key: &str) -> Option<String> {
    opt_str(args, key).filter(|s| !s.is_empty())
}

fn opt_str_array(args: &Value, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(Value::as_array).map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
}

fn opt_f64(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(Value::as_f64)
}

fn parse_enum<T: serde::de::DeserializeOwned>(args: &Value, key: &str) -> Option<T> {
    args.get(key).cloned().and_then(|v| serde_json::from_value(v).ok())
}

fn invalid_args(message: &str) -> Value {
    json!({ "success": false, "error": message, "errorCode": "invalid_args" })
}

fn not_found(message: String) -> Value {
    json!({ "success": false, "error": message, "errorCode": "not_found" })
}

/// Um erro de `CatalogStore` já vem como texto nomeando a causa ("Person not found: X" ou uma falha
/// de I/O); a única distinção que a tool precisa fazer é se é "não achou" (visível ao LLM como
/// `not_found`) ou uma falha de armazenamento (`unavailable`).
fn store_error(message: String) -> Value {
    let code = if message.starts_with("Person not found") { "not_found" } else { "unavailable" };
    json!({ "success": false, "error": message, "errorCode": code })
}

fn parse_relationships(args: &Value) -> Option<Vec<PersonRelationship>> {
    let arr = args.get("relationships")?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|item| {
                Some(PersonRelationship {
                    name: item.get("name")?.as_str()?.to_string(),
                    kind: item.get("type")?.as_str()?.to_string(),
                    affinity: item.get("affinity").and_then(Value::as_f64),
                    trust: item.get("trust").and_then(Value::as_f64),
                    description: item.get("description").and_then(Value::as_str).map(str::to_string),
                    last_interaction_at: None,
                })
            })
            .collect(),
    )
}

/// Espelha `attributes?.map(attr => ({...attr, value: redactPii(attr.value)}))` — PII redigida ANTES
/// de chegar em `CatalogStore::upsert`, nunca depois.
fn parse_attributes(args: &Value) -> Option<Vec<UpsertAttribute>> {
    let arr = args.get("attributes")?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|item| {
                let key = item.get("key")?.as_str()?.to_string();
                let raw_value = item.get("value")?.as_str()?;
                let confidence = item.get("confidence").cloned().and_then(|v| serde_json::from_value::<Confidence>(v).ok());
                Some(UpsertAttribute { key, value: redact_pii(raw_value), confidence })
            })
            .collect(),
    )
}

/// Espelha o `mention` que `handlers/index.ts::upsert` monta a partir de `mentionText`/
/// `mentionSource`/`mentionUrl`/`mentionAppName`/`mentionWindowTitle` — texto redigido de PII ANTES
/// de tocar o store.
fn build_upsert_mention(args: &Value) -> Option<NewMention> {
    let text = non_empty(args, "mentionText")?;
    Some(NewMention {
        source: parse_enum::<MentionSource>(args, "mentionSource").unwrap_or(MentionSource::Chat),
        text: redact_pii(&text),
        timestamp: now_ms(),
        url: opt_str(args, "mentionUrl"),
        app_name: opt_str(args, "mentionAppName"),
        window_title: opt_str(args, "mentionWindowTitle"),
        context: None,
    })
}

/// Porte do corpo de `handlers/index.ts::upsert` que monta `UpsertInput` — bio/userNotes/atributos
/// redigidos de PII no MESMO chokepoint que o TS usa, antes de `CatalogStore::upsert` gravar.
fn build_upsert_input(args: &Value) -> UpsertInput {
    UpsertInput {
        id: opt_str(args, "id"),
        name: opt_str(args, "name").unwrap_or_default(),
        aliases: opt_str_array(args, "aliases"),
        kind: opt_str(args, "type").and_then(|t| PersonType::parse(&t)),
        bio: opt_str(args, "bio").map(|b| redact_pii(&b)),
        gender: opt_str(args, "gender"),
        birth_date: opt_str(args, "birthDate"),
        birth_place: opt_str(args, "birthPlace"),
        death_date: opt_str(args, "deathDate"),
        nationality: opt_str(args, "nationality"),
        languages: opt_str_array(args, "languages"),
        occupation: opt_str_array(args, "occupation"),
        known_for: opt_str_array(args, "knownFor"),
        active_years: opt_str(args, "activeYears"),
        awards: opt_str_array(args, "awards"),
        social_links: args.get("socialLinks").and_then(Value::as_object).cloned(),
        relationships: parse_relationships(args),
        relationship_type: parse_enum::<RelationshipType>(args, "relationshipType"),
        relation_to_user: opt_str(args, "relationToUser"),
        affinity: opt_f64(args, "affinity"),
        trust_level: opt_f64(args, "trustLevel"),
        origin: opt_str(args, "origin"),
        role: opt_str(args, "role"),
        physical_description: opt_str(args, "physicalDescription"),
        abilities: opt_str_array(args, "abilities"),
        affiliation: opt_str(args, "affiliation"),
        species: opt_str(args, "species"),
        first_appearance: opt_str(args, "firstAppearance"),
        voice_actor: opt_str(args, "voiceActor"),
        user_rating: opt_str(args, "userRating"),
        user_notes: opt_str(args, "userNotes").map(|n| redact_pii(&n)),
        attributes: parse_attributes(args),
        mention: build_upsert_mention(args),
        tags: opt_str_array(args, "tags"),
        notes: opt_str(args, "notes"),
        image_url: None,
    }
}

fn handle_upsert(store: &CatalogStore, args: &Value) -> Value {
    if non_empty(args, "name").is_none() {
        return invalid_args("action='upsert' requires name.");
    }
    match store.upsert(build_upsert_input(args), now_ms(), DEFAULT_MAX_MENTIONS) {
        Ok(outcome) => match outcome.person {
            Some(p) => {
                // `Events.PERSON_CREATED` nunca teve ouvinte real no TS (só `updated`/`deleted`/
                // `mention-added` — ver `mika-event-channels.ts`), então um upsert publica
                // `PersonUpdated` nos dois casos (criado ou atualizado): é o mesmo "recarregue a
                // lista" para a UI, independente de `outcome.created`.
                super::events::publish_updated(&p, outcome.created);
                json!({
                    "success": true,
                    "data": {
                        "action": "upsert",
                        "id": p.id,
                        "primaryName": p.primary_name,
                        "relationshipType": p.relationship_type,
                        "affinity": p.affinity,
                        "trustLevel": p.trust_level,
                        "created": outcome.created,
                        "ambiguousWith": outcome.ambiguous_with,
                    },
                })
            }
            None => json!({ "success": false, "error": "This person is blocked by the user and was not cataloged.", "errorCode": "denied" }),
        },
        Err(error) => store_error(error),
    }
}

fn find_person(store: &CatalogStore, identifier: &str) -> Option<super::types::PersonRecord> {
    store.get(identifier).or_else(|| store.find_by_alias(identifier))
}

fn handle_get(store: &CatalogStore, args: &Value) -> Value {
    let Some(identifier) = non_empty(args, "identifier") else {
        return invalid_args("action='get' requires identifier.");
    };
    match find_person(store, &identifier) {
        Some(person) => json!({ "success": true, "data": person }),
        None => not_found(format!("Person not found: {identifier}")),
    }
}

fn handle_search(store: &CatalogStore, args: &Value) -> Value {
    let Some(query) = non_empty(args, "query") else {
        return invalid_args("action='search' requires query.");
    };
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let data: Vec<Value> = store
        .search(&query, limit)
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id, "primaryName": p.primary_name, "aliases": p.aliases, "type": p.kind,
                "role": p.role, "origin": p.origin,
                "bio": p.bio.map(|b| b.chars().take(200).collect::<String>()),
                "nationality": p.nationality, "occupation": p.occupation, "knownFor": p.known_for,
                "tags": p.tags, "mentionCount": p.mention_count, "lastSeenAt": p.last_seen_at,
                "updatedAt": p.updated_at, "confidence": p.confidence,
            })
        })
        .collect();
    json!({ "success": true, "data": data })
}

fn handle_list(store: &CatalogStore, args: &Value) -> Value {
    let kind = opt_str(args, "type").and_then(|t| PersonType::parse(&t));
    let sort_by = args.get("sortBy").and_then(Value::as_str);
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let data: Vec<Value> = store
        .list(kind, sort_by, limit)
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id, "primaryName": p.primary_name, "type": p.kind, "role": p.role, "origin": p.origin,
                "occupation": p.occupation, "mentionCount": p.mention_count, "lastSeenAt": p.last_seen_at,
                "tags": p.tags, "imageUrl": p.image_url, "enriched": p.enriched, "confidence": p.confidence,
            })
        })
        .collect();
    json!({ "success": true, "data": data })
}

/// Espelha `handlers/person-catalog-delete.ts::remove` + `forget-person.ts::forgetPersonEverywhere`
/// — varre as TRÊS pernas (catálogo, grafo social, memória episódica) SEMPRE, mesmo quando a pessoa
/// não está no catálogo: mesma ordem do TS, onde a ausência no catálogo não impede a varredura
/// social/episódica, só muda o que `catalogDeleted` reporta. Os termos varridos na memória episódica
/// são a UNIÃO do nome/aliases do catálogo com o nome/aliases do grafo social (o TS faz em duas
/// chamadas — `forgetPersonEverywhere` e depois `sweepExtraTerms` — aqui é uma só, mesmo efeito).
fn handle_delete(store: &CatalogStore, social_mem: Option<&'static MemoryStore>, args: &Value) -> Value {
    let identifier = non_empty(args, "id").or_else(|| non_empty(args, "identifier"));
    let Some(identifier) = identifier else {
        return invalid_args("action='delete' requires id or identifier.");
    };
    if !args.get("confirm").and_then(Value::as_bool).unwrap_or(false) {
        return invalid_args("Confirmation is required to erase a person from memory.");
    }

    let person = find_person(store, &identifier);
    let catalog_deleted = match &person {
        Some(p) => {
            let deleted = store.delete(&p.id).is_ok();
            if deleted {
                super::events::publish_deleted(&p.id);
            }
            deleted
        }
        None => false,
    };

    let mut terms: Vec<String> = Vec::new();
    if let Some(p) = &person {
        terms.push(p.primary_name.clone());
        terms.extend(p.aliases.clone());
    }

    let mut social_deleted = false;
    let mut episodic_removed = 0usize;
    let mut episodic_error: Option<String> = None;

    if let Some(mem) = social_mem {
        let legs = super::forget::forget_in_memory(mem, &identifier, terms);
        social_deleted = legs.social_deleted;
        episodic_removed = legs.episodic_removed;
        episodic_error = legs.episodic_error;
    }

    let deleted = catalog_deleted || social_deleted;
    if !deleted {
        return not_found(format!("Person not found: {identifier}"));
    }

    let mut data = json!({
        "action": "delete",
        "catalogDeleted": catalog_deleted,
        "socialDeleted": social_deleted,
        "episodicRemoved": episodic_removed,
    });
    if social_mem.is_none() {
        data["warning"] = json!(
            "Social memory graph is unavailable right now (the process memory store did not open); only the catalog entry was swept."
        );
    } else if let Some(error) = episodic_error {
        data["warning"] = json!(format!(
            "Person and social profile were permanently deleted, but episodic memory cleanup failed: {error}"
        ));
    }
    json!({ "success": true, "data": data })
}

fn handle_update_social(store: &CatalogStore, args: &Value) -> Value {
    let Some(identifier) = non_empty(args, "identifier") else {
        return invalid_args("action='update_social' requires identifier.");
    };
    let Some(person) = find_person(store, &identifier) else {
        return not_found(format!("Person not found: {identifier}"));
    };

    let input = UpsertInput {
        // Pelo `id`: resolver só pelo nome clonava a ficha quando o nome é ambíguo.
        id: Some(person.id),
        name: person.primary_name,
        affinity: opt_f64(args, "affinityChange"),
        trust_level: opt_f64(args, "trustChange"),
        relationship_type: parse_enum::<RelationshipType>(args, "relationshipType"),
        user_notes: non_empty(args, "notes").map(|n| format!("Relacionamento atualizado: {}", redact_pii(&n))),
        ..Default::default()
    };
    match store.upsert(input, now_ms(), DEFAULT_MAX_MENTIONS) {
        Ok(outcome) => match outcome.person {
            Some(p) => {
                super::events::publish_updated(&p, outcome.created);
                json!({
                    "success": true,
                    "data": {
                        "action": "update_social", "id": p.id, "name": p.primary_name,
                        "affinity": p.affinity, "trustLevel": p.trust_level, "relationshipType": p.relationship_type,
                    },
                })
            }
            None => json!({ "success": false, "error": "This person is blocked by the user.", "errorCode": "denied" }),
        },
        Err(error) => store_error(error),
    }
}

fn handle_add_mention(store: &CatalogStore, args: &Value) -> Value {
    let Some(identifier) = non_empty(args, "identifier") else {
        return invalid_args("action='add_mention' requires identifier.");
    };
    let Some(text) = non_empty(args, "text") else {
        return invalid_args("action='add_mention' requires text.");
    };
    let mention = NewMention {
        source: parse_enum::<MentionSource>(args, "source").unwrap_or(MentionSource::Chat),
        text: redact_pii(&text),
        timestamp: now_ms(),
        url: opt_str(args, "url"),
        app_name: opt_str(args, "appName"),
        window_title: opt_str(args, "windowTitle"),
        context: opt_str(args, "context").map(|c| redact_pii(&c)),
    };
    match store.add_mention(&identifier, mention, now_ms(), DEFAULT_MAX_MENTIONS) {
        Ok(person) => {
            super::events::publish_mention_added(&person);
            json!({ "success": true, "data": { "action": "add_mention", "person": person } })
        }
        Err(error) => store_error(error),
    }
}

/// Resposta comum às 4 ações sociais quando o `MemoryStore` de processo não abriu — não é o mesmo
/// texto de antes ("depende do módulo brain"): a partir desta rodada o grafo social TEM porte Rust
/// ([`crate::modules::memory::social`]); a única causa real de indisponibilidade agora é o store de
/// processo não ter aberto (ver `register_tools`).
fn no_social_memory() -> Value {
    json!({
        "success": false,
        "error": "Memory of facts and relationships is unavailable right now (the process memory store did not open).",
        "errorCode": "unavailable",
    })
}

/// = `MEMORY_DISABLED` de `handlers/person-catalog-social-actions.ts` — `memory.enabled=false`.
fn memory_recording_disabled() -> Value {
    json!({
        "success": false,
        "error": "Memory recording is disabled (memory.enabled=false); nothing was saved.",
        "errorCode": "denied",
    })
}

/// Toggle mestre da feature desligado (`person-catalog.enabled=false`) — nenhuma das 11 ações
/// dispacha, catalogação e grafo social inclusos.
fn catalog_disabled() -> Value {
    json!({
        "success": false,
        "error": "The person-catalog feature is disabled (person-catalog.enabled=false); no action was taken.",
        "errorCode": "denied",
    })
}

/// = `person-catalog-social-actions.ts::addFact`.
fn handle_add_fact(social_mem: Option<&'static MemoryStore>, recording_allowed: bool, args: &Value) -> Result<Value, String> {
    let Some(fact) = non_empty(args, "fact") else {
        return Ok(invalid_args("action='add_fact' requires fact."));
    };
    let Some(mem) = social_mem else { return Ok(no_social_memory()) };
    let input = AddFactInput { subject: opt_str(args, "subject"), fact, confidence: opt_f64(args, "confidence"), source_turn_id: None, predicate: None, object: None };
    match SocialMemoryService::new(mem).add_fact(recording_allowed, input, now_ms())? {
        Some(fact) => {
            let mut data = serde_json::to_value(&fact).unwrap_or_else(|_| json!({}));
            data["action"] = json!("add_fact");
            Ok(json!({ "success": true, "data": data }))
        }
        None => Ok(memory_recording_disabled()),
    }
}

/// = `person-catalog-social-actions.ts::deleteFact`.
fn handle_delete_fact(social_mem: Option<&'static MemoryStore>, args: &Value) -> Result<Value, String> {
    let Some(id) = non_empty(args, "id") else {
        return Ok(invalid_args("action='delete_fact' requires id."));
    };
    let Some(mem) = social_mem else { return Ok(no_social_memory()) };
    if !SocialMemoryService::new(mem).delete_fact(&id)? {
        return Ok(not_found(format!("Fact not found: {id}")));
    }
    Ok(json!({ "success": true, "data": { "action": "delete_fact", "deleted": true } }))
}

/// = `person-catalog-social-actions.ts::addRelationship`.
fn handle_add_relationship(social_mem: Option<&'static MemoryStore>, recording_allowed: bool, args: &Value) -> Result<Value, String> {
    if non_empty(args, "name").is_none() && non_empty(args, "identifier").is_none() {
        return Ok(invalid_args("action='add_relationship' requires name."));
    }
    let Some(label) = non_empty(args, "label") else {
        return Ok(invalid_args("action='add_relationship' requires label."));
    };
    let Some(mem) = social_mem else { return Ok(no_social_memory()) };
    let name = non_empty(args, "name").or_else(|| non_empty(args, "identifier")).unwrap_or_default();
    let input = AddRelationshipInput { name, label, confidence: opt_f64(args, "confidence") };
    match SocialMemoryService::new(mem).add_relationship(recording_allowed, input, now_ms())? {
        Some(edge) => {
            let mut data = serde_json::to_value(&edge).unwrap_or_else(|_| json!({}));
            data["action"] = json!("add_relationship");
            Ok(json!({ "success": true, "data": data }))
        }
        None => Ok(memory_recording_disabled()),
    }
}

/// = `person-catalog-social-actions.ts::deleteRelationship`.
fn handle_delete_relationship(social_mem: Option<&'static MemoryStore>, args: &Value) -> Result<Value, String> {
    let Some(id) = non_empty(args, "id") else {
        return Ok(invalid_args("action='delete_relationship' requires id."));
    };
    let Some(mem) = social_mem else { return Ok(no_social_memory()) };
    if !SocialMemoryService::new(mem).delete_relationship(&id)? {
        return Ok(not_found(format!("Relationship not found: {id}")));
    }
    Ok(json!({ "success": true, "data": { "action": "delete_relationship", "deleted": true } }))
}

/// = item 4 do pacote "people catalog + relationship graph": "quem é o Pedro [pra mim]?" — toda
/// aresta ativa que toca `name` (pessoa ou "user"), nos dois sentidos.
fn handle_relationships(social_mem: Option<&'static MemoryStore>, args: &Value) -> Result<Value, String> {
    let Some(name) = non_empty(args, "name").or_else(|| non_empty(args, "identifier")) else {
        return Ok(invalid_args("action='relationships' requires name."));
    };
    let Some(mem) = social_mem else { return Ok(no_social_memory()) };
    let edges = SocialMemoryService::new(mem).relationships_for(&name)?;
    Ok(json!({ "success": true, "data": { "action": "relationships", "name": name, "relationships": edges } }))
}

/// = item 4: "como o João conhece a Maria?" — BFS até `maxDepth` (teto 3) sobre o grafo social
/// inteiro. `path: null` (não vazio) é a resposta quando nada conecta os dois dentro do teto — a
/// tool nunca inventa um caminho.
fn handle_path(social_mem: Option<&'static MemoryStore>, args: &Value) -> Result<Value, String> {
    let (Some(from), Some(to)) = (non_empty(args, "from"), non_empty(args, "to")) else {
        return Ok(invalid_args("action='path' requires from and to."));
    };
    let Some(mem) = social_mem else { return Ok(no_social_memory()) };
    let max_depth = args.get("maxDepth").and_then(Value::as_u64).map(|d| d as usize).unwrap_or(3);
    let hops = SocialMemoryService::new(mem).relationship_path(&from, &to, max_depth)?;
    Ok(json!({ "success": true, "data": { "action": "path", "from": from, "to": to, "path": hops } }))
}

struct PersonCatalogToolHandler {
    store: &'static CatalogStore,
    /// `None` só quando `modules::memory::memory_store` falhou ao abrir (ver `register_tools`) — as
    /// 4 ações sociais e a varredura de `delete` degradam nomeando essa causa em vez da antiga
    /// ("depende do módulo brain").
    social_mem: Option<&'static MemoryStore>,
    /// `None` só quando `crate::state::config_store` falhou — `add_fact`/`add_relationship` assumem
    /// `memory.enabled=true` (mesmo default "aberto" de `memory_recording_allowed`) nesse caso.
    config: Option<Arc<ConfigStore>>,
}

impl PersonCatalogToolHandler {
    /// Lê `memory.enabled` do `ConfigStore` REAL a cada chamada (nunca cacheado) — `add_fact`/
    /// `add_relationship` têm `AppHandle` disponível no registro da tool, ao contrário de
    /// `memory_console::add_fact` (ver `modules::memory::recording_allowed_cached` para o porquê de
    /// UM dos dois caminhos precisar de cache).
    fn recording_allowed(&self) -> bool {
        self.config.as_deref().map(crate::modules::memory::memory_recording_allowed).unwrap_or(true)
    }

    /// Lê `person-catalog.enabled` do `ConfigStore` REAL a cada chamada, mesma técnica de
    /// [`Self::recording_allowed`] — `SettingsTabHero` já desenha o toggle mestre da aba
    /// (`enabledConfigPath`), mas até esta rodada nada no backend o lia: desligar não fazia a tool
    /// parar de agir (gate `config-schema-parity` listava a lacuna como `GAP:`).
    fn catalog_enabled(&self) -> bool {
        self.config.as_deref().and_then(|c| c.get("person-catalog.enabled")).and_then(|v| v.as_bool()) != Some(false)
    }
}

#[async_trait]
impl ToolHandler for PersonCatalogToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        if !self.catalog_enabled() {
            return Ok(catalog_disabled());
        }
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        let result = match action {
            "upsert" => handle_upsert(&self.store, &args),
            "get" => handle_get(&self.store, &args),
            "search" => handle_search(&self.store, &args),
            "list" => handle_list(&self.store, &args),
            "delete" => handle_delete(&self.store, self.social_mem, &args),
            "add_mention" => handle_add_mention(&self.store, &args),
            "update_social" => handle_update_social(&self.store, &args),
            "add_fact" => handle_add_fact(self.social_mem, self.recording_allowed(), &args)?,
            "delete_fact" => handle_delete_fact(self.social_mem, &args)?,
            "add_relationship" => handle_add_relationship(self.social_mem, self.recording_allowed(), &args)?,
            "delete_relationship" => handle_delete_relationship(self.social_mem, &args)?,
            "relationships" => handle_relationships(self.social_mem, &args)?,
            "path" => handle_path(self.social_mem, &args)?,
            other => return Err(format!("person_catalog: ação desconhecida \"{other}\"")),
        };
        Ok(result)
    }
}

/// Handler-tampão registrado SEM `AppHandle` (primeira passada, `features::modules()`) — antes do
/// Tauri montar o app não há como abrir `catalog.json` (precisa de `crate::state::app_data_dir(app)`, só
/// disponível com `AppHandle`). Garante que `person_catalog` sempre aparece em `mika:tools-list` /
/// `ToolRegistry::get` mesmo que a segunda passada ainda não tenha rodado, ou falhe (ex.: diretório
/// sem permissão de escrita) — sem isto, uma falha ao abrir o store apagaria a tool do catálogo em
/// vez de só continuar degradando. [`register_tools`] SOBRESCREVE este handler pelo de verdade assim
/// que `AppHandle` existe (`ToolRegistry::register` substitui por nome, ver o cabeçalho de
/// `modules/tools/registry/mod.rs`).
struct StubHandler;

#[async_trait]
impl ToolHandler for StubHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        if !ACTIONS.contains(&action) {
            return Err(format!("person_catalog: ação desconhecida \"{action}\""));
        }
        Ok(json!({
            "success": false,
            "error": format!(
                "catálogo de pessoas ainda inicializando — o store de catalog.json só monta depois que o \
                 app Tauri termina de subir; action='{action}' não tem onde gravar nem de onde ler por \
                 enquanto."
            ),
            "errorCode": "unavailable",
        }))
    }
}

/// Chamado por `features::modules()` (primeira passada, sem `AppHandle`) — ver o cabeçalho de
/// [`StubHandler`]. `ToolRegistry::register_stub` (não `register`): marca o nome como substituição
/// pretendida para a segunda passada (`register_tools(app)` abaixo), em vez de uma colisão de nome
/// comum.
pub(crate) fn register_stub() {
    crate::modules::tools::registry().register_stub(tool_definition(), Arc::new(StubHandler));
}

/// Chamado por `modules::register_config_bound_tools(app)` (segunda passada) — precisa do
/// `AppHandle` para resolver `catalog.json` (ver `super::store::db_path`) e, a partir desta rodada,
/// o `MemoryStore`/`ConfigStore` de processo por trás do grafo social (ver o cabeçalho do arquivo).
/// Sobrescreve o [`StubHandler`] da primeira passada. Falha em resolver `MemoryStore`/`ConfigStore`
/// NUNCA impede o registro: o catálogo (`upsert`/`get`/`search`/`list`/`add_mention`/`update_social`)
/// continua funcionando igual, só as ações sociais degradam nomeadas (mesma política de resiliência
/// já usada pelo resto de `register_config_bound_tools`, ver `modules/mod.rs`).
pub(crate) fn register_tools(app: &tauri::AppHandle) -> Result<(), String> {
    // Singleton de processo (`super::store_singleton::catalog_store`), NÃO uma abertura própria: um
    // segundo `CatalogStore` independente sobre o mesmo `catalog.json` faria a próxima escrita de um
    // desfazer o `clear_all()` do outro — ver o cabeçalho de `store_singleton.rs`.
    let store = super::catalog_store(app)?;

    let social_mem = match crate::modules::memory::memory_store(app) {
        Ok(mem) => Some(mem),
        Err(error) => {
            log::error!("person_catalog: grafo social e varredura episódica de 'delete' vão degradar — memory store não abriu: {error}");
            None
        }
    };
    let config = match crate::state::config_store(app) {
        Ok(config) => Some(config),
        Err(error) => {
            log::error!("person_catalog: 'add_fact'/'add_relationship' vão assumir memory.enabled=true — config store não resolveu: {error}");
            None
        }
    };

    crate::modules::tools::registry().register(tool_definition(), Arc::new(PersonCatalogToolHandler { store, social_mem, config }));
    super::plugin_test::register();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::episodic::EpisodicMemoryService;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-person-catalog-tool-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `Box::leak` para satisfazer `&'static CatalogStore` em teste — mesmo truque que
    /// `social_mem`/`features::privacy::clear.rs`/`modules::memory::mod.rs` já usam para
    /// `&'static MemoryStore` (ver o comentário deles para o porquê): o handler de produção fecha
    /// sobre o `'static` que `store_singleton::catalog_store` devolve.
    fn handler() -> (PersonCatalogToolHandler, TempDir) {
        let dir = TempDir::new();
        let store: &'static CatalogStore = Box::leak(Box::new(CatalogStore::open(&dir.0.join("catalog.json")).unwrap()));
        (PersonCatalogToolHandler { store, social_mem: None, config: None }, dir)
    }

    fn handler_with_social() -> (PersonCatalogToolHandler, TempDir) {
        let dir = TempDir::new();
        let store: &'static CatalogStore = Box::leak(Box::new(CatalogStore::open(&dir.0.join("catalog.json")).unwrap()));
        let social_mem: &'static MemoryStore = Box::leak(Box::new(MemoryStore::open_in_memory().unwrap()));
        (PersonCatalogToolHandler { store, social_mem: Some(social_mem), config: None }, dir)
    }

    #[tokio::test]
    async fn upsert_then_get_round_trips_through_the_tool() {
        let (handler, _dir) = handler();
        let created = handler.call(json!({ "action": "upsert", "name": "Ada Lovelace", "bio": "Matemática" })).await.unwrap();
        assert_eq!(created["success"], json!(true));
        let id = created["data"]["id"].as_str().unwrap().to_string();

        let fetched = handler.call(json!({ "action": "get", "identifier": id })).await.unwrap();
        assert_eq!(fetched["success"], json!(true));
        assert_eq!(fetched["data"]["primaryName"], json!("Ada Lovelace"));
    }

    #[tokio::test]
    async fn upsert_redacts_pii_from_bio_before_it_ever_reaches_disk() {
        let (handler, dir) = handler();
        handler
            .call(json!({ "action": "upsert", "name": "Ada", "bio": "email dela é ada@example.com" }))
            .await
            .unwrap();

        let raw = std::fs::read_to_string(dir.0.join("catalog.json")).unwrap();
        assert!(!raw.contains("ada@example.com"), "PII vazou pro disco: {raw}");
        assert!(raw.contains("[EMAIL]"));
    }

    #[tokio::test]
    async fn search_and_list_are_bounded_even_when_the_caller_asks_for_more() {
        let (handler, _dir) = handler();
        for i in 0..5 {
            handler.call(json!({ "action": "upsert", "name": format!("Fulano Repetido {i}") })).await.unwrap();
        }
        let listed = handler.call(json!({ "action": "list", "limit": 200 })).await.unwrap();
        assert_eq!(listed["data"].as_array().unwrap().len(), 5, "5 pessoas cabem no teto de 50, nenhuma some");

        let searched = handler.call(json!({ "action": "search", "query": "fulano repetido", "limit": 2 })).await.unwrap();
        assert_eq!(searched["data"].as_array().unwrap().len(), 2, "search não respeitou o limit pedido");
    }

    #[tokio::test]
    async fn delete_requires_confirm_and_names_social_memory_as_unavailable_without_a_memory_store() {
        let (handler, _dir) = handler();
        let created = handler.call(json!({ "action": "upsert", "name": "Ada" })).await.unwrap();
        let id = created["data"]["id"].as_str().unwrap().to_string();

        let refused = handler.call(json!({ "action": "delete", "id": id.clone() })).await.unwrap();
        assert_eq!(refused["errorCode"], json!("invalid_args"), "delete sem confirm=true tinha de recusar");

        let deleted = handler.call(json!({ "action": "delete", "id": id, "confirm": true })).await.unwrap();
        assert_eq!(deleted["success"], json!(true));
        assert_eq!(deleted["data"]["catalogDeleted"], json!(true));
        assert!(deleted["data"]["warning"].as_str().unwrap().contains("Social memory graph is unavailable"));
    }

    /// Prova de completude do `delete`: uma pessoa com o MESMO nome no catálogo E no grafo social
    /// sai das duas pernas numa chamada só, e a varredura episódica usa a UNIÃO dos termos das duas
    /// pernas — nenhuma sobra em nenhum dos três stores.
    ///
    /// Deleta por `identifier: "Ada Lovelace"`, não pelo `id` do catálogo (`pc_<hex>`): o `id` do
    /// catálogo e o `id` do grafo social (`personId`, um slug do nome — `person-graph.ts::getPerson`)
    /// são namespaces DIFERENTES, e `handle_delete`/TS's `forgetPersonEverywhere` passam o MESMO
    /// `identifier` bruto para as duas pernas (ver o cabeçalho de `handle_delete`) — deletar pelo
    /// `id` do catálogo nunca alcançaria o grafo social, nem aqui nem no TS original.
    #[tokio::test]
    async fn delete_sweeps_catalog_social_graph_and_episodic_mentions_together() {
        let (handler, _dir) = handler_with_social();
        let created = handler.call(json!({ "action": "upsert", "name": "Ada Lovelace" })).await.unwrap();
        assert_eq!(created["success"], json!(true));

        let added = handler.call(json!({ "action": "add_relationship", "name": "Ada Lovelace", "label": "amiga" })).await.unwrap();
        assert_eq!(added["success"], json!(true), "fixture precisa da pessoa também no grafo social: {added}");

        let episodic = EpisodicMemoryService::new(handler.social_mem.unwrap());
        episodic.store(true, "conversei com Ada Lovelace ontem", None, None).unwrap();

        let deleted = handler.call(json!({ "action": "delete", "identifier": "Ada Lovelace", "confirm": true })).await.unwrap();
        assert_eq!(deleted["success"], json!(true));
        assert_eq!(deleted["data"]["catalogDeleted"], json!(true));
        assert_eq!(deleted["data"]["socialDeleted"], json!(true));
        assert_eq!(deleted["data"]["episodicRemoved"], json!(1), "menção episódica tinha de ser varrida");
        assert!(deleted["data"].get("warning").is_none(), "varredura completa não pode carregar warning");

        assert!(episodic.get_all().unwrap().is_empty());
    }

    /// = `remove()` no TS: a pessoa some do catálogo, mas continua achável no grafo social até o
    /// `delete` rodar de novo com esse identifier — a busca por catálogo e por grafo social são
    /// INDEPENDENTES, então `delete` tem de tentar as duas em vez de desistir na primeira ausência.
    #[tokio::test]
    async fn delete_of_a_person_only_in_the_social_graph_still_succeeds() {
        let (handler, _dir) = handler_with_social();
        let added = handler.call(json!({ "action": "add_relationship", "name": "Bea", "label": "amiga" })).await.unwrap();
        assert_eq!(added["success"], json!(true));

        let deleted = handler.call(json!({ "action": "delete", "identifier": "Bea", "confirm": true })).await.unwrap();
        assert_eq!(deleted["success"], json!(true));
        assert_eq!(deleted["data"]["catalogDeleted"], json!(false), "nunca esteve no catálogo");
        assert_eq!(deleted["data"]["socialDeleted"], json!(true));
    }

    #[tokio::test]
    async fn update_social_adds_affinity_as_a_delta_to_an_existing_person() {
        let (handler, _dir) = handler();
        let created = handler.call(json!({ "action": "upsert", "name": "Ada", "affinity": 10.0 })).await.unwrap();
        let id = created["data"]["id"].as_str().unwrap().to_string();

        let updated = handler.call(json!({ "action": "update_social", "identifier": id, "affinityChange": 5.0 })).await.unwrap();
        assert_eq!(updated["data"]["affinity"], json!(15.0));
    }

    /// `update_social` regravava a ficha via `upsert` PELO NOME: com o nome ambíguo (dois candidatos: o
    /// alias de uma ficha e o nome principal de outra) o upsert criava uma ficha nova em vez de
    /// atualizar a pedida, e devolvia sucesso com outro `id`.
    #[tokio::test]
    async fn update_social_on_an_ambiguous_name_updates_that_person_instead_of_cloning_it() {
        let (handler, _dir) = handler();
        handler.call(json!({ "action": "upsert", "name": "Ana Souza", "aliases": ["Ana"] })).await.unwrap();
        let lima = handler.call(json!({ "action": "upsert", "name": "Ana Lima" })).await.unwrap();
        let lima_id = lima["data"]["id"].as_str().unwrap().to_string();
        // Promoção/renomeio por id: agora "Ana" é alias de uma ficha e nome principal de outra.
        handler.call(json!({ "action": "upsert", "id": lima_id, "name": "Ana" })).await.unwrap();

        let updated = handler.call(json!({ "action": "update_social", "identifier": lima_id, "affinityChange": 5.0 })).await.unwrap();

        assert_eq!(updated["data"]["id"], json!(lima_id), "atualizou outra ficha: {updated}");
        assert_eq!(updated["data"]["affinity"], json!(5.0));
        let listed = handler.call(json!({ "action": "list" })).await.unwrap();
        assert_eq!(listed["data"].as_array().unwrap().len(), 2, "o update clonou a ficha");
    }

    #[tokio::test]
    async fn add_mention_finds_the_person_and_redacts_the_mention_text() {
        let (handler, _dir) = handler();
        let created = handler.call(json!({ "action": "upsert", "name": "Ada" })).await.unwrap();
        let id = created["data"]["id"].as_str().unwrap().to_string();

        let result = handler
            .call(json!({ "action": "add_mention", "identifier": id, "text": "liga pra 11 98765-4321" }))
            .await
            .unwrap();
        assert_eq!(result["success"], json!(true));
        let mentions = result["data"]["person"]["mentions"].as_array().unwrap();
        assert!(mentions[0]["text"].as_str().unwrap().contains("[PHONE]"));
    }

    /// Sem `MemoryStore` de processo (fixture `handler()`), as 4 ações sociais degradam nomeando a
    /// causa REAL (store não abriu) — não mais "depende do módulo brain", já que o grafo social tem
    /// porte Rust a partir desta rodada (ver `no_social_memory`).
    #[tokio::test]
    async fn the_four_social_memory_actions_degrade_naming_the_memory_store_not_the_brain_module() {
        let (handler, _dir) = handler();
        let with_args = [
            json!({ "action": "add_fact", "fact": "gosta de rust" }),
            json!({ "action": "delete_fact", "id": "x" }),
            json!({ "action": "add_relationship", "name": "Ada", "label": "amiga" }),
            json!({ "action": "delete_relationship", "id": "x" }),
        ];
        for args in with_args {
            let result = handler.call(args.clone()).await.unwrap();
            assert_eq!(result["errorCode"], json!("unavailable"), "{args}");
            assert!(result["error"].as_str().unwrap().contains("memory store did not open"), "{args} não nomeou a causa certa");
        }
    }

    /// Validação de argumento roda ANTES de checar se o grafo social está disponível — mesma ordem
    /// do TS (`if (!args.fact) return fail(...)` antes de `socialMemoryOrNull()`).
    #[tokio::test]
    async fn the_four_social_memory_actions_validate_arguments_before_checking_availability() {
        let (handler, _dir) = handler();
        for action in ["add_fact", "delete_fact", "add_relationship", "delete_relationship"] {
            let result = handler.call(json!({ "action": action })).await.unwrap();
            assert_eq!(result["errorCode"], json!("invalid_args"), "{action}");
        }
    }

    #[tokio::test]
    async fn add_fact_then_delete_fact_dispatch_for_real_through_the_social_memory_graph() {
        let (handler, _dir) = handler_with_social();
        let added = handler.call(json!({ "action": "add_fact", "fact": "gosta de rust" })).await.unwrap();
        assert_eq!(added["success"], json!(true));
        assert_eq!(added["data"]["fact"], json!("gosta de rust"));
        let id = added["data"]["id"].as_str().unwrap().to_string();

        let deleted = handler.call(json!({ "action": "delete_fact", "id": id.clone() })).await.unwrap();
        assert_eq!(deleted["success"], json!(true));

        let again = handler.call(json!({ "action": "delete_fact", "id": id })).await.unwrap();
        assert_eq!(again["errorCode"], json!("not_found"), "segunda vez não pode fingir sucesso");
    }

    #[tokio::test]
    async fn add_relationship_then_delete_relationship_dispatch_for_real() {
        let (handler, _dir) = handler_with_social();
        let added = handler.call(json!({ "action": "add_relationship", "name": "Ada Lovelace", "label": "namorada" })).await.unwrap();
        assert_eq!(added["success"], json!(true));
        assert_eq!(added["data"]["label"], json!("namorada"));
        let id = added["data"]["id"].as_str().unwrap().to_string();

        let deleted = handler.call(json!({ "action": "delete_relationship", "id": id })).await.unwrap();
        assert_eq!(deleted["success"], json!(true));
    }

    /// Item 4 do pacote "people catalog + relationship graph": `relationships {name}` lê o que
    /// `add_relationship` gravou.
    #[tokio::test]
    async fn relationships_action_reads_back_what_add_relationship_wrote() {
        let (handler, _dir) = handler_with_social();
        handler.call(json!({ "action": "add_relationship", "name": "Ada Lovelace", "label": "namorada" })).await.unwrap();

        let result = handler.call(json!({ "action": "relationships", "name": "Ada Lovelace" })).await.unwrap();
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["relationships"].as_array().unwrap().len(), 1);
    }

    /// Item 4: "como o João conhece a Maria?" — `add_fact`/`add_relationship` só cobrem
    /// usuário-pessoa; a aresta pessoa-pessoa nasce de `SocialMemoryService::add_person_to_person`
    /// (chamada pela extração automática, testada em `lifecycle.rs`) — aqui a tool só LÊ o grafo que
    /// já existe, então o teste semeia a aresta direto pelo serviço.
    #[tokio::test]
    async fn path_action_finds_how_two_people_are_connected() {
        let (handler, _dir) = handler_with_social();
        let svc = SocialMemoryService::new(handler.social_mem.unwrap());
        svc.add_person_to_person(
            true,
            crate::modules::memory::social::AddPersonToPersonInput { from_name: "João".into(), to_name: "Maria".into(), label: "trabalha com".into(), confidence: None },
            1,
        )
        .unwrap();

        let result = handler.call(json!({ "action": "path", "from": "João", "to": "Maria" })).await.unwrap();
        assert_eq!(result["success"], json!(true));
        let hops = result["data"]["path"].as_array().unwrap();
        assert_eq!(hops.len(), 1);
        assert_eq!(hops[0]["label"], json!("trabalha com"));
    }

    #[tokio::test]
    async fn path_action_with_no_connection_answers_null_path_not_an_error() {
        let (handler, _dir) = handler_with_social();
        let result = handler.call(json!({ "action": "path", "from": "fantasma-1", "to": "fantasma-2" })).await.unwrap();
        assert_eq!(result["success"], json!(true));
        assert!(result["data"]["path"].is_null());
    }

    #[tokio::test]
    async fn add_fact_blocked_by_memory_enabled_false_reports_denied_and_persists_nothing() {
        let dir = TempDir::new();
        let store: &'static CatalogStore = Box::leak(Box::new(CatalogStore::open(&dir.0.join("catalog.json")).unwrap()));
        let social_mem: &'static MemoryStore = Box::leak(Box::new(MemoryStore::open_in_memory().unwrap()));
        let config_dir = TempDir::new();
        let config = Arc::new(ConfigStore::open(&config_dir.0, crate::config::schemas::registry()).unwrap());
        config.set("memory.enabled", json!(false)).unwrap();
        let handler = PersonCatalogToolHandler { store, social_mem: Some(social_mem), config: Some(config) };

        let result = handler.call(json!({ "action": "add_fact", "fact": "nao deveria gravar" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("denied"));
        assert!(SocialMemoryService::new(social_mem).get_graph().unwrap().facts.is_empty(), "memory.enabled=false não pode ter deixado passar uma escrita");
    }

    /// `person-catalog.enabled=false` é o toggle MESTRE da feature (`SettingsTabHero`) — tinha UI
    /// desde sempre, mas nenhum leitor no backend: desligar não fazia nada. A partir desta rodada,
    /// nenhuma das 11 ações dispacha com o mestre desligado, nem `upsert` (que não depende de
    /// `social_mem`/`config` para funcionar de outra forma).
    #[tokio::test]
    async fn any_action_is_denied_when_the_catalog_master_toggle_is_off() {
        let dir = TempDir::new();
        let store: &'static CatalogStore = Box::leak(Box::new(CatalogStore::open(&dir.0.join("catalog.json")).unwrap()));
        let config_dir = TempDir::new();
        let config = Arc::new(ConfigStore::open(&config_dir.0, crate::config::schemas::registry()).unwrap());
        config.set("person-catalog.enabled", json!(false)).unwrap();
        let handler = PersonCatalogToolHandler { store, social_mem: None, config: Some(config) };

        let result = handler.call(json!({ "action": "upsert", "name": "Ada" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("denied"));
        assert!(store.list(None, None, 10).is_empty(), "person-catalog.enabled=false não pode ter deixado passar uma escrita");
    }

    #[tokio::test]
    async fn unknown_action_is_a_named_error() {
        let (handler, _dir) = handler();
        let err = handler.call(json!({ "action": "fly" })).await.unwrap_err();
        assert!(err.contains("ação desconhecida"));
    }

    #[tokio::test]
    async fn register_stub_answers_unavailable_until_the_real_store_is_wired() {
        // `registry()` é `OnceLock` processo-inteiro e os testes rodam em paralelo — só afirmação
        // POSITIVA é válida aqui (ver o cabeçalho de `mutate/mod.rs`/`store.rs` para o mesmo aviso).
        // Nenhum teste deste binário chama `register_tools(app)` (exige `AppHandle` real), então o
        // stub nunca é sobrescrito pelo handler de verdade durante a suíte.
        register_stub();
        let definition = crate::modules::tools::registry().get("person_catalog").expect("person_catalog não registrada pelo stub");
        assert_eq!(definition.plugin_id, "mika.person-catalog");

        let result = crate::modules::tools::registry()
            .execute("person_catalog", json!({ "action": "get", "identifier": "x" }))
            .await
            .unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("unavailable"));
        assert!(result["error"].as_str().unwrap().contains("inicializando"), "{result}");
    }

    /// `tools/schema.ts` foi removido (backend Node deletado na migração para Tauri); as 11 ações
    /// que ele declarava viram snapshot congelado aqui — `ACTIONS` continua sendo a única fonte de
    /// verdade, mas uma ação removida sem querer ainda reprova, agora contra o literal. `relationships`/
    /// `path` (item 4 do pacote "people catalog + relationship graph") entraram depois do fim do
    /// snapshot original — apêndice explícito, não reescrita das 11 primeiras.
    #[test]
    fn actions_match_the_frozen_ts_schema_snapshot() {
        const FROZEN_ACTIONS: [&str; 13] = [
            "upsert",
            "get",
            "search",
            "list",
            "delete",
            "add_mention",
            "update_social",
            "add_fact",
            "delete_fact",
            "add_relationship",
            "delete_relationship",
            "relationships",
            "path",
        ];
        assert_eq!(ACTIONS, FROZEN_ACTIONS);
    }

    #[test]
    fn tool_definition_keeps_the_manifest_plugin_id() {
        assert_eq!(tool_definition().plugin_id, "mika.person-catalog");
    }
}
