//! Texto → ids do Supertonic 3: porte de `preprocess_text`/`UnicodeProcessor` do exemplo oficial
//! (`supertonic/rust/src/helper.rs`, commit `1e9799e`). O modelo lê CARACTERES (NFKD), não tokens:
//! cada code point vira um id pela tabela `unicode_indexer.json`, e a frase vai embrulhada na tag
//! do idioma (`<pt>…</pt>`), que é o contrato de entrada do próprio modelo.

use std::sync::OnceLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

const REPLACEMENTS: [(&str, &str); 17] = [
    ("–", "-"),
    ("‑", "-"),
    ("—", "-"),
    ("_", " "),
    ("\u{201C}", "\""),
    ("\u{201D}", "\""),
    ("\u{2018}", "'"),
    ("\u{2019}", "'"),
    ("´", "'"),
    ("`", "'"),
    ("[", " "),
    ("]", " "),
    ("|", " "),
    ("/", " "),
    ("#", " "),
    ("→", " "),
    ("←", " "),
];
const REMOVED: [&str; 5] = ["♥", "☆", "♡", "©", "\\"];

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("regex fixa"))
}

fn emoji() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(
        &CELL,
        r"[\x{1F600}-\x{1F64F}\x{1F300}-\x{1F5FF}\x{1F680}-\x{1F6FF}\x{1F700}-\x{1F77F}\x{1F780}-\x{1F7FF}\x{1F800}-\x{1F8FF}\x{1F900}-\x{1F9FF}\x{1FA00}-\x{1FA6F}\x{1FA70}-\x{1FAFF}\x{2600}-\x{26FF}\x{2700}-\x{27BF}\x{1F1E6}-\x{1F1FF}]+",
    )
}

/// Espaço antes de pontuação (`oi ,` → `oi,`).
fn space_before_punct() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(&CELL, r" ([,.!?;:'])")
}

fn spaces() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(&CELL, r"\s+")
}

fn ends_with_punct() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(
        &CELL,
        r#"[.!?;:,'"\u{201C}\u{201D}\u{2018}\u{2019})\]}…。」』】〉》›»]$"#,
    )
}

fn collapse(mut text: String, pair: &str, single: &str) -> String {
    while text.contains(pair) {
        text = text.replace(pair, single);
    }
    text
}

/// Normaliza e embrulha na tag do idioma. `None` quando não sobra texto falável (só emoji, só símbolo).
pub(super) fn preprocess(text: &str, language: &str) -> Option<String> {
    let mut text: String = emoji()
        .replace_all(&text.nfkd().collect::<String>(), "")
        .into_owned();
    for (from, to) in REPLACEMENTS {
        text = text.replace(from, to);
    }
    for symbol in REMOVED {
        text = text.replace(symbol, "");
    }
    text = space_before_punct().replace_all(&text, "$1").into_owned();
    text = collapse(collapse(text, "\"\"", "\""), "''", "'");
    let mut text = spaces().replace_all(&text, " ").trim().to_string();
    if text.is_empty() {
        return None;
    }
    if !ends_with_punct().is_match(&text) {
        text.push('.');
    }
    Some(format!("<{language}>{text}</{language}>"))
}

/// Cada caractere vira o id da tabela; fora da tabela vira -1, como no exemplo oficial.
pub(super) fn to_ids(indexer: &[i64], text: &str) -> Vec<i64> {
    text.chars()
        .map(|c| indexer.get(c as usize).copied().unwrap_or(-1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_in_the_language_tag_and_ends_with_punctuation() {
        assert_eq!(
            preprocess("Oi tudo bem", "pt").as_deref(),
            Some("<pt>Oi tudo bem.</pt>")
        );
        assert_eq!(
            preprocess("Vamos?", "pt").as_deref(),
            Some("<pt>Vamos?</pt>")
        );
    }

    #[test]
    fn cleans_spacing_dashes_quotes_and_emoji() {
        assert_eq!(
            preprocess("Boa ,  essa — foi 🎮 “top” !", "pt").as_deref(),
            Some("<pt>Boa, essa - foi \"top\"!</pt>")
        );
    }

    #[test]
    fn nothing_speakable_left_is_none() {
        assert_eq!(preprocess("  🎮🔥 ", "pt"), None);
    }

    /// NFKD separa o acento da letra: "é" vira dois caracteres, e o modelo foi treinado assim.
    #[test]
    fn accents_are_decomposed_before_indexing() {
        let text = preprocess("é", "pt").unwrap();
        assert!(text.contains("e\u{301}"), "{text:?}");
    }

    #[test]
    fn unknown_code_points_map_to_minus_one() {
        let indexer = vec![0i64, 10, 20];
        assert_eq!(to_ids(&indexer, "\u{1}\u{2}\u{7}"), vec![10, 20, -1]);
    }
}
