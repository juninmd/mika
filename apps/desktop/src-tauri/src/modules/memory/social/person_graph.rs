//! Porte de `apps/desktop/src/main/services/memory/social/person-graph.ts`.

use crate::modules::memory::MemoryStore;

use super::rules;
use super::store;
use super::types::{LearnedFact, PersonRecord, PersonType, RelationshipEdge, SocialEvent, SOCIAL_TABLE_EVENTS, SOCIAL_TABLE_FACTS, SOCIAL_TABLE_PEOPLE, SOCIAL_TABLE_RELATIONSHIPS};

/// = os campos de entrada de `SocialMemoryService.upsertPerson`.
#[derive(Default)]
pub struct UpsertPersonInput {
    pub name: String,
    pub aliases: Option<Vec<String>>,
    pub kind: Option<PersonType>,
    pub universe: Option<String>,
    pub appearance: Option<String>,
    pub notes: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub affinity: Option<f64>,
    /// `person_catalog`'s `pc_*` id for this same person (single-identity link, work package
    /// "people catalog + relationship graph"). Once set it is never cleared by a later upsert that
    /// omits it — same "never zero what the input doesn't mention" rule as every other optional
    /// field here.
    pub catalog_id: Option<String>,
}

fn merged(current: Option<&Vec<String>>, added: Option<Vec<String>>, limit: usize) -> Vec<String> {
    let mut values = current.cloned().unwrap_or_default();
    values.extend(added.unwrap_or_default());
    rules::compact_unique(values, limit)
}

/// = `upsertPerson`: cria ou funde com o registro existente (mesmo `id` determinístico de
/// [`rules::person_id`]) — nunca duplica uma pessoa por causa de maiúscula/acento/espaço.
pub fn upsert_person(mem: &MemoryStore, input: UpsertPersonInput, now: i64) -> Result<PersonRecord, String> {
    if !rules::is_valid_person_name(&input.name) {
        return Err(format!("Invalid person name: {}", input.name));
    }
    let id = rules::person_id(&input.name);
    let current: Option<PersonRecord> = store::get_by_id(mem, SOCIAL_TABLE_PEOPLE, &id)?;

    let person = PersonRecord {
        id: id.clone(),
        primary_name: current.as_ref().map(|c| c.primary_name.clone()).unwrap_or_else(|| input.name.trim().to_string()),
        aliases: merged(current.as_ref().map(|c| &c.aliases), input.aliases, 20),
        kind: input.kind.unwrap_or_else(|| current.as_ref().map(|c| c.kind).unwrap_or_default()),
        universe: input.universe.or_else(|| current.as_ref().and_then(|c| c.universe.clone())),
        appearance: input.appearance.or_else(|| current.as_ref().and_then(|c| c.appearance.clone())),
        notes: merged(current.as_ref().map(|c| &c.notes), input.notes, 50),
        tags: merged(current.as_ref().map(|c| &c.tags), input.tags, 30),
        affinity: input.affinity.unwrap_or_else(|| current.as_ref().map(|c| c.affinity).unwrap_or(0.0)),
        last_mentioned_at: now,
        created_at: current.as_ref().map(|c| c.created_at).unwrap_or(now),
        updated_at: now,
        catalog_id: input.catalog_id.or_else(|| current.as_ref().and_then(|c| c.catalog_id.clone())),
    };
    store::upsert(mem, SOCIAL_TABLE_PEOPLE, &id, now as f64, &person)?;
    Ok(person)
}

/// = `getPerson`: acha por id determinístico, senão por alias (varredura completa — mesma tabela
/// pequena que `getAll` já assume em todo o resto deste módulo).
pub fn get_person(mem: &MemoryStore, identifier: &str) -> Result<Option<PersonRecord>, String> {
    let id = rules::person_id(identifier);
    if let Some(direct) = store::get_by_id::<PersonRecord>(mem, SOCIAL_TABLE_PEOPLE, &id)? {
        return Ok(Some(direct));
    }
    let query = rules::normalize_text(identifier);
    let people: Vec<PersonRecord> = store::get_all(mem, SOCIAL_TABLE_PEOPLE)?;
    Ok(people.into_iter().find(|p| p.aliases.iter().any(|a| rules::normalize_text(a) == query)))
}

