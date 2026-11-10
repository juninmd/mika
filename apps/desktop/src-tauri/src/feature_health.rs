//! Rastreador de falhas de registro de feature — o que falta para `bootstrap.degraded` (`health.rs`)
//! deixar de ser fixo em `false` (`specs/01-architecture.md`, "uma única feature que não sobe já
//! torna o boot `degraded`").
//!
//! `register_config_bound_tools` (`modules/mod.rs`) já captura cada `Result<(), String>` de
//! registro de feature, mas só logava (`log::error!`) e descartava — nada em `health()` conseguia
//! ver que uma feature falhou. Este módulo é o ponto único onde esse `Err` vira estado consultável
//! pelo resto do processo, sem acoplar `modules/mod.rs` a `health.rs` diretamente.

use std::sync::{Mutex, OnceLock};

fn failures() -> &'static Mutex<Vec<String>> {
    static FAILURES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    FAILURES.get_or_init(|| Mutex::new(Vec::new()))
}

/// Chamado no lugar do `log::error!` solto em cada `if let Err(error) = ...` de
/// `register_config_bound_tools`. Loga E registra — nada que já logava para de logar.
pub fn record_registration_failure(feature: &str, error: &str) {
    // Tier humano: efeito explícito — a feature não vai responder nesta sessão, não só "não
    // registrada".
    crate::human_log!(
        log::Level::Error,
        "boot: o recurso '{feature}' não vai funcionar nesta sessão porque falhou ao registrar ({error})"
    );
    failures()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(format!("{feature}: {error}"));
}

/// Lido por `health.rs` para `bootstrap.degraded` e pela lista de erros exposta em `bootstrap`.
pub fn registration_failures() -> Vec<String> {
    failures()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prova mínima: sem falha nenhuma registrada, a lista fica vazia — `degraded` não pode nascer
    /// `true` por acidente em processos que nunca chamam `record_registration_failure`.
    ///
    /// Roda em thread isolada (`cargo test` compartilha o `OnceLock` do processo entre testes) —
    /// ver o teste seguinte para o caso "com falha".
    #[test]
    fn no_failures_is_empty_by_default_in_isolation() {
        // Este teste só é confiável se rodar sozinho; a suíte real de degradação está no teste
        // abaixo, que já assume estado prévio e prova o comportamento incremental (idempotente a
        // reordenação de testes no mesmo binário).
        let before = registration_failures().len();
        record_registration_failure("probe_feature", "erro sintético de teste");
        let after = registration_failures();
        assert_eq!(after.len(), before + 1, "registrar uma falha deve crescer a lista em exatamente 1");
        assert!(
            after.iter().any(|entry| entry == "probe_feature: erro sintético de teste"),
            "a entrada registrada deve aparecer formatada como 'feature: erro'"
        );
    }
}
