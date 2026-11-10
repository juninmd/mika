//! Redação do que NÃO deve ser gravado em memória, nem em texto nem em vetor.
//!
//! Três famílias: segredos de software (chave de API, token, JWT, `chave=valor`), identificadores
//! financeiros/civis (CPF, número de cartão válido) e senhas ditas em frase ("a senha do wifi é
//! X"). E-mail e telefone NÃO entram aqui de propósito: lembrar de "o e-mail dela" é o que a
//! memória social existe para fazer.
//!
//! Antes, só a memória social redigia (e só a primeira família): o turno inteiro ia cru para o
//! episódico — e dali para o provedor de embedding, que pode ser nuvem — e um CPF ditado no chat
//! ficava gravado em texto e em vetor.
//!
//! A cópia da primeira família em `features::person_catalog::pii` é intencional (núcleo não
//! importa de feature); um teste em `social::fact_store` trava a paridade entre as duas.

use std::sync::OnceLock;

use regex::{Captures, Regex};

fn secret_patterns() -> &'static [(Regex, &'static str)] {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            (Regex::new(r"\bsk-(?:ant-)?(?:live-)?[a-zA-Z0-9]{32,}\b").unwrap(), "[KEY]"),
            (Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b").unwrap(), "[KEY]"),
            (Regex::new(r"\bey[A-Za-z0-9_-]{10,}\.ey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b").unwrap(), "[JWT]"),
        ]
    })
}

fn key_value_secret_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)([a-z0-9_-]*(?:token|key|secret|password|auth)\s*[=:]\s*)[^\s&,;]+").unwrap())
}

/// = `redactSecrets` (`packages/sdk/src/utils/redaction.ts`), os 4 padrões de segredo.
pub(crate) fn redact_secrets(text: &str) -> String {
    let mut out = text.to_string();
    for (pattern, replacement) in secret_patterns() {
        out = pattern.replace_all(&out, *replacement).into_owned();
    }
    key_value_secret_pattern().replace_all(&out, "$1[KEY]").into_owned()
}

fn cpf_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b\d{3}\.?\d{3}\.?\d{3}-?\d{2}\b").unwrap())
}

fn card_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b\d(?:[ -]?\d){12,18}\b").unwrap())
}

fn spoken_password_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)\b((?:senha|password|passcode|pin)\b[^\n.]{0,24}?(?:\bé\b|\beh\b|\bis\b|=|:)\s*)\S+").unwrap())
}

/// Dígitos verificadores do CPF: sem isto, qualquer número de 11 dígitos (telefone, id) seria
/// apagado.
fn valid_cpf(digits: &str) -> bool {
    let d: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 11 || d.iter().all(|x| *x == d[0]) {
        return false;
    }
    let check = |len: usize| {
        let sum: u32 = d[..len].iter().enumerate().map(|(i, x)| x * (len as u32 + 1 - i as u32)).sum();
        let rest = (sum * 10) % 11;
        if rest == 10 { 0 } else { rest }
    };
    check(9) == d[9] && check(10) == d[10]
}

/// Luhn: separa número de cartão de timestamp ou id longo.
fn luhn_valid(digits: &str) -> bool {
    let d: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&d.len()) {
        return false;
    }
    let sum: u32 = d
        .iter()
        .rev()
        .enumerate()
        .map(|(i, x)| if i % 2 == 1 { let y = x * 2; if y > 9 { y - 9 } else { y } } else { *x })
        .sum();
    sum % 10 == 0
}

/// CPF válido, cartão (Luhn) e senha dita em frase viram marcadores.
pub(crate) fn redact_sensitive_ids(text: &str) -> String {
    let out = card_pattern().replace_all(text, |c: &Captures| if luhn_valid(&c[0]) { "[CARD]".to_string() } else { c[0].to_string() });
    let out = cpf_pattern().replace_all(&out, |c: &Captures| if valid_cpf(&c[0]) { "[CPF]".to_string() } else { c[0].to_string() });
    spoken_password_pattern().replace_all(&out, "$1[KEY]").into_owned()
}

/// O que toda gravação de memória aplica ao texto antes de persistir ou de mandar ao embedding.
pub(crate) fn redact_for_storage(text: &str) -> String {
    redact_sensitive_ids(&redact_secrets(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_cpf_is_redacted_but_an_arbitrary_eleven_digit_number_is_not() {
        assert_eq!(redact_sensitive_ids("meu cpf é 529.982.247-25 ok"), "meu cpf é [CPF] ok");
        assert_eq!(redact_sensitive_ids("cpf 52998224725"), "cpf [CPF]");
        assert_eq!(redact_sensitive_ids("liga pro 11987654321"), "liga pro 11987654321", "telefone de 11 dígitos sem DV válido fica");
    }

    #[test]
    fn a_luhn_valid_card_is_redacted_but_a_timestamp_is_not() {
        assert_eq!(redact_sensitive_ids("cartão 4111 1111 1111 1111 vence"), "cartão [CARD] vence");
        assert_eq!(redact_sensitive_ids("em 1792512696448 ms"), "em 1792512696448 ms");
    }

    #[test]
    fn a_spoken_password_loses_its_value_but_keeps_the_sentence() {
        assert_eq!(redact_sensitive_ids("a senha do wifi é abc12345 e pronto"), "a senha do wifi é [KEY] e pronto");
        assert_eq!(redact_sensitive_ids("my password is hunter2"), "my password is [KEY]");
        assert_eq!(redact_sensitive_ids("não lembro a senha"), "não lembro a senha");
    }

    #[test]
    fn social_pii_the_memory_exists_for_is_left_alone() {
        let text = "o email dela é ada@example.com e o fone 11 98765-4321";
        assert_eq!(redact_for_storage(text), text);
    }

    #[test]
    fn storage_redaction_applies_both_families() {
        let key = format!("sk-ant-{}", "a".repeat(40));
        let out = redact_for_storage(&format!("chave {key}, cpf 529.982.247-25"));
        assert!(!out.contains(&key) && !out.contains("529.982"), "{out}");
    }
}
