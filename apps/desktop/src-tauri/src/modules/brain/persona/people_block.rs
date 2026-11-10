//! People cards for the chat turn — long-term people memory retrieved by relevance: whoever the
//! user's message names (name, alias) or points at by relation ("minha esposa" → Dani), with the
//! facts Mika knows about them, plus the ambiguities she must ask about instead of guessing ("meu
//! irmão" with two brothers). Matching lives in `person_catalog::people_memory` (pure, no LLM —
//! this is the synchronous path of the turn); this file only formats.

use crate::modules::features::person_catalog::people_memory::Ambiguity;
use crate::modules::features::person_catalog::types::PersonRecord;

use super::truncate_chars;

/// "MUST NOT dump the dossier" (05-person-catalog.md): a few people, a few facts each.
const MENTIONED_PEOPLE_LIMIT: usize = 3;
const FACTS_PER_PERSON: usize = 4;
const FACT_CHAR_CAP: usize = 120;
const PERSON_BIO_CHAR_CAP: usize = 200;
const RELATIONSHIPS_PER_PERSON_CAP: usize = 2;

pub(super) fn build_mentioned_people_block(user_content: &str) -> String {
    if user_content.trim().is_empty() {
        return String::new();
    }
    let Some(catalog) = crate::modules::features::person_catalog::catalog_reader() else { return String::new() };
    let (context, unasked) = catalog.people_for_turn(user_content, MENTIONED_PEOPLE_LIMIT);
    format_people_block(&context.people, &context.ambiguities, &unasked)
}

fn format_people_block(people: &[PersonRecord], ambiguous: &[Ambiguity], unasked: &[Ambiguity]) -> String {
    let mut out = String::new();
    if !people.is_empty() {
        let lines = people.iter().map(format_person_line).collect::<Vec<_>>().join("\n");
        out.push_str(&format!("\n\n## People In This Message\n{lines}"));
    }
    let asks: Vec<String> = ambiguous.iter().map(ask_now_line).chain(unasked.iter().map(ask_later_line)).collect();
    if !asks.is_empty() {
        out.push_str(&format!("\n\n## Ask, Don't Guess\n{}", asks.join("\n")));
    }
    out
}

fn ask_now_line(a: &Ambiguity) -> String {
    format!(
        "- The user said \"{}\" but has more than one: {}. If it matters for your reply, ask which one, naturally and in the user's language. Never pick one yourself.",
        a.label,
        a.candidates.join(", ")
    )
}

fn ask_later_line(a: &Ambiguity) -> String {
    format!(
        "- Earlier the user mentioned their \"{}\" without saying which one ({}). If it fits naturally, ask once which one it was.",
        a.label,
        a.candidates.join(" or ")
    )
}

/// `Dani — the user's esposa; also called: Esposa; works with Ana. Known: gosta de sorvete`.
/// Relation, aliases and facts stay in the user's own words.
fn format_person_line(p: &PersonRecord) -> String {
    let name = if p.unnamed { format!("{} (name not known yet)", p.primary_name) } else { p.primary_name.clone() };
    let mut parts: Vec<String> = Vec::new();
    match p.relation_to_user.as_deref().filter(|r| !r.trim().is_empty()) {
        Some(rel) => parts.push(format!("the user's {rel}")),
        None => parts.push(affinity_label(p)),
    }
    if !p.aliases.is_empty() {
        parts.push(format!("also called: {}", p.aliases.join(", ")));
    }
    if let Some(rels) = p.relationships.as_ref().filter(|r| !r.is_empty()) {
        parts.extend(rels.iter().take(RELATIONSHIPS_PER_PERSON_CAP).map(|r| format!("{} {}", r.kind, r.name)));
    }
    let mut line = format!("- {name} — {}", parts.join("; "));
    if let Some(bio) = p.bio.as_deref().filter(|b| !b.trim().is_empty()) {
        line.push_str(&format!(". {}", truncate_chars(bio, PERSON_BIO_CHAR_CAP)));
    }
    let facts = top_facts(p);
    if !facts.is_empty() {
        line.push_str(&format!(". Known: {}", facts.join("; ")));
    }
    line
}

fn affinity_label(p: &PersonRecord) -> String {
    p.relationship_type
        .and_then(|r| serde_json::to_value(r).ok())
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "acquaintance".to_string())
}

fn top_facts(p: &PersonRecord) -> Vec<String> {
    let mut facts: Vec<_> = p.facts.iter().collect();
    facts.sort_by_key(|f| std::cmp::Reverse(f.updated_at));
    facts.into_iter().take(FACTS_PER_PERSON).map(|f| truncate_chars(&f.text, FACT_CHAR_CAP)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::features::person_catalog::types::{PersonFact, PersonRelationship, RelationshipType};

    fn person(name: &str) -> PersonRecord {
        PersonRecord { primary_name: name.to_string(), ..Default::default() }
    }

    fn fact(text: &str, at: i64) -> PersonFact {
        PersonFact { id: text.to_string(), text: text.to_string(), confidence: 0.9, source: None, updated_at: at }
    }

    #[test]
    fn the_card_carries_relation_aliases_and_facts_in_the_users_words() {
        let mut dani = person("Dani");
        dani.relation_to_user = Some("esposa".to_string());
        dani.aliases = vec!["Esposa".to_string()];
        dani.facts = vec![fact("gosta de sorvete", 1), fact("trabalha de casa", 2)];
        let block = format_people_block(&[dani], &[], &[]);
        assert_eq!(block, "\n\n## People In This Message\n- Dani — the user's esposa; also called: Esposa. Known: trabalha de casa; gosta de sorvete");
    }

    #[test]
    fn facts_are_capped_newest_first() {
        let mut p = person("Pedro");
        p.facts = (0..7).map(|i| fact(&format!("fato {i}"), i)).collect();
        let block = format_people_block(&[p], &[], &[]);
        assert!(block.contains("Known: fato 6; fato 5; fato 4; fato 3"), "{block}");
        assert!(!block.contains("fato 2"));
    }

    #[test]
    fn an_unnamed_placeholder_says_so_and_a_person_without_relation_uses_the_affinity_ladder() {
        let mut wife = person("Esposa");
        wife.unnamed = true;
        wife.relation_to_user = Some("esposa".to_string());
        let mut bruno = person("Bruno");
        bruno.relationship_type = Some(RelationshipType::Friend);
        bruno.relationships = Some(vec![PersonRelationship { name: "Ana".into(), kind: "namora".into(), affinity: None, trust: None, description: None, last_interaction_at: None }]);
        let block = format_people_block(&[wife, bruno], &[], &[]);
        assert!(block.contains("- Esposa (name not known yet) — the user's esposa"), "{block}");
        assert!(block.contains("- Bruno — friend; namora Ana"), "{block}");
    }

    #[test]
    fn an_ambiguity_tells_mika_to_ask_instead_of_guessing() {
        let now = Ambiguity { relation: "brother", label: "irmão".into(), candidates: vec!["Pedro".into(), "João".into()] };
        let block = format_people_block(&[], std::slice::from_ref(&now), &[]);
        assert!(block.starts_with("\n\n## Ask, Don't Guess\n- The user said \"irmão\" but has more than one: Pedro, João."), "{block}");
        let later = format_people_block(&[], &[], &[now]);
        assert!(later.contains("(Pedro or João)"), "{later}");
    }

    #[test]
    fn nothing_mentioned_adds_nothing() {
        assert_eq!(format_people_block(&[], &[], &[]), "");
    }
}
