//! Applying extraction ops to the index — the only writer of person facts, placeholder names and
//! relation-driven merges. Pure over `&mut PersonCatalogIndex`; the store saves afterwards.

use super::super::mutate::{add_mention_to_person, merge_two_persons, rebuild_alias_map};
use super::super::normalize::{catalog_key, is_likely_person_name};
use super::super::types::{MentionSource, NewMention, PersonCatalogIndex, PersonFact, PersonRecord, PersonType};
use super::relations::{fold, is_multi_relation_word, placeholder_name};
use super::resolver::{find_by_name, placeholder_to_absorb, resolve, target_of, Resolution, Target};
use super::{clarify, ApplyCtx, ApplyReport, PersonOp, PersonRef};

/// Below this the extractor is guessing — "talvez goste de chá" is not a memory.
pub(super) const MIN_FACT_CONFIDENCE: f64 = 0.6;
const DEFAULT_FACT_CONFIDENCE: f64 = 0.8;
const MAX_FACTS_PER_PERSON: usize = 40;
const FACT_SOURCE_CHARS: usize = 200;

enum Ensured {
    Person(String),
    Ambiguous(Vec<String>),
    Nothing,
}

pub(crate) fn apply_ops(index: &mut PersonCatalogIndex, ops: &[PersonOp], ctx: &ApplyCtx) -> ApplyReport {
    let mut report = ApplyReport::default();
    clarify::expire(index, ctx.now);
    report.clarifications_resolved += clarify::resolve_by_text(index, ctx);
    let mut born: Vec<String> = Vec::new();
    for op in ops {
        apply_one(index, op, ctx, &mut report, &mut born);
    }
    report.clarifications_resolved += clarify::resolve_by_newcomers(index, &born, ctx);
    record_mentions(index, &report.touched, ctx);
    rebuild_alias_map(index);
    report
}

fn apply_one(index: &mut PersonCatalogIndex, op: &PersonOp, ctx: &ApplyCtx, report: &mut ApplyReport, born: &mut Vec<String>) {
    match op {
        PersonOp::UpsertPerson { target, kind } => {
            let t = target_of(target);
            match ensure(index, &t, kind.as_deref(), ctx, report, born) {
                Ensured::Ambiguous(ids) => clarify::open(index, &t, ids, None, ctx, report),
                Ensured::Person(_) | Ensured::Nothing => {}
            }
        }
        PersonOp::AddFact { target, fact, confidence } => apply_fact(index, target, fact, *confidence, ctx, report, born),
        PersonOp::SetName { target, name } => set_name(index, target, name, ctx, report, born),
        PersonOp::AddAlias { target, alias } => {
            if let Ensured::Person(id) = ensure(index, &target_of(target), None, ctx, report, born) {
                add_alias(index, &id, alias);
            }
        }
        PersonOp::AskClarification { target } => {
            let t = target_of(target);
            if let Resolution::Ambiguous(ids) = resolve(index, &t) {
                clarify::open(index, &t, ids, None, ctx, report);
            }
        }
    }
}

fn apply_fact(index: &mut PersonCatalogIndex, target: &PersonRef, fact: &str, confidence: Option<f64>, ctx: &ApplyCtx, report: &mut ApplyReport, born: &mut Vec<String>) {
    let confidence = confidence.unwrap_or(DEFAULT_FACT_CONFIDENCE);
    let fact = fact.trim();
    if fact.is_empty() || confidence < MIN_FACT_CONFIDENCE {
        return;
    }
    let t = target_of(target);
    match ensure(index, &t, None, ctx, report, born) {
        Ensured::Person(id) => {
            if let Some(person) = index.persons.iter_mut().find(|p| p.id == id) {
                add_fact(person, fact, confidence, ctx);
            }
        }
        Ensured::Ambiguous(ids) => clarify::open(index, &t, ids, Some(fact), ctx, report),
        Ensured::Nothing => {}
    }
}

