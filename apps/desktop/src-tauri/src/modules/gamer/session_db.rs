//! Persistência SQLite de sessão de jogo — porte de `services/session-memory-schema.ts` +
//! `session-memory-rows.ts`. `rusqlite`, mesma decisão já validada por `modules/chat/chat_db.rs`
//! para `chat.db`: `session-memory.ts` só usa DDL/DML padrão (`getDb` devolve um `@libsql/client`
//! aberto em `file:`, modo SQLite puro) — sem função só-libSQL.
//!
//! **Mesmo arquivo, não um novo.** `db_path` reproduz
//! `<app_data_dir>/plugins/mika.gamer-companion/gamer-sessions.db` — o caminho exato que
//! `GameSessionMemory` (`apps/desktop/src/features/gamer-companion/index.ts`) abre hoje via
//! `ctx.getDatabase('gamer-sessions.db')`. Esse `getDatabase` é o genérico de
//! `plugin-activation.ts::createDatabaseGetter`, que resolve `pluginDataDir = join(dataRootDir,
//! manifest.id)` com `dataRootDir = join(app.getPath('userData'), 'plugins')`
//! (`main/infrastructure/bootstrap/plugins.ts`) e `manifest.id = 'mika.gamer-companion'`
//! (`gamer-companion/index.ts`) — os mesmos três segmentos que `chat_db.rs` já confirmou para
//! `mika.chat`. `crate::state::app_data_dir(app)` é o equivalente Tauri de `app.getPath('userData')`.
//!
//! `start_time`/`end_time` ficam em TEXT ISO 8601 ([`super::iso_time`]), não epoch-ms: é o formato
//! que `toISOString()` já gravou nas linhas existentes, e misturar formatos na mesma coluna
//! quebraria `ORDER BY start_time DESC` (comparação lexicográfica) entre linhas antigas e novas.
//!
//! `CREATE TABLE game_locations` está aqui por completude de schema (mesmo `createSessionTables`
//! do TS cria as duas tabelas juntas), mas CRUD de localização não foi portado nesta rodada —
//! `services/game-locations-store.ts` fica de fora, nomeado no relatório, não fingido.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection};
use tauri::AppHandle;

use super::iso_time::{format_iso8601, parse_iso8601};
use super::session_types::{GameEvent, GameSession};

const CREATE_SESSIONS: &str = "
    CREATE TABLE IF NOT EXISTS game_sessions (
        id TEXT PRIMARY KEY, game_id TEXT NOT NULL, game_name TEXT NOT NULL,
        start_time TEXT NOT NULL, end_time TEXT,
        events TEXT NOT NULL DEFAULT '[]', summary TEXT
    )
";

// Schema completo por paridade com `createSessionTables`, mesmo sem CRUD de localização nesta
// rodada — ver o cabeçalho do arquivo.
const CREATE_LOCATIONS: &str = "
    CREATE TABLE IF NOT EXISTS game_locations (
        id TEXT PRIMARY KEY,
        game_name TEXT NOT NULL,
        location_name TEXT NOT NULL,
        description TEXT NOT NULL,
        connections TEXT NOT NULL DEFAULT '[]',
        important_items TEXT NOT NULL DEFAULT '[]',
        notes TEXT,
        created_at TEXT NOT NULL
    )
";

pub struct SessionDb {
    conn: Mutex<Connection>,
}

