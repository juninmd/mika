//! Núcleo do chokepoint de ESCRITA que `modules::chat::append_message` expõe fora do módulo —
//! porte do par (`role: "user"`, `role: "assistant"`) que
//! `chat-generate.runtime.ts::runGenerateRaw` grava sempre que `saveToHistory !== false`. Chamam isto
//! `brain::tools::collaboration_session` (cada turno do debate) e `HistoryStore::push`, da mesma forma que
//! `chat_recap.rs::recent_messages` é a metade de LEITURA que só `brain::tools::content` chama.
//!
//! **Por que retentativa em vez de um `seq` calculado uma vez.** `HistoryStore` (o escritor real de
//! `mika:chat-send`) mantém seu PRÓPRIO `next_seq` em memória, sob um mutex que `chat_db.rs` nunca
//! vê — este arquivo abre uma `ChatDb` SEPARADA sobre o MESMO arquivo (mesmo padrão de
//! `chat_recap.rs`, "WAL torna leitura concorrente segura"; aqui é ESCRITA concorrente). As duas
//! conexões podem calcular o mesmo `seq` livre ao mesmo tempo; `idx_messages_seq`
//! (`UNIQUE(session_id, seq)`, `chat_db.rs`) rejeita a segunda gravação com esse `seq`, e é esse
//! erro específico que aciona a retentativa — não uma falha genérica engolida.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use uuid::Uuid;

use super::chat_db::ChatDb;
use super::history::ChatMessage;

const MAX_SEQ_RETRIES: u32 = 5;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Grava `role`/`content` na sessão "atual" com `id`/`timestamp` gerados aqui — mesmo par que o TS
/// grava com `randomUUID()`/`Date.now()` a cada `generateRaw({ saveToHistory: true })`.
/// Devolve a mensagem gravada, para quem escreveu espelhá-la na memória do processo
/// (`HistoryStore::mirror_external`).
pub(super) fn append_message(db: &ChatDb, session_id: &str, role: &str, content: &str) -> Result<ChatMessage, String> {
    let message = ChatMessage { id: Uuid::new_v4().to_string(), role: role.to_string(), content: content.to_string(), timestamp_ms: now_ms() };
    append_with_retry(db, session_id, &message)?;
    Ok(message)
}

/// Devolve o `seq` gravado.
pub(super) fn append_with_retry(db: &ChatDb, session_id: &str, message: &ChatMessage) -> Result<i64, String> {
    for attempt in 0..MAX_SEQ_RETRIES {
        let seq = next_seq(db, session_id)?;
        match db.append(session_id, seq, message) {
            Ok(()) => return Ok(seq),
            Err(error) if is_seq_conflict(&error) && attempt + 1 < MAX_SEQ_RETRIES => continue,
            Err(error) => return Err(error),
        }
    }
    Err(format!("append_message: não conseguiu um seq livre para a sessão {session_id} após {MAX_SEQ_RETRIES} tentativas"))
}

fn is_seq_conflict(error: &str) -> bool {
    error.contains("UNIQUE constraint failed")
}

/// Mesma consulta `MAX(seq)+1` de `chat_db.rs::restore`, sem carregar as mensagens — este
/// chokepoint só precisa do próximo `seq` livre, não do histórico inteiro.
fn next_seq(db: &ChatDb, session_id: &str) -> Result<i64, String> {
    let conn = db.lock()?;
    let next = conn
        .query_row("SELECT MAX(seq) FROM messages WHERE session_id = ?1", params![session_id], |row| row.get::<_, Option<i64>>(0))
        .map_err(|e| format!("next_seq: {e}"))?
        .map(|max| max + 1)
        .unwrap_or(0);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_message_generates_id_and_timestamp_and_writes_the_given_role_and_content() {
        let db = ChatDb::open_in_memory().unwrap();
        let session_id = db.current_session_id().unwrap();

        append_message(&db, &session_id, "user", "topico do debate").unwrap();

        let (messages, next) = db.restore(&session_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "topico do debate");
        assert!(!messages[0].id.is_empty());
        assert_eq!(next, 1);
    }

    #[test]
    fn two_calls_in_a_row_land_on_consecutive_seq_without_colliding() {
        let db = ChatDb::open_in_memory().unwrap();
        let session_id = db.current_session_id().unwrap();

        append_message(&db, &session_id, "user", "primeira").unwrap();
        append_message(&db, &session_id, "assistant", "segunda").unwrap();

        let (messages, _) = db.restore(&session_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "primeira");
        assert_eq!(messages[1].content, "segunda");
    }

    /// Prova a retentativa de verdade: duas `ChatDb` DISTINTAS sobre o MESMO arquivo (mesmo desenho
    /// de `HistoryStore` + este chokepoint concorrendo pelo mesmo `chat.db`) — sem a retentativa, a
    /// segunda gravação bateria de frente no `seq` que a primeira acabou de ocupar e falharia com
    /// `UNIQUE constraint failed`.
    #[test]
    fn concurrent_connections_to_the_same_file_never_collide_on_seq() {
        let dir_path = std::env::temp_dir().join(format!("mika-chat-write-retry-test-{}", std::process::id()));
        // PIDs get reused on Windows: a stale chat.db from an earlier run would add its messages.
        let _ = std::fs::remove_dir_all(&dir_path);
        std::fs::create_dir_all(&dir_path).unwrap();
        let path = dir_path.join("chat.db");

        let db_a = ChatDb::open(&path).unwrap();
        let db_b = ChatDb::open(&path).unwrap();
        let session_id = db_a.current_session_id().unwrap();

        // Ambas leem `seq=0` como livre ANTES de qualquer uma escrever — a corrida que a
        // retentativa existe para resolver.
        let seq_a = next_seq(&db_a, &session_id).unwrap();
        let seq_b = next_seq(&db_b, &session_id).unwrap();
        assert_eq!(seq_a, seq_b, "pré-condição do teste: as duas deviam ver o mesmo seq livre");

        db_a.append(&session_id, seq_a, &ChatMessage { id: "a".into(), role: "user".into(), content: "de a".into(), timestamp_ms: 1 }).unwrap();

        // `db_b` tentaria o MESMO seq: sem a retentativa isto seria um erro de UNIQUE constraint.
        append_message(&db_b, &session_id, "assistant", "de b").unwrap();

        let (messages, _) = db_a.restore(&session_id).unwrap();
        assert_eq!(messages.len(), 2);

        drop((db_a, db_b));
        std::fs::remove_dir_all(&dir_path).unwrap();
    }
}
