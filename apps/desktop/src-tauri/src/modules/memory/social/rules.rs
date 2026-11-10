//! Porte de `apps/desktop/src/main/services/memory/social/rules.ts`. Reaproveita
//! [`crate::modules::memory::text::strip_diacritics`] — MESMA função que `episodic::search` usa —
//! em vez de duplicar a tabela de acentos (ver o cabeçalho daquele arquivo).
//!
//! **`normalize_text` não é `episodic::search::normalize_memory_text`.** O original TS de cada
//! lado é uma função DIFERENTE (`social/rules.ts::normalizeText` preserva `-`;
//! `episodic-search.ts`'s normalizador não foi conferido aqui, e não precisa: os dois lados JS já
//! divergem). Preservar o hífen importa para `personId`: "Jean-Paul" vira `jean-paul`, não
//! `jean paul`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::modules::memory::text::strip_diacritics;

use super::types::PersonType;

/// = `normalizeText`: sem acento, minúsculo, só letras/dígitos (de qualquer escrita) e `-`
/// sobrevivem, o resto vira espaço, espaços colapsados e aparados. O original só deixava
/// `[a-z0-9]`: texto em coreano, japonês, russo ou chinês virava string vazia, e todo fato (ou
/// pessoa) nesses idiomas ganhava o MESMO id e similaridade 1.0 — cada um sobrescrevia o anterior.
pub fn normalize_text(value: &str) -> String {
    let lower = strip_diacritics(value).to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut last_was_space = false;
    for ch in lower.chars() {
        if ch.is_alphanumeric() || ch == '-' {
            out.push(ch);
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

/// = `personId`: `normalizeText` com os espaços restantes virando `-`.
pub fn person_id(name: &str) -> String {
    normalize_text(name).split(' ').collect::<Vec<_>>().join("-")
}

/// Subject canônico gravado para fatos sobre o PRÓPRIO usuário (`fact_store::add_fact`'s default,
/// `post_turn::facts::default_subject`). Pt-BR, não o `"user"` em inglês que o TS usava: o resto da
/// extração já é pt-BR (`fact-extraction.md`) e um modelo local pequeno confunde menos quando não
/// precisa alternar de idioma dentro da MESMA resposta — a inconsistência de subject era exatamente
/// o que impedia "do que eu gosto?"/"qual jogo eu jogo?" de bater de forma confiável com o fato
/// gravado (defects.md "memory-relationship" P2, "no real graph beyond people").
pub const USER_SUBJECT: &str = "usuário";

/// Apelidos que a extração (ou o modelo, ao pedir uma CONSULTA via `memory_console`) pode usar para
/// se referir ao próprio usuário — comparados por [`normalize_text`] (sem acento, minúsculo), então
/// "Usuario"/"USER"/"usuário" caem todos no mesmo balde. Fora desta lista o valor passa intacto: é o
/// nome de outra pessoa/personagem, e forçá-lo para [`USER_SUBJECT`] esconderia um fato de terceiro
/// sob o subject errado.
pub fn resolve_self_subject(subject: &str) -> String {
    const SELF_ALIASES: &[&str] = &["usuario", "user", "eu", "o usuario", "mim"];
    if SELF_ALIASES.contains(&normalize_text(subject).as_str()) {
        USER_SUBJECT.to_string()
    } else {
        subject.trim().to_string()
    }
}

/// = `isValidPersonName`.
pub fn is_valid_person_name(name: &str) -> bool {
    let clean = normalize_text(name);
    let len = clean.chars().count();
    if !(2..=80).contains(&len) {
        return false;
    }
    !matches!(
        clean.as_str(),
        "eu" | "voce" | "mika" | "ele" | "ela" | "eles" | "elas" | "meu" | "minha" | "the" | "a" | "o"
    )
}

fn rel_to_type() -> &'static HashMap<&'static str, PersonType> {
    static MAP: OnceLock<HashMap<&'static str, PersonType>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("mae", PersonType::Family),
            ("pai", PersonType::Family),
            ("irmao", PersonType::Family),
            ("irma", PersonType::Family),
            ("esposa", PersonType::Family),
            ("esposo", PersonType::Family),
            ("namorada", PersonType::Family),
            ("namorado", PersonType::Family),
            ("filho", PersonType::Family),
            ("filha", PersonType::Family),
            ("amigo", PersonType::Friend),
            ("amiga", PersonType::Friend),
            ("chefe", PersonType::Other),
            ("colega", PersonType::Other),
        ])
    })
}

