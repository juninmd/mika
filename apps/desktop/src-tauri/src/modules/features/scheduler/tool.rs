//! Tool `reminder` (LLM) — port PARCIAL de `packages/scheduler/src/tools/reminder.tool.ts`. Ver o
//! cabeçalho de `mod.rs` para a tabela de paridade e as divergências documentadas.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeDelta, TimeZone};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::store::ConfigStore;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::store::SchedulerStore;

const MAX_SUBJECT_CHARS: usize = 280;

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn unit_to_minutes(unit: &str) -> Option<i64> {
    match unit {
        "minutes" => Some(1),
        "hours" => Some(60),
        "days" => Some(1440),
        _ => None,
    }
}

fn invalid_args(message: &str) -> Value {
    json!({ "success": false, "error": message, "errorCode": "invalid_args" })
}

/// `"HH:mm"` 24h estrito (`HHMM_PATTERN` do TS): o modelo só manda o texto, o epoch é do código.
fn parse_hh_mm(text: &str) -> Option<(u32, u32)> {
    let (h, m) = text.split_once(':')?;
    let digits = |s: &str| s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(h) || !digits(m) {
        return None;
    }
    let (h, m) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some((h, m))
}

/// Resolve a hora de parede no fuso; num buraco de DST (hora que não existe) pula para a hora seguinte.
fn resolve_local<Tz: TimeZone>(tz: &Tz, naive: NaiveDateTime) -> Option<DateTime<Tz>> {
    tz.from_local_datetime(&naive).earliest().or_else(|| tz.from_local_datetime(&(naive + TimeDelta::hours(1))).earliest())
}

/// Próximo `hour:minute` no fuso de `now` (hoje ou amanhã) — porta `nextLocalTimeAt` (`local-time.ts`).
/// O fuso vem do SO (`chrono::Local`): ele é o banco de fusos, sem `chrono-tz`.
pub(super) fn next_local_time_at<Tz: TimeZone>(now: &DateTime<Tz>, hour: u32, minute: u32) -> i64 {
    let tz = now.timezone();
    let at = |date: NaiveDate| date.and_hms_opt(hour, minute, 0).and_then(|n| resolve_local(&tz, n));
    let today = now.date_naive();
    match at(today) {
        Some(t) if t > *now => t.timestamp_millis(),
        _ => today.succ_opt().and_then(at).map_or(now.timestamp_millis() + 86_400_000, |t| t.timestamp_millis()),
    }
}

/// Hora de parede local (minutos desde 00:00) de um instante — a âncora de um lembrete diário relativo.
fn local_minutes(at_ms: i64) -> Option<u32> {
    use chrono::Timelike;
    Local.timestamp_millis_opt(at_ms).earliest().map(|at| at.hour() * 60 + at.minute())
}

fn relative_due_at(args: &Value, now: i64) -> Result<i64, Value> {
    let amount = args.get("amount").and_then(Value::as_i64);
    let unit = args.get("unit").and_then(Value::as_str).and_then(unit_to_minutes);
    let (Some(amount), Some(unit_minutes)) = (amount, unit) else {
        return Err(invalid_args("Provide atTime, or amount and unit."));
    };
    if amount <= 0 {
        return Err(invalid_args("amount must be positive."));
    }
    amount
        .checked_mul(unit_minutes * 60_000)
        .and_then(|span| now.checked_add(span))
        .ok_or_else(|| invalid_args("amount is too large."))
}

