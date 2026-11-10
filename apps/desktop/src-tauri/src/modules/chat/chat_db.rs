//! Persistência SQLite do histórico de chat — porte de `messages-table.ts`
//! (`apps/desktop/src/main/services/chat/messages-table.ts`), schema campo a campo.
//!
//! **`rusqlite`, não `libsql`** (`docs/tauri-rust-migration.md` §1, decisão aberta): conferido que
//! `messages-table.ts` só usa DDL/DML padrão (`CREATE TABLE`, `CREATE UNIQUE INDEX`,
//! `ALTER TABLE ADD COLUMN`, `SELECT MAX`, `INSERT ... ON CONFLICT DO UPDATE`, `DELETE`) — nenhuma
//! função só-libSQL, mesma verificação que `modules/memory.rs` já documentou para `memory.db`. O
//! arquivo que `bootstrapChat` abre hoje (`db.getDatabase('chat.db', 'plugins/mika.chat')`) é
//! SQLite puro em modo `file:` do libSQL — byte compatível com `rusqlite`. `libsql` (crate Rust)
//! segue em pré-lançamento (`0.10.0-pre.4`); sem motivo para arriscar essa API instável aqui.
//!
//! **Mesmo arquivo, não um novo.** `db_path` reproduz `<app_data_dir>/plugins/mika.chat/chat.db` —
//! o caminho exato que `bootstrapChat` já abre. `CREATE ... IF NOT EXISTS`/`ADD COLUMN` idempotente
//! (mesmo truque de `addColumnIfMissing`) preservam as linhas que o usuário já tem. Os 3 canais
//! `mika:chat-*` que tocam histórico já são todos resolvidos aqui, nunca mais no sidecar
//! (`bridge.rs`) — não há dois processos Rust/Node escrevendo a MESMA sessão ao mesmo tempo.
//!
//! **Sessão "atual":** ver `session_adoption.rs` — resolvida uma vez, gravada em `chat_current_session` (tabela própria, nunca escrita pelo TypeScript).
//!
//! **Banco corrompido falha alto:** `open` propaga o erro do SQLite (ex.: "file is not a
//! database") em vez de devolver um `ChatDb` que mais tarde parece "conversa vazia" — histórico
//! vazio é indistinguível de conversa nova, e é exatamente essa confusão que não se pode introduzir.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::AppHandle;

use super::history::ChatMessage;
use super::session_adoption;
use crate::modules::sqlite_migrate::migrate;

const CREATE_MESSAGES: &str = "
    CREATE TABLE IF NOT EXISTS messages (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        seq INTEGER NOT NULL,
        role TEXT NOT NULL,
        content TEXT NOT NULL,
        tool_call_id TEXT,
        tool_name TEXT,
        tokens INTEGER,
        created_at INTEGER NOT NULL,
        source TEXT
    )
";

const CREATE_MESSAGES_INDEX: &str = "CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_seq ON messages(session_id, seq)";

const CREATE_CURRENT_SESSION: &str = "
    CREATE TABLE IF NOT EXISTS chat_current_session (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        session_id TEXT NOT NULL
    )
";

// 04-memory.md §"Curto prazo", MUST: "o resumo rolante é gerado por LLM e guardado, não
// recalculado a cada turno" — uma linha por sessão, `summarized_through` marca até qual índice
// (0-based, exclusivo) da janela completa de mensagens o resumo já cobre, para
// `HistoryStore::overflow_for_summary` só mandar ao LLM o que ainda não foi resumido.
const CREATE_ROLLING_SUMMARY: &str = "
    CREATE TABLE IF NOT EXISTS chat_rolling_summary (
        session_id TEXT PRIMARY KEY,
        summary TEXT NOT NULL,
        summarized_through INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    )
";

