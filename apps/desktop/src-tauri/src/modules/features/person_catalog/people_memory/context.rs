//! Read path for the chat turn (synchronous, no LLM): which people cards the user's message pulls
//! in, by name/alias or by relation, and which relation references are ambiguous right now.

use super::super::types::{PersonCatalogIndex, PersonRecord};
use super::relations::{fold, is_multi_relation_word, relations_in_text, Cardinality};
use super::resolver::with_relation;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Ambiguity {
    pub relation: &'static str,
    /// The relation as the user once said it ("irmão").
    pub label: String,
    pub candidates: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct PeopleContext {
    pub people: Vec<PersonRecord>,
    pub ambiguities: Vec<Ambiguity>,
}

/// Whole-word match of the person's name or a non-relation alias in already-folded text.
pub(super) fn mentions_person(folded_text: &str, person: &PersonRecord) -> bool {
    let haystack = format!(" {folded_text} ");
    let names = std::iter::once(&person.primary_name).filter(|_| !person.unnamed).chain(person.aliases.iter().filter(|a| !is_multi_relation_word(a)));
    names.map(|n| fold(n)).any(|n| n.len() >= 2 && haystack.contains(&format!(" {n} ")))
}

/// "meu irmão" with ≥2 brothers and neither named in the same message.
pub(crate) fn ambiguities(index: &PersonCatalogIndex, text: &str) -> Vec<Ambiguity> {
    let folded = fold(text);
    relations_in_text(text)
        .into_iter()
        .filter(|m| m.relation.cardinality == Cardinality::Multi)
        .filter_map(|m| {
            let candidates = with_relation(index, m.relation);
            let named_here = candidates.iter().any(|p| mentions_person(&folded, p));
            (candidates.len() >= 2 && !named_here).then(|| Ambiguity {
                relation: m.relation.key,
                label: candidates[0].relation_to_user.clone().unwrap_or(m.label),
                candidates: candidates.iter().map(|p| p.primary_name.clone()).collect(),
            })
        })
        .collect()
}

pub(crate) fn prompt_context(index: &PersonCatalogIndex, text: &str, limit: usize) -> PeopleContext {
    let folded = fold(text);
    let mut people: Vec<&PersonRecord> = index.persons.iter().filter(|p| mentions_person(&folded, p)).collect();
    for mention in relations_in_text(text) {
        let mut candidates = with_relation(index, mention.relation);
        candidates.sort_by_key(|p| std::cmp::Reverse(p.updated_at));
        let unique = candidates.len() == 1 || (mention.relation.cardinality == Cardinality::Single && !candidates.is_empty());
        let take = if unique { 1 } else { candidates.len() };
        if candidates.iter().any(|c| people.iter().any(|p| p.id == c.id)) {
            continue;
        }
        people.extend(candidates.into_iter().take(take));
    }
    let mut seen = std::collections::HashSet::new();
    people.retain(|p| seen.insert(p.id.clone()));
    PeopleContext { people: people.into_iter().take(limit).cloned().collect(), ambiguities: ambiguities(index, text) }
}
