//! "## What You Know About The User" — the user's own long-term facts for the chat turn (item 3 of
//! "Recuperação para o prompt", 04-memory.md): only ACTIVE facts (a contradicted one was already
//! superseded by `fact_store::add_fact`), at most [`LONG_TERM_FACTS_LIMIT`], each truncated to
//! [`FACT_CHAR_CAP`] — the whole dossier never fits a turn's budget.
//!
//! **Which five: relevance to the message, recency as tiebreak.** Picking the five newest dropped
//! exactly the fact the user was talking about whenever newer, unrelated ones existed. Relevance is
//! cheap on purpose — this runs on the reply's critical path:
//! - lexical: distinct words (>= 3 chars, no stopword, accent-insensitive — the same tokenizer as
//!   episodic keyword search) the fact shares with the message;
//! - semantic, when embeddings are on: cosine between the message and the fact, only from vectors
//!   already in hand (the message's was fetched by episodic recall this very turn; facts' are warmed
//!   in the background, [`crate::modules::memory::episodic::warm_fact_vectors`]) — never a network
//!   wait here. Below [`SEMANTIC_FLOOR`] a similarity counts as unrelated.

use crate::modules::memory::social::types::LearnedFact;

use super::truncate_chars;

/// Teto de fatos de longo prazo injetados por turno — orçamento de prompt (04-memory.md: "quando o
/// orçamento estoura, corta-se de baixo para cima"; um teto fixo aqui é o corte mais simples que já
/// respeita isso, sem precisar contar tokens).
const LONG_TERM_FACTS_LIMIT: usize = 5;
const FACT_CHAR_CAP: usize = 140;
/// Unrelated sentences still score 0.4-0.8 on common embedding models; only what clears this counts.
const SEMANTIC_FLOOR: f64 = 0.5;
/// Facts whose vectors one background pass warms, at most.
const WARM_BATCH: usize = 64;

/// Reads the fact graph and ranks against `message`. Called after episodic recall has embedded the
/// message (see `compose_system_prompt`), so its vector is a cache hit.
pub(super) fn build_long_term_facts_block(message: &str) -> String {
    let Some(social) = crate::modules::memory::social_reader() else { return String::new() };
    let Ok(graph) = social.get_graph() else { return String::new() };
    let embeddings = crate::modules::memory::episodic_reader().is_some_and(|episodic| episodic.embedding_enabled);
    if !embeddings {
        return format_facts_block(graph.facts, message, &|_| None);
    }
    let texts = user_facts(graph.facts.clone()).into_iter().take(WARM_BATCH).map(|f| f.fact).collect();
    crate::modules::memory::episodic::warm_fact_vectors(texts);
    let similarity = crate::modules::memory::episodic::fact_similarity_to(message);
    format_facts_block(graph.facts, message, &similarity)
}

/// Only the user's own active facts: people facts live on their person card (`people_block`).
fn user_facts(mut facts: Vec<LearnedFact>) -> Vec<LearnedFact> {
    let user = crate::modules::memory::social::USER_SUBJECT;
    facts.retain(|f| f.active != Some(false) && crate::modules::memory::social::resolve_self_subject(&f.subject) == user);
    facts
}

fn relevance(fact: &str, message_words: &[String], similarity: &dyn Fn(&str) -> Option<f64>) -> f64 {
    let words = crate::modules::memory::episodic::tokenize_query(fact);
    let shared = words.iter().filter(|word| message_words.contains(word)).count() as f64;
    let semantic = similarity(fact).filter(|sim| *sim >= SEMANTIC_FLOOR).unwrap_or(0.0);
    shared + semantic
}

