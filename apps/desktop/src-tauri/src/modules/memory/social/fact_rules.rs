//! Porte de `apps/desktop/src/main/services/memory/social/fact-rules.ts` — dedup/contradição de
//! fatos aprendidos, usado só por [`super::fact_store::add_fact`].

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;
use sha2::{Digest, Sha256};

use super::rules::normalize_text;

/// = `factContentHash`: SHA-256 do texto completo, os 16 primeiros hex chars.
pub fn fact_content_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    hex::encode(digest)[..16].to_string()
}

/// Palavras funcionais de 2 letras (pt/en) que não distinguem um fato de outro.
const SHORT_STOPWORDS: &[&str] = &["de", "da", "do", "em", "no", "na", "eu", "um", "os", "as", "se", "me", "to", "in", "of", "is", "at", "on", "an", "it", "ou", "ao", "ja"];

/// = `factTokens`, mas sem descartar toda palavra de até 2 letras: "joga CS" e "joga R6" davam
/// Jaccard 1.0 e o segundo fato apagava o primeiro. Só as palavras funcionais curtas ficam fora.
pub fn fact_tokens(text: &str) -> HashSet<String> {
    normalize_text(text)
        .split(' ')
        .filter(|w| {
            let len = w.chars().count();
            len > 2 || (len == 2 && !SHORT_STOPWORDS.contains(w))
        })
        .map(String::from)
        .collect()
}

/// = `factSimilarity`: Jaccard entre os tokens de `a` e um conjunto já tokenizado.
pub fn fact_similarity(a: &str, set_b: &HashSet<String>) -> f64 {
    let set_a = fact_tokens(a);
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    let inter = set_a.iter().filter(|w| set_b.contains(*w)).count() as f64;
    inter / (set_a.len() as f64 + set_b.len() as f64 - inter)
}

fn negation_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"\bparou de\b",
            r"\bdeixou de\b",
            r"\bnao\b",
            r"\bnunca\b",
            r"\bsaiu d[aeo]s?\b",
            r"\blargou\b",
            r"\bex\b",
            r"\bstopped\b",
            r"\bno longer\b",
            r"\bquit\b",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("padrão de negação estático inválido"))
        .collect()
    })
}

fn negation_words() -> &'static HashSet<&'static str> {
    static WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        HashSet::from(["parou", "deixou", "nao", "nunca", "mais", "stopped", "longer", "quit", "never"])
    })
}

fn core_stopwords() -> &'static HashSet<&'static str> {
    static WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        HashSet::from([
            "que", "com", "para", "por", "uma", "uns", "umas", "dos", "das", "the", "and", "for", "with", "was",
            "has", "have", "ele", "ela", "esse", "essa", "este", "esta", "seu", "sua", "mas", "tem", "ser",
        ])
    })
}

const CONTRADICTION_CORE_THRESHOLD: f64 = 0.75;

fn has_negation(text: &str) -> bool {
    let norm = normalize_text(text);
    negation_patterns().iter().any(|p| p.is_match(&norm))
}

fn core_tokens(text: &str) -> HashSet<String> {
    normalize_text(text)
        .replace('-', " ")
        .split(' ')
        .filter(|w| w.chars().count() > 2 && !negation_words().contains(w) && !core_stopwords().contains(w))
        .map(String::from)
        .collect()
}

/// Mesma palavra em flexões diferentes ("joga"/"jogou", "fuma"/"fumar"): o prefixo comum cobre
/// quase tudo, faltando no máximo uma letra da menor, e nunca menos de 3. Substitui o antigo
/// `stem` (cortava 2 letras de tokens longos e mantinha tokens de 4 intactos), que fazia "joga" e
/// "jogou" parecerem palavras diferentes.
fn same_word(a: &str, b: &str) -> bool {
    let common = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let shortest = a.chars().count().min(b.chars().count());
    common >= shortest.saturating_sub(1).max(3)
}

fn share_core(a: &str, b: &str) -> bool {
    let set_a = core_tokens(a);
    let set_b = core_tokens(b);
    if set_a.is_empty() || set_b.is_empty() {
        return false;
    }
    let (small, large) = if set_a.len() <= set_b.len() { (&set_a, &set_b) } else { (&set_b, &set_a) };
    let inter = small.iter().filter(|t| large.iter().any(|u| same_word(t, u))).count() as f64;
    inter / small.len() as f64 >= CONTRADICTION_CORE_THRESHOLD
}

