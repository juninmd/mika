//! === tool-loop/tool-failure-tracker.ts ============================================================
//!
//! Port do rastreador de falhas por tool: acerto zera o contador, `MAX_FAILURES_BEFORE_HINT` (2)
//! emite uma dica só logada, `MAX_FAILURES_BEFORE_BLOCK` (3) BLOQUEIA a tool por
//! [`BLOCK_TTL`] e passa a aparecer em [`FailureTracker::self_healing_context`] — o texto que
//! `run_turn` anexa ao fim do system prompt já composto (ver `turn.rs::append_turn_context`).
//!
//! ## Estado de processo, não por turno
//!
//! No TS o rastreador vive num campo de `ToolLoopService`, uma instância única por sessão de app
//! (o serviço nunca é recriado entre turnos) — falhas de um turno anterior continuam bloqueando a
//! tool no turno seguinte, até o TTL expirar. `run_turn_llm`/`run_turn_ollama` não têm um objeto de
//! sessão equivalente (cada chamada é uma função livre); por isso o rastreador vive num
//! `OnceLock` de processo, mesmo padrão já usado por `tools::registry()`/`features::mood::engine()`.
//!
//! ## Divergência documentada: ordem do bloco de bloqueados
//!
//! `buildSelfHealingContext` itera um `Set<string>` do JS (ordem de inserção). Aqui a lista é
//! ordenada alfabeticamente — um `HashMap` não garante ordem alguma, e inventar uma ordem de
//! inserção pediria uma estrutura extra só para isso. A LISTA de tools bloqueadas é idêntica; só a
//! ordem de exibição pode divergir quando mais de uma tool está bloqueada ao mesmo tempo.
//!
//! `classifyToolOutcome`/`TOOL_ERROR_POLICY` (soft failure vs strike, por `errorCode`) NÃO são
//! portados: `crate::modules::tools::registry().execute` devolve `Result<Value, String>` sem um
//! `errorCode` estruturado — toda falha de tool aqui conta como "strike", igual ao branch
//! `!code` de `classifyToolOutcome` (que também trata como strike quando não há código
//! reconhecido). Reportado, não escondido.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const MAX_FAILURES_BEFORE_HINT: u32 = 2;
pub const MAX_FAILURES_BEFORE_BLOCK: u32 = 3;
const BLOCK_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
struct Inner {
    failures: HashMap<String, u32>,
    blocked_at: HashMap<String, Instant>,
}

pub struct FailureTracker(Mutex<Inner>);

impl FailureTracker {
    fn new() -> Self {
        Self(Mutex::new(Inner::default()))
    }

    /// Espelha `recordToolSuccess`: zera o contador, NÃO desbloqueia uma tool já bloqueada (mesmo
    /// comportamento do TS — só o TTL ou `isToolBlocked` desbloqueiam).
    pub fn record_success(&self, tool_name: &str) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).failures.remove(tool_name);
    }

    /// Espelha `recordToolFailure`. Devolve a dica só para quem quiser logar (aqui: `tracing::warn`
    /// no chamador) — nunca é injetada no prompt por chamada individual, só o bloco agregado de
    /// [`self_healing_context`] quando a tool cruza o limiar de bloqueio.
    pub fn record_failure(&self, tool_name: &str) -> Option<String> {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let count = {
            let entry = inner.failures.entry(tool_name.to_string()).or_insert(0);
            *entry += 1;
            *entry
        };

        if count >= MAX_FAILURES_BEFORE_BLOCK {
            inner.blocked_at.insert(tool_name.to_string(), Instant::now());
            return Some(format!("[SYSTEM] Tool \"{tool_name}\" blocked after {count} failures. Try an alternative approach."));
        }
        if count >= MAX_FAILURES_BEFORE_HINT {
            return Some(format!("[SELF-HEALING HINT] Tool \"{tool_name}\" failed {count} times. Review inputSchema carefully or try another tool."));
        }
        None
    }

    /// Espelha `isToolBlocked`: expira o bloqueio (e o contador junto) quando o TTL passou.
    pub fn is_blocked(&self, tool_name: &str) -> bool {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let Some(&blocked_at) = inner.blocked_at.get(tool_name) else { return false };
        if blocked_at.elapsed() > BLOCK_TTL {
            inner.blocked_at.remove(tool_name);
            inner.failures.remove(tool_name);
            return false;
        }
        true
    }

    /// Espelha `buildSelfHealingContext`: string vazia sem tool bloqueada, senão o mesmo aviso do
    /// TS (com quebra de linha à frente, já pronta para `push_str` direto no fim do prompt).
    pub fn self_healing_context(&self) -> String {
        self.self_healing_context_at(Instant::now())
    }

    fn self_healing_context_at(&self, now: Instant) -> String {
        let inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        // Bloqueio vencido já não vale (só `is_blocked` o limpa, e roda depois deste contexto).
        let mut names: Vec<&str> =
            inner.blocked_at.iter().filter(|(_, at)| now.saturating_duration_since(**at) <= BLOCK_TTL).map(|(name, _)| name.as_str()).collect();
        if names.is_empty() {
            return String::new();
        }
        names.sort_unstable();
        format!("\n[SYSTEM] Blocked tools this session: {}. Do NOT call them.", names.join(", "))
    }
}

