//! Extração de campos a partir dos trechos de busca DuckDuckGo — porte de
//! `buildSearchQuery`/`mentionsPerson`/`applySearchPatch`
//! (`apps/desktop/src/features/person-catalog/services/person-enrich.ts`). Único chamador:
//! `enrichment.rs`, depois que a Wikipedia já rodou e deixou os campos vazios.
//!
//! **`extractField`** (`person-enrich.ts`) testa os `snippets` (título+trecho dos até 4 resultados
//! relevantes, nessa ordem) contra um regex e devolve o grupo 1 do PRIMEIRO que casar — [`extract_field`]
//! é o mesmo algoritmo. `birthDate`/`birthPlace` tentam o padrão em inglês primeiro, depois o
//! equivalente em português, exatamente como `patchBirthDateFromSearch`/`patchBirthPlaceFromSearch`
//! (`?? extractField(snippets, /nasceu.../)`).
//!
//! **Nacionalidade: TS primeiro, lista pt-BR como fallback ADITIVO.** `NATIONALITY_PATTERN` porta o
//! regex do TS (gentílicos em inglês, case insensitive) e roda primeiro — paridade com a fonte
//! original. `PT_NATIONALITY_WORDS` é a lista curta em português que já existia nesta base antes do
//! port da Wikipedia; o usuário é pt-BR e os trechos de busca vêm majoritariamente em português, então
//! removê-la seria regressão de comportamento que já funcionava, não fidelidade ao TS. Só roda quando
//! o padrão em inglês não casa — nunca troca um resultado que o TS já daria.

use std::sync::OnceLock;

use regex::Regex;

use crate::modules::browser::SearchResult;

use super::types::{PersonRecord, PersonType};

/// = `buildSearchQuery`. Consultas por tipo — ficção/game/anime/filme/série pedem "personagem" em
/// vez de biografia, mesma distinção do TS.
pub(super) fn build_search_query(person: &PersonRecord) -> String {
    let name = &person.primary_name;
    match person.kind {
        PersonType::FictionalCharacter | PersonType::Anime => format!("{name} personagem ficção"),
        PersonType::Game => format!("{name} jogo videogame"),
        PersonType::Movie => format!("{name} filme"),
        PersonType::Series => format!("{name} série"),
        PersonType::Book => format!("{name} livro"),
        _ => format!("{name} biografia"),
    }
}

/// = `mentionsPerson` — heurística de sobreposição de token: o nome (ou pelo menos uma palavra dele
/// com 3+ letras) precisa aparecer no título ou no trecho, senão o resultado é ruído de busca.
pub(super) fn mentions_person(result: &SearchResult, primary_name: &str) -> bool {
    let haystack = format!("{} {}", result.title, result.snippet).to_lowercase();
    primary_name
        .split_whitespace()
        .filter(|w| w.chars().count() >= 3)
        .any(|w| haystack.contains(&w.to_lowercase()))
}

/// = `snippets` em `applySearchPatch`: `${title} ${snippet}` de cada resultado relevante, na ordem.
pub(super) fn result_snippets(relevant: &[&SearchResult]) -> Vec<String> {
    relevant.iter().map(|r| format!("{} {}", r.title, r.snippet)).collect()
}

/// = `extractField`: primeiro grupo-1 que casar, testando os snippets em ordem.
fn extract_field(snippets: &[String], pattern: &Regex) -> Option<String> {
    snippets.iter().find_map(|s| pattern.captures(s).and_then(|c| c.get(1)).map(|m| m.as_str().trim().to_string()))
}

/// = os dois regex de `patchBirthDateFromSearch` (inglês, depois `nasceu`).
fn birth_date_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"born[:\s]+([A-Z][a-z]+ \d{1,2},? \d{4})").expect("regex válida"),
            Regex::new(r"nasceu[:\s]+([^.,]{5,30})").expect("regex válida"),
        ]
    })
}

/// = os dois regex de `patchBirthPlaceFromSearch` (`born in ...,`, depois `nascida? em ...,`).
fn birth_place_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"born in ([A-Z][^,.]{3,40}),").expect("regex válida"),
            Regex::new(r"nascida? em ([^,.]{3,40}),").expect("regex válida"),
        ]
    })
}

/// = `NATIONALITY_PATTERN` — mesma lista de gentílicos, case insensitive, word-boundary.
fn nationality_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b(Cuban|Brazilian|American|British|French|Japanese|Korean|Spanish|Italian|Australian)\b")
            .expect("regex válida")
    })
}

/// = `patchBirthDateFromSearch`. Pura.
pub(super) fn search_birth_date(snippets: &[String]) -> Option<String> {
    birth_date_patterns().iter().find_map(|pattern| extract_field(snippets, pattern))
}

/// = `patchBirthPlaceFromSearch`. Pura.
pub(super) fn search_birth_place(snippets: &[String]) -> Option<String> {
    birth_place_patterns().iter().find_map(|pattern| extract_field(snippets, pattern))
}

/// Lista curta e literal de gentílicos em português — fallback aditivo, ver o cabeçalho do arquivo.
/// Não é o `NATIONALITY_PATTERN` do TS; só entra em jogo quando aquele não casa.
const PT_NATIONALITY_WORDS: &[&str] = &[
    "brasileiro", "brasileira", "americano", "americana", "japonês", "japonesa", "britânico", "britânica", "canadense", "francês", "francesa", "alemão", "alemã", "coreano", "coreana", "chinês", "chinesa", "italiano", "italiana",
];

fn search_nationality_pt(snippets: &[String]) -> Option<String> {
    snippets.iter().find_map(|s| {
        let lower = s.to_lowercase();
        PT_NATIONALITY_WORDS.iter().find(|word| lower.contains(**word)).map(|word| word.to_string())
    })
}

/// = `patchNationalityFromSearch` + fallback pt-BR. Pura — o ramo em inglês devolve o texto
/// exatamente como casou (preserva o case do trecho original, igual ao TS: `extractField` nunca
/// normaliza o que captura); o fallback em português devolve a palavra da lista (minúscula).
pub(super) fn search_nationality(snippets: &[String]) -> Option<String> {
    extract_field(snippets, nationality_pattern()).or_else(|| search_nationality_pt(snippets))
}

#[cfg(test)]
mod tests;