/// Predicados que só admitem UM objeto por vez: um novo "mora em Recife" aposenta "mora em São
/// Paulo" mesmo sem nenhuma palavra de negação. "Gosta de" fica de fora de propósito (gostar de
/// duas coisas é normal).
const FUNCTIONAL_PREDICATES: &[&str] = &["mora em", "vive em", "trabalha em", "trabalha na", "trabalha no", "nasceu em", "lives in", "works at", "works in"];

/// Mesmo predicado exclusivo, objetos diferentes (comparados sem acento/caixa).
pub fn functional_conflict(old_predicate: Option<&str>, old_object: Option<&str>, new_predicate: Option<&str>, new_object: Option<&str>) -> bool {
    let (Some(op), Some(oo), Some(np), Some(no)) = (old_predicate, old_object, new_predicate, new_object) else { return false };
    let (op, np) = (normalize_text(op), normalize_text(np));
    op == np && FUNCTIONAL_PREDICATES.contains(&op.as_str()) && normalize_text(oo) != normalize_text(no)
}

/// = `contradicts`: mesmo núcleo, negação divergente.
pub fn contradicts(a: &str, b: &str) -> bool {
    has_negation(a) != has_negation(b) && share_core(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fact_content_hash_is_deterministic_and_16_hex_chars() {
        let a = fact_content_hash("gosta de rust");
        let b = fact_content_hash("gosta de rust");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn fact_content_hash_differs_for_different_text() {
        assert_ne!(fact_content_hash("a"), fact_content_hash("b"));
    }

    #[test]
    fn fact_similarity_of_identical_text_is_1() {
        let tokens = fact_tokens("gosta de programar em rust");
        assert_eq!(fact_similarity("gosta de programar em rust", &tokens), 1.0);
    }

    #[test]
    fn fact_similarity_of_unrelated_text_is_0() {
        let tokens = fact_tokens("gosta de rust");
        assert_eq!(fact_similarity("mora em sao paulo", &tokens), 0.0);
    }

    #[test]
    fn contradicts_detects_negated_restatement_of_the_same_core_fact() {
        assert!(contradicts("fuma cigarro", "parou de fumar cigarro"));
    }

    #[test]
    fn contradicts_is_false_for_unrelated_facts() {
        assert!(!contradicts("gosta de rust", "mora em sao paulo"));
    }

    #[test]
    fn contradicts_is_false_when_both_sides_agree() {
        assert!(!contradicts("fuma cigarro", "fuma cigarro todo dia"));
    }

    #[test]
    fn short_non_stopword_tokens_are_not_dropped_so_cs_and_r6_stay_different_facts() {
        let tokens = fact_tokens("joga CS");
        assert_eq!(fact_similarity("joga R6", &tokens), 1.0 / 3.0);
        assert!(!fact_tokens("gosta de rust").contains("de"), "palavra funcional curta continua fora");
    }

    #[test]
    fn non_latin_text_keeps_its_own_identity_instead_of_collapsing_to_empty() {
        assert_ne!(normalize_text("고양이를 좋아해요"), "");
        assert_ne!(fact_content_hash(&normalize_text("고양이를 좋아해요")), fact_content_hash(&normalize_text("개를 좋아해요")));
        assert!(fact_similarity("고양이를 좋아해요", &fact_tokens("개를 좋아해요")) < 0.75);
    }

    #[test]
    fn plain_nunca_negates_too() {
        assert!(contradicts("joga valorant", "nunca jogou valorant"));
        assert!(contradicts("trabalha na acme", "saiu da acme trabalha"));
    }

    #[test]
    fn functional_predicates_conflict_on_a_different_object_only() {
        assert!(functional_conflict(Some("mora em"), Some("Sao Paulo"), Some("mora em"), Some("Recife")));
        assert!(!functional_conflict(Some("mora em"), Some("Recife"), Some("mora em"), Some("recife")));
        assert!(!functional_conflict(Some("gosta de"), Some("rust"), Some("gosta de"), Some("go")), "gostar não é exclusivo");
        assert!(!functional_conflict(None, None, Some("mora em"), Some("Recife")));
    }
}
