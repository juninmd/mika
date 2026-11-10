//! Firing loop for due reminders — port of `SchedulerService.tick` (TS, 20 s sweep).
//!
//! Each sweep takes the due reminders OUT of the store first (`once` removed, `daily` moved to its
//! next slot), then shows the notice and asks the LLM for the spoken line. Speech is best-effort:
//! the notice already delivered the reminder, and keeping it in the store until speech succeeds
//! made a downed LLM re-fire the same reminder every sweep (the TS needed a retry cap for that).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, TimeZone, Timelike};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::events::{bus, EventKey};

use super::store::{Reminder, SchedulerStore};
use super::tool::next_local_time_at;

const TICK: Duration = Duration::from_secs(20);
/// Past this, the app was closed when the reminder came due and Mika says she is late.
const LATE_NOTICE_MS: i64 = 5 * 60_000;

/// Next occurrence of a `daily` reminder: same wall-clock time, first slot after `now`. The time is
/// the ORIGINAL one (`at_minutes`): `due_at` of a reminder that landed in a DST gap was pushed an
/// hour, and reading the hour back from it drifted the reminder forever. Legacy rows without the
/// anchor fall back to `due_at`.
fn next_daily_due<Tz: TimeZone>(reminder: &Reminder, now: &DateTime<Tz>) -> i64 {
    if let Some(minutes) = reminder.at_minutes {
        return next_local_time_at(now, minutes / 60, minutes % 60);
    }
    match now.timezone().timestamp_millis_opt(reminder.due_at).earliest() {
        Some(at) => next_local_time_at(now, at.hour(), at.minute()),
        None => now.timestamp_millis() + 86_400_000,
    }
}

/// Claims the reminders due at `now`; one cancelled mid-sweep (row already gone) is skipped.
pub(super) fn take_due<Tz: TimeZone>(store: &SchedulerStore, now: &DateTime<Tz>) -> Result<Vec<Reminder>, String> {
    let mut claimed = Vec::new();
    for reminder in store.due(now.timestamp_millis())? {
        let kept = if reminder.recurrence == "daily" {
            store.reschedule(&reminder.id, next_daily_due(&reminder, now))?
        } else {
            store.remove(&reminder.id)?
        };
        if kept {
            claimed.push(reminder);
        }
    }
    Ok(claimed)
}

fn notice(reminder: &Reminder, locale: &str) -> Value {
    json!({
        "type": "info",
        "source": crate::modules::settings::t("settings.scheduler.tab.label", locale),
        "message": reminder.subject,
    })
}

fn prompt_vars(reminder: &Reminder, now_ms: i64, locale: &str) -> HashMap<String, String> {
    let late_ms = now_ms - reminder.due_at;
    let late_line = if late_ms > LATE_NOTICE_MS {
        format!("You are reminding them {} minutes late (the app was closed). Acknowledge it lightly.", late_ms / 60_000)
    } else {
        String::new()
    };
    [("subject", reminder.subject.clone()), ("lateLine", late_line), ("locale", crate::config::locale::language_name(locale).to_string())]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}

async fn fire_due(app: &AppHandle, store: &SchedulerStore) {
    let config = match crate::state::config_store(app) {
        Ok(config) => config,
        Err(error) => return log::error!("lembretes venceram sem config para avisar: {error}"),
    };
    // Toggle mestre (`scheduler.enabled`) — checado ANTES de `take_due`: um lembrete vencido
    // enquanto a feature está desligada continua pendente no store, em vez de ser consumido sem
    // nunca avisar (varredura de controles de UI de 2026-09-27).
    if !super::feature::master_enabled(&config) {
        return;
    }
    let now = Local::now();
    let due = match take_due(store, &now) {
        Ok(due) if !due.is_empty() => due,
        Ok(_) => return,
        Err(error) => return log::warn!("varredura de lembretes falhou: {error}"),
    };
    let locale = crate::config::locale::app_locale(&config);
    for reminder in due {
        bus().publish(EventKey::Notification, notice(&reminder, &locale));
        let vars = prompt_vars(&reminder, now.timestamp_millis(), &locale);
        match crate::modules::chat::load_prompt_no_handle("scheduler", "reminder-speech", &vars) {
            Ok(prompt) if !spoke_within_two_tries(|| crate::modules::chat::proactive::send_reminder(app, &config, &prompt)).await => {
                log::warn!("lembrete notificado sem fala: o turno proativo não gerou texto");
            }
            Ok(_) => log::info!("lembrete disparado ({})", reminder.recurrence),
            Err(error) => log::error!("lembrete notificado sem fala: {error}"),
        }
    }
}

