//! Lógica pura de busca/scoring da memória episódica — porte de `episodic-search.ts`. Sem I/O,
//! sem log: o mesmo contrato do original ("lógica pura ... sem logger, sem eventos").

use std::collections::HashSet;

use super::types::MemoryEntry;
use crate::modules::memory::text::strip_diacritics;

fn stopwords() -> &'static HashSet<&'static str> {
    static WORDS: std::sync::OnceLock<HashSet<&'static str>> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| {
        HashSet::from([
            "a", "as", "ao", "aos", "com", "como", "da", "das", "de", "do", "dos", "e", "ela", "ele", "em", "eu",
            "foi", "na", "nas", "no", "nos", "o", "os", "ou", "para", "pra", "por", "que", "se", "sobre", "um",
            "uma", "the", "and", "for", "with", "about",
        ])
    })
}

/// = `normalizeMemoryText`: acento fora, minúsculo, só letra/número Unicode viram token, resto vira
/// espaço, e o resultado é aparado (o `trim()` do TS; sem colapsar espaço interno — igual ao original,
/// que só faz `.trim()`, não `.replace(/\s+/g, ' ')`).
pub fn normalize_memory_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_was_space = false;
    for ch in strip_diacritics(value).chars() {
        if ch.is_alphanumeric() {
            out.push(ch.to_lowercase().next().unwrap_or(ch));
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

/// Han, Hiragana, Katakana ou Hangul: scripts sem espaço entre palavras (ou com partícula colada).
fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{1100}'..='\u{11FF}' | '\u{3040}'..='\u{30FF}' | '\u{3130}'..='\u{318F}' | '\u{31F0}'..='\u{31FF}'
        | '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{AC00}'..='\u{D7AF}' | '\u{F900}'..='\u{FAFF}'
        | '\u{20000}'..='\u{2FA1F}')
}

/// `term` aparece em `normalized` (saída de [`normalize_memory_text`], só letra/número e espaço
/// único): como PALAVRA INTEIRA nos scripts com espaço (sem isso "Ana" bateria em "banana"), e como
/// substring quando o termo tem CJK — "田中さん"/"철수가" não têm fronteira de palavra a casar.
pub fn mentions_term(normalized: &str, term: &str) -> bool {
    if term.chars().any(is_cjk) {
        return normalized.contains(term);
    }
    normalized.match_indices(term).any(|(at, m)| {
        normalized[..at].chars().next_back().is_none_or(|c| c == ' ')
            && normalized[at + m.len()..].chars().next().is_none_or(|c| c == ' ')
    })
}

/// Menor termo que `forget_mentions` aceita (em caracteres, CJK incluído). Apagar uma ficha é
/// irreversível: "Al", "Ed" ou `ゆき` casam em memória de outra coisa.
const MIN_FORGET_TERM_CHARS: usize = 3;

/// O casamento de "esquecer uma pessoa" sobre o TEXTO ORIGINAL (não o normalizado): palavra inteira
/// e, quando o termo é escrito com inicial maiúscula ("Link", "Doom"), só a ocorrência capitalizada —
/// "clique no link" não é o personagem. Termo com CJK segue o casamento por trecho de
/// [`mentions_term`] (sem fronteira de palavra a respeitar).
pub fn mentions_name(content: &str, term: &str) -> bool {
    let needle = normalize_memory_text(term);
    if needle.chars().count() < MIN_FORGET_TERM_CHARS {
        return false;
    }
    if needle.chars().any(is_cjk) {
        return mentions_term(&normalize_memory_text(content), &needle);
    }
    let words = |text: &str| -> Vec<(String, bool)> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| (normalize_memory_text(word), word.chars().next().is_some_and(char::is_uppercase)))
            .collect()
    };
    let wanted = words(term);
    let capitalized = wanted.first().is_some_and(|(_, upper)| *upper);
    if wanted.is_empty() {
        return false;
    }
    let text = words(content);
    text.windows(wanted.len()).any(|window| {
        window.iter().zip(&wanted).all(|((a, _), (b, _))| a == b) && (!capitalized || window[0].1)
    })
}