impl SessionDb {
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
        let migration_1 = [CREATE_SESSIONS, CREATE_LOCATIONS].join(";\n");
        crate::modules::sqlite_migrate::migrate(&conn, &[&migration_1]).map_err(|e| format!("sessões de jogo: {e}"))?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// `pub(super)` (não privado): [`super::game_locations`] reusa esta MESMA conexão para o CRUD de
    /// `game_locations` em vez de abrir uma segunda `Connection` para o mesmo arquivo — a tabela já é
    /// criada por [`Self::from_connection`] acima.
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|_| "banco de sessão de jogo envenenado".to_string())
    }

    pub fn insert(&self, session: &GameSession) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO game_sessions (id, game_id, game_name, start_time, events) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session.id, session.game_id, session.game_name, format_iso8601(session.start_time_ms), "[]"],
        )
        .map_err(|e| format!("insert session: {e}"))?;
        Ok(())
    }

    pub fn finish(&self, id: &str, end_time_ms: u64, summary: Option<&str>, events: &[GameEvent]) -> Result<(), String> {
        let json = serde_json::to_string(events).map_err(|e| format!("serializar events: {e}"))?;
        let conn = self.lock()?;
        conn.execute(
            "UPDATE game_sessions SET end_time = ?1, summary = ?2, events = ?3 WHERE id = ?4",
            params![format_iso8601(end_time_ms), summary, json, id],
        )
        .map_err(|e| format!("finish session: {e}"))?;
        Ok(())
    }

    /// Atualiza SÓ o resumo de uma sessão já encerrada — usado quando o resumo do LLM chega depois
    /// do `fallback_summary` síncrono que `end_session` já persistiu (ver o cabeçalho de
    /// `session_memory.rs`: `end_session` nunca bloqueia esperando o LLM).
    pub fn update_summary(&self, id: &str, summary: &str) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute("UPDATE game_sessions SET summary = ?1 WHERE id = ?2", params![summary, id])
            .map_err(|e| format!("update summary: {e}"))?;
        Ok(())
    }

    /// `rowid DESC` desempata: `start_time` tem resolução de milissegundo, e duas sessões abertas
    /// no mesmo milissegundo deixavam o SQLite livre para devolver qualquer uma das duas como "a
    /// mais recente". O `rowid` é a ordem de inserção, que é exatamente o que "mais recente"
    /// significa aqui. Sem isto, `session_for_game` devolvia a sessão errada de forma intermitente.
    pub fn history(&self, limit: i64) -> Result<Vec<GameSession>, String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT id, game_id, game_name, start_time, end_time, events, summary FROM game_sessions ORDER BY start_time DESC, rowid DESC LIMIT ?1")
            .map_err(|e| format!("history: {e}"))?;
        collect_sessions(&mut stmt, params![limit])
    }

    pub fn by_game_id(&self, game_id: &str) -> Result<Option<GameSession>, String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT id, game_id, game_name, start_time, end_time, events, summary FROM game_sessions WHERE game_id = ?1 ORDER BY start_time DESC, rowid DESC LIMIT 1")
            .map_err(|e| format!("by_game_id: {e}"))?;
        Ok(collect_sessions(&mut stmt, params![game_id])?.into_iter().next())
    }

    pub fn by_id(&self, id: &str) -> Result<Option<GameSession>, String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT id, game_id, game_name, start_time, end_time, events, summary FROM game_sessions WHERE id = ?1")
            .map_err(|e| format!("by_id: {e}"))?;
        Ok(collect_sessions(&mut stmt, params![id])?.into_iter().next())
    }

    /// Espelha `GameSessionMemory.clearAll` (TS): apaga AS DUAS tabelas do mesmo arquivo —
    /// `game_sessions` e `game_locations` — e soma as linhas removidas. `game_locations` não tem
    /// CRUD portado nesta rodada (ver o cabeçalho do arquivo), mas a tabela É deste módulo (criada
    /// por `from_connection` acima), então apagar as linhas dela não é escopo alheio.
    pub fn clear_all(&self) -> Result<usize, String> {
        let conn = self.lock()?;
        let sessions = conn.execute("DELETE FROM game_sessions", []).map_err(|e| format!("clear_all sessions: {e}"))?;
        let locations = conn.execute("DELETE FROM game_locations", []).map_err(|e| format!("clear_all locations: {e}"))?;
        Ok(sessions + locations)
    }
}

