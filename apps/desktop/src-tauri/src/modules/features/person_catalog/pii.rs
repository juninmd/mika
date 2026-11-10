//! Redação de PII — porte de `packages/sdk/src/utils/pii-redactor.ts::redactPii` +
//! `core/security-constants.ts::REDACTION_PATTERNS`.
//!
//! **SecOps tem veto aqui.** Toda bio/nota/texto de menção que entra no catálogo via
//! `handlers/person-catalog-upsert-args.ts`/`handlers/index.ts` passa por `redactPii` ANTES de
//! tocar disco — é a única barreira entre "o usuário mencionou o telefone de alguém" e esse
//! telefone persistido em `catalog.json` para sempre. Os 5 padrões de PII social (email, CPF,
//! CNPJ, cartão, telefone) e os 4 de segredo (chave `sk-`, token do GitHub, JWT, `key=valor`) são
//! copiados literalmente, na MESMA ordem — a ordem importa porque um padrão mais genérico
//! (`KEY_VALUE_SECRET`) rodando antes de um mais específico pode mascarar parte do que o outro
//! reconheceria.
//!
//! Enfraquecer qualquer regex aqui (torná-la menos abrangente que a original) é a classe de defeito
//! que este módulo existe para nunca introduzir — daí os testes negativos abaixo, um por padrão.

use std::sync::OnceLock;

use regex::Regex;

/// Como um padrão substitui o que casa. `regex` (crate) não tem lookaround — os dois casos do TS
/// que dependiam de `(?<!...)` (cartão, telefone) viram um filtro pós-match em vez de parte do
/// padrão: [`Rule::ExcludingPrecedingDigitOrPlus`].
enum Rule {
    /// Substituição fixa (`[EMAIL]`, `[DOC]`...).
    Fixed(&'static str),
    /// Como `Fixed`, mas descarta o match cujo caractere anterior seja dígito ou `+` — o
    /// equivalente de `(?<![\d+])`/`(?<!\+)` do TS.
    ExcludingPrecedingDigitOrPlus(&'static str),
    /// `KEY_VALUE_SECRET`: preserva o grupo 1 (o nome do campo) e troca só o valor.
    KeepFieldNamePrefix,
}

struct Pattern {
    regex: Regex,
    rule: Rule,
}

/// PII social — email, CPF, CNPJ, cartão, telefone (com/sem +55). Mesma ordem de `PII_PATTERNS`.
fn pii_patterns() -> &'static [Pattern] {
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Pattern {
                regex: Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").unwrap(),
                rule: Rule::Fixed("[EMAIL]"),
            },
            Pattern {
                regex: Regex::new(r"\b\d{3}\.\d{3}\.\d{3}-\d{2}\b").unwrap(),
                rule: Rule::Fixed("[DOC]"),
            },
            Pattern {
                regex: Regex::new(r"\b\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}\b").unwrap(),
                rule: Rule::Fixed("[DOC]"),
            },
            Pattern {
                regex: Regex::new(r"\b(?:\d{4}[ -]){3}\d{4}\b|\b\d{13,16}\b").unwrap(),
                rule: Rule::ExcludingPrecedingDigitOrPlus("[CARD]"),
            },
            Pattern {
                regex: Regex::new(r"(?:\+55\s?)?(?:\(?\d{2}\)?[\s-]?)?\d{4,5}[\s-]?\d{4}\b").unwrap(),
                rule: Rule::ExcludingPrecedingDigitOrPlus("[PHONE]"),
            },
        ]
    })
}

/// Segredos — `sk-...`, token do GitHub, JWT, `algumaKey=valor`. Mesma ordem de `SECRET_ONLY_PATTERNS`.
fn secret_patterns() -> &'static [Pattern] {
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Pattern {
                regex: Regex::new(r"\bsk-(?:ant-)?(?:live-)?[a-zA-Z0-9]{32,}\b").unwrap(),
                rule: Rule::Fixed("[KEY]"),
            },
            Pattern {
                regex: Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b").unwrap(),
                rule: Rule::Fixed("[KEY]"),
            },
            Pattern {
                regex: Regex::new(r"\bey[A-Za-z0-9_-]{10,}\.ey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b").unwrap(),
                rule: Rule::Fixed("[JWT]"),
            },
            Pattern {
                regex: Regex::new(r"(?i)([a-z0-9_-]*(?:token|key|secret|password|auth)\s*[=:]\s*)[^\s&,;]+").unwrap(),
                rule: Rule::KeepFieldNamePrefix,
            },
        ]
    })
}