/// Espelha `scheduleReminder` (`reminder.tool.ts`): `atTime` tem precedência sobre `amount`+`unit`.
fn schedule(store: &SchedulerStore, args: &Value) -> Value {
    let subject = args.get("subject").and_then(Value::as_str);
    let Some(subject) = subject.map(str::trim) else {
        return invalid_args("subject is required for action=schedule.");
    };
    if subject.is_empty() {
        return invalid_args("subject must not be blank.");
    }
    if subject.chars().count() > MAX_SUBJECT_CHARS {
        return invalid_args("subject is too long (max 280 characters); shorten it and try again.");
    }

    let now = now_ms();
    let (due_at, at_minutes) = match args.get("atTime").and_then(Value::as_str) {
        Some(text) => match parse_hh_mm(text) {
            Some((hour, minute)) => (next_local_time_at(&Local::now(), hour, minute), Some(hour * 60 + minute)),
            None => return invalid_args("atTime must use the 24h format \"HH:mm\" (e.g. \"18:30\")."),
        },
        None => match relative_due_at(args, now) {
            Ok(due_at) => (due_at, local_minutes(due_at)),
            Err(error) => return error,
        },
    };
    if due_at <= now {
        return invalid_args("The requested time has already passed.");
    }

    let recurrence = args.get("recurrence").and_then(Value::as_str).unwrap_or("once");
    let recurrence = if recurrence == "daily" { "daily" } else { "once" };
    let id = Uuid::new_v4().to_string();

    let at_minutes = at_minutes.filter(|_| recurrence == "daily");
    match store.add(id, subject.to_string(), due_at, recurrence, now, at_minutes) {
        Ok(reminder) => json!({
            "success": true,
            "data": { "id": reminder.id, "subject": reminder.subject, "dueAt": reminder.due_at, "recurrence": reminder.recurrence },
        }),
        Err(error) => json!({ "success": false, "error": error, "errorCode": "unavailable" }),
    }
}

fn list(store: &SchedulerStore) -> Value {
    match store.list() {
        Ok(reminders) => json!({ "success": true, "data": { "reminders": reminders } }),
        Err(error) => json!({ "success": false, "error": error, "errorCode": "unavailable" }),
    }
}

fn cancel(store: &SchedulerStore, id: &str) -> Value {
    match store.remove(id) {
        Ok(true) => json!({ "success": true, "data": { "id": id } }),
        Ok(false) => json!({ "success": false, "error": "Reminder not found.", "errorCode": "not_found" }),
        Err(error) => json!({ "success": false, "error": error, "errorCode": "unavailable" }),
    }
}

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "reminder".into(),
        description: Some(
            "The user's reminders. Call it in the SAME turn they ask — saying \"got it\" without the call schedules nothing. action=schedule: `subject` + either `atTime` or `amount`+`unit` from now; `recurrence` \"daily\" repeats. action=list: pending reminders with ids. action=cancel: remove one by `id`."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["schedule", "list", "cancel"] },
                "subject": { "type": "string", "minLength": 1, "maxLength": 280, "description": "What to remind them of, in their own words." },
                "amount": { "type": "integer", "minimum": 1 },
                "unit": { "type": "string", "enum": ["minutes", "hours", "days"] },
                "atTime": { "type": "string", "pattern": "^([01]\\d|2[0-3]):[0-5]\\d$", "description": "\"HH:mm\" 24h in the user's timezone; if already past today, it fires tomorrow." },
                "recurrence": { "type": "string", "enum": ["once", "daily"], "default": "once" },
                "id": { "type": "string", "minLength": 1 },
            },
            "required": ["action"],
        }),
        plugin_id: "mika.scheduler".into(),
        requires_window: false,
    }
}

struct ReminderToolHandler {
    store: Arc<SchedulerStore>,
    config: Arc<ConfigStore>,
}

#[async_trait]
impl ToolHandler for ReminderToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        // Toggle mestre da feature (`scheduler.enabled`) — lido do `ConfigStore` REAL a cada
        // chamada, mesma técnica de `person_catalog::tool::catalog_enabled` (varredura de
        // controles de UI de 2026-09-27: `list`/`schedule`/`cancel` dispachavam com a aba
        // desligada).
        if !super::feature::master_enabled(&self.config) {
            return Ok(super::feature::master_disabled_response());
        }
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        match action {
            "list" => Ok(list(&self.store)),
            "cancel" => match args.get("id").and_then(Value::as_str) {
                Some(id) => Ok(cancel(&self.store, id)),
                None => Ok(invalid_args("id is required for action=cancel.")),
            },
            "schedule" => Ok(schedule(&self.store, &args)),
            other => Err(format!("reminder: unknown action \"{other}\"")),
        }
    }
}

