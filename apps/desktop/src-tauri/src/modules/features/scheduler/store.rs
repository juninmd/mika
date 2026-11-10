//! Persistência SQLite dos lembretes — port 1:1 de
//! `packages/scheduler/src/adapters/sqlite-reminder.repository.ts`, mesmo schema de tabela.
//!
//! `rusqlite`, mesmo padrão de `modules/chat/chat_db.rs` (WAL + `synchronous=NORMAL` +
//! `busy_timeout` + `foreign_keys=ON`) — o mesmo arquivo que `bootstrapChat`/`bootstrapScheduler`
//! já abrem hoje em modo `file:` do libSQL é SQLite puro, byte-compatível com `rusqlite`.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::AppHandle;

const CREATE_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS reminders (
        id TEXT PRIMARY KEY,
        subject TEXT NOT NULL,
        due_at INTEGER NOT NULL,
        recurrence TEXT NOT NULL DEFAULT 'once',
        created_at INTEGER NOT NULL
    )
";

const CREATE_INDEX: &str = "CREATE INDEX IF NOT EXISTS idx_reminders_due ON reminders(due_at)";

/// Hora de parede original (minutos desde 00:00) de um lembrete diário. `due_at` sozinho não basta:
/// num buraco de DST ele é empurrado uma hora, e reler o horário dele derivava o lembrete para sempre.
const ADD_AT_MINUTES: &str = "ALTER TABLE reminders ADD COLUMN at_minutes INTEGER";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub subject: String,
    pub due_at: i64,
    pub recurrence: String,
    pub created_at: i64,
    /// `None` em lembrete `once` e nos diários gravados antes da coluna existir (derivam de `due_at`).
    #[serde(skip)]
    pub at_minutes: Option<u32>,
}

pub struct SchedulerStore {
    conn: Mutex<Connection>,
}

impl SchedulerStore {
    /// Abre (ou cria) `scheduler.db` em `path`, garante diretório pai e schema — idempotente,
    /// mesmo contrato de `ChatDb::open`.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("não criou {}: {e}", parent.display()))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("não abriu {}: {e}", path.display()))?;
        Self::from_connection(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("não abriu :memory:: {e}"))?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Result<Self, String> {
        apply_pragmas(&conn)?;
        let migration_1 = [CREATE_TABLE, CREATE_INDEX].join(";\n");
        crate::modules::sqlite_migrate::migrate(&conn, &[&migration_1, ADD_AT_MINUTES]).map_err(|e| format!("scheduler.db: {e}"))?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|_| "banco de lembretes envenenado".to_string())
    }

    /// `at_minutes`: hora de parede original de um lembrete diário (`None` em `once`).
    pub fn add(
        &self,
        id: String,
        subject: String,
        due_at: i64,
        recurrence: &str,
        created_at: i64,
        at_minutes: Option<u32>,
    ) -> Result<Reminder, String> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO reminders (id, subject, due_at, recurrence, created_at, at_minutes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, subject, due_at, recurrence, created_at, at_minutes],
        )
        .map_err(|e| format!("inserir lembrete: {e}"))?;
        Ok(Reminder { id, subject, due_at, recurrence: recurrence.to_string(), created_at, at_minutes })
    }

    pub fn list(&self) -> Result<Vec<Reminder>, String> {
        let conn = self.lock()?;
        query_reminders(&conn, "SELECT id, subject, due_at, recurrence, created_at, at_minutes FROM reminders ORDER BY due_at ASC", [])
    }

    pub fn due(&self, now: i64) -> Result<Vec<Reminder>, String> {
        let conn = self.lock()?;
        query_reminders(
            &conn,
            "SELECT id, subject, due_at, recurrence, created_at, at_minutes FROM reminders WHERE due_at <= ?1 ORDER BY due_at ASC",
            params![now],
        )
    }

    pub fn remove(&self, id: &str) -> Result<bool, String> {
        let conn = self.lock()?;
        let affected =
            conn.execute("DELETE FROM reminders WHERE id = ?1", params![id]).map_err(|e| format!("remover lembrete: {e}"))?;
        Ok(affected > 0)
    }

    pub fn reschedule(&self, id: &str, due_at: i64) -> Result<bool, String> {
        let conn = self.lock()?;
        let affected = conn
            .execute("UPDATE reminders SET due_at = ?1 WHERE id = ?2", params![due_at, id])
            .map_err(|e| format!("reagendar lembrete: {e}"))?;
        Ok(affected > 0)
    }

    /// Espelha `REMINDER_STORE.clearAll()` — usado por `features::privacy::clear_history`. Devolve
    /// quantas linhas saíram, nunca finge sucesso sobre uma tabela já vazia.
    pub fn clear_all(&self) -> Result<usize, String> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM reminders", []).map_err(|e| format!("limpar lembretes: {e}"))
    }
}