/// = `inferPersonType`.
pub fn infer_person_type(label: Option<&str>) -> PersonType {
    let Some(label) = label else { return PersonType::Other };
    rel_to_type().get(normalize_text(label).as_str()).copied().unwrap_or(PersonType::Other)
}

/// = `compactUnique`: dedup por `normalizeText`, mantendo a ÚLTIMA ocorrência (não a primeira) e a
/// ordem original — mesmo truque do TS (reverte, dedup mantendo o primeiro visto, reverte de novo).
pub fn compact_unique(values: Vec<String>, limit: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for value in values.into_iter().rev() {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let key = normalize_text(&trimmed);
        if seen.insert(key) {
            out.push(trimmed);
            if out.len() == limit {
                break;
            }
        }
    }
    out.reverse();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_text_strips_accents_lowercases_and_keeps_hyphens() {
        assert_eq!(normalize_text("São  Paulo!"), "sao paulo");
        assert_eq!(normalize_text("Jean-Paul"), "jean-paul");
    }

    #[test]
    fn person_id_turns_spaces_into_hyphens() {
        assert_eq!(person_id("Ada Lovelace"), "ada-lovelace");
    }

    #[test]
    fn resolve_self_subject_normalizes_every_self_referring_alias_to_user_subject() {
        for alias in ["usuario", "USUÁRIO", "user", "User", "eu", "Eu", "o usuario", "mim"] {
            assert_eq!(resolve_self_subject(alias), USER_SUBJECT, "alias {alias:?} deveria resolver para o subject canônico");
        }
    }

    /// NEGATIVO — o nome de outra pessoa nunca vira o subject do usuário: a comparação é por
    /// IGUALDADE do texto normalizado inteiro, nunca substring (senão "Eumar" colidiria com "eu").
    #[test]
    fn resolve_self_subject_leaves_a_third_party_name_untouched() {
        assert_eq!(resolve_self_subject("Dani"), "Dani");
        assert_eq!(resolve_self_subject("mika"), "mika");
        assert_eq!(resolve_self_subject("Eumar"), "Eumar", "prefixo 'eu' não pode colidir com o alias 'eu'");
    }

    #[test]
    fn is_valid_person_name_rejects_pronouns_and_out_of_range_length() {
        assert!(!is_valid_person_name("eu"));
        assert!(!is_valid_person_name("a"));
        assert!(!is_valid_person_name(&"x".repeat(81)));
        assert!(is_valid_person_name("Ada"));
    }

    #[test]
    fn infer_person_type_maps_known_labels_and_falls_back_to_other() {
        assert_eq!(infer_person_type(Some("namorada")), PersonType::Family);
        assert_eq!(infer_person_type(Some("amigo")), PersonType::Friend);
        assert_eq!(infer_person_type(Some("desconhecido")), PersonType::Other);
        assert_eq!(infer_person_type(None), PersonType::Other);
    }

    #[test]
    fn compact_unique_dedupes_by_normalized_key_keeping_the_last_occurrence_and_order() {
        let out = compact_unique(vec!["Ada".into(), "Bea".into(), "ada".into()], 20);
        assert_eq!(out, vec!["Bea".to_string(), "ada".to_string()], "'ada' repetido mantém a ÚLTIMA grafia, na posição original");
    }

    #[test]
    fn compact_unique_respects_the_cap() {
        let values = (0..5).map(|i| format!("p{i}")).collect();
        assert_eq!(compact_unique(values, 3).len(), 3);
    }
}