/// One retry: a toolless reminder turn sometimes came back as call syntax and said nothing.
async fn spoke_within_two_tries<F, Fut>(mut attempt: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = String>,
{
    for _ in 0..2 {
        if !attempt().await.is_empty() {
            return true;
        }
    }
    false
}

/// Starts the sweep; the first tick is immediate, so reminders that came due while the app was
/// closed fire at boot.
pub(super) fn spawn(app: AppHandle, store: Arc<SchedulerStore>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(TICK);
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = interval.tick() => {}
            }
            fire_due(&app, &store).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn at(tz: &FixedOffset, text: &str) -> DateTime<FixedOffset> {
        tz.from_local_datetime(&chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap()).unwrap()
    }

    #[tokio::test]
    async fn a_reminder_that_came_back_silent_is_asked_once_more_and_no_more() {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let spoke = spoke_within_two_tries(|| {
            let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move { if n == 0 { String::new() } else { "Bebe água!".to_string() } }
        })
        .await;
        assert!(spoke);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);

        let calls = std::sync::atomic::AtomicUsize::new(0);
        assert!(!spoke_within_two_tries(|| { calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst); async { String::new() } }).await);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2, "a downed LLM is not hammered");
    }

    #[test]
    fn once_reminder_is_claimed_once_and_leaves_the_store() {
        let store = SchedulerStore::open_in_memory().unwrap();
        let tz = FixedOffset::west_opt(3 * 3600).unwrap();
        let now = at(&tz, "2026-09-25 10:00");
        store.add("a".into(), "tomar água".into(), now.timestamp_millis() - 1, "once", 0, None).unwrap();
        store.add("later".into(), "reunião".into(), now.timestamp_millis() + 60_000, "once", 0, None).unwrap();

        let fired = take_due(&store, &now).unwrap();
        assert_eq!(fired.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["a"]);
        // The next sweep must not fire it again — the defect class the TS retry cap existed for.
        assert!(take_due(&store, &now).unwrap().is_empty());
        assert_eq!(store.list().unwrap().len(), 1, "the future reminder stays");
    }

    #[test]
    fn daily_reminder_moves_to_the_same_time_tomorrow() {
        let store = SchedulerStore::open_in_memory().unwrap();
        let tz = FixedOffset::west_opt(3 * 3600).unwrap();
        let due = at(&tz, "2026-09-25 09:30");
        store.add("d".into(), "remédio".into(), due.timestamp_millis(), "daily", 0, None).unwrap();

        let now = at(&tz, "2026-09-25 09:30");
        assert_eq!(take_due(&store, &now).unwrap().len(), 1);
        assert_eq!(store.list().unwrap()[0].due_at, at(&tz, "2026-09-26 09:30").timestamp_millis());
    }

    #[test]
    fn daily_reminder_missed_for_days_fires_once_and_catches_up_to_the_next_slot() {
        let store = SchedulerStore::open_in_memory().unwrap();
        let tz = FixedOffset::east_opt(0).unwrap();
        store.add("d".into(), "alongar".into(), at(&tz, "2026-09-20 08:00").timestamp_millis(), "daily", 0, None).unwrap();

        let now = at(&tz, "2026-09-25 12:00");
        assert_eq!(take_due(&store, &now).unwrap().len(), 1);
        assert_eq!(store.list().unwrap()[0].due_at, at(&tz, "2026-09-26 08:00").timestamp_millis());
        assert!(take_due(&store, &now).unwrap().is_empty());
    }

    /// Fuso de teste com buraco de DST (estilo US/Eastern: em 2026-03-08, 02:00-03:00 não existe).
    #[derive(Clone, Copy, Debug)]
    struct Eastern;
    #[derive(Clone, Copy, Debug)]
    struct Off(i32);
    impl chrono::Offset for Off {
        fn fix(&self) -> chrono::FixedOffset {
            chrono::FixedOffset::east_opt(self.0).unwrap()
        }
    }
    impl std::fmt::Display for Off {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
    fn mar8(h: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 3, 8).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }
    impl TimeZone for Eastern {
        type Offset = Off;
        fn from_offset(_: &Off) -> Self {
            Eastern
        }
        fn offset_from_local_date(&self, date: &chrono::NaiveDate) -> chrono::LocalResult<Off> {
            self.offset_from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
        }
        fn offset_from_local_datetime(&self, local: &chrono::NaiveDateTime) -> chrono::LocalResult<Off> {
            match (*local < mar8(2), *local < mar8(3)) {
                (true, _) => chrono::LocalResult::Single(Off(-5 * 3600)),
                (false, true) => chrono::LocalResult::None,
                (false, false) => chrono::LocalResult::Single(Off(-4 * 3600)),
            }
        }
        fn offset_from_utc_date(&self, date: &chrono::NaiveDate) -> Off {
            self.offset_from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
        }
        fn offset_from_utc_datetime(&self, utc: &chrono::NaiveDateTime) -> Off {
            Off(if *utc < mar8(7) { -5 * 3600 } else { -4 * 3600 })
        }
    }

    /// Lembrete diário das 02:30 num dia em que 02:30 não existe dispara às 03:30 UMA vez — e volta às
    /// 02:30 no dia seguinte. Antes, o horário era relido do `due_at` já empurrado (03:30) e o lembrete
    /// derivava para 03:30 para sempre.
    #[test]
    fn a_daily_reminder_swallowed_by_a_dst_gap_returns_to_its_original_time() {
        let store = SchedulerStore::open_in_memory().unwrap();
        let fired_at = Eastern.from_local_datetime(&mar8(3).with_minute(30).unwrap()).unwrap();
        store.add("d".into(), "remédio".into(), fired_at.timestamp_millis(), "daily", 0, Some(2 * 60 + 30)).unwrap();

        assert_eq!(take_due(&store, &fired_at).unwrap().len(), 1);

        let expected = Eastern.from_local_datetime(&(mar8(2) + chrono::TimeDelta::days(1) + chrono::TimeDelta::minutes(30))).unwrap();
        assert_eq!(store.list().unwrap()[0].due_at, expected.timestamp_millis(), "deveria voltar às 02:30 de 9/3");
    }

    #[test]
    fn late_line_only_when_the_reminder_is_more_than_five_minutes_late() {
        let reminder = Reminder { id: "a".into(), subject: "x".into(), due_at: 0, recurrence: "once".into(), created_at: 0, at_minutes: None };
        assert_eq!(prompt_vars(&reminder, LATE_NOTICE_MS, "pt-BR")["lateLine"], "");
        assert!(prompt_vars(&reminder, 42 * 60_000, "pt-BR")["lateLine"].contains("42 minutes"));
    }

    #[test]
    fn reminder_speech_prompt_ships_and_renders_the_subject() {
        let reminder = Reminder { id: "a".into(), subject: "ligar pra mãe".into(), due_at: 0, recurrence: "once".into(), created_at: 0, at_minutes: None };
        let prompt = crate::modules::chat::load_prompt_no_handle("scheduler", "reminder-speech", &prompt_vars(&reminder, 0, "pt-BR")).unwrap();
        assert!(prompt.contains("\"ligar pra mãe\""), "{prompt}");
        assert!(!prompt.contains("{{"), "placeholder left unrendered: {prompt}");
    }
}
