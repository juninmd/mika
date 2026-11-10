//! Who does a reference point at? Pure over the index — see the module header for the rules.

use super::super::normalize::catalog_key;
use super::super::types::{PersonCatalogIndex, PersonRecord};
use super::relations::{canonical_relation, exact_relation, Cardinality, Relation};
use super::PersonRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Resolution {
    /// That person (by id). `adopt_name` is set when an unnamed placeholder takes the ref's name.
    Existing { id: String, adopt_name: bool },
    Create,
    /// ≥2 people fit and nothing tells them apart — ask, never guess.
    Ambiguous(Vec<String>),
    Invalid,
}

/// A ref normalized once: a "name" that is only a relation word ("esposa") is a relation.
pub(super) struct Target<'a> {
    pub name: Option<&'a str>,
    pub label: Option<&'a str>,
    pub relation: Option<Relation>,
}

pub(super) fn target_of(r: &PersonRef) -> Target<'_> {
    let name = r.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let label = r.relation.as_deref().map(str::trim).filter(|l| !l.is_empty());
    if let Some(word) = name.and_then(|n| exact_relation(n).map(|rel| (n, rel))) {
        let (n, rel) = word;
        return Target { name: None, label: label.or(Some(n)), relation: label.and_then(canonical_relation).or(Some(rel)) };
    }
    Target { name, label, relation: label.and_then(canonical_relation) }
}

pub(super) fn relation_key(person: &PersonRecord) -> Option<&'static str> {
    person.relation_to_user.as_deref().and_then(canonical_relation).map(|r| r.key)
}

pub(super) fn with_relation<'a>(index: &'a PersonCatalogIndex, relation: Relation) -> Vec<&'a PersonRecord> {
    index.persons.iter().filter(|p| relation_key(p) == Some(relation.key)).collect()
}

/// By primary name or alias — never by a placeholder's relation word (that is a relation lookup).
pub(super) fn find_by_name<'a>(index: &'a PersonCatalogIndex, name: &str) -> Option<&'a PersonRecord> {
    let key = catalog_key(name);
    index.persons.iter().find(|p| !p.unnamed && std::iter::once(&p.primary_name).chain(p.aliases.iter()).any(|n| catalog_key(n) == key))
}

pub(super) fn resolve(index: &PersonCatalogIndex, target: &Target) -> Resolution {
    match (target.name, target.relation) {
        (Some(name), relation) => resolve_named(index, name, relation),
        (None, Some(relation)) => resolve_relation_only(index, relation),
        (None, None) => Resolution::Invalid,
    }
}

fn resolve_named(index: &PersonCatalogIndex, name: &str, relation: Option<Relation>) -> Resolution {
    if let Some(person) = find_by_name(index, name) {
        return Resolution::Existing { id: person.id.clone(), adopt_name: false };
    }
    let Some(relation) = relation else { return Resolution::Create };
    let unnamed: Vec<&PersonRecord> = with_relation(index, relation).into_iter().filter(|p| p.unnamed).collect();
    match (relation.cardinality, unnamed.len()) {
        (_, 0) => Resolution::Create,
        (Cardinality::Single, _) | (Cardinality::Multi, 1) => Resolution::Existing { id: unnamed[0].id.clone(), adopt_name: true },
        (Cardinality::Multi, _) => Resolution::Ambiguous(unnamed.iter().map(|p| p.id.clone()).collect()),
    }
}

fn resolve_relation_only(index: &PersonCatalogIndex, relation: Relation) -> Resolution {
    let mut candidates = with_relation(index, relation);
    match (relation.cardinality, candidates.len()) {
        (_, 0) => Resolution::Create,
        (_, 1) => Resolution::Existing { id: candidates[0].id.clone(), adopt_name: false },
        (Cardinality::Single, _) => {
            // Unique by definition: the most recently touched one is "a esposa".
            candidates.sort_by_key(|p| std::cmp::Reverse(p.updated_at));
            Resolution::Existing { id: candidates[0].id.clone(), adopt_name: false }
        }
        (Cardinality::Multi, _) => Resolution::Ambiguous(candidates.iter().map(|p| p.id.clone()).collect()),
    }
}

/// Single-relation placeholder that a named person should absorb ("minha esposa é a Dani" while
/// both "Esposa" and "Dani" exist). Multi relations never auto-merge: "meu irmão Pedro" does not
/// prove the unnamed brother was Pedro.
pub(super) fn placeholder_to_absorb(index: &PersonCatalogIndex, relation: Relation, keep_id: &str) -> Option<String> {
    if relation.cardinality != Cardinality::Single {
        return None;
    }
    with_relation(index, relation).into_iter().find(|p| p.unnamed && p.id != keep_id).map(|p| p.id.clone())
}
