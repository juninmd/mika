//! People-memory extraction by LLM: one call per qualifying user message, prompt in
//! `prompts/person-catalog/entity-extraction.md`, answer = `{"ops":[…],"relationships":[…]}`.
//!
//! `ops` are the people-memory operations ([`PersonOp`]: upsert a person, add a fact, name a
//! relation-only placeholder, add an alias, flag an ambiguous reference) that
//! `people_memory::apply_ops` resolves against the catalog. `relationships` are person↔person
//! links for the social graph ("o João trabalha com a Maria"). Parsing is tolerant item by item:
//! a malformed op is dropped, never the whole answer — a small local model is the norm here.
//!
//! No test seam: the LLM call is [`extract`]; everything after it (parsing, resolution, writing)
//! is pure and tested at its own boundary (`people_memory::tests`, `ingest`).

use std::collections::HashMap;

use serde::Deserialize;

use crate::config::store::ConfigStore;

use super::people_memory::{parse_ops, PersonOp};
use super::types::{PendingClarification, PersonRecord};

/// Known people injected into the prompt — enough for resolution, bounded for a small model.
pub(super) const KNOWN_PEOPLE_HINT_LIMIT: usize = 40;
const PROMPT_NAMESPACE: &str = "person-catalog";
const PROMPT_KEY: &str = "entity-extraction";

/// A person↔person link extracted in the same call ("João trabalha com Maria").
#[derive(Debug, Clone, Deserialize)]
pub(super) struct ExtractedRelationship {
    pub from: String,
    pub to: String,
    pub label: String,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ExtractionResult {
    pub ops: Vec<PersonOp>,
    pub relationships: Vec<ExtractedRelationship>,
}

/// What the prompt is filled with, resolved synchronously before the call.
pub(super) struct ExtractionInput {
    pub text: String,
    pub known_people: String,
    pub pending: String,
    pub app_context: Option<String>,
    pub character_name: String,
}

/// `- Dani (esposa; also: Esposa)` / `- Irmão (unnamed; irmão)` — one line per known person.
pub(super) fn known_people_lines(people: &[PersonRecord]) -> String {
    if people.is_empty() {
        return "none".to_string();
    }
    let line = |p: &PersonRecord| {
        let mut notes: Vec<String> = Vec::new();
        if p.unnamed {
            notes.push("unnamed".to_string());
        }
        notes.extend(p.relation_to_user.clone());
        if !p.aliases.is_empty() {
            notes.push(format!("also: {}", p.aliases.join(", ")));
        }
        if notes.is_empty() { format!("- {}", p.primary_name) } else { format!("- {} ({})", p.primary_name, notes.join("; ")) }
    };
    people.iter().map(line).collect::<Vec<_>>().join("\n")
}

/// `- which "irmão": Pedro or João` — the open clarifications the message may be answering.
pub(super) fn pending_lines(pending: &[PendingClarification], people: &[PersonRecord]) -> String {
    if pending.is_empty() {
        return "none".to_string();
    }
    let name = |id: &String| people.iter().find(|p| &p.id == id).map(|p| p.primary_name.clone());
    pending
        .iter()
        .map(|p| format!("- which \"{}\": {}", p.label, p.candidate_ids.iter().filter_map(name).collect::<Vec<_>>().join(" or ")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn prompt_vars(input: &ExtractionInput) -> HashMap<String, String> {
    let context = input.app_context.as_deref().map(|c| format!("App in use: {c}")).unwrap_or_default();
    HashMap::from([
        ("text".to_string(), input.text.clone()),
        ("knownPeople".to_string(), input.known_people.clone()),
        ("pending".to_string(), input.pending.clone()),
        ("appContext".to_string(), context),
        ("characterName".to_string(), input.character_name.clone()),
    ])
}

/// The only network step. Offline mode is refused inside `run_turn_llm_raw`.
pub(super) async fn extract(store: &ConfigStore, input: &ExtractionInput) -> Result<ExtractionResult, String> {
    let prompt = crate::modules::chat::load_prompt_no_handle(PROMPT_NAMESPACE, PROMPT_KEY, &prompt_vars(input))?;
    let (_handle, abort) = crate::modules::brain::turn::TurnAbortHandle::new();
    let observer = crate::modules::brain::turn::NullObserver;
    let result = crate::modules::brain::turn::run_turn_llm_raw(store, Some(&prompt), &input.text, abort, &observer).await.map_err(|e| e.to_string())?;
    Ok(parse_extraction_json(&result.text))
}

fn parse_extraction_json(raw: &str) -> ExtractionResult {
    #[derive(Deserialize)]
    struct Links {
        #[serde(default)]
        relationships: Vec<serde_json::Value>,
    }
    let relationships = extract_json_object(raw)
        .and_then(|s| serde_json::from_str::<Links>(s).ok())
        .map(|l| l.relationships.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
        .unwrap_or_default();
    ExtractionResult { ops: parse_ops(raw), relationships }
}

/// First balanced `{…}` — some models still wrap JSON in fences or prose.
pub(super) fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let mut depth = 0i32;
    for (offset, ch) in raw[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&raw[start..start + offset + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_and_links_come_from_one_answer_and_a_bad_link_costs_nothing_else() {
        let raw = r#"```json
        {"ops":[{"op":"upsert_person","ref":{"name":"João","relation":"irmão"}}],
         "relationships":[{"from":"João","to":"Maria","label":"trabalha com"},{"from":"João","to":"Maria"}]}
        ```"#;
        let result = parse_extraction_json(raw);
        assert_eq!(result.ops.len(), 1);
        assert_eq!(result.relationships.len(), 1, "the link without a label is dropped, the rest survives");
        assert_eq!(result.relationships[0].label, "trabalha com");
    }

    #[test]
    fn prose_or_garbage_is_an_empty_extraction() {
        let result = parse_extraction_json("desculpe, não entendi");
        assert!(result.ops.is_empty() && result.relationships.is_empty());
    }

    /// The shipped prompt renders with every placeholder filled — a missing variable would reach
    /// the model as an empty hole.
    #[test]
    fn the_prompt_file_renders_every_placeholder() {
        let people = vec![PersonRecord { primary_name: "Esposa".into(), unnamed: true, relation_to_user: Some("esposa".into()), ..Default::default() }];
        let input = ExtractionInput {
            text: "minha esposa se chama Dani".into(),
            known_people: known_people_lines(&people),
            pending: "none".into(),
            app_context: Some("Elden Ring".into()),
            character_name: "Mika".into(),
        };
        let prompt = crate::modules::chat::load_prompt_no_handle(PROMPT_NAMESPACE, PROMPT_KEY, &prompt_vars(&input)).expect("prompt file");
        assert!(prompt.contains("- Esposa (unnamed; esposa)"));
        assert!(prompt.contains("App in use: Elden Ring"));
        assert!(prompt.contains("Message: \"minha esposa se chama Dani\""));
        assert!(!prompt.contains("{{"), "unfilled placeholder");
    }

    #[test]
    fn pending_lines_name_the_candidates() {
        let people = vec![
            PersonRecord { id: "a".into(), primary_name: "Pedro".into(), ..Default::default() },
            PersonRecord { id: "b".into(), primary_name: "João".into(), ..Default::default() },
        ];
        let pending = vec![PendingClarification { id: "q".into(), relation: "brother".into(), label: "irmão".into(), candidate_ids: vec!["a".into(), "b".into()], facts: vec![], asked: true, created_at: 0 }];
        assert_eq!(pending_lines(&pending, &people), "- which \"irmão\": Pedro or João");
    }
}
