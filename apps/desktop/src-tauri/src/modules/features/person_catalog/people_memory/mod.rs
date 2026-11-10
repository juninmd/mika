//! People memory — the user's people by relation and by name, and what Mika knows about each.
//!
//! The extractor (`mention_extract`, post-turn, off the reply path) turns a user message into
//! [`PersonOp`]s; [`apply_ops`] resolves each reference against the catalog and writes:
//!
//! - **relation-only, unique relation** ("minha esposa gosta de sorvete") → find or create an
//!   unnamed placeholder (`primary_name` = "Esposa") and attach the fact;
//! - **naming** ("minha esposa se chama Dani") → the placeholder becomes Dani, "Esposa" stays as an
//!   alias, facts are kept; a separately catalogued "Dani" absorbs the placeholder instead;
//! - **relation-only, multi relation with ≥2 matches** ("meu irmão", two brothers) → no guess: a
//!   [`PendingClarification`](super::types::PendingClarification) holds the fact and the turn
//!   context asks "o Pedro ou o João?"; the answer (a candidate's name, or a new brother) settles it.
//!
//! [`prompt_context`] is the read side the chat turn uses (name/alias/relation → cards, plus the
//! ambiguities to ask about). Everything here is pure over `PersonCatalogIndex`; the store locks,
//! calls, and saves.

mod clarify;
mod context;
mod ops;
pub(crate) mod relations;
mod resolver;

use serde::Deserialize;

use super::types::PersonCatalogIndex;

pub(crate) use clarify::settle;
pub(crate) use context::{prompt_context, Ambiguity, PeopleContext};
pub(crate) use ops::{apply_ops, merge_facts};

/// A clarification nobody answered in two days is dropped (its facts with it).
pub(crate) const PENDING_TTL_MS: i64 = 2 * 24 * 3_600_000;

/// How the extractor points at someone: by name, by relation to the user, or both.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct PersonRef {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub relation: Option<String>,
}

/// The extraction schema (`prompts/person-catalog/entity-extraction.md`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum PersonOp {
    UpsertPerson {
        #[serde(rename = "ref")]
        target: PersonRef,
        #[serde(default, rename = "type")]
        kind: Option<String>,
    },
    AddFact {
        #[serde(rename = "ref")]
        target: PersonRef,
        fact: String,
        #[serde(default)]
        confidence: Option<f64>,
    },
    SetName {
        #[serde(rename = "ref")]
        target: PersonRef,
        name: String,
    },
    AddAlias {
        #[serde(rename = "ref")]
        target: PersonRef,
        alias: String,
    },
    AskClarification {
        #[serde(rename = "ref")]
        target: PersonRef,
    },
}

pub(crate) struct ApplyCtx<'a> {
    /// The user message, already PII-redacted — kept as each new fact's `source`.
    pub text: &'a str,
    pub now: i64,
    pub gen_id: fn() -> String,
    pub max_mentions: usize,
}

#[derive(Debug, Default)]
pub(crate) struct ApplyReport {
    pub touched: Vec<String>,
    pub created: Vec<String>,
    pub clarifications_opened: usize,
    pub clarifications_resolved: usize,
}

impl ApplyReport {
    fn touch(&mut self, id: &str) {
        if !self.touched.iter().any(|t| t == id) {
            self.touched.push(id.to_string());
        }
    }

    pub(crate) fn changed(&self) -> bool {
        !self.touched.is_empty() || self.clarifications_opened > 0 || self.clarifications_resolved > 0
    }
}

/// Tolerant, item by item: a malformed op is skipped, never the whole batch. Accepts `{"ops":[…]}`
/// or a bare array, fenced or not.
pub(crate) fn parse_ops(raw: &str) -> Vec<PersonOp> {
    let value = super::mention_extract::extract_json_object(raw).and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
    let items = match value {
        Some(serde_json::Value::Object(mut map)) => map.remove("ops"),
        _ => raw.find('[').and_then(|start| serde_json::from_str(&raw[start..=raw.rfind(']').filter(|&end| end > start)?]).ok()),
    };
    let Some(serde_json::Value::Array(items)) = items else { return Vec::new() };
    items.into_iter().filter_map(|item| serde_json::from_value(item).ok()).collect()
}

/// Clarifications not yet asked, with their candidates' display names — marked asked by the
/// caller once injected, so Mika asks once, not every turn.
pub(crate) fn unasked_clarifications(index: &PersonCatalogIndex) -> Vec<Ambiguity> {
    index
        .pending_clarifications
        .iter()
        .filter(|p| !p.asked)
        .map(|p| Ambiguity {
            relation: relations::canonical_relation(&p.label).map_or("", |r| r.key),
            label: p.label.clone(),
            candidates: index.persons.iter().filter(|person| p.candidate_ids.contains(&person.id)).map(|person| person.primary_name.clone()).collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