/// "minha esposa se chama Dani" / "a Dani na verdade é Daniela".
fn set_name(index: &mut PersonCatalogIndex, target: &PersonRef, name: &str, ctx: &ApplyCtx, report: &mut ApplyReport, born: &mut Vec<String>) {
    let name = name.trim();
    if !is_likely_person_name(name) {
        return;
    }
    let t = target_of(target);
    if let Some(old) = t.name {
        let Some(id) = find_by_name(index, old).map(|p| p.id.clone()) else { return };
        // Merged: `id` was absorbed and no longer exists — the survivor is the one touched.
        let survivor = match find_by_name(index, name).map(|p| p.id.clone()) {
            Some(other) if other != id => {
                merge_into(index, &other, &id, ctx);
                other
            }
            _ => {
                rename(index, &id, name, ctx.now);
                id
            }
        };
        report.touch(&survivor);
        return;
    }
    let named = Target { name: Some(name), label: t.label, relation: t.relation };
    if let Ensured::Ambiguous(ids) = ensure(index, &named, None, ctx, report, born) {
        clarify::open(index, &t, ids, None, ctx, report);
    }
}

/// Resolve, then make it true: create what is missing, name a placeholder, link the relation and
/// absorb a single-relation placeholder the named person turned out to be.
fn ensure(index: &mut PersonCatalogIndex, t: &Target, kind: Option<&str>, ctx: &ApplyCtx, report: &mut ApplyReport, born: &mut Vec<String>) -> Ensured {
    match resolve(index, t) {
        Resolution::Existing { id, adopt_name } => {
            if adopt_name {
                rename(index, &id, t.name.unwrap_or_default(), ctx.now);
                born.push(id.clone());
            }
            link_relation(index, &id, t, ctx);
            report.touch(&id);
            Ensured::Person(id)
        }
        Resolution::Create => match create(index, t, kind, ctx) {
            Some(id) => {
                born.push(id.clone());
                report.created.push(id.clone());
                report.touch(&id);
                Ensured::Person(id)
            }
            None => Ensured::Nothing,
        },
        Resolution::Ambiguous(ids) => Ensured::Ambiguous(ids),
        Resolution::Invalid => Ensured::Nothing,
    }
}

fn link_relation(index: &mut PersonCatalogIndex, id: &str, t: &Target, ctx: &ApplyCtx) {
    let Some(label) = t.label else { return };
    if let Some(person) = index.persons.iter_mut().find(|p| p.id == id) {
        if person.relation_to_user.is_none() {
            person.relation_to_user = Some(label.to_string());
            person.updated_at = ctx.now;
        }
    }
    if let Some(placeholder) = t.relation.and_then(|rel| placeholder_to_absorb(index, rel, id)) {
        merge_into(index, id, &placeholder, ctx);
    }
}

fn merge_into(index: &mut PersonCatalogIndex, keep: &str, absorb: &str, ctx: &ApplyCtx) {
    if let Err(error) = merge_two_persons(index, keep, absorb, ctx.now, ctx.max_mentions) {
        log::warn!("people memory: merge skipped: {error}");
    }
}

fn create(index: &mut PersonCatalogIndex, t: &Target, kind: Option<&str>, ctx: &ApplyCtx) -> Option<String> {
    let (name, unnamed) = match t.name {
        Some(name) if is_likely_person_name(name) => (name.to_string(), false),
        Some(_) => return None,
        None => (placeholder_name(t.label?), true),
    };
    let key = catalog_key(&name);
    if index.blocked.as_ref().is_some_and(|b| b.contains(&key)) {
        return None;
    }
    let id = (ctx.gen_id)();
    index.persons.push(PersonRecord {
        id: id.clone(),
        primary_name: name,
        kind: kind.and_then(PersonType::parse).unwrap_or(PersonType::RealPerson),
        relation_to_user: t.label.map(str::to_string),
        unnamed,
        enriched: Some(false),
        confidence: Some(0.3),
        first_seen_at: Some(ctx.now),
        last_seen_at: Some(ctx.now),
        created_at: ctx.now,
        updated_at: ctx.now,
        ..Default::default()
    });
    Some(id)
}

