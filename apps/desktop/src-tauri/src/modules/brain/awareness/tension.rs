//! Relógio de tensão de jogo — port de `shared/game-tension.ts`. No TS, visão e comentário de jogo
//! tinham `tenseUntil` separados com janelas diferentes para o MESMO evento e dessincronizavam sobre
//! quando falar de novo; aqui um único [`GameTensionState`] é a fonte de verdade, consumido pelo gate
//! de `ProactiveEngine::should_speak` (ver `engine.rs`) — nunca um `static` global, injetado como
//! `Arc<GameTensionState>` no `spawn_proactive_engine`, mesma regra de estado de AGENTS.md.
//!
//! Alimentado por `EventKey::GameMoment` (`crate::modules::gamer::rpc::MiniGameResult`, hoje o único
//! publicador Rust real — ver o cabeçalho de `events/keys.rs`).

use std::sync::atomic::{AtomicU64, Ordering};

/// Momentos que ABREM tensão: a partida ficou séria, não é hora de puxar assunto.
const TENSE_MOMENT_TYPES: &[&str] = &["boss", "cutscene"];
/// Momentos que FECHAM tensão: o desfecho aconteceu, pode falar.
const RESOLVING_MOMENT_TYPES: &[&str] = &["death", "victory", "defeat"];

const FALLBACK_TENSION_WINDOW_MS: u64 = 45_000;

fn default_window_ms(moment_type: &str) -> u64 {
    match moment_type {
        "boss" => 90_000,
        "cutscene" => 45_000,
        _ => FALLBACK_TENSION_WINDOW_MS,
    }
}

/// Estado do relógio de tensão — `AtomicU64` porque o único dado é "até quando" (epoch ms), lido do
/// gate de cadência e escrito pelo assinante de `EventKey::GameMoment`, potencialmente em tasks
/// concorrentes diferentes.
pub struct GameTensionState {
    tense_until_ms: AtomicU64,
}

impl GameTensionState {
    pub fn new() -> Self {
        Self { tense_until_ms: AtomicU64::new(0) }
    }

    pub fn is_tense(&self, now_ms: u64) -> bool {
        now_ms < self.tense_until_ms.load(Ordering::SeqCst)
    }

    /// Port de `applyGameMomentTension`. `override_window_ms` vem de
    /// `gamer-companion.momentCooldowns` (o usuário pode alargar/encurtar a janela por tipo);
    /// devolve se ESTAMOS tensos depois de aplicar o momento.
    pub fn apply_moment(&self, moment_type: &str, tense_flag: bool, override_window_ms: Option<u64>, now_ms: u64) -> bool {
        if RESOLVING_MOMENT_TYPES.contains(&moment_type) {
            self.tense_until_ms.store(0, Ordering::SeqCst);
            return false;
        }

        if TENSE_MOMENT_TYPES.contains(&moment_type) || tense_flag {
            let window_ms = override_window_ms.unwrap_or_else(|| default_window_ms(moment_type));
            let candidate = now_ms.saturating_add(window_ms);
            // `fetch_max` para um momento curto não encurtar uma tensão longa já em curso — mesmo
            // `Math.max` do TS.
            self.tense_until_ms.fetch_max(candidate, Ordering::SeqCst);
            return true;
        }

        self.is_tense(now_ms)
    }
}

impl Default for GameTensionState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_untense() {
        let state = GameTensionState::new();
        assert!(!state.is_tense(0));
    }

    #[test]
    fn a_boss_moment_opens_tension_for_its_default_window() {
        let state = GameTensionState::new();
        assert!(state.apply_moment("boss", false, None, 1_000));
        assert!(state.is_tense(1_000 + 89_000));
        assert!(!state.is_tense(1_000 + 90_001));
    }

    #[test]
    fn a_cutscene_moment_uses_the_shorter_default_window() {
        let state = GameTensionState::new();
        assert!(state.apply_moment("cutscene", false, None, 0));
        assert!(state.is_tense(44_000));
        assert!(!state.is_tense(45_001));
    }

    #[test]
    fn an_unknown_type_with_the_tense_flag_uses_the_fallback_window() {
        let state = GameTensionState::new();
        assert!(state.apply_moment("random", true, None, 0));
        assert!(state.is_tense(44_000));
        assert!(!state.is_tense(45_001));
    }

    #[test]
    fn a_resolving_moment_closes_tension_immediately() {
        let state = GameTensionState::new();
        state.apply_moment("boss", false, None, 0);
        assert!(!state.apply_moment("victory", false, None, 1_000));
        assert!(!state.is_tense(1_000));
    }

    #[test]
    fn a_short_moment_never_shortens_a_longer_tension_already_in_progress() {
        let state = GameTensionState::new();
        state.apply_moment("boss", false, None, 0); // tense_until = 90_000
        state.apply_moment("cutscene", false, None, 10_000); // would-be tense_until = 55_000, shorter
        assert!(state.is_tense(89_999));
    }

    #[test]
    fn a_non_special_type_reports_the_current_tension_without_changing_it() {
        let state = GameTensionState::new();
        state.apply_moment("boss", false, None, 0);
        assert!(state.apply_moment("dialogue", false, None, 1_000));
        assert!(state.is_tense(89_999));
    }

    #[test]
    fn an_override_window_replaces_the_default_for_its_type() {
        let state = GameTensionState::new();
        assert!(state.apply_moment("boss", false, Some(5_000), 0));
        assert!(!state.is_tense(5_001));
    }
}
