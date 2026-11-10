//! Leituras puras sobre o índice — porte de
//! `apps/desktop/src/features/person-catalog/services/person-catalog-query.ts`. Nenhuma função
//! aqui muta nada nem grava em disco; [`super::store::CatalogStore`] chama estas com o lock já
//! segurado.

use std::collections::HashSet;

use super::normalize::catalog_key;
use super::types::{PersonCatalogIndex, PersonRecord, PersonType};

pub(super) fn find_person_by_alias<'a>(index: &'a PersonCatalogIndex, alias: &str) -> Option<&'a PersonRecord> {
    let key = catalog_key(alias);
    let id = index.alias_map.get(&key)?;
    index.persons.iter().find(|p| &p.id == id)
}

pub(super) fn get_person<'a>(index: &'a PersonCatalogIndex, id: &str) -> Option<&'a PersonRecord> {
    index.persons.iter().find(|p| p.id == id)
}

/// Porte de `resolveCandidates` — usado pelo `upsert` para achar quem `name`/`aliases` já
/// referenciam, ANTES de decidir se cria um registro novo ou funde num existente.
pub(super) fn resolve_candidates<'a>(index: &'a PersonCatalogIndex, names: &[String]) -> Vec<&'a PersonRecord> {
    let keys: HashSet<String> = names.iter().map(|n| catalog_key(n)).filter(|k| !k.is_empty()).collect();
    if keys.is_empty() {
        return vec![];
    }
    index
        .persons
        .iter()
        .filter(|p| std::iter::once(&p.primary_name).chain(p.aliases.iter()).any(|a| keys.contains(&catalog_key(a))))
        .collect()
}

fn contains_ci(haystack: &str, needle_lower: &str) -> bool {
    haystack.to_lowercase().contains(needle_lower)
}

/// Porte de `searchPersons` — varredura por substring case-insensitive nos mesmos 10 campos do TS.
/// `limit` é aplicado aqui (não pelo chamador) para nunca materializar mais que o necessário —
/// mesmo motivo do comentário de `listPersons` no TS ("`limit` corta ANTES de materializar a
/// cópia").
pub(super) fn search_persons<'a>(index: &'a PersonCatalogIndex, query: &str, limit: usize) -> Vec<&'a PersonRecord> {
    let q = query.to_lowercase();
    index
        .persons
        .iter()
        .filter(|p| {
            contains_ci(&p.primary_name, &q)
                || p.aliases.iter().any(|a| contains_ci(a, &q))
                || p.bio.as_deref().is_some_and(|b| contains_ci(b, &q))
                || p.tags.iter().any(|t| contains_ci(t, &q))
                || p.attributes.iter().any(|a| contains_ci(&a.key, &q) || contains_ci(&a.value, &q))
                || p.role.as_deref().is_some_and(|r| contains_ci(r, &q))
                || p.origin.as_deref().is_some_and(|o| contains_ci(o, &q))
                || p.occupation.as_ref().is_some_and(|list| list.iter().any(|o| contains_ci(o, &q)))
                || p.known_for.as_ref().is_some_and(|list| list.iter().any(|k| contains_ci(k, &q)))
                || p.nationality.as_deref().is_some_and(|n| contains_ci(n, &q))
                || p.relation_to_user.as_deref().is_some_and(|r| contains_ci(r, &q))
                || p.facts.iter().any(|f| contains_ci(&f.text, &q))
        })
        .take(limit)
        .collect()
}

/// Porte de `listPersons` + o `sortBy`/limite final que `handlers/person-catalog-queries.ts::list`
/// aplica por cima: filtra por tipo, ORDENA sobre o conjunto inteiro, só then trunca — a mesma ordem
/// de operações do TS (o handler nunca passa `limit` para `service.list`, então o corte "antecipado"
/// documentado em `person-catalog-query.ts` nunca dispara neste caminho).
pub(super) fn list_persons<'a>(
    index: &'a PersonCatalogIndex,
    kind: Option<PersonType>,
    sort_by: Option<&str>,
    limit: usize,
) -> Vec<&'a PersonRecord> {
    let mut matches: Vec<&PersonRecord> = match kind {
        Some(k) => index.persons.iter().filter(|p| p.kind == k).collect(),
        None => index.persons.iter().collect(),
    };
    match sort_by {
        Some("mentionCount") => matches.sort_by(|a, b| b.mention_count.cmp(&a.mention_count)),
        Some("lastSeenAt") => matches.sort_by(|a, b| b.last_seen_at.unwrap_or(0).cmp(&a.last_seen_at.unwrap_or(0))),
        Some("createdAt") => matches.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        // Sem `Intl`/ICU disponível nesta frente: comparação lexicográfica case-insensitive em vez de
        // `localeCompare` — diferença aceita, mesma classe de simplificação de `session_db.rs` para
        // ordenação sem depender de uma crate nova.
        _ => matches.sort_by(|a, b| a.primary_name.to_lowercase().cmp(&b.primary_name.to_lowercase())),
    }
    matches.truncate(limit);
    matches
}

#[cfg(test)]
mod tests {
    use super::super::types::{minimal_person, PersonCatalogIndex};
    use super::*;

    fn index_with(persons: Vec<super::super::types::PersonRecord>) -> PersonCatalogIndex {
        PersonCatalogIndex { version: 2, persons, alias_map: Default::default(), blocked: None, social_imported_at: None, ..Default::default() }
    }

    #[test]
    fn search_matches_case_insensitively_across_bio() {
        let mut ada = minimal_person("p1", "Ada Lovelace");
        ada.bio = Some("Pioneira da computação".into());
        let idx = index_with(vec![ada]);
        assert_eq!(search_persons(&idx, "COMPUTAÇÃO", 20).len(), 1);
    }

    #[test]
    fn search_is_bounded_by_limit() {
        let persons = (0..5).map(|i| minimal_person(&format!("p{i}"), "Repetido")).collect();
        let idx = index_with(persons);
        assert_eq!(search_persons(&idx, "repetido", 3).len(), 3);
    }

    #[test]
    fn list_sorts_by_name_by_default() {
        let idx = index_with(vec![minimal_person("p1", "Zeca"), minimal_person("p2", "Ana")]);
        let listed = list_persons(&idx, None, None, 20);
        assert_eq!(listed[0].primary_name, "Ana");
    }

    #[test]
    fn list_sorts_by_mention_count_descending_when_requested() {
        let mut low = minimal_person("p1", "Low");
        low.mention_count = 1;
        let mut high = minimal_person("p2", "High");
        high.mention_count = 9;
        let idx = index_with(vec![low, high]);
        let listed = list_persons(&idx, None, Some("mentionCount"), 20);
        assert_eq!(listed[0].primary_name, "High");
    }

    #[test]
    fn list_is_bounded_by_limit() {
        let persons = (0..5).map(|i| minimal_person(&format!("p{i}"), &format!("Nome{i}"))).collect();
        let idx = index_with(persons);
        assert_eq!(list_persons(&idx, None, None, 2).len(), 2);
    }
}
