//! Cálculos puros de estatística de sessão — porte de `handlers/game-session-stats.ts`
//! (`playtimeSummary`/`playStreak`). Agrupa por dia civil no fuso injetado — o chamador passa
//! `chrono::Local` (fuso do SO, mesma escolha de `brain/persona/user_clock.rs`); UTC fazia uma
//! sessão perto da meia-noite local cair no dia errado e quebrar a streak.

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, TimeZone};

use super::session_types::GameSession;

/// Porte de `formatDurationMs` (`packages/sdk/src/utils/time-format.ts`).
pub fn format_duration_ms(ms: Option<u64>) -> String {
    let Some(ms) = ms.filter(|&ms| ms > 0) else { return "0min".to_string() };
    let minutes = ((ms as f64) / 60_000.0).round() as i64;
    if minutes < 60 {
        return format!("{minutes}min");
    }
    let hours = minutes / 60;
    let rest = minutes % 60;
    if rest > 0 { format!("{hours}h{rest}min") } else { format!("{hours}h") }
}

/// Dia civil de `epoch_ms` no fuso `tz`, como contagem de dias (só a diferença entre chaves importa).
pub fn day_key<Tz: TimeZone>(epoch_ms: u64, tz: &Tz) -> i64 {
    DateTime::from_timestamp_millis(epoch_ms as i64)
        .map_or(0, |utc| i64::from(utc.with_timezone(tz).date_naive().num_days_from_ce()))
}

/// Porte do `sum()` interno de `playtimeSummary`: soma `durationMs` por jogo, só sessões cujo dia local
/// caia em `[today_key - max_day_diff, today_key]`. Devolve pares (jogo, ms) ordenados por ms desc —
/// mesma ordenação de `Object.entries(totals).sort((a, b) => b[1] - a[1])`.
pub fn playtime_by_game<Tz: TimeZone>(
    history: &[GameSession],
    today_key: i64,
    max_day_diff: i64,
    tz: &Tz,
) -> Vec<(String, u64)> {
    let mut totals: BTreeMap<String, u64> = BTreeMap::new();
    for session in history {
        let day_diff = today_key - day_key(session.start_time_ms, tz);
        if day_diff < 0 || day_diff > max_day_diff {
            continue;
        }
        let ms = session.duration_ms().unwrap_or(0);
        *totals.entry(session.game_name.clone()).or_insert(0) += ms;
    }
    let mut entries: Vec<(String, u64)> = totals.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
}