/// = `tokenizeQuery`: termos >= 3 chars, sem stopword, deduplicados, no máx 8 — com fallback pro
/// texto bruto normalizado quando a consulta é só stopword (ex. "o que foi?").
pub fn tokenize_query(query: &str) -> Vec<String> {
    let normalized = normalize_memory_text(query);
    let mut seen = HashSet::new();
    let mut words = Vec::new();
    for w in normalized.split_whitespace() {
        let min_chars = if w.chars().any(is_cjk) { 2 } else { 3 };
        if w.chars().count() >= min_chars && !stopwords().contains(w) && seen.insert(w.to_string()) {
            words.push(w.to_string());
        }
    }
    if words.is_empty() {
        return if normalized.is_empty() { vec![] } else { vec![normalized] };
    }
    words.truncate(8);
    words
}

/// Teto da soma dos bônus, como fração do peso da similaridade — ver `calculate_candidate_score`.
const BOOST_CAP_RATIO: f64 = 0.15;

#[derive(Debug, Clone, Copy)]
pub struct ScoreFlags {
    pub reranking_enabled: bool,
    pub emotional_tagging: bool,
}

pub struct ScoredCandidate {
    pub score: f64,
    pub sim: f64,
}

/// = `cosineSimilarity` (pacote `ai`). Vetores de dimensão diferente nunca chegam aqui — o chamador
/// (`calculate_candidate_score`) já filtra por igualdade de tamanho antes.
pub(super) fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let (mut dot, mut norm_a, mut norm_b) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// = `calculateCandidateScore`. `current_game` entra separado (não em `ScoreFlags`) porque muda por
/// chamada (sessão de jogo atual), diferente dos dois flags de config.
pub fn calculate_candidate_score(
    entry_type: &str,
    entry_metadata: &str,
    embedding: &[f64],
    query_embedding: &[f64],
    recency_bias: f64,
    now_ms: i64,
    timestamp_ms: i64,
    flags: ScoreFlags,
    current_game: Option<&str>,
) -> ScoredCandidate {
    // Embedding de provider/modelo anterior tem dimensão diferente — score 0 em vez de lançar.
    let sim = candidate_similarity(embedding, query_embedding);
    score_candidate(sim, entry_type, entry_metadata, recency_bias, now_ms, timestamp_ms, flags, current_game)
}

/// Similaridade pura — separada do score para o chamador descartar abaixo de `minSimilarity` ANTES
/// de parsear o `metadata` (JSON) de cada candidato, que só os aprovados precisam.
pub(super) fn candidate_similarity(embedding: &[f64], query_embedding: &[f64]) -> f64 {
    if embedding.len() == query_embedding.len() { cosine_similarity(query_embedding, embedding) } else { 0.0 }
}

