//! Normalização de nomes — porte de
//! `apps/desktop/src/features/person-catalog/services/person-normalize.ts` e
//! `services/person-name-quality.ts::isLikelyPersonName`.
//!
//! `strip_diacritics` é uma cópia privada da MESMA tabela fixa que `modules/memory/text.rs` e
//! `modules/brain/guards.rs` já usam — `mod text` de `modules/memory` é privado (`mod`, não `pub
//! mod`) e este módulo não pode alcançá-lo sem editar `modules/memory/mod.rs`, fora do meu
//! diretório. Cobre acentos latinos comuns (pt-BR e vizinhos); a divergência aceita com o original
//! (`text.normalize('NFD').replace(/\p{Diacritic}/gu, '')`, que cobre QUALQUER script) já está
//! documentada nas outras duas cópias.
pub(super) fn strip_diacritics(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            'ý' | 'ÿ' => 'y',
            'Ý' => 'Y',
            other => other,
        })
        .collect()
}

/// Porte de `catalogKey` — chave de identidade para `aliasMap`/`blocked` (mantém pontuação, só o
/// acento cai).
pub(super) fn catalog_key(alias: &str) -> String {
    strip_diacritics(alias.to_lowercase().trim())
}

/// Porte de `normalize` — usado por fatos/relações órfãs. `[^\w\s]` no JS sem flag `/u` é ASCII-only
/// (`\w` = `[A-Za-z0-9_]`): qualquer caractere fora disso (pontuação OU letra não-latina) é removido,
/// não só pontuação. Replicado literal, mesma pegadinha do original.
pub(super) fn normalize(text: &str) -> String {
    strip_diacritics(text.to_lowercase().trim())
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || c.is_whitespace())
        .collect()
}

/// Porte de `normalizePersonName` — colapsa espaços múltiplos em um só, sem remover pontuação.
fn normalize_person_name(name: &str) -> String {
    let stripped = strip_diacritics(name.to_lowercase().trim());
    let collapsed: Vec<&str> = stripped.split_whitespace().collect();
    collapsed.join(" ")
}

const MIN_NAME_LENGTH: usize = 2;
const MAX_NAME_LENGTH: usize = 48;
const MAX_NAME_WORDS: usize = 5;

const NON_ANSWER_PREFIXES: [&str; 11] = [
    "nenhum",
    "nao ha",
    "nao foi",
    "nao identific",
    "nao visivel",
    "desconhecid",
    "sem pessoa",
    "n/a",
    "na o",
    "null",
    "undefined",
];

fn category_word_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\b(anime|character|personage\w*|vtuber|avatar|npc)\b").unwrap())
}

fn generic_descriptor_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^(um|uma|o|a)\s|^personage|^garot[oa]\b|^menin[oa]\b|^mulher\b|^homem\b|^pessoa\b|^usuari[oa]\b|^apresentador|^candidat[oa]\b|^anime\s|^(young|old)\s+(wo)?man\b|^(wo)?man\b|^(girl|boy)\b|^character\b|^(the\s+)?(user|streamer|player|host|presenter)\b|^unknown\b",
        )
        .unwrap()
    })
}

fn has_letter(text: &str) -> bool {
    text.chars().any(char::is_alphabetic)
}

/// Porte de `isLikelyPersonName` — mesmo gate que a tool `person_catalog` usa antes de criar um
/// registro novo: recusa descrições genéricas ("um garoto", "unknown player") em vez de catalogar
/// lixo de auto-extração.
pub(super) fn is_likely_person_name(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.chars().count() < MIN_NAME_LENGTH || trimmed.chars().count() > MAX_NAME_LENGTH {
        return false;
    }
    let description_shape = trimmed.contains('(') || trimmed.contains(')') || trimmed.contains('/');
    if category_word_re().is_match(trimmed) && description_shape {
        return false;
    }
    if trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?') {
        return false;
    }
    if trimmed.contains(',') || trimmed.contains(';') || trimmed.contains(':') {
        return false;
    }

    let normalized = normalize_person_name(trimmed);
    if NON_ANSWER_PREFIXES.iter().any(|prefix| normalized.starts_with(prefix)) {
        return false;
    }
    if generic_descriptor_re().is_match(&normalized) {
        return false;
    }
    if normalized.split(' ').count() > MAX_NAME_WORDS {
        return false;
    }
    if !has_letter(trimmed) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_pt_br_accents() {
        assert_eq!(strip_diacritics("São Paulo é ótimo"), "Sao Paulo e otimo");
    }

    #[test]
    fn catalog_key_lowercases_trims_and_strips_accents_but_keeps_punctuation() {
        assert_eq!(catalog_key("  É.LICE  "), "e.lice");
    }

    #[test]
    fn normalize_drops_punctuation_and_non_ascii_letters() {
        assert_eq!(normalize("Olá, mundo! 世界"), "ola mundo ");
    }

    #[test]
    fn accepts_a_plausible_human_name() {
        assert!(is_likely_person_name("Ada Lovelace"));
    }

    #[test]
    fn rejects_a_generic_descriptor() {
        assert!(!is_likely_person_name("um garoto"));
        assert!(!is_likely_person_name("unknown player"));
    }

    #[test]
    fn rejects_a_non_answer() {
        assert!(!is_likely_person_name("desconhecido"));
    }

    #[test]
    fn rejects_a_name_with_too_many_words() {
        assert!(!is_likely_person_name("Maria Da Silva Santos Pereira Junior"));
    }

    #[test]
    fn rejects_text_ending_in_punctuation_or_containing_a_comma() {
        assert!(!is_likely_person_name("Quem é essa pessoa?"));
        assert!(!is_likely_person_name("Ada, Lovelace"));
    }

    /// NEGATIVO — nome curto demais não é confundido com um nome válido.
    #[test]
    fn rejects_a_single_character() {
        assert!(!is_likely_person_name("A"));
    }
}
