//! Redação de PII do título/dono da janela ativa antes de publicar/guardar — porte de
//! `packages/sdk/src/utils/pii-redactor.ts::redactPii`, aplicado pelo TS em
//! `infrastructure/os/foreground-window.ts::remember()` a TODO contexto de janela em foco antes de
//! ele sair dali (cache, evento, log).
//!
//! **Cópia intencional, não importação.** Os mesmos 9 padrões já existem em
//! `features::person_catalog::pii::redact_pii`, mas `system_signals` é núcleo e `pii.rs` mora numa
//! feature (AGENTS.md §1 — "núcleo nunca importa de feature"). Mesmo padrão de duplicação já aceito
//! em `modules::memory::social::fact_store::redact_secrets` — só o teste de paridade abaixo cruza a
//! fronteira núcleo/feature; o código de produção nunca faz isso.
//!
//! Título de janela raramente carrega CPF/CNPJ/cartão/JWT, mas pode carregar email ("Caixa de
//! Entrada — ada@empresa.com — Outlook") ou telefone — por isso o conjunto INTEIRO é copiado, não só
//! os 4 padrões de segredo que `fact_store.rs` precisava.

use std::sync::OnceLock;

use regex::Regex;

enum Rule {
    Fixed(&'static str),
    ExcludingPrecedingDigitOrPlus(&'static str),
    KeepFieldNamePrefix,
}

struct Pattern {
    regex: Regex,
    rule: Rule,
}

fn patterns() -> &'static [Pattern] {
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Pattern {
                regex: Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").unwrap(),
                rule: Rule::Fixed("[EMAIL]"),
            },
            Pattern { regex: Regex::new(r"\b\d{3}\.\d{3}\.\d{3}-\d{2}\b").unwrap(), rule: Rule::Fixed("[DOC]") },
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
            Pattern {
                regex: Regex::new(r"\bsk-(?:ant-)?(?:live-)?[a-zA-Z0-9]{32,}\b").unwrap(),
                rule: Rule::Fixed("[KEY]"),
            },
            Pattern { regex: Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b").unwrap(), rule: Rule::Fixed("[KEY]") },
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

/// Cópia núcleo de `redact_pii` — ver o cabeçalho do arquivo para o porquê de não reexportar.
pub(crate) fn redact_pii(text: &str) -> String {
    let mut out = text.to_string();
    for pattern in patterns() {
        out = apply(&out, pattern);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_an_email_in_a_window_title() {
        assert_eq!(redact_pii("Caixa de Entrada — ada@empresa.com — Outlook"), "Caixa de Entrada — [EMAIL] — Outlook");
    }

    #[test]
    fn plain_title_without_pii_is_unchanged() {
        assert_eq!(redact_pii("index.ts - Visual Studio Code"), "index.ts - Visual Studio Code");
    }

    /// Trava a paridade entre esta cópia (núcleo) e `person_catalog::pii::redact_pii` (feature) —
    /// só o teste cruza a fronteira núcleo/feature, mesmo padrão de
    /// `fact_store::redact_secrets_matches_the_person_catalog_copy_pattern_for_pattern`.
    #[test]
    fn redact_pii_matches_the_person_catalog_copy_pattern_for_pattern() {
        use crate::modules::features::person_catalog::pii::redact_pii as feature_redact_pii;

        let key = format!("sk-ant-{}", "a".repeat(40));
        let samples = [
            "contact me at ada@example.com please".to_string(),
            "cpf 123.456.789-01".to_string(),
            "cnpj 12.345.678/0001-99".to_string(),
            "card 4111 1111 1111 1111 on file".to_string(),
            "me liga no 11 98765-4321 depois".to_string(),
            format!("use a chave {key} no cliente"),
            "apiKey=abcdef1234567890".to_string(),
            "texto sem nenhum dado sensível".to_string(),
        ];
        for sample in samples {
            assert_eq!(redact_pii(&sample), feature_redact_pii(&sample), "divergiu para {sample:?}");
        }
    }
}