fn collect_sessions(stmt: &mut rusqlite::Statement<'_>, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<GameSession>, String> {
    let rows = stmt
        .query_map(args, |row| {
            let start_time: String = row.get(3)?;
            let end_time: Option<String> = row.get(4)?;
            let events_json: String = row.get(5)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                start_time,
                end_time,
                events_json,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|e| format!("query: {e}"))?;

    let mut sessions = Vec::new();
    for row in rows {
        let (id, game_id, game_name, start_time, end_time, events_json, summary) = row.map_err(|e| format!("row: {e}"))?;
        let Some(start_time_ms) = parse_iso8601(&start_time) else {
            log::warn!("game_sessions.start_time ilegível ({id}); linha descartada da listagem: {start_time}");
            continue;
        };
        let end_time_ms = end_time.as_deref().and_then(parse_iso8601);
        let events = parse_events_column(&events_json, &id);
        sessions.push(GameSession { id, game_id, game_name, start_time_ms, end_time_ms, events, summary });
    }
    Ok(sessions)
}

/// Porte de `session-memory-rows.ts::parseJsonColumn`: JSON ilegível vira lista vazia, nunca erro
/// (a sessão continua legível sem os eventos), com aviso — mesma política da origem.
fn parse_events_column(raw: &str, session_id: &str) -> Vec<GameEvent> {
    if raw.is_empty() {
        return Vec::new();
    }
    match serde_json::from_str::<Vec<GameEvent>>(raw) {
        Ok(events) => events,
        Err(error) => {
            log::warn!("game_sessions.events ilegível ({session_id}): {error}; linha lida sem eventos");
            Vec::new()
        }
    }
}

fn apply_pragmas(conn: &Connection) -> Result<(), String> {
    conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| format!("pragma journal_mode: {e}"))?;
    conn.pragma_update(None, "synchronous", "NORMAL").map_err(|e| format!("pragma synchronous: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(5000)).map_err(|e| format!("busy_timeout: {e}"))?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(|e| format!("pragma foreign_keys: {e}"))?;
    Ok(())
}

/// `<app_data_dir>/plugins/mika.gamer-companion/gamer-sessions.db` — ver o cabeçalho do arquivo
/// para a cadeia de resolução que prova a paridade com o caminho que o TS já abre.
pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?;
    Ok(data_dir.join("plugins").join("mika.gamer-companion").join("gamer-sessions.db"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-gamer-session-db-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn session(id: &str, game_id: &str, start_ms: u64) -> GameSession {
        GameSession {
            id: id.into(),
            game_id: game_id.into(),
            game_name: "Hades".into(),
            start_time_ms: start_ms,
            end_time_ms: None,
            events: vec![],
            summary: None,
        }
    }

    #[test]
    fn insert_then_history_round_trips() {
        let db = SessionDb::open_in_memory().unwrap();
        db.insert(&session("s1", "g1", 1_000)).unwrap();
        let history = db.history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, "s1");
        assert_eq!(history[0].start_time_ms, 1_000);
        assert_eq!(history[0].end_time_ms, None);
    }

    #[test]
    fn finish_persists_end_time_summary_and_events() {
        let db = SessionDb::open_in_memory().unwrap();
        db.insert(&session("s1", "g1", 0)).unwrap();
        let events = vec![GameEvent::new(500, "death", "morreu pro chefe", None)];
        db.finish("s1", 60_000, Some("resumo"), &events).unwrap();

        let found = db.by_id("s1").unwrap().unwrap();
        assert_eq!(found.end_time_ms, Some(60_000));
        assert_eq!(found.summary.as_deref(), Some("resumo"));
        assert_eq!(found.events.len(), 1);
        assert_eq!(found.events[0].description, "morreu pro chefe");
    }

    #[test]
    fn history_orders_most_recent_start_first() {
        let db = SessionDb::open_in_memory().unwrap();
        db.insert(&session("older", "g1", 1_000)).unwrap();
        db.insert(&session("newer", "g1", 2_000)).unwrap();
        let history = db.history(10).unwrap();
        assert_eq!(history[0].id, "newer");
        assert_eq!(history[1].id, "older");
    }

    #[test]
    fn by_game_id_returns_the_most_recent_session_for_that_game() {
        let db = SessionDb::open_in_memory().unwrap();
        db.insert(&session("s1", "same-game", 1_000)).unwrap();
        db.insert(&session("s2", "same-game", 2_000)).unwrap();
        db.insert(&session("s3", "other-game", 3_000)).unwrap();

        let found = db.by_game_id("same-game").unwrap().unwrap();
        assert_eq!(found.id, "s2");
    }

    #[test]
    fn clear_all_removes_every_session_and_reports_the_count() {
        let db = SessionDb::open_in_memory().unwrap();
        db.insert(&session("s1", "g1", 0)).unwrap();
        db.insert(&session("s2", "g2", 0)).unwrap();
        assert_eq!(db.clear_all().unwrap(), 2);
        assert!(db.history(10).unwrap().is_empty());
    }

    /// `game_locations` não tem CRUD portado, mas a tabela é deste módulo — `clear_all` apaga as
    /// duas, mesmo efeito de `GameSessionMemory.clearAll` (TS) sobre o mesmo arquivo.
    #[test]
    fn clear_all_also_empties_game_locations_and_counts_both_tables() {
        let db = SessionDb::open_in_memory().unwrap();
        db.insert(&session("s1", "g1", 0)).unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO game_locations (id, game_name, location_name, description, created_at) VALUES ('l1', 'Hades', 'Tartarus', 'início', '2026-09-20T00:00:00.000Z')",
                [],
            )
            .unwrap();
        }

        assert_eq!(db.clear_all().unwrap(), 2, "1 sessão + 1 localização");
        assert!(db.history(10).unwrap().is_empty());
        let remaining: i64 =
            db.conn.lock().unwrap().query_row("SELECT COUNT(*) FROM game_locations", [], |row| row.get(0)).unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn an_unreadable_events_column_falls_back_to_an_empty_list_instead_of_failing_the_read() {
        let db = SessionDb::open_in_memory().unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO game_sessions (id, game_id, game_name, start_time, events) VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["broken", "g1", "Jogo", format_iso8601(0), "isto não é json"],
            )
            .unwrap();
        }
        let found = db.by_id("broken").unwrap().unwrap();
        assert!(found.events.is_empty());
    }

    #[test]
    fn reopening_the_same_file_preserves_sessions_across_restarts() {
        let dir = TempDir::new();
        let path = dir.0.join("gamer-sessions.db");
        {
            let db = SessionDb::open(&path).unwrap();
            db.insert(&session("s1", "g1", 5_000)).unwrap();
        }
        {
            let db = SessionDb::open(&path).unwrap();
            let found = db.by_id("s1").unwrap().unwrap();
            assert_eq!(found.start_time_ms, 5_000);
        }
    }

    #[test]
    fn opening_a_corrupted_file_fails_loud_instead_of_returning_empty() {
        let dir = TempDir::new();
        let path = dir.0.join("gamer-sessions.db");
        std::fs::write(&path, b"isto nao e um banco sqlite").unwrap();
        assert!(SessionDb::open(&path).is_err());
    }

    /// Prova de compatibilidade: uma linha gravada no formato EXATO que o TypeScript já grava
    /// (`toISOString()`, sem o helper deste módulo) tem de ler certo — não só o par
    /// format/parse deste arquivo testando a si mesmo.
    #[test]
    fn reads_a_row_written_in_the_literal_typescript_toisostring_format() {
        let db = SessionDb::open_in_memory().unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO game_sessions (id, game_id, game_name, start_time, end_time, events, summary) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "legacy-ts-row",
                    "570",
                    "Dota 2",
                    "2026-09-20T18:00:00.000Z",
                    "2026-09-20T19:30:00.000Z",
                    r#"[{"timestamp":"2026-09-20T18:15:00.000Z","type":"death","description":"morreu na rosh"}]"#,
                    "Session: 90min, 1 events (death).",
                ],
            )
            .unwrap();
        }
        let found = db.by_id("legacy-ts-row").unwrap().unwrap();
        assert_eq!(found.game_name, "Dota 2");
        assert_eq!(found.duration_ms(), Some(90 * 60_000));
        assert_eq!(found.events.len(), 1);
        assert_eq!(found.events[0].kind, "death");
        assert_eq!(found.events[0].description, "morreu na rosh");
    }
}