// `ALTER TABLE ... ADD COLUMN` não tem `IF NOT EXISTS` no SQLite: `migrate` tolera "duplicate column"
// nesse statement (banco legado `user_version = 0` que já tem `source`). Parte da migração 1.
const ADD_SOURCE_COLUMN: &str = "ALTER TABLE messages ADD COLUMN source TEXT";

// Migração 2: `summarized_through` passou a contar só mensagens user/assistant (as que
// `to_turn_history` entrega ao modelo). O valor gravado antes contava as mensagens CRUAS, `system`
// (turno que falhou) incluído — lido como está, o recorte deslocava e o resumo cobria a mensagem
// errada. Converte uma única vez: conversacionais entre as primeiras `summarized_through` cruas.
const MIGRATE_SUMMARIZED_THROUGH_TO_CONVERSATIONAL: &str = "
    UPDATE chat_rolling_summary SET summarized_through = (
        SELECT COUNT(*) FROM messages m
        WHERE m.session_id = chat_rolling_summary.session_id
          AND m.role IN ('user', 'assistant')
          AND (SELECT COUNT(*) FROM messages p WHERE p.session_id = m.session_id AND p.seq < m.seq) < chat_rolling_summary.summarized_through
    )
";

pub struct ChatDb {
    conn: Mutex<Connection>,
}

