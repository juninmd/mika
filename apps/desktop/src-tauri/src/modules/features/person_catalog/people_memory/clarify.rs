//! Pending "qual irmão?" questions: opened when a relation-only reference fits ≥2 people, resolved
//! when the user names one (or a newcomer of that relation appears), expired after a while.

use super::super::types::{PendingClarification, PersonCatalogIndex, PersonRecord};
use super::context::{ambiguities, mentions_person};
use super::ops::add_fact;
use super::relations::fold;
use super::resolver::{relation_key, Target};
use super::{ApplyCtx, ApplyReport, PENDING_TTL_MS};

const MAX_PENDING: usize = 10;
/// Chat answers count only shortly after the question; later, "o Pedro" is just a new message
/// (the settings UI can still settle it until the entry expires).
const ANSWER_WINDOW_MS: i64 = 30 * 60_000;
/// Facts that waited for an answer were confirmed by the user naming the person.
const RESOLVED_FACT_CONFIDENCE: f64 = 0.8;

pub(super) fn expire(index: &mut PersonCatalogIndex, now: i64) {
    index.pending_clarifications.retain(|p| now - p.created_at <= PENDING_TTL_MS);
}

/// Opens (or extends, one per relation) a clarification. `asked` is true when this same user
/// message already made the turn context tell Mika to ask — so the next turn does not ask again.
pub(super) fn open(index: &mut PersonCatalogIndex, t: &Target, ids: Vec<String>, fact: Option<&str>, ctx: &ApplyCtx, report: &mut ApplyReport) {
    let Some(relation) = t.relation else { return };
    let fact = fact.map(str::to_string);
    if let Some(pending) = index.pending_clarifications.iter_mut().find(|p| p.relation == relation.key) {
        for id in ids {
            if !pending.candidate_ids.contains(&id) {
                pending.candidate_ids.push(id);
            }
        }
        pending.facts.extend(fact.filter(|f| !pending.facts.contains(f)));
        return;
    }
    let asked = ambiguities(index, ctx.text).iter().any(|a| a.relation == relation.key);
    let label = t.label.map(str::to_string).unwrap_or_else(|| relation.key.to_string());
    index.pending_clarifications.push(PendingClarification {
        id: (ctx.gen_id)(),
        relation: relation.key.to_string(),
        label,
        candidate_ids: ids,
        facts: fact.into_iter().collect(),
        asked,
        created_at: ctx.now,
    });
    if index.pending_clarifications.len() > MAX_PENDING {
        index.pending_clarifications.remove(0);
    }
    report.clarifications_opened += 1;
}

/// "o Pedro" → exactly one candidate named in the message takes the waiting facts.
pub(super) fn resolve_by_text(index: &mut PersonCatalogIndex, ctx: &ApplyCtx) -> usize {
    let folded = fold(ctx.text);
    let picks: Vec<(String, String)> = index
        .pending_clarifications
        .iter()
        .filter(|pending| ctx.now - pending.created_at <= ANSWER_WINDOW_MS)
        .filter_map(|pending| {
            let named: Vec<&PersonRecord> = candidates(index, pending).filter(|p| mentions_person(&folded, p)).collect();
            (named.len() == 1).then(|| (pending.id.clone(), named[0].id.clone()))
        })
        .collect();
    picks.iter().filter(|(pending, person)| settle(index, pending, Some(person), ctx)).count()
}

/// A person of the pending relation created or named in this batch ("o Lucas, meu outro irmão").
pub(super) fn resolve_by_newcomers(index: &mut PersonCatalogIndex, born: &[String], ctx: &ApplyCtx) -> usize {
    let picks: Vec<(String, String)> = index
        .pending_clarifications
        .iter()
        .filter(|pending| ctx.now - pending.created_at <= ANSWER_WINDOW_MS)
        .filter_map(|pending| {
            let fresh: Vec<&String> = born
                .iter()
                .filter(|id| !pending.candidate_ids.contains(id))
                .filter(|id| index.persons.iter().any(|p| &p.id == *id && relation_key(p) == Some(pending.relation.as_str())))
                .collect();
            (fresh.len() == 1).then(|| (pending.id.clone(), fresh[0].clone()))
        })
        .collect();
    picks.iter().filter(|(pending, person)| settle(index, pending, Some(person), ctx)).count()
}

/// Closes a clarification: its facts go to `person_id` (or are discarded when `None`). Also the
/// settings UI path ("era o Pedro" / dismiss).
pub(crate) fn settle(index: &mut PersonCatalogIndex, pending_id: &str, person_id: Option<&str>, ctx: &ApplyCtx) -> bool {
    let Some(position) = index.pending_clarifications.iter().position(|p| p.id == pending_id) else { return false };
    let pending = index.pending_clarifications.remove(position);
    let Some(person) = person_id.and_then(|id| index.persons.iter_mut().find(|p| p.id == id)) else { return person_id.is_none() };
    for fact in &pending.facts {
        add_fact(person, fact, RESOLVED_FACT_CONFIDENCE, ctx);
    }
    true
}

fn candidates<'a>(index: &'a PersonCatalogIndex, pending: &'a PendingClarification) -> impl Iterator<Item = &'a PersonRecord> {
    index.persons.iter().filter(|p| pending.candidate_ids.contains(&p.id))
}