/// Chamado por `modules::register_config_bound_tools(app)` — usa o `AppHandle`, não um
/// `ConfigStore`, porque `scheduler.db` mora em `<app_data_dir>/plugins/mika.scheduler/` (ver
/// `store::db_path`). Nenhum namespace `scheduler` existe em `config/schemas/` ainda, então não há
/// declaração de settings para registrar em `feature_registry::shared()` nesta rodada.
pub fn register(app: &tauri::AppHandle) -> Result<(), String> {
    let path = super::store::db_path(app)?;
    let store = Arc::new(SchedulerStore::open(&path)?);
    super::firing::spawn(app.clone(), Arc::clone(&store));
    // `register_config_bound_tools` (chamador, fora do meu escopo) só chega aqui depois de já ter
    // resolvido o MESMO `ConfigStore` com sucesso — ver o cabeçalho daquela função.
    let config = crate::state::config_store(app)?;
    crate::modules::tools::registry().register(tool_definition(), Arc::new(ReminderToolHandler { store, config }));
    super::plugin_test::register();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, FixedOffset};

    /// Dono do diretório escrachável do `ConfigStore` de [`handler`] — mantido vivo pelo chamador
    /// (não dropado no fim da função) enquanto o handler existir.
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn handler() -> (ReminderToolHandler, Scratch) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mika-scheduler-tool-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = Arc::new(ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap());
        (ReminderToolHandler { store: Arc::new(SchedulerStore::open_in_memory().unwrap()), config }, Scratch(dir))
    }

    /// Toggle mestre da feature (`scheduler.enabled=false`) — nenhuma ação de `reminder` dispacha
    /// (varredura de controles de UI de 2026-09-27).
    #[tokio::test]
    async fn call_is_denied_when_the_master_toggle_is_off() {
        let (handler, _scratch) = handler();
        handler.config.set("scheduler.enabled", json!(false)).unwrap();
        let result = handler.call(json!({ "action": "list" })).await.unwrap();
        assert_eq!(result["success"], false);
        assert_eq!(result["errorCode"], "denied");
    }

    /// `check:tool-coverage` (Rust) casa por NOME de tool, não por handler — os testes abaixo
    /// constroem `ReminderToolHandler` direto e nunca escrevem a string "reminder", então o gate
    /// acusava a tool como só exercitada pelo modelo. Esta asserção fecha o nome ao contrato público
    /// (`ToolDefinition.name`, o que `mika:tool-execute` e `mika:tools-list` expõem).
    #[test]
    fn tool_definition_name_is_reminder() {
        assert_eq!(tool_definition().name, "reminder");
    }

    #[tokio::test]
    async fn schedule_with_amount_and_unit_persists_a_future_reminder() {
        let (handler, _scratch) = handler();
        let result = handler
            .call(json!({ "action": "schedule", "subject": "reunião", "amount": 10, "unit": "minutes" }))
            .await
            .unwrap();
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["subject"], json!("reunião"));
        assert_eq!(result["data"]["recurrence"], json!("once"));
        assert!(result["data"]["dueAt"].as_i64().unwrap() > now_ms());
    }

    #[tokio::test]
    async fn schedule_with_blank_subject_is_invalid_args() {
        let (handler, _scratch) = handler();
        let result = handler.call(json!({ "action": "schedule", "subject": "   ", "amount": 1, "unit": "minutes" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    #[tokio::test]
    async fn schedule_with_overlong_subject_is_invalid_args() {
        let (handler, _scratch) = handler();
        let subject = "a".repeat(281);
        let result = handler.call(json!({ "action": "schedule", "subject": subject, "amount": 1, "unit": "minutes" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    #[tokio::test]
    async fn schedule_trims_the_subject_before_saving() {
        let (handler, _scratch) = handler();
        let result = handler.call(json!({ "action": "schedule", "subject": "  reunião  ", "amount": 1, "unit": "minutes" })).await.unwrap();
        assert_eq!(result["data"]["subject"], json!("reunião"));
    }

    #[tokio::test]
    async fn schedule_without_subject_is_a_named_invalid_args_error() {
        let (handler, _scratch) = handler();
        let result = handler.call(json!({ "action": "schedule", "amount": 5, "unit": "minutes" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    fn brt(h: u32, m: u32) -> DateTime<FixedOffset> {
        FixedOffset::west_opt(3 * 3600).unwrap().with_ymd_and_hms(2026, 9, 25, h, m, 0).unwrap()
    }

    #[test]
    fn at_time_later_today_lands_today_in_the_user_zone() {
        assert_eq!(next_local_time_at(&brt(17, 0), 18, 30), brt(18, 30).timestamp_millis());
    }

    /// "Se já passou hoje, agenda para amanhã" — inclusive o minuto exato de agora.
    #[test]
    fn at_time_already_past_or_now_rolls_to_tomorrow() {
        let tomorrow = brt(16, 0).timestamp_millis() + 86_400_000;
        assert_eq!(next_local_time_at(&brt(17, 0), 16, 0), tomorrow);
        assert_eq!(next_local_time_at(&brt(17, 0), 17, 0), brt(17, 0).timestamp_millis() + 86_400_000);
    }

    #[test]
    fn at_time_parser_accepts_only_24h_hh_mm() {
        assert_eq!(parse_hh_mm("00:00"), Some((0, 0)));
        assert_eq!(parse_hh_mm("23:59"), Some((23, 59)));
        for bad in ["24:00", "18:60", "8:30", "18:3", "18h30", "", "１８:３０", "18:30 "] {
            assert_eq!(parse_hh_mm(bad), None, "{bad:?}");
        }
    }

    #[tokio::test]
    async fn schedule_with_at_time_persists_within_the_next_day() {
        let (handler, _scratch) = handler();
        let result = handler
            .call(json!({ "action": "schedule", "subject": "x", "atTime": "18:30" }))
            .await
            .unwrap();
        assert_eq!(result["success"], json!(true));
        let due = result["data"]["dueAt"].as_i64().unwrap();
        let now = now_ms();
        assert!(due > now && due <= now + 86_400_000 + 3_600_000, "dueAt fora da janela: {due}");
    }

    #[tokio::test]
    async fn schedule_with_malformed_at_time_is_invalid_args() {
        let (handler, _scratch) = handler();
        let result = handler
            .call(json!({ "action": "schedule", "subject": "x", "atTime": "25:00" }))
            .await
            .unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    /// `amount * unit * 60_000` estourava i64: pânico em debug, `due_at` negativo (lembrete que
    /// dispara na hora) em release. Quantidade absurda é recusada com erro nomeado.
    #[tokio::test]
    async fn an_amount_that_overflows_the_clock_is_rejected_not_wrapped() {
        let (handler, _scratch) = handler();
        for amount in [i64::MAX, i64::MAX / 1_000, 9_000_000_000_000_000] {
            let result = handler.call(json!({ "action": "schedule", "subject": "x", "amount": amount, "unit": "days" })).await.unwrap();
            assert_eq!(result["errorCode"], json!("invalid_args"), "amount={amount}: {result}");
        }
        assert_eq!(handler.call(json!({ "action": "list" })).await.unwrap()["data"]["reminders"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn a_daily_at_time_reminder_remembers_its_wall_clock_anchor() {
        let (handler, _scratch) = handler();
        let scheduled = handler.call(json!({ "action": "schedule", "subject": "x", "atTime": "02:30", "recurrence": "daily" })).await.unwrap();
        assert_eq!(scheduled["success"], json!(true));
        let stored = handler.store.list().unwrap();
        assert_eq!(stored[0].at_minutes, Some(150));
    }

    #[tokio::test]
    async fn schedule_without_amount_or_unit_or_at_time_is_invalid_args() {
        let (handler, _scratch) = handler();
        let result = handler.call(json!({ "action": "schedule", "subject": "x" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    #[tokio::test]
    async fn list_then_cancel_round_trips() {
        let (handler, _scratch) = handler();
        let scheduled =
            handler.call(json!({ "action": "schedule", "subject": "x", "amount": 1, "unit": "hours" })).await.unwrap();
        let id = scheduled["data"]["id"].as_str().unwrap().to_string();

        let listed = handler.call(json!({ "action": "list" })).await.unwrap();
        assert_eq!(listed["data"]["reminders"].as_array().unwrap().len(), 1);

        let cancelled = handler.call(json!({ "action": "cancel", "id": id })).await.unwrap();
        assert_eq!(cancelled["success"], json!(true));

        let empty = handler.call(json!({ "action": "list" })).await.unwrap();
        assert_eq!(empty["data"]["reminders"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn cancel_unknown_id_is_not_found() {
        let (handler, _scratch) = handler();
        let result = handler.call(json!({ "action": "cancel", "id": "does-not-exist" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("not_found"));
    }
}