/// Metade de `calculate_candidate_score` que depende do `metadata` — `sim` já calculada.
#[allow(clippy::too_many_arguments)]
pub(super) fn score_candidate(
    sim: f64,
    entry_type: &str,
    entry_metadata: &str,
    recency_bias: f64,
    now_ms: i64,
    timestamp_ms: i64,
    flags: ScoreFlags,
    current_game: Option<&str>,
) -> ScoredCandidate {
    const DAY_MS: f64 = 86_400_000.0;
    let age_in_days = (now_ms - timestamp_ms) as f64 / DAY_MS;
    let recency = (-0.05 * age_in_days).exp();
    let type_boost = if flags.reranking_enabled && entry_type == "summary" { 0.05 } else { 0.0 };

    let mut emotional_boost = 0.0;
    let mut importance_boost = 0.0;
    let mut game_boost = 0.0;
    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(entry_metadata) {
        if flags.emotional_tagging {
            if let Some(emotion) = meta.get("emotion").and_then(|v| v.as_str()) {
                if emotion != "neutral" {
                    emotional_boost = 0.1;
                }
            }
        }
        if let Some(importance) = meta.get("importance") {
            if let Some(imp) = importance.as_f64().or_else(|| importance.as_str().and_then(|s| s.trim().parse().ok())) {
                importance_boost = (imp * 0.02).min(0.1);
            } else if importance == &serde_json::Value::Bool(true) || importance.as_str() == Some("true") {
                importance_boost = 0.1;
            }
        }
        if let Some(game) = current_game {
            if meta.get("game").and_then(|v| v.as_str()) == Some(game) {
                game_boost = 0.15;
            }
        }
    }

    // Bônus só desempata candidatos parecidos, nunca reordena o ranking: o teto é fração do peso
    // da similaridade.
    let boost_cap = BOOST_CAP_RATIO * (1.0 - recency_bias);
    let boost = (type_boost + emotional_boost + importance_boost + game_boost).min(boost_cap);
    let score = sim * (1.0 - recency_bias) + recency * recency_bias + boost;
    ScoredCandidate { score, sim }
}

/// Metade pura de `search()`: pontua por termos casados e ordena (hits > timestamp).
pub fn score_keyword_candidates(candidates: &[MemoryEntry], terms: &[String], limit: usize) -> Vec<MemoryEntry> {
    let mut scored: Vec<(usize, &MemoryEntry)> = candidates
        .iter()
        .map(|entry| {
            let haystack = normalize_memory_text(&entry.content);
            let hits = terms.iter().filter(|t| mentions_term(&haystack, t)).count();
            (hits, entry)
        })
        .filter(|(hits, _)| *hits > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.timestamp.cmp(&a.1.timestamp)));
    scored.into_iter().take(limit).map(|(_, e)| e.clone()).collect()
}

/// Fração de termos da consulta achados no conteúdo — sinal esparso que complementa o denso (embedding).
pub fn lexical_overlap_score(content: &str, terms: &[String]) -> f64 {
    if terms.is_empty() {
        return 0.0;
    }
    let haystack = normalize_memory_text(content);
    let hits = terms.iter().filter(|t| mentions_term(&haystack, t)).count();
    hits as f64 / terms.len() as f64
}

pub struct LexicalRerank<'a> {
    pub query: &'a str,
    pub weight: f64,
}