impl ChatDb {
    /// Abre (ou cria) `chat.db` em `path`, garante o diretório pai e o schema — idempotente e
    /// reversível, igual a `modules::memory::MemoryStore::open`.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("não criou {}: {e}", parent.display()))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("não abriu {}: {e}", path.display()))?;
        Self::from_connection(conn)
    }

    /// Banco efêmero em memória — só para teste (ver `HistoryStore::attach_for_test`).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("não abriu :memory:: {e}"))?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Result<Self, String> {
        apply_pragmas(&conn)?;
        let migration_1 = [CREATE_MESSAGES, CREATE_MESSAGES_INDEX, ADD_SOURCE_COLUMN, CREATE_CURRENT_SESSION, CREATE_ROLLING_SUMMARY].join(";\n");
        migrate(&conn, &[&migration_1, MIGRATE_SUMMARIZED_THROUGH_TO_CONVERSATIONAL]).map_err(|e| format!("chat.db: {e}"))?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// `pub(super)`: `chat_recap.rs` reusa esta conexão — ver o cabeçalho de lá.
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|_| "banco de chat envenenado".to_string())
    }

    /// A sessão "atual" — resolvida e gravada uma única vez (ver `session_adoption.rs`); toda
    /// chamada seguinte só lê `chat_current_session`.
    pub fn current_session_id(&self) -> Result<String, String> {
        let conn = self.lock()?;
        if let Some(id) = read_current_session_id(&conn)? {
            return Ok(id);
        }
        let adopted = session_adoption::most_recent_ts_session(&conn)?.unwrap_or_else(session_adoption::new_session_id);
        // `OR IGNORE` + releitura: se duas aberturas concorrentes do MESMO arquivo caírem aqui ao
        // mesmo tempo (ver `HistoryStore::ensure_persistence`), só a primeira grava — a segunda lê
        // de volta o valor que venceu, nunca duas sessões "atuais" divergentes.
        conn.execute("INSERT OR IGNORE INTO chat_current_session (id, session_id) VALUES (1, ?1)", params![adopted])
            .map_err(|e| format!("current_session_id: {e}"))?;
        read_current_session_id(&conn)?.ok_or_else(|| "current_session_id: não gravou".to_string())
    }

    /// Mensagens na ordem de `seq`, e o próximo `seq` livre — porte de `nextSeqFor` (`MAX(seq)+1`,
    /// ou `0` para sessão vazia).
    pub fn restore(&self, session_id: &str) -> Result<(Vec<ChatMessage>, i64), String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT id, role, content, created_at FROM messages WHERE session_id = ?1 ORDER BY seq ASC")
            .map_err(|e| format!("restore: {e}"))?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok(ChatMessage {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    timestamp_ms: row.get::<_, i64>(3)? as u64,
                })
            })
            .map_err(|e| format!("restore: {e}"))?;
        let messages = rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("restore: {e}"))?;

        let next_seq = conn
            .query_row("SELECT MAX(seq) FROM messages WHERE session_id = ?1", params![session_id], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .map_err(|e| format!("restore: {e}"))?
            .map(|max| max + 1)
            .unwrap_or(0);

        Ok((messages, next_seq))
    }

    /// Grava uma mensagem em `seq`. `HistoryStore` mantém `seq` sob o MESMO lock do array em
    /// memória — duas chamadas concorrentes nunca disputam `seq` (teste em `history_store.rs`).
    pub fn append(&self, session_id: &str, seq: i64, message: &ChatMessage) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO messages (id, session_id, seq, role, content, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(id) DO UPDATE SET content = excluded.content",
            params![message.id, session_id, seq, message.role, message.content, message.timestamp_ms as i64],
        )
        .map_err(|e| format!("append: {e}"))?;
        Ok(())
    }

    /// Porte de `clearMessageRows(db, sessionId)`, mais o resumo rolante da sessão — o resumo
    /// descreve exatamente as mensagens apagadas, então sai na MESMA transação (nunca um sem o
    /// outro). Devolve quantas mensagens saíram (ver `history_store.rs`).
    pub fn clear_session(&self, session_id: &str) -> Result<usize, String> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|e| format!("clear_session: {e}"))?;
        tx.execute("DELETE FROM chat_rolling_summary WHERE session_id = ?1", params![session_id]).map_err(|e| format!("clear_session: {e}"))?;
        let removed = tx.execute("DELETE FROM messages WHERE session_id = ?1", params![session_id]).map_err(|e| format!("clear_session: {e}"))?;
        tx.commit().map_err(|e| format!("clear_session: {e}"))?;
        Ok(removed)
    }

    /// "Apagar tudo": todas as sessões, não só a corrente. Quem migrou da versão TS tem várias
    /// sessões em `chat.db`, e apagar só a corrente deixava as outras (com o texto das conversas)
    /// no disco enquanto a tool respondia sucesso. A sessão corrente continua existindo, vazia.
    pub fn clear_all(&self) -> Result<usize, String> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|e| format!("clear_all: {e}"))?;
        tx.execute("DELETE FROM chat_rolling_summary", []).map_err(|e| format!("clear_all: {e}"))?;
        let removed = tx.execute("DELETE FROM messages", []).map_err(|e| format!("clear_all: {e}"))?;
        tx.commit().map_err(|e| format!("clear_all: {e}"))?;
        Ok(removed)
    }

    /// Resumo rolante persistido + até onde ele cobre — `None` quando a sessão nunca transbordou o
    /// `historyLimit` (04-memory.md §"Curto prazo").
    pub fn rolling_summary(&self, session_id: &str) -> Result<Option<(String, usize)>, String> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT summary, summarized_through FROM chat_rolling_summary WHERE session_id = ?1",
            params![session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize)),
        )
        .optional()
        .map_err(|e| format!("rolling_summary: {e}"))
    }

    /// `INSERT ... ON CONFLICT DO UPDATE`: uma linha por sessão, sempre a versão mais recente.
    pub fn set_rolling_summary(&self, session_id: &str, summary: &str, summarized_through: usize, updated_at_ms: u64) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO chat_rolling_summary (session_id, summary, summarized_through, updated_at) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(session_id) DO UPDATE SET summary = excluded.summary, summarized_through = excluded.summarized_through, updated_at = excluded.updated_at",
            params![session_id, summary, summarized_through as i64, updated_at_ms as i64],
        )
        .map_err(|e| format!("set_rolling_summary: {e}"))?;
        Ok(())
    }
}

fn read_current_session_id(conn: &Connection) -> Result<Option<String>, String> {
    conn.query_row("SELECT session_id FROM chat_current_session WHERE id = 1", [], |row| row.get::<_, String>(0))
        .optional()
        .map_err(|e| format!("read_current_session_id: {e}"))
}

