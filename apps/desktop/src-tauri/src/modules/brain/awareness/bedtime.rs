//! "Vai dormir": the only late-night line before this was `IdleProvider`'s, which needs 30 min
//! WITHOUT input (the user away) and runs at priority 3, which quiet hours silence anyway — so she
//! never told someone still at the PC at 2am to go to bed. This one asks the opposite question:
//! inside quiet hours, is the user still ACTIVE? Priority 9 is the tier quiet hours let through.

use std::sync::Mutex;

use chrono::Timelike;

use super::gates::is_quiet_hours;
use super::prompts;
use super::types::{ProactiveCandidate, ProactiveProvider};

/// Input in the last few minutes = still at the PC, not asleep with the screen on.
const ACTIVE_WITHIN_SECONDS: u64 = 5 * 60;
/// Once is a reminder, every 90 s would be nagging.
pub const BEDTIME_RENUDGE_MS: u64 = 45 * 60 * 1000;
const BEDTIME_PRIORITY: u8 = 9;

/// Pure core: quiet hours, user active, and no nudge in the last `BEDTIME_RENUDGE_MS`.
pub fn bedtime_due(hour: u32, quiet: (u32, u32), idle_seconds: u64, last_nudge_ms: Option<u64>, now_ms: u64) -> bool {
    is_quiet_hours(hour, quiet.0, quiet.1)
        && idle_seconds < ACTIVE_WITHIN_SECONDS
        && last_nudge_ms.is_none_or(|last| now_ms.saturating_sub(last) >= BEDTIME_RENUDGE_MS)
}

pub struct BedtimeProvider {
    nickname: String,
    quiet_hours: Box<dyn Fn() -> (u32, u32) + Send + Sync>,
    /// The user's wall clock (`user.timezone`), not the machine's — see `awareness::user_clock_now`.
    clock: Box<dyn Fn() -> chrono::NaiveDateTime + Send + Sync>,
    last_nudge_ms: Mutex<Option<u64>>,
}

impl BedtimeProvider {
    pub fn new(
        nickname: String,
        quiet_hours: Box<dyn Fn() -> (u32, u32) + Send + Sync>,
        clock: Box<dyn Fn() -> chrono::NaiveDateTime + Send + Sync>,
    ) -> Self {
        Self { nickname, quiet_hours, clock, last_nudge_ms: Mutex::new(None) }
    }
}

impl ProactiveProvider for BedtimeProvider {
    fn id(&self) -> &'static str {
        "bedtime"
    }

    // Recorded when offered, not when spoken: a tick lost to the cadence costs one renudge window,
    // never a nudge every tick.
    fn due(&self, now_ms: u64) -> Option<ProactiveCandidate> {
        let idle_seconds = crate::power::snapshot().ok()?.get("idleSeconds")?.as_u64()?;
        let now = (self.clock)();
        let mut last = self.last_nudge_ms.lock().unwrap_or_else(|e| e.into_inner());
        if !bedtime_due(now.hour(), (self.quiet_hours)(), idle_seconds, *last, now_ms) {
            return None;
        }
        *last = Some(now_ms);
        let prompt = prompts::bedtime_nudge(&self.nickname, &now.format("%H:%M").to_string())
            .inspect_err(|error| log::error!("provider proativo 'bedtime': prompt não carregou: {error}"))
            .ok()?;
        Some(ProactiveCandidate { key: "bedtime".into(), priority: BEDTIME_PRIORITY, label: "bedtime nudge".into(), prompt })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIET: (u32, u32) = (23, 7);
    const NOW: u64 = 10 * 60 * 60 * 1000;

    #[test]
    fn at_the_pc_at_two_in_the_morning_she_says_go_to_sleep() {
        assert!(bedtime_due(2, QUIET, 10, None, NOW));
        assert!(bedtime_due(23, QUIET, 10, None, NOW));
    }

    #[test]
    fn not_outside_quiet_hours_nor_when_the_user_is_already_away() {
        assert!(!bedtime_due(21, QUIET, 10, None, NOW));
        assert!(!bedtime_due(7, QUIET, 10, None, NOW));
        assert!(!bedtime_due(2, QUIET, ACTIVE_WITHIN_SECONDS, None, NOW));
    }

    #[test]
    fn a_nudge_waits_the_renudge_window_before_the_next() {
        assert!(!bedtime_due(2, QUIET, 10, Some(NOW - BEDTIME_RENUDGE_MS + 1), NOW));
        assert!(bedtime_due(2, QUIET, 10, Some(NOW - BEDTIME_RENUDGE_MS), NOW));
    }

    #[test]
    fn priority_passes_the_quiet_hours_gate() {
        use std::collections::HashMap;
        let empty = HashMap::new();
        let state = super::super::gates::CadenceState {
            last_spoke_at: 0,
            last_for_key: &empty,
            hourly_speak_timestamps: &[],
            base_interval_ms: 120_000,
        };
        let candidate = ProactiveCandidate { key: "bedtime".into(), priority: BEDTIME_PRIORITY, label: String::new(), prompt: String::new() };
        assert!(super::super::gates::passes_cadence(&state, &candidate, NOW, true, 50.0));
    }

    #[test]
    fn the_prompt_names_the_user_and_the_time() {
        let prompt = prompts::bedtime_nudge("Ana", "02:14").expect("awareness/bedtime.md");
        assert!(prompt.contains("Ana") && prompt.contains("02:14"), "{prompt}");
    }
}