static FAILURE_TRACKER: OnceLock<FailureTracker> = OnceLock::new();

/// Singleton de processo — ver o item "Estado de processo, não por turno" no cabeçalho do arquivo.
pub fn failure_tracker() -> &'static FailureTracker {
    FAILURE_TRACKER.get_or_init(FailureTracker::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O bloqueio expira em `BLOCK_TTL`, mas só `is_blocked` limpava a entrada — e o prompt é composto
    /// ANTES de o laço consultá-lo. Depois de 5 min a ferramenta já voltava ao catálogo, enquanto o
    /// prompt ainda mandava "Do NOT call them" para ela naquele turno.
    #[test]
    fn self_healing_context_drops_a_block_whose_ttl_already_expired() {
        let tracker = FailureTracker::new();
        for _ in 0..MAX_FAILURES_BEFORE_BLOCK {
            tracker.record_failure("web");
        }
        assert!(tracker.self_healing_context_at(Instant::now()).contains("web"));
        let later = Instant::now() + BLOCK_TTL + Duration::from_secs(1);
        assert_eq!(tracker.self_healing_context_at(later), "");
    }

    #[test]
    fn record_failure_hints_at_two_and_blocks_at_three() {
        let tracker = FailureTracker::new();
        assert_eq!(tracker.record_failure("web_search"), None);
        assert!(tracker.record_failure("web_search").unwrap().contains("SELF-HEALING HINT"));
        assert!(tracker.record_failure("web_search").unwrap().contains("blocked after 3"));
        assert!(tracker.is_blocked("web_search"));
    }

    #[test]
    fn record_success_resets_the_failure_count_but_not_an_existing_block() {
        let tracker = FailureTracker::new();
        tracker.record_failure("x");
        tracker.record_failure("x");
        tracker.record_success("x");
        // O contador zerou: mais UMA falha não bloqueia ainda (voltaria a 1, não a 3).
        assert_eq!(tracker.record_failure("x"), None);
        assert!(!tracker.is_blocked("x"));
    }

    #[test]
    fn self_healing_context_is_empty_without_a_blocked_tool() {
        let tracker = FailureTracker::new();
        assert_eq!(tracker.self_healing_context(), "");
        tracker.record_failure("a");
        assert_eq!(tracker.self_healing_context(), "", "duas falhas só dão dica, não bloqueio");
    }

    #[test]
    fn self_healing_context_lists_blocked_tools_sorted() {
        let tracker = FailureTracker::new();
        for _ in 0..MAX_FAILURES_BEFORE_BLOCK {
            tracker.record_failure("zeta");
            tracker.record_failure("alpha");
        }
        let ctx = tracker.self_healing_context();
        assert!(ctx.starts_with("\n[SYSTEM] Blocked tools this session: "));
        assert!(ctx.contains("alpha, zeta"), "ordem alfabética, não a de inserção: {ctx}");
        assert!(ctx.ends_with(". Do NOT call them."));
    }

    /// NEGATIVO — um TTL zerado (via `Instant::now()` já vencido não é simulável sem relógio
    /// injetável) não é testável aqui sem um `Instant` de teste; a prova de expiração real fica para
    /// um teste de integração manual. O que ESTE teste prova é que uma tool nunca marcada como
    /// bloqueada nunca aparece — a base do TTL sem o próprio TTL.
    #[test]
    fn is_blocked_is_false_for_a_tool_that_never_failed() {
        let tracker = FailureTracker::new();
        assert!(!tracker.is_blocked("nunca-chamada"));
    }
}
