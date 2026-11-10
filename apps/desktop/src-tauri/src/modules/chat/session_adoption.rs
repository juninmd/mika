//! Adoção, uma única vez, da sessão "atual" que o TypeScript já tinha antes deste porte — ver o
//! porquê completo no cabeçalho de `chat_db.rs`. Fica em arquivo próprio porque ler a tabela
//! `sessions` do `PluginStore` (`packages/sdk/src/plugins/plugin-store.ts`) é uma preocupação à
//! parte do resto da persistência: só roda uma vez, na primeira leitura do arquivo.

use rusqlite::Connection;
use serde_json::Value;
use uuid::Uuid;

/// Tabela de sessões do TypeScript (`PluginStore`, schema `id TEXT PRIMARY KEY, data TEXT NOT
/// NULL`, `data` = JSON de `SessionMetadata`). Só leitura — nunca escrita por este módulo, pela
/// mesma razão que `history.rs` documenta para `session-manager.ts` não ter sido portado: gravar
/// aqui arriscaria os dois lados divergindo sobre o mesmo dado.
const TS_SESSIONS_TABLE: &str = "sessions";

/// A sessão que `session-manager.ts::first()`/`getLatestAfterDelete()` escolheria no boot do
/// sidecar: a de maior `updatedAt`. `None` se a tabela não existir (app que nunca rodou a versão
/// Electron) ou não tiver nenhuma linha decodificável.
pub fn most_recent_ts_session(conn: &Connection) -> Result<Option<String>, String> {
    if !table_exists(conn, TS_SESSIONS_TABLE)? {
        return Ok(None);
    }

    let mut stmt = conn
        .prepare(&format!("SELECT id, data FROM {TS_SESSIONS_TABLE}"))
        .map_err(|e| format!("most_recent_ts_session: {e}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| format!("most_recent_ts_session: {e}"))?;

    let mut best: Option<(String, f64)> = None;
    for row in rows {
        let (id, data) = row.map_err(|e| format!("most_recent_ts_session: {e}"))?;
        let Ok(parsed) = serde_json::from_str::<Value>(&data) else { continue };
        let updated_at = parsed.get("updatedAt").and_then(Value::as_f64).unwrap_or(0.0);
        let better = match &best {
            Some((_, current)) => updated_at > *current,
            None => true,
        };
        if better {
            best = Some((id, updated_at));
        }
    }
    Ok(best.map(|(id, _)| id))
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |row| {
        row.get::<_, i64>(0)
    })
    .map(|count| count > 0)
    .map_err(|e| format!("table_exists({name}): {e}"))
}

/// Sessão nova, para um app que nunca teve `sessions` do TypeScript — mesmo formato de id
/// (`randomUUID()`) que `session-manager.ts::create()` usa.
pub fn new_session_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn no_sessions_table_returns_none() {
        assert_eq!(most_recent_ts_session(&conn()).unwrap(), None);
    }

    #[test]
    fn picks_the_session_with_the_highest_updated_at() {
        let conn = conn();
        conn.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY, data TEXT NOT NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id, data) VALUES ('old', '{\"updatedAt\": 100}')", []).unwrap();
        conn.execute("INSERT INTO sessions (id, data) VALUES ('new', '{\"updatedAt\": 200}')", []).unwrap();
        assert_eq!(most_recent_ts_session(&conn).unwrap(), Some("new".to_string()));
    }

    #[test]
    fn skips_rows_with_unparseable_json() {
        let conn = conn();
        conn.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY, data TEXT NOT NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id, data) VALUES ('broken', 'not json')", []).unwrap();
        conn.execute("INSERT INTO sessions (id, data) VALUES ('ok', '{\"updatedAt\": 5}')", []).unwrap();
        assert_eq!(most_recent_ts_session(&conn).unwrap(), Some("ok".to_string()));
    }

    #[test]
    fn new_session_id_is_a_valid_uuid() {
        let id = new_session_id();
        assert!(Uuid::parse_str(&id).is_ok());
    }
}
