use super::*;
use super::super::types::minimal_person;

fn snippet(title: &str, text: &str) -> String {
    format!("{title} {text}")
}

#[test]
fn build_search_query_differs_for_fictional_types() {
    let mut character = minimal_person("pc_1", "Geralt");
    character.kind = PersonType::FictionalCharacter;
    assert!(build_search_query(&character).contains("personagem"));

    let mut real = minimal_person("pc_2", "Ada Lovelace");
    real.kind = PersonType::RealPerson;
    assert!(build_search_query(&real).contains("biografia"));
}

#[test]
fn mentions_person_requires_the_name_to_appear() {
    let result = SearchResult { title: "Ada Lovelace - Wikipedia".into(), url: "https://x".into(), snippet: "matemática inglesa".into() };
    assert!(mentions_person(&result, "Ada Lovelace"));

    let unrelated = SearchResult { title: "Receita de bolo".into(), url: "https://y".into(), snippet: "farinha e ovos".into() };
    assert!(!mentions_person(&unrelated, "Ada Lovelace"));
}

// --- search_birth_date: inglês primeiro, português como fallback ----------------------------

#[test]
fn search_birth_date_matches_the_english_pattern() {
    let snippets = vec![snippet("Ada Lovelace", "Ada Lovelace was born December 10, 1815 in London.")];
    assert_eq!(search_birth_date(&snippets), Some("December 10, 1815".to_string()));
}

#[test]
fn search_birth_date_falls_back_to_the_portuguese_pattern() {
    let snippets = vec![snippet("Ada Lovelace", "Ada Lovelace nasceu em 10/12/1815 em Londres.")];
    assert_eq!(search_birth_date(&snippets), Some("em 10/12/1815 em Londres".to_string()));
}

#[test]
fn search_birth_date_returns_none_without_a_match() {
    let snippets = vec![snippet("Ada Lovelace", "matemática e escritora britânica")];
    assert_eq!(search_birth_date(&snippets), None);
}

#[test]
fn search_birth_date_tries_snippets_in_order() {
    let snippets = vec![snippet("irrelevante", "sem data aqui"), snippet("Ada Lovelace", "was born January 1, 2000 somewhere")];
    assert_eq!(search_birth_date(&snippets), Some("January 1, 2000".to_string()));
}

// --- search_birth_place: `born in X,` primeiro, `nascida? em X,` como fallback ---------------

#[test]
fn search_birth_place_matches_the_english_pattern() {
    let snippets = vec![snippet("Ada Lovelace", "She was born in London, England in 1815.")];
    assert_eq!(search_birth_place(&snippets), Some("London".to_string()));
}

#[test]
fn search_birth_place_falls_back_to_the_portuguese_pattern() {
    let snippets = vec![snippet("Ada Lovelace", "Ela nascida em Londres, no Reino Unido.")];
    assert_eq!(search_birth_place(&snippets), Some("Londres".to_string()));
}

#[test]
fn search_birth_place_returns_none_without_a_match() {
    let snippets = vec![snippet("Ada Lovelace", "matemática e escritora britânica")];
    assert_eq!(search_birth_place(&snippets), None);
}

// --- search_nationality: regex único, case insensitive, word boundary ------------------------

#[test]
fn search_nationality_matches_a_known_demonym_case_insensitively() {
    let snippets = vec![snippet("Ada Lovelace", "she was a brazilian mathematician")];
    assert_eq!(search_nationality(&snippets), Some("brazilian".to_string()));
}

#[test]
fn search_nationality_preserves_the_original_casing_of_the_match() {
    let snippets = vec![snippet("Ada Lovelace", "She was a British mathematician")];
    assert_eq!(search_nationality(&snippets), Some("British".to_string()));
}

#[test]
fn search_nationality_falls_back_to_the_portuguese_word_list() {
    let snippets = vec![snippet("Ada Lovelace", "cientista brasileira premiada")];
    // "brasileira" não está no regex do TS (só inglês) — pin do fallback aditivo pt-BR: o usuário é
    // pt-BR e esse casamento já funcionava nesta base antes do port da Wikipedia; não é regressão.
    assert_eq!(search_nationality(&snippets), Some("brasileira".to_string()));
}

#[test]
fn search_nationality_returns_none_when_neither_list_matches() {
    let snippets = vec![snippet("Ada Lovelace", "sem gentílico conhecido aqui")];
    assert_eq!(search_nationality(&snippets), None);
}

#[test]
fn search_nationality_prefers_the_english_pattern_when_both_lists_would_match() {
    // Inglês primeiro é paridade com o TS: quando o próprio trecho já casa o regex em inglês, o
    // fallback pt-BR nunca chega a rodar, mesmo que a lista pt-BR também casasse nesse mesmo texto.
    let snippets = vec![snippet("Ada Lovelace", "She is British, mas seria descrita como brasileira também.")];
    assert_eq!(search_nationality(&snippets), Some("British".to_string()));
}

#[test]
fn search_nationality_respects_word_boundaries() {
    // "Americanism" não é "American" isolado — \b não deveria casar dentro da palavra maior.
    let snippets = vec![snippet("x", "a philosophy called Americanism")];
    assert_eq!(search_nationality(&snippets), None);
}

// --- result_snippets: monta "{title} {snippet}" na ordem dos resultados relevantes -----------

#[test]
fn result_snippets_concatenates_title_and_snippet_in_order() {
    let a = SearchResult { title: "A".into(), url: "https://a".into(), snippet: "primeiro".into() };
    let b = SearchResult { title: "B".into(), url: "https://b".into(), snippet: "segundo".into() };
    let relevant = vec![&a, &b];
    assert_eq!(result_snippets(&relevant), vec!["A primeiro".to_string(), "B segundo".to_string()]);
}