/// Metade pura de `searchSemantic()`: ordena por score (opcionalmente misturado com casamento
/// léxico) e corta no limite.
pub fn rank_semantic_candidates(
    candidates: Vec<(MemoryEntry, f64)>,
    limit: usize,
    lexical_rerank: Option<LexicalRerank>,
) -> Vec<MemoryEntry> {
    let Some(rerank) = lexical_rerank.filter(|r| r.weight > 0.0) else {
        let mut sorted = candidates;
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        return sorted.into_iter().take(limit).map(|(e, _)| e).collect();
    };

    let terms = tokenize_query(rerank.query);
    let weight = rerank.weight;
    let mut mixed: Vec<(MemoryEntry, f64)> = candidates
        .into_iter()
        .map(|(entry, score)| {
            let lexical = lexical_overlap_score(&entry.content, &terms);
            let mixed_score = score * (1.0 - weight) + lexical * weight;
            (entry, mixed_score)
        })
        .collect();
    mixed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    mixed.into_iter().take(limit).map(|(e, _)| e).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, content: &str, ts: i64) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            content: content.to_string(),
            r#type: "conversation".to_string(),
            timestamp: ts,
            metadata: "{}".to_string(),
            embedding: None,
        }
    }

    #[test]
    fn tokenize_query_drops_stopwords_and_short_words() {
        assert_eq!(tokenize_query("o que é isso"), vec!["isso".to_string()]);
    }

    #[test]
    fn tokenize_query_falls_back_to_raw_text_when_everything_is_a_stopword() {
        assert_eq!(tokenize_query("o que foi"), vec!["o que foi".to_string()]);
    }

    #[test]
    fn tokenize_query_matches_accented_and_unaccented_the_same_way() {
        assert_eq!(tokenize_query("Sao Paulo"), tokenize_query("São Paulo"));
    }

    #[test]
    fn score_keyword_candidates_requires_whole_word_match_not_substring() {
        let candidates = vec![entry("1", "Ana foi ao mercado", 1), entry("2", "comi uma banana", 2)];
        let hits = score_keyword_candidates(&candidates, &["ana".to_string()], 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "1");
    }

    #[test]
    fn keyword_search_finds_cjk_terms_without_word_boundaries() {
        let candidates = vec![entry("ko", "철수가 게임을 했다", 1), entry("ja", "田中さんが来た", 2), entry("x", "nada a ver", 3)];
        for (query, id) in [("철수", "ko"), ("田中", "ja")] {
            let terms = tokenize_query(query);
            assert_eq!(terms, vec![query.to_string()], "termo CJK de 2 caracteres não pode ser descartado");
            let hits = score_keyword_candidates(&candidates, &terms, 10);
            assert_eq!(hits.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec![id]);
        }
    }

    #[test]
    fn score_keyword_candidates_orders_by_hits_then_recency() {
        let candidates = vec![
            entry("older-2hits", "rust e tokio juntos", 1),
            entry("newer-1hit", "rust sozinho", 5),
        ];
        let terms = vec!["rust".to_string(), "tokio".to_string()];
        let ranked = score_keyword_candidates(&candidates, &terms, 10);
        assert_eq!(ranked[0].id, "older-2hits");
    }

    #[test]
    fn calculate_candidate_score_zeroes_similarity_on_dimension_mismatch() {
        let scored = calculate_candidate_score(
            "conversation",
            "{}",
            &[1.0, 0.0],
            &[1.0, 0.0, 0.0],
            0.25,
            0,
            0,
            ScoreFlags { reranking_enabled: true, emotional_tagging: true },
            None,
        );
        assert_eq!(scored.sim, 0.0);
    }

    #[test]
    fn calculate_candidate_score_boost_never_exceeds_its_cap() {
        let meta = serde_json::json!({ "emotion": "happy", "importance": 10, "game": "Elden Ring" }).to_string();
        let scored = calculate_candidate_score(
            "summary",
            &meta,
            &[1.0, 0.0],
            &[1.0, 0.0],
            0.0,
            0,
            0,
            ScoreFlags { reranking_enabled: true, emotional_tagging: true },
            Some("Elden Ring"),
        );
        // sim=1, recency=1 (idade 0) -> score = 1*(1-0) + 1*0 + boost, boost capado em 0.15*(1-0).
        assert!((scored.score - (1.0 + BOOST_CAP_RATIO)).abs() < 1e-9);
    }

    #[test]
    fn rank_semantic_candidates_without_rerank_sorts_by_score_desc() {
        let candidates = vec![(entry("low", "a", 1), 0.2), (entry("high", "b", 1), 0.9)];
        let ranked = rank_semantic_candidates(candidates, 10, None);
        assert_eq!(ranked[0].id, "high");
    }

    #[test]
    fn rank_semantic_candidates_mixes_in_lexical_overlap_when_weighted() {
        let candidates = vec![(entry("semantic-only", "irrelevante", 1), 0.9), (entry("lexical-match", "rust", 1), 0.5)];
        let ranked = rank_semantic_candidates(
            candidates,
            10,
            Some(LexicalRerank { query: "rust", weight: 1.0 }),
        );
        // Peso 1.0 = 100% léxico: quem casa a palavra vence mesmo com score semântico menor.
        assert_eq!(ranked[0].id, "lexical-match");
    }
}