/// = `forgetPerson`: apaga a pessoa e TUDO que a referencia (relações, fatos pelo `subject`, eventos
/// que a citam) — sem isso o grafo fica com pontas soltas apontando para um id que não existe mais.
pub fn forget_person(mem: &MemoryStore, identifier: &str) -> Result<bool, String> {
    let Some(person) = get_person(mem, identifier)? else { return Ok(false) };
    store::delete(mem, SOCIAL_TABLE_PEOPLE, &person.id)?;

    let relationships: Vec<RelationshipEdge> = store::get_all(mem, SOCIAL_TABLE_RELATIONSHIPS)?;
    // As arestas pessoa↔pessoa são espelhadas: a pessoa esquecida pode estar em `to_person_id` de
    // uma aresta que pertence a OUTRA pessoa, e ela sobrevivia ao esquecimento.
    for edge in relationships.iter().filter(|r| r.person_id == person.id || r.to_person_id.as_deref() == Some(person.id.as_str())) {
        store::delete(mem, SOCIAL_TABLE_RELATIONSHIPS, &edge.id)?;
    }

    let facts: Vec<LearnedFact> = store::get_all(mem, SOCIAL_TABLE_FACTS)?;
    // Fatos arquivados sob qualquer nome da pessoa (o primário e os apelidos), não só pelo id dela.
    let own_ids: std::collections::HashSet<String> =
        std::iter::once(person.id.clone()).chain(person.aliases.iter().map(|a| rules::person_id(a))).collect();
    for fact in facts.iter().filter(|f| own_ids.contains(&rules::person_id(&f.subject))) {
        store::delete(mem, SOCIAL_TABLE_FACTS, &fact.id)?;
    }

    let events: Vec<SocialEvent> = store::get_all(mem, SOCIAL_TABLE_EVENTS)?;
    for event in events.iter().filter(|e| e.people.iter().any(|name| rules::person_id(name) == person.id)) {
        store::delete(mem, SOCIAL_TABLE_EVENTS, &event.id)?;
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::RelationshipDirection;

    fn store_kv() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    fn input(name: &str) -> UpsertPersonInput {
        UpsertPersonInput { name: name.to_string(), ..Default::default() }
    }

    #[test]
    fn upsert_person_rejects_an_invalid_name() {
        let mem = store_kv();
        assert!(upsert_person(&mem, input("eu"), 1).is_err());
    }

    #[test]
    fn upsert_person_twice_merges_into_the_same_deterministic_id() {
        let mem = store_kv();
        let first = upsert_person(&mem, UpsertPersonInput { name: "Ada Lovelace".into(), tags: Some(vec!["math".into()]), ..Default::default() }, 1).unwrap();
        let second = upsert_person(&mem, UpsertPersonInput { name: "ADA LOVELACE".into(), tags: Some(vec!["cs".into()]), ..Default::default() }, 2).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.tags, vec!["math".to_string(), "cs".to_string()], "tags anteriores preservadas, nova adicionada");
        assert_eq!(second.created_at, 1, "created_at não muda numa fusão");
        assert_eq!(second.updated_at, 2);
    }

    #[test]
    fn get_person_finds_by_alias_when_the_direct_id_does_not_match() {
        let mem = store_kv();
        upsert_person(&mem, UpsertPersonInput { name: "Ada".into(), aliases: Some(vec!["Lovelace".into()]), ..Default::default() }, 1).unwrap();

        let found = get_person(&mem, "Lovelace").unwrap().unwrap();
        assert_eq!(found.primary_name, "Ada");
    }

    #[test]
    fn get_person_of_an_unknown_identifier_is_none_not_an_error() {
        let mem = store_kv();
        assert_eq!(get_person(&mem, "ghost").unwrap(), None);
    }

    /// Prova de completude: apagar uma pessoa varre as TRÊS tabelas que a referenciam, não só o
    /// registro dela — uma pessoa "esquecida" pela metade é o defeito de privacidade que este teste
    /// existe para travar.
    #[test]
    fn forget_person_sweeps_relationships_facts_and_events_that_reference_her() {
        let mem = store_kv();
        let ada = upsert_person(&mem, input("Ada Lovelace"), 1).unwrap();

        let edge = RelationshipEdge {
            id: format!("{}:friend", ada.id),
            person_id: ada.id.clone(),
            to_person_id: None,
            label: "friend".into(),
            direction: RelationshipDirection::UserToPerson,
            confidence: 0.9,
            updated_at: 1,
            valid_to: None,
        };
        store::upsert(&mem, SOCIAL_TABLE_RELATIONSHIPS, &edge.id, 1.0, &edge).unwrap();

        let fact = LearnedFact {
            id: "f1".into(),
            subject: "Ada Lovelace".into(),
            fact: "gosta de matematica".into(),
            confidence: 0.8,
            predicate: None,
            object: None,
            active: Some(true),
            superseded_by: None,
            source_turn_id: None,
            created_at: Some(1),
            updated_at: 1,
        };
        store::upsert(&mem, SOCIAL_TABLE_FACTS, &fact.id, 1.0, &fact).unwrap();

        let event = SocialEvent { id: "e1".into(), text: "conversa".into(), people: vec!["Ada Lovelace".into()], keywords: vec![], created_at: 1 };
        store::upsert(&mem, SOCIAL_TABLE_EVENTS, &event.id, 1.0, &event).unwrap();

        assert!(forget_person(&mem, "Ada Lovelace").unwrap());

        assert!(get_person(&mem, "Ada Lovelace").unwrap().is_none());
        assert!(store::get_all::<RelationshipEdge>(&mem, SOCIAL_TABLE_RELATIONSHIPS).unwrap().is_empty(), "relação órfã sobrou");
        assert!(store::get_all::<LearnedFact>(&mem, SOCIAL_TABLE_FACTS).unwrap().is_empty(), "fato órfão sobrou");
        assert!(store::get_all::<SocialEvent>(&mem, SOCIAL_TABLE_EVENTS).unwrap().is_empty(), "evento órfão sobrou");
    }

    #[test]
    fn forget_person_of_an_unknown_identifier_returns_false_and_touches_nothing() {
        let mem = store_kv();
        assert!(!forget_person(&mem, "ghost").unwrap());
    }

    /// Item 2 do pacote "people catalog + relationship graph": `catalog_id` liga este registro ao
    /// `pc_*` do `person_catalog` — sem ele as duas listas de pessoas nunca convergem numa identidade
    /// só (ver o cabeçalho de `mod.rs`: "duas listas... sem id compartilhado" era o defeito).
    #[test]
    fn upsert_person_sets_the_catalog_id_link_and_keeps_it_on_a_later_upsert_that_omits_it() {
        let mem = store_kv();
        let linked = upsert_person(&mem, UpsertPersonInput { catalog_id: Some("pc_ada1234".into()), ..input("Ada Lovelace") }, 1).unwrap();
        assert_eq!(linked.catalog_id.as_deref(), Some("pc_ada1234"));

        let again = upsert_person(&mem, UpsertPersonInput { tags: Some(vec!["math".into()]), ..input("Ada Lovelace") }, 2).unwrap();
        assert_eq!(again.catalog_id.as_deref(), Some("pc_ada1234"), "catalog_id não pode sumir num upsert que não o menciona");
    }
}