/// Porte de `playStreak`: `(currentStreakDays, longestStreakDays)`, ambos por dia civil local.
pub fn streak<Tz: TimeZone>(history: &[GameSession], now_ms: u64, tz: &Tz) -> (i64, i64) {
    if history.is_empty() {
        return (0, 0);
    }
    let mut day_keys: Vec<i64> = history.iter().map(|s| day_key(s.start_time_ms, tz)).collect();
    day_keys.sort_unstable();
    day_keys.dedup();
    day_keys.reverse(); // desc, mesmo efeito de `.sort((a, b) => b - a)` após o `new Set(...)`.

    let today_key = day_key(now_ms, tz);
    let mut current = 0i64;
    if day_keys[0] == today_key || day_keys[0] == today_key - 1 {
        current = 1;
        for i in 1..day_keys.len() {
            if day_keys[i - 1] - day_keys[i] == 1 {
                current += 1;
            } else {
                break;
            }
        }
    }

    let mut longest = 1i64;
    let mut run = 1i64;
    for i in 1..day_keys.len() {
        run = if day_keys[i - 1] - day_keys[i] == 1 { run + 1 } else { 1 };
        if run > longest {
            longest = run;
        }
    }

    (current, longest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};

    const DAY_MS: i64 = 86_400_000;

    fn session(game: &str, start_ms: u64, end_ms: Option<u64>) -> GameSession {
        GameSession {
            id: "s".into(),
            game_id: "g".into(),
            game_name: game.into(),
            start_time_ms: start_ms,
            end_time_ms: end_ms,
            events: vec![],
            summary: None,
        }
    }

    #[test]
    fn format_duration_ms_matches_the_ts_boundaries() {
        assert_eq!(format_duration_ms(None), "0min");
        assert_eq!(format_duration_ms(Some(0)), "0min");
        assert_eq!(format_duration_ms(Some(45 * 60_000)), "45min");
        assert_eq!(format_duration_ms(Some(60 * 60_000)), "1h");
        assert_eq!(format_duration_ms(Some(125 * 60_000)), "2h5min");
    }

    #[test]
    fn playtime_by_game_sums_only_sessions_within_the_day_window() {
        let day = DAY_MS as u64;
        let history = vec![
            session("Hades", 10 * day, Some(10 * day + 3_600_000)), // hoje, 1h
            session("Hades", 3 * day, Some(3 * day + 1_800_000)),   // fora da janela de 6 dias
            session("Dota 2", 9 * day, Some(9 * day + 1_800_000)),  // ontem, 30min
        ];
        let today_key = day_key(10 * day, &Utc);

        let today = playtime_by_game(&history, today_key, 0, &Utc);
        assert_eq!(today, vec![("Hades".to_string(), 3_600_000)]);

        let week = playtime_by_game(&history, today_key, 6, &Utc);
        assert_eq!(week[0], ("Hades".to_string(), 3_600_000));
        assert_eq!(week[1], ("Dota 2".to_string(), 1_800_000));
    }

    #[test]
    fn playtime_by_game_ignores_sessions_still_ongoing() {
        let day = DAY_MS as u64;
        let history = vec![session("Hades", 10 * day, None)];
        let totals = playtime_by_game(&history, day_key(10 * day, &Utc), 0, &Utc);
        assert_eq!(totals, vec![("Hades".to_string(), 0)]);
    }

    #[test]
    fn streak_is_zero_with_no_history() {
        assert_eq!(streak(&[], 0, &Utc), (0, 0));
    }

    #[test]
    fn streak_counts_consecutive_days_ending_today_or_yesterday() {
        let day = DAY_MS as u64;
        let history = vec![
            session("A", 10 * day, Some(10 * day + 1)),
            session("A", 9 * day, Some(9 * day + 1)),
            session("A", 8 * day, Some(8 * day + 1)),
            session("A", 5 * day, Some(5 * day + 1)), // quebra a sequência
        ];
        let now = 10 * day + 500;
        assert_eq!(streak(&history, now, &Utc), (3, 3));
    }

    #[test]
    fn streak_resets_to_zero_when_the_last_session_is_older_than_yesterday() {
        let day = DAY_MS as u64;
        let history = vec![session("A", 2 * day, Some(2 * day + 1))];
        let now = 10 * day;
        assert_eq!(streak(&history, now, &Utc).0, 0, "última sessão há 8 dias não é streak ativa");
    }

    /// Sessão às 22h BRT (01h UTC do dia seguinte) é do dia em que a pessoa jogou: com UTC, jogar
    /// todo fim de noite contava o dia errado e "hoje" já tinha a sessão de ontem.
    #[test]
    fn day_boundary_follows_the_user_zone_not_utc() {
        let brt = FixedOffset::west_opt(3 * 3600).unwrap();
        let day = DAY_MS as u64;
        let late_night = |d: u64| session("A", d * day + 3_600_000, Some(d * day + 3_600_001));
        let history = vec![late_night(10), late_night(9), late_night(8)];
        let now = 10 * day + 20 * 3_600_000; // 17h BRT do dia 10, ainda sem sessão hoje
        assert_eq!(day_key(10 * day + 3_600_000, &brt), day_key(9 * day + 12 * 3_600_000, &Utc));
        assert_eq!(streak(&history, now, &brt), (3, 3));
        let today = playtime_by_game(&history, day_key(now, &brt), 0, &brt);
        assert!(today.is_empty(), "a sessão das 22h de ontem não é de hoje: {today:?}");
    }
}