fn apply(text: &str, pattern: &Pattern) -> String {
    match &pattern.rule {
        Rule::Fixed(replacement) => pattern.regex.replace_all(text, *replacement).into_owned(),
        Rule::KeepFieldNamePrefix => pattern.regex.replace_all(text, "$1[KEY]").into_owned(),
        Rule::ExcludingPrecedingDigitOrPlus(replacement) => {
            let mut result = String::with_capacity(text.len());
            let mut last = 0;
            for m in pattern.regex.find_iter(text) {
                let excluded = text[..m.start()].chars().next_back().is_some_and(|c| c.is_ascii_digit() || c == '+');
                if excluded {
                    continue;
                }
                result.push_str(&text[last..m.start()]);
                result.push_str(replacement);
                last = m.end();
            }
            result.push_str(&text[last..]);
            result
        }
    }
}

/// Porte de `redactPii` — PII social + segredos, nesta ordem exata.
pub fn redact_pii(text: &str) -> String {
    let mut out = text.to_string();
    for pattern in pii_patterns() {
        out = apply(&out, pattern);
    }
    for pattern in secret_patterns() {
        out = apply(&out, pattern);
    }
    out
}

/// Porte de `redactSecrets` — só os 4 padrões de segredo, nunca PII social (usado fora deste
/// módulo pela memória episódica; replicado aqui porque `pii.rs` é a única fonte destes padrões
/// no lado Rust).
pub fn redact_secrets(text: &str) -> String {
    let mut out = text.to_string();
    for pattern in secret_patterns() {
        out = apply(&out, pattern);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_an_email() {
        assert_eq!(redact_pii("contact me at ada@example.com please"), "contact me at [EMAIL] please");
    }

    #[test]
    fn redacts_a_cpf() {
        assert_eq!(redact_pii("cpf 123.456.789-01"), "cpf [DOC]");
    }

    #[test]
    fn redacts_a_cnpj() {
        assert_eq!(redact_pii("cnpj 12.345.678/0001-99"), "cnpj [DOC]");
    }

    #[test]
    fn redacts_a_card_number() {
        assert_eq!(redact_pii("card 4111 1111 1111 1111 on file"), "card [CARD] on file");
    }

    /// NEGATIVO — um telefone internacional (`+55...`) não deve ser confundido com número de
    /// cartão, mesma exceção que `(?<!\+)` garante no TS.
    #[test]
    fn does_not_treat_an_international_phone_prefix_as_a_card() {
        let redacted = redact_pii("liga pra +5511987654321 hoje");
        assert!(!redacted.contains("[CARD]"), "tratou telefone como cartão: {redacted}");
    }

    #[test]
    fn redacts_a_brazilian_phone_number() {
        let redacted = redact_pii("me liga no 11 98765-4321 depois");
        assert!(redacted.contains("[PHONE]"), "não redigiu o telefone: {redacted}");
    }

    #[test]
    fn redacts_an_anthropic_style_api_key() {
        let key = format!("sk-ant-{}", "a".repeat(40));
        let redacted = redact_pii(&format!("use a chave {key} no cliente"));
        assert!(!redacted.contains(&key), "vazou a api key: {redacted}");
        assert!(redacted.contains("[KEY]"));
    }

    #[test]
    fn redacts_a_github_token() {
        let token = format!("ghp_{}", "b".repeat(36));
        let redacted = redact_pii(&format!("token: {token}"));
        assert!(!redacted.contains(&token));
    }

    #[test]
    fn redacts_a_jwt() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dQw4w9WgXcQ_dGVzdHNpZ25hdHVyZQ";
        let redacted = redact_pii(&format!("token={jwt}"));
        assert!(!redacted.contains(jwt), "vazou o jwt: {redacted}");
    }

    #[test]
    fn redacts_a_generic_key_value_secret_keeping_the_field_name() {
        let redacted = redact_pii("apiKey=abcdef1234567890");
        assert!(redacted.starts_with("apiKey="), "perdeu o nome do campo: {redacted}");
        assert!(redacted.contains("[KEY]"));
        assert!(!redacted.contains("abcdef1234567890"));
    }

    /// NEGATIVO — `redact_secrets` NUNCA toca PII social (episodic memory é o produto, mesmo
    /// comentário do TS original).
    #[test]
    fn redact_secrets_never_touches_social_pii() {
        let redacted = redact_secrets("email ada@example.com, cpf 123.456.789-01");
        assert!(redacted.contains("ada@example.com"));
        assert!(redacted.contains("123.456.789-01"));
    }

    #[test]
    fn plain_text_without_pii_is_unchanged() {
        assert_eq!(redact_pii("bom dia, tudo certo por aqui"), "bom dia, tudo certo por aqui");
    }
}
