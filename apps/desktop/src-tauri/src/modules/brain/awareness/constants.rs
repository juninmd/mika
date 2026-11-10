//! Constantes de cadência — port de `awareness.constants.ts` (só a fatia que o motor proativo usa
//! nesta rodada).

pub const PROACTIVE_BASE_INTERVAL_MS: u64 = 2 * 60 * 1000;
pub const PROACTIVE_MIN_GAP_MS: u64 = 60_000;

/// Sem teto por hora, sinais distintos batendo o cooldown individual em sequência produziam uma
/// fala a cada `PROACTIVE_MIN_GAP_MS` — companion virava incômodo, não companhia (mesma nota do TS).
pub const PROACTIVE_MAX_PER_HOUR: usize = 12;
pub const PROACTIVE_HOUR_WINDOW_MS: u64 = 60 * 60 * 1000;

pub const PROACTIVE_STRONG_PRIORITY: u8 = 8;

pub const PROACTIVE_TIMEOUT_SECS: u64 = 180;

pub const IDLE_THRESHOLD_MS: u64 = 30 * 60 * 1000;

pub const GAME_ABSENCE_THRESHOLD_MS: u64 = 3 * 24 * 60 * 60 * 1000;
pub const GAME_ABSENCE_RENUDGE_MS: u64 = 24 * 60 * 60 * 1000;