/// The new name takes over; the old one stays reachable as an alias unless it is a multi-relation
/// word ("Irmão"), which would hijack every later "meu irmão".
fn rename(index: &mut PersonCatalogIndex, id: &str, name: &str, now: i64) {
    let Some(person) = index.persons.iter_mut().find(|p| p.id == id) else { return };
    let old = std::mem::replace(&mut person.primary_name, name.to_string());
    person.unnamed = false;
    person.updated_at = now;
    let key = catalog_key(name);
    person.aliases.retain(|a| catalog_key(a) != key);
    if catalog_key(&old) != key && !is_multi_relation_word(&old) && !person.aliases.contains(&old) {
        person.aliases.push(old);
    }
}

fn add_alias(index: &mut PersonCatalogIndex, id: &str, alias: &str) {
    let alias = alias.trim();
    let key = catalog_key(alias);
    let owned_by_other = index.alias_map.get(&key).is_some_and(|owner| owner != id);
    if key.is_empty() || owned_by_other || is_multi_relation_word(alias) {
        return;
    }
    if let Some(person) = index.persons.iter_mut().find(|p| p.id == id) {
        if catalog_key(&person.primary_name) != key && !person.aliases.iter().any(|a| catalog_key(a) == key) {
            person.aliases.push(alias.to_string());
        }
    }
}

/// Same fact twice (or one refining the other: "gosta de sorvete" → "gosta de sorvete de
/// chocolate") updates in place instead of stacking duplicates.
fn same_fact(a: &str, b: &str) -> bool {
    a == b || (a.len().min(b.len()) >= 8 && (a.contains(b) || b.contains(a)))
}

pub(super) fn add_fact(person: &mut PersonRecord, text: &str, confidence: f64, ctx: &ApplyCtx) {
    let folded = fold(text);
    match person.facts.iter_mut().find(|f| same_fact(&fold(&f.text), &folded)) {
        Some(existing) => {
            if text.chars().count() > existing.text.chars().count() {
                existing.text = text.to_string();
            }
            existing.confidence = existing.confidence.max(confidence);
            existing.updated_at = ctx.now;
        }
        None => person.facts.push(PersonFact {
            id: (ctx.gen_id)(),
            text: text.to_string(),
            confidence,
            source: Some(ctx.text.chars().take(FACT_SOURCE_CHARS).collect()).filter(|s: &String| !s.is_empty()),
            updated_at: ctx.now,
        }),
    }
    cap_facts(&mut person.facts);
    person.updated_at = ctx.now;
}

/// Decay by budget: past the cap, the least confident (then oldest) facts go first.
fn cap_facts(facts: &mut Vec<PersonFact>) {
    if facts.len() <= MAX_FACTS_PER_PERSON {
        return;
    }
    let mut ranked: Vec<usize> = (0..facts.len()).collect();
    ranked.sort_by(|&a, &b| facts[a].confidence.total_cmp(&facts[b].confidence).then(facts[a].updated_at.cmp(&facts[b].updated_at)));
    let drop: Vec<usize> = ranked.into_iter().take(facts.len() - MAX_FACTS_PER_PERSON).collect();
    let mut i = 0;
    facts.retain(|_| {
        let keep = !drop.contains(&i);
        i += 1;
        keep
    });
}

/// Union for a merge — `a` wins on a duplicate.
pub(crate) fn merge_facts(a: &[PersonFact], b: &[PersonFact]) -> Vec<PersonFact> {
    let mut merged = a.to_vec();
    for fact in b {
        let folded = fold(&fact.text);
        if !merged.iter().any(|f| same_fact(&fold(&f.text), &folded)) {
            merged.push(fact.clone());
        }
    }
    merged
}

fn record_mentions(index: &mut PersonCatalogIndex, touched: &[String], ctx: &ApplyCtx) {
    if ctx.text.trim().is_empty() {
        return;
    }
    for id in touched {
        let mention = NewMention { source: MentionSource::Chat, text: ctx.text.to_string(), timestamp: ctx.now, url: None, app_name: None, window_title: None, context: None };
        let _ = add_mention_to_person(index, id, mention, ctx.now, ctx.max_mentions, ctx.gen_id);
    }
}