/// Pure: filter, rank by relevance to `message` (newest first on ties) and format.
fn format_facts_block(facts: Vec<LearnedFact>, message: &str, similarity: &dyn Fn(&str) -> Option<f64>) -> String {
    let message_words = if message.trim().is_empty() { Vec::new() } else { crate::modules::memory::episodic::tokenize_query(message) };
    let mut ranked: Vec<(f64, LearnedFact)> =
        user_facts(facts).into_iter().map(|f| (relevance(&f.fact, &message_words, similarity), f)).collect();
    ranked.sort_by(|(a_score, a), (b_score, b)| b_score.total_cmp(a_score).then(b.updated_at.cmp(&a.updated_at)));
    ranked.truncate(LONG_TERM_FACTS_LIMIT);
    if ranked.is_empty() {
        return String::new();
    }
    let lines = ranked.iter().map(|(_, f)| format!("- {}", truncate_chars(&f.fact, FACT_CHAR_CAP))).collect::<Vec<_>>().join("\n");
    format!("\n\n## What You Know About The User\n{lines}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(id: &str, text: &str, active: Option<bool>, updated_at: i64) -> LearnedFact {
        LearnedFact {
            id: id.to_string(),
            subject: "user".to_string(),
            fact: text.to_string(),
            confidence: 1.0,
            predicate: None,
            object: None,
            active,
            superseded_by: None,
            source_turn_id: None,
            created_at: None,
            updated_at,
        }
    }

    /// Item 3 de "Recuperação para o prompt" (04-memory.md): só fatos ATIVOS entram, mais recentes
    /// primeiro, e o contraditado (`active: Some(false)`, já superado por `fact_store::add_fact`)
    /// nunca aparece — provaria a regressão que reviveria um fato já corrigido no prompt.
    #[test]
    fn format_facts_block_only_includes_active_facts_most_recent_first() {
        let facts = vec![
            fact("f1", "jogava Valorant todo dia", Some(false), 1),
            fact("f2", "parou de jogar Valorant", Some(true), 3),
            fact("f3", "prefere jogos coop", Some(true), 2),
        ];
        let block = format_facts_block(facts, "", &no_vectors);
        assert_eq!(block, "\n\n## What You Know About The User\n- parou de jogar Valorant\n- prefere jogos coop");
        assert!(!block.contains("jogava Valorant todo dia"), "fato superado não pode voltar ao prompt");
    }

    #[test]
    fn format_facts_block_caps_at_the_budget_and_truncates_long_facts() {
        let facts = (0..8).map(|i| fact(&format!("f{i}"), &"x".repeat(200), Some(true), i)).collect::<Vec<_>>();
        let block = format_facts_block(facts, "", &no_vectors);
        assert_eq!(block.matches("- ").count(), LONG_TERM_FACTS_LIMIT, "teto de fatos por turno não pode estourar o orçamento");
        assert!(block.contains('…'), "fato mais longo que o teto de caracteres tem de ser truncado, não cortado sem aviso");
    }

    fn no_vectors(_: &str) -> Option<f64> {
        None
    }

    /// Only 5 facts fit a turn, and they were the 5 newest: asked "bora de Hollow Knight?", she did
    /// not know the user had platinumed it last month — that fact had been pushed out by newer,
    /// unrelated ones. A fact that shares words with the message outranks newer unrelated ones;
    /// among equally (ir)relevant facts, newest still wins.
    #[test]
    fn facts_relevant_to_the_message_outrank_newer_unrelated_ones() {
        let mut facts = vec![fact("old", "platinou Hollow Knight mês passado", Some(true), 1)];
        facts.extend((2..9).map(|i| fact(&format!("f{i}"), &format!("fato recente número {i}"), Some(true), i)));

        let block = format_facts_block(facts, "bora jogar hollow knight hoje?", &no_vectors);
        assert!(block.contains("platinou Hollow Knight"), "{block}");
        let lines: Vec<&str> = block.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(lines[0], "- platinou Hollow Knight mês passado");
        assert_eq!(lines[1], "- fato recente número 8", "recency breaks the tie among the rest");
    }

    /// Embeddings catch what words miss ("carro" vs "Civic"): a semantically close fact outranks
    /// newer unrelated ones, and a similarity under the floor counts as unrelated.
    #[test]
    fn a_semantically_close_fact_outranks_newer_unrelated_ones() {
        let mut facts = vec![fact("civic", "tem um Civic 2012", Some(true), 1)];
        facts.extend((2..9).map(|i| fact(&format!("f{i}"), &format!("fato recente número {i}"), Some(true), i)));
        let similarity = |text: &str| Some(if text.contains("Civic") { 0.91 } else { 0.40 });

        let block = format_facts_block(facts, "meu carro quebrou", &similarity);
        assert_eq!(block.lines().find(|l| l.starts_with("- ")), Some("- tem um Civic 2012"), "{block}");
    }

    /// A fact the old extraction filed under someone else ("esposa") is not the user's.
    #[test]
    fn format_facts_block_leaves_other_peoples_facts_to_their_cards() {
        let mut wife = fact("f1", "gosta de sorvete", Some(true), 2);
        wife.subject = "esposa".to_string();
        let block = format_facts_block(vec![wife, fact("f2", "mora em Recife", Some(true), 1)], "", &no_vectors);
        assert_eq!(block, "\n\n## What You Know About The User\n- mora em Recife");
    }

    #[test]
    fn format_facts_block_is_empty_without_any_fact() {
        assert_eq!(format_facts_block(vec![], "", &no_vectors), "");
    }
}
