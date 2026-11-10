//! Regras de cadência — port de `proactive-gates.ts` + `proactivity-risk.ts`.
//!
//! Toda função recebe `now_ms` (ou o relógio já resolvido, como `hour`) por parâmetro; nenhuma lê o
//! relógio de parede, para os testes controlarem o tempo sem `sleep` — mesma regra que
//! `modules/mod.rs`/AGENTS.md pedem para lógica de cadência.

use std::collections::HashMap;

use super::constants::{
    PROACTIVE_HOUR_WINDOW_MS, PROACTIVE_MAX_PER_HOUR, PROACTIVE_MIN_GAP_MS, PROACTIVE_STRONG_PRIORITY,
};
use super::types::ProactiveCandidate;

pub struct CadenceState<'a> {
    pub last_spoke_at: u64,
    pub last_for_key: &'a HashMap<String, u64>,
    pub hourly_speak_timestamps: &'a [u64],
    pub base_interval_ms: u64,
}

/// `character.quietHoursStart`/`character.quietHoursEnd` (0-23, default 23/7). Intervalo "vira a
/// meia-noite" quando `start > end` — mesma regra de `isQuietHours` no TS.
pub fn is_quiet_hours(hour: u32, start: u32, end: u32) -> bool {
    if start <= end {
        hour >= start && hour < end
    } else {
        hour >= start || hour < end
    }
}

/// `character.proactiveness` (0-100, default 50): 0 estica o cooldown em 1.5x ("só responde"), 100
/// encurta pela metade ("puxa assunto sempre"), 50 é neutro — mesma fórmula de
/// `proactivenessMultiplier` no TS.
pub fn proactiveness_multiplier(proactiveness: f64) -> f64 {
    let clamped = proactiveness.clamp(0.0, 100.0);
    1.0 + (50.0 - clamped) / 100.0
}

/// Port de `classifyProactivePriority` + `getProactiveCooldownMs` (`proactivity-risk.ts`): cooldown
/// por FAIXA de prioridade, não por valor contínuo.
fn base_cooldown_ms(priority: u8) -> u64 {
    if priority >= 8 {
        90_000
    } else if priority >= 4 {
        180_000
    } else {
        300_000
    }
}

/// `cooldownFor` do TS — cooldown-base da faixa de prioridade, escalado pelo dial de proatividade.
pub fn cooldown_for(priority: u8, proactiveness: f64) -> u64 {
    (base_cooldown_ms(priority) as f64 * proactiveness_multiplier(proactiveness)).round() as u64
}

fn is_hourly_cap_reached(hourly_speak_timestamps: &[u64], now_ms: u64) -> bool {
    let recent = hourly_speak_timestamps
        .iter()
        .filter(|&&at| now_ms.saturating_sub(at) < PROACTIVE_HOUR_WINDOW_MS)
        .count();
    recent >= PROACTIVE_MAX_PER_HOUR
}

/// Port de `passesCadence`. `quiet_hour`/`proactiveness` chegam já resolvidos pelo chamador (que lê
/// o `ConfigStore`) — esta função permanece pura.
/// Talking to nobody spaces out: from the second unanswered line on, each one doubles the base
/// interval, up to 8x (2 min -> 16 min). Strong candidates (a fact worth saying) skip the base
/// interval anyway, so this only quiets small talk.
pub fn stretched_interval(base_interval_ms: u64, unanswered: u32) -> u64 {
    base_interval_ms << unanswered.saturating_sub(1).min(3)
}

/// Keys that are one subject for cooldown purposes: a window change and the screen observation
/// that follows it both talk about the app in front of the user, so they wait out one cooldown.
pub fn cooldown_key(key: &str) -> &str {
    match key {
        "window:change" | "screen-observation" => "screen",
        other => other,
    }
}