/// WAL + `synchronous=NORMAL` + `busy_timeout` + `foreign_keys=ON` — os mesmos pragmas que
/// `modules::memory::MemoryStore` já usa para o outro banco rusqlite desta árvore, e que
/// `packages/sdk/src/services/database.ts::applyPragmas` aplica do lado TypeScript.
fn apply_pragmas(conn: &Connection) -> Result<(), String> {
    conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| format!("pragma journal_mode: {e}"))?;
    conn.pragma_update(None, "synchronous", "NORMAL").map_err(|e| format!("pragma synchronous: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(5000)).map_err(|e| format!("busy_timeout: {e}"))?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(|e| format!("pragma foreign_keys: {e}"))?;
    Ok(())
}

/// `<app_data_dir>/plugins/mika.chat/chat.db` — o mesmo caminho que `bootstrapChat` abre hoje.
pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?;
    Ok(data_dir.join("plugins").join("mika.chat").join("chat.db"))
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
            let path = std::env::temp_dir().join(format!("mika-chat-db-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn message(role: &str, content: &str) -> ChatMessage {
        ChatMessage { id: format!("{role}-{content}"), role: role.to_string(), content: content.to_string(), timestamp_ms: 1 }
    }

    #[test]
    fn current_session_id_is_stable_across_calls() {
        let db = ChatDb::open_in_memory().unwrap();
        let first = db.current_session_id().unwrap();
        let second = db.current_session_id().unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn current_session_id_adopts_the_most_recent_ts_session() {
        let db = ChatDb::open_in_memory().unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY, data TEXT NOT NULL)", []).unwrap();
            conn.execute("INSERT INTO sessions (id, data) VALUES ('ts-session', '{\"updatedAt\": 999}')", []).unwrap();
        }
        assert_eq!(db.current_session_id().unwrap(), "ts-session");
    }

    #[test]
    fn append_then_restore_round_trips_in_seq_order() {
        let db = ChatDb::open_in_memory().unwrap();
        let session_id = db.current_session_id().unwrap();
        db.append(&session_id, 0, &message("user", "oi")).unwrap();
        db.append(&session_id, 1, &message("assistant", "oi, tudo bem?")).unwrap();

        let (messages, next_seq) = db.restore(&session_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "oi");
        assert_eq!(messages[1].content, "oi, tudo bem?");
        assert_eq!(next_seq, 2);
    }

    #[test]
    fn restore_on_empty_session_starts_seq_at_zero() {
        let db = ChatDb::open_in_memory().unwrap();
        let (messages, next_seq) = db.restore("nunca-usada").unwrap();
        assert!(messages.is_empty());
        assert_eq!(next_seq, 0);
    }

    #[test]
    fn clear_all_removes_every_session_and_every_summary() {
        let db = ChatDb::open_in_memory().unwrap();
        db.append("a", 0, &message("user", "de a")).unwrap();
        db.append("b", 0, &message("user", "de b")).unwrap();
        db.set_rolling_summary("b", "resumo de b", 1, 0).unwrap();

        assert_eq!(db.clear_all().unwrap(), 2);
        assert!(db.restore("a").unwrap().0.is_empty());
        assert!(db.restore("b").unwrap().0.is_empty());
        assert_eq!(db.rolling_summary("b").unwrap(), None);
    }

    #[test]
    fn clear_session_removes_only_that_session() {
        let db = ChatDb::open_in_memory().unwrap();
        db.append("a", 0, &message("user", "de a")).unwrap();
        db.append("b", 0, &message("user", "de b")).unwrap();

        let removed = db.clear_session("a").unwrap();
        assert_eq!(removed, 1);
        assert!(db.restore("a").unwrap().0.is_empty());
        assert_eq!(db.restore("b").unwrap().0.len(), 1);
    }

    #[test]
    fn reopening_the_same_file_preserves_messages_and_the_current_session() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        let session_id;

        {
            let db = ChatDb::open(&path).unwrap();
            session_id = db.current_session_id().unwrap();
            db.append(&session_id, 0, &message("user", "sobrevive ao restart")).unwrap();
        }
        {
            let db = ChatDb::open(&path).unwrap();
            assert_eq!(db.current_session_id().unwrap(), session_id);
            let (messages, _) = db.restore(&session_id).unwrap();
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].content, "sobrevive ao restart");
        }
    }

    /// Banco gravado ANTES do versionamento: `user_version = 0` e `messages` sem a coluna `source`.
    #[test]
    fn legacy_unversioned_file_migrates_without_losing_messages() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE messages (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, seq INTEGER NOT NULL, role TEXT NOT NULL,
                    content TEXT NOT NULL, tool_call_id TEXT, tool_name TEXT, tokens INTEGER, created_at INTEGER NOT NULL);
                 INSERT INTO messages (id, session_id, seq, role, content, created_at) VALUES ('m1', 's1', 0, 'user', 'do passado', 1);",
            )
            .unwrap();
        }

        let db = ChatDb::open(&path).unwrap();
        let (messages, next_seq) = db.restore("s1").unwrap();
        assert_eq!((messages.len(), next_seq), (1, 1));
        assert_eq!(messages[0].content, "do passado");
        db.append("s1", 1, &message("assistant", "usa a coluna nova")).unwrap();
        let version: i64 = db.lock().unwrap().pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, 2, "migração 1 (schema) + 2 (contagem do resumo rolante)");
    }

    #[test]
    fn opening_a_file_from_a_newer_schema_is_refused() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        Connection::open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap();

        let err = ChatDb::open(&path).err().expect("deveria recusar");
        assert!(err.contains("chat.db") && err.contains("atualize"), "{err}");
    }

    #[test]
    fn opening_a_corrupted_file_fails_loud_instead_of_returning_empty() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        std::fs::write(&path, b"isto nao e um banco sqlite").unwrap();

        assert!(ChatDb::open(&path).is_err());
    }

    /// `summarized_through` gravado antes de a contagem passar a ser só user/assistant contava as
    /// mensagens CRUAS (inclusive `system`, o turno que falhou): lido como está, o recorte deslocava e
    /// resumia a mensagem errada. A migração v2 converte a linha existente uma única vez.
    #[test]
    fn a_legacy_raw_summarized_through_is_converted_to_a_conversational_count() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        {
            let conn = Connection::open(&path).unwrap();
            let migration_1 = [CREATE_MESSAGES, CREATE_MESSAGES_INDEX, ADD_SOURCE_COLUMN, CREATE_CURRENT_SESSION, CREATE_ROLLING_SUMMARY].join(";\n");
            migrate(&conn, &[&migration_1]).unwrap();
            for (seq, role) in ["user", "system", "assistant", "system", "user", "assistant"].iter().enumerate() {
                conn.execute(
                    "INSERT INTO messages (id, session_id, seq, role, content, created_at) VALUES (?1, 's1', ?2, ?3, 'x', 1)",
                    params![format!("m{seq}"), seq as i64, role],
                )
                .unwrap();
            }
            // as 4 primeiras mensagens cruas (user, system, assistant, system) = 2 conversacionais
            conn.execute("INSERT INTO chat_rolling_summary (session_id, summary, summarized_through, updated_at) VALUES ('s1', 'resumo', 4, 1)", []).unwrap();
        }

        let db = ChatDb::open(&path).unwrap();
        assert_eq!(db.rolling_summary("s1").unwrap(), Some(("resumo".to_string(), 2)));

        // reabrir não converte de novo (já é contagem conversacional)
        drop(db);
        let db = ChatDb::open(&path).unwrap();
        assert_eq!(db.rolling_summary("s1").unwrap(), Some(("resumo".to_string(), 2)));
    }
}
