//! Relation vocabulary — maps how the user names someone ("minha esposa", "my brother", "irmão")
//! to a canonical key plus its cardinality. Pure, pt-BR + English, accent-insensitive.
//!
//! **Cardinality is what decides whether Mika may guess.** A single relation (wife, mother, boss)
//! names one person, so "minha esposa" always resolves to the same entity. A multi relation
//! (brother, friend, cousin) may name several; with two brothers known, "meu irmão" is ambiguous
//! and becomes a clarification instead of a guess. Ex-partners are multi on purpose: "ex-esposa"
//! is a different person from "esposa", and there can be more than one.

use super::super::normalize::strip_diacritics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cardinality {
    Single,
    Multi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Relation {
    pub key: &'static str,
    pub cardinality: Cardinality,
}

use Cardinality::{Multi, Single};

/// `(canonical key, cardinality, normalized spellings)`. Multi-word spellings are matched before
/// single words, so "melhor amigo" never falls through to "amigo".
const VOCABULARY: &[(&str, Cardinality, &[&str])] = &[
    ("ex_wife", Multi, &["ex esposa", "ex mulher", "ex wife"]),
    ("ex_husband", Multi, &["ex marido", "ex esposo", "ex husband"]),
    ("ex_girlfriend", Multi, &["ex namorada", "ex girlfriend"]),
    ("ex_boyfriend", Multi, &["ex namorado", "ex boyfriend"]),
    ("best_friend", Single, &["melhor amigo", "melhor amiga", "best friend"]),
    ("coworker", Multi, &["colega de trabalho", "colega", "coworker", "colleague"]),
    ("mother_in_law", Single, &["sogra", "mother in law"]),
    ("father_in_law", Single, &["sogro", "father in law"]),
    ("wife", Single, &["esposa", "mulher", "wife", "patroa"]),
    ("husband", Single, &["marido", "esposo", "husband"]),
    ("girlfriend", Single, &["namorada", "girlfriend"]),
    ("boyfriend", Single, &["namorado", "boyfriend"]),
    ("fiancee", Single, &["noiva", "fiancee"]),
    ("fiance", Single, &["noivo", "fiance"]),
    ("partner", Single, &["parceira", "parceiro", "companheira", "companheiro", "partner"]),
    ("mother", Single, &["mae", "mamae", "mother", "mom", "mum"]),
    ("father", Single, &["pai", "papai", "father", "dad"]),
    ("stepmother", Single, &["madrasta", "stepmother"]),
    ("stepfather", Single, &["padrasto", "stepfather"]),
    ("boss", Single, &["chefe", "patrao", "boss"]),
    ("brother", Multi, &["irmao", "brother"]),
    ("sister", Multi, &["irma", "sister"]),
    ("son", Multi, &["filho", "son"]),
    ("daughter", Multi, &["filha", "daughter"]),
    ("cousin", Multi, &["primo", "prima", "cousin"]),
    ("uncle", Multi, &["tio", "uncle"]),
    ("aunt", Multi, &["tia", "aunt"]),
    ("nephew", Multi, &["sobrinho", "nephew"]),
    ("niece", Multi, &["sobrinha", "niece"]),
    ("grandparent", Multi, &["avo", "vovo", "grandfather", "grandmother", "grandpa", "grandma"]),
    ("brother_in_law", Multi, &["cunhado", "brother in law"]),
    ("sister_in_law", Multi, &["cunhada", "sister in law"]),
    ("friend", Multi, &["amigo", "amiga", "friend", "parca"]),
    ("neighbor", Multi, &["vizinho", "vizinha", "neighbor", "neighbour"]),
];

const POSSESSIVES: &[&str] = &["meu", "minha", "my"];

/// Lowercase, accent-free, punctuation (hyphen included) folded to single spaces.
pub(crate) fn fold(text: &str) -> String {
    let lowered = strip_diacritics(&text.to_lowercase());
    let spaced: String = lowered.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn relation_of(entry: &(&'static str, Cardinality, &[&str])) -> Relation {
    Relation { key: entry.0, cardinality: entry.1 }
}

/// A label as the extractor or the user wrote it ("esposa", "Irmão mais velho", "my wife",
/// "ex-esposa") → its relation. Leading possessives and trailing qualifiers are tolerated, but the
/// label has to START with a known spelling: "amigo do João" is a friend, "João" is nothing.
pub(crate) fn canonical_relation(label: &str) -> Option<Relation> {
    let joined = strip_determiners(label);
    VOCABULARY
        .iter()
        .find(|(_, _, spellings)| spellings.iter().any(|s| joined == *s || joined.starts_with(&format!("{s} "))))
        .map(relation_of)
}

/// Only when the WHOLE label is a relation word ("Esposa", "meu irmão") — "Tio Chico" is a name.
pub(crate) fn exact_relation(label: &str) -> Option<Relation> {
    let joined = strip_determiners(label);
    VOCABULARY.iter().find(|(_, _, spellings)| spellings.contains(&joined.as_str())).map(relation_of)
}

fn strip_determiners(label: &str) -> String {
    let folded = fold(label);
    folded.split(' ').filter(|w| !POSSESSIVES.contains(w) && !["o", "a", "the"].contains(w)).collect::<Vec<_>>().join(" ")
}

/// A relation reference found in free text: "minha esposa gosta de sorvete" → wife, said as
/// "esposa". Needs a possessive right before it ("meu/minha/my"), so "a esposa do João" (someone
/// else's wife) never resolves to the user's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationMention {
    pub relation: Relation,
    pub label: String,
}

pub(crate) fn relations_in_text(text: &str) -> Vec<RelationMention> {
    let folded = fold(text);
    let words: Vec<&str> = folded.split(' ').collect();
    let mut found: Vec<RelationMention> = Vec::new();
    for (i, word) in words.iter().enumerate() {
        if !POSSESSIVES.contains(word) {
            continue;
        }
        if let Some(mention) = match_after(&words[i + 1..]) {
            if !found.iter().any(|m| m.relation == mention.relation) {
                found.push(mention);
            }
        }
    }
    found
}

/// Longest spelling first (3 words, then 2, then 1) right after the possessive.
fn match_after(rest: &[&str]) -> Option<RelationMention> {
    for len in (1..=3).rev() {
        if rest.len() < len {
            continue;
        }
        let phrase = rest[..len].join(" ");
        if let Some(entry) = VOCABULARY.iter().find(|(_, _, spellings)| spellings.contains(&phrase.as_str())) {
            return Some(RelationMention { relation: relation_of(entry), label: phrase });
        }
    }
    None
}

/// True when `alias` is only a relation word of a MULTI relation ("Irmão") — such an alias would
/// point every "meu irmão" at one brother, so it is dropped once the person gets a real name.
pub(crate) fn is_multi_relation_word(alias: &str) -> bool {
    exact_relation(alias).is_some_and(|r| r.cardinality == Multi)
}

/// Placeholder display name for an unnamed person: the label exactly as the user said it, first
/// letter capitalized ("esposa" → "Esposa").
pub(crate) fn placeholder_name(label: &str) -> String {
    let trimmed = label.trim();
    let mut chars = trimmed.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_resolves_accent_and_language_insensitively() {
        assert_eq!(canonical_relation("esposa").unwrap().key, "wife");
        assert_eq!(canonical_relation("My Wife").unwrap().key, "wife");
        assert_eq!(canonical_relation("Irmão mais velho").unwrap().key, "brother");
        assert_eq!(canonical_relation("irmao").unwrap().cardinality, Cardinality::Multi);
        assert_eq!(canonical_relation("minha mãe").unwrap().key, "mother");
        assert_eq!(canonical_relation("João"), None);
    }

    /// "ex-esposa" is someone else, never the current wife.
    #[test]
    fn an_ex_partner_is_a_distinct_multi_relation() {
        let ex = canonical_relation("ex-esposa").unwrap();
        assert_eq!(ex.key, "ex_wife");
        assert_eq!(ex.cardinality, Cardinality::Multi);
        assert_eq!(relations_in_text("minha ex-esposa ligou")[0].relation.key, "ex_wife");
    }

    #[test]
    fn text_mentions_need_a_possessive() {
        let found = relations_in_text("Minha esposa gosta de sorvete e meu irmão também");
        let keys: Vec<_> = found.iter().map(|m| m.relation.key).collect();
        assert_eq!(keys, vec!["wife", "brother"]);
        assert_eq!(found[1].label, "irmao");
        assert!(relations_in_text("a esposa do João gosta de sorvete").is_empty(), "someone else's wife");
        assert_eq!(relations_in_text("my wife likes ice cream")[0].relation.key, "wife");
    }

    #[test]
    fn the_longest_spelling_wins() {
        assert_eq!(relations_in_text("meu melhor amigo")[0].relation.key, "best_friend");
        assert_eq!(relations_in_text("meu colega de trabalho")[0].relation.key, "coworker");
    }

    #[test]
    fn only_multi_relation_words_are_dropped_as_aliases() {
        assert!(is_multi_relation_word("Irmão"));
        assert!(!is_multi_relation_word("Esposa"));
        assert!(!is_multi_relation_word("Dani"));
        assert!(!is_multi_relation_word("Tio Chico"), "a name that starts with a relation word is still a name");
    }
}