pub fn passes_cadence(state: &CadenceState<'_>, candidate: &ProactiveCandidate, now_ms: u64, quiet_hour: bool, proactiveness: f64) -> bool {
    if quiet_hour && candidate.priority < 9 {
        return false;
    }

    if now_ms.saturating_sub(state.last_spoke_at) < PROACTIVE_MIN_GAP_MS {
        return false;
    }
    if is_hourly_cap_reached(state.hourly_speak_timestamps, now_ms) {
        return false;
    }

    let last_for_signal = state.last_for_key.get(cooldown_key(&candidate.key)).copied().unwrap_or(0);
    if now_ms.saturating_sub(last_for_signal) < cooldown_for(candidate.priority, proactiveness) {
        return false;
    }

    if candidate.priority < PROACTIVE_STRONG_PRIORITY && now_ms.saturating_sub(state.last_spoke_at) < state.base_interval_ms {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(key: &str, priority: u8) -> ProactiveCandidate {
        ProactiveCandidate { key: key.into(), priority, label: "test".into(), prompt: "oi".into() }
    }

    #[test]
    fn quiet_hours_wraps_past_midnight() {
        assert!(is_quiet_hours(23, 23, 7));
        assert!(is_quiet_hours(2, 23, 7));
        assert!(!is_quiet_hours(12, 23, 7));
    }

    #[test]
    fn quiet_hours_same_day_range_does_not_wrap() {
        assert!(is_quiet_hours(10, 9, 17));
        assert!(!is_quiet_hours(20, 9, 17));
    }

    #[test]
    fn proactiveness_multiplier_is_neutral_at_fifty_and_clamped_outside_range() {
        assert_eq!(proactiveness_multiplier(50.0), 1.0);
        assert_eq!(proactiveness_multiplier(0.0), 1.5);
        assert_eq!(proactiveness_multiplier(100.0), 0.5);
        assert_eq!(proactiveness_multiplier(-10.0), proactiveness_multiplier(0.0));
        assert_eq!(proactiveness_multiplier(200.0), proactiveness_multiplier(100.0));
    }

    #[test]
    fn cooldown_tiers_match_the_typescript_thresholds() {
        assert_eq!(cooldown_for(9, 50.0), 90_000);
        assert_eq!(cooldown_for(8, 50.0), 90_000);
        assert_eq!(cooldown_for(4, 50.0), 180_000);
        assert_eq!(cooldown_for(0, 50.0), 300_000);
    }

    #[test]
    fn quiet_hours_blocks_low_priority_but_not_priority_nine() {
        let empty = HashMap::new();
        let state = CadenceState { last_spoke_at: 0, last_for_key: &empty, hourly_speak_timestamps: &[], base_interval_ms: 120_000 };
        assert!(!passes_cadence(&state, &candidate("k", 5), 10_000_000, true, 50.0));
        assert!(passes_cadence(&state, &candidate("k", 9), 10_000_000, true, 50.0));
    }

    #[test]
    fn min_gap_since_last_speech_blocks_every_priority() {
        let empty = HashMap::new();
        let state = CadenceState { last_spoke_at: 1_000_000, last_for_key: &empty, hourly_speak_timestamps: &[], base_interval_ms: 120_000 };
        assert!(!passes_cadence(&state, &candidate("k", 9), 1_000_000 + 30_000, false, 50.0));
        assert!(passes_cadence(&state, &candidate("k", 9), 1_000_000 + 61_000, false, 50.0));
    }

    #[test]
    fn hourly_cap_blocks_the_thirteenth_speech_in_the_window() {
        let empty = HashMap::new();
        let now = 10_000_000_u64;
        let timestamps: Vec<u64> = (0..12).map(|i| now - i * 1_000).collect();
        let state = CadenceState { last_spoke_at: 0, last_for_key: &empty, hourly_speak_timestamps: &timestamps, base_interval_ms: 0 };
        assert!(!passes_cadence(&state, &candidate("k", 9), now, false, 50.0));
    }

    #[test]
    fn per_key_cooldown_blocks_the_same_signal_repeating_too_soon() {
        let mut last_for_key = HashMap::new();
        last_for_key.insert("idle".to_string(), 1_000_000_u64);
        let state = CadenceState { last_spoke_at: 0, last_for_key: &last_for_key, hourly_speak_timestamps: &[], base_interval_ms: 0 };
        // prioridade 3 (baixa) -> cooldown de 300s
        assert!(!passes_cadence(&state, &candidate("idle", 3), 1_000_000 + 60_000, false, 50.0));
        assert!(passes_cadence(&state, &candidate("idle", 3), 1_000_000 + 301_000, false, 50.0));
    }

    #[test]
    fn below_strong_priority_also_waits_out_the_base_interval() {
        let empty = HashMap::new();
        let state = CadenceState { last_spoke_at: 1_000_000, last_for_key: &empty, hourly_speak_timestamps: &[], base_interval_ms: 120_000 };
        // prioridade 7 < PROACTIVE_STRONG_PRIORITY (8): mesmo passando o min-gap (60s) e o cooldown
        // por chave, o intervalo-base (120s) ainda bloqueia.
        assert!(!passes_cadence(&state, &candidate("game", 7), 1_000_000 + 65_000, false, 50.0));
        assert!(passes_cadence(&state, &candidate("game", 7), 1_000_000 + 121_000, false, 50.0));
    }

    #[test]
    fn strong_priority_bypasses_the_base_interval_gate() {
        let empty = HashMap::new();
        let state = CadenceState { last_spoke_at: 1_000_000, last_for_key: &empty, hourly_speak_timestamps: &[], base_interval_ms: 120_000 };
        assert!(passes_cadence(&state, &candidate("urgent", 9), 1_000_000 + 65_000, false, 50.0));
    }
}