fn query_reminders(conn: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<Vec<Reminder>, String> {
    let mut statement = conn.prepare(sql).map_err(|e| format!("preparar consulta: {e}"))?;
    let rows = statement
        .query_map(params, |row| {
            Ok(Reminder {
                id: row.get(0)?,
                subject: row.get(1)?,
                due_at: row.get(2)?,
                recurrence: row.get(3)?,
                created_at: row.get(4)?,
                at_minutes: row.get(5)?,
            })
        })
        .map_err(|e| format!("consultar lembretes: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("ler lembretes: {e}"))
}

/// Mesmos pragmas de `modules/chat/chat_db.rs::apply_pragmas`.
fn apply_pragmas(conn: &Connection) -> Result<(), String> {
    conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| format!("pragma journal_mode: {e}"))?;
    conn.pragma_update(None, "synchronous", "NORMAL").map_err(|e| format!("pragma synchronous: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(5000)).map_err(|e| format!("busy_timeout: {e}"))?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(|e| format!("pragma foreign_keys: {e}"))?;
    Ok(())
}

/// `<app_data_dir>/plugins/mika.scheduler/scheduler.db` — mesma convenção de
/// `chat_db.rs::db_path` (`getDatabaseAtDir(pluginDataDir, filename)`,
/// `packages/sdk/src/plugins/plugin-activation.ts`, `pluginDataDir = join(dataRootDir, manifest.id)`
/// com `manifest.id = 'mika.scheduler'`).
pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?;
    Ok(data_dir.join("plugins").join("mika.scheduler").join("scheduler.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_list_round_trips_in_due_order() {
        let store = SchedulerStore::open_in_memory().unwrap();
        store.add("b".into(), "segunda".into(), 200, "once", 1, None).unwrap();
        store.add("a".into(), "primeira".into(), 100, "once", 1, None).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "a", "não ordenou por due_at ASC");
    }

    #[test]
    fn due_returns_only_expired_reminders() {
        let store = SchedulerStore::open_in_memory().unwrap();
        store.add("expired".into(), "x".into(), 100, "once", 1, None).unwrap();
        store.add("future".into(), "y".into(), 999_999, "once", 1, None).unwrap();

        let due = store.due(500).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, "expired");
    }

    #[test]
    fn remove_reports_whether_a_row_existed() {
        let store = SchedulerStore::open_in_memory().unwrap();
        store.add("x".into(), "s".into(), 100, "once", 1, None).unwrap();

        assert!(store.remove("x").unwrap());
        assert!(!store.remove("x").unwrap(), "remoção repetida deveria reportar false");
    }

    #[test]
    fn reschedule_moves_due_at_forward() {
        let store = SchedulerStore::open_in_memory().unwrap();
        store.add("x".into(), "s".into(), 100, "daily", 1, None).unwrap();

        assert!(store.reschedule("x", 200).unwrap());
        assert_eq!(store.list().unwrap()[0].due_at, 200);
    }

    #[test]
    fn clear_all_removes_every_reminder_and_reports_how_many() {
        let store = SchedulerStore::open_in_memory().unwrap();
        store.add("a".into(), "x".into(), 100, "once", 1, None).unwrap();
        store.add("b".into(), "y".into(), 200, "once", 1, None).unwrap();

        assert_eq!(store.clear_all().unwrap(), 2);
        assert!(store.list().unwrap().is_empty());
    }

    /// NEGATIVO — tabela já vazia não finge ter apagado nada.
    #[test]
    fn clear_all_on_an_empty_table_reports_zero() {
        let store = SchedulerStore::open_in_memory().unwrap();
        assert_eq!(store.clear_all().unwrap(), 0);
    }
}
