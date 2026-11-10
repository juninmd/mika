//! Leitura BOUNDED de "as últimas N mensagens" — porte do `.slice(-50)` que
//! `ContentPipelineService.generateDailyRecap` aplica sobre `HistoryService.getAll()`
//! (`content-pipeline.service.ts`). Reaproveita a MESMA conexão de [`super::chat_db::ChatDb`]
//! (`chat_db.rs::lock`, visibilidade elevada para `pub(super)` só para este arquivo poder chamar)
//! em vez de abrir uma segunda `Connection` para o mesmo arquivo — mesmo precedente de
//! `gamer::session_db.rs::lock` + `game_locations.rs`. Arquivo separado só para manter `chat_db.rs`
//! (o schema em si) abaixo do limite de 300 linhas (`AGENTS.md` §3).
//!
//! Consumido por `modules::chat::recent_messages`, o chokepoint de leitura que
//! `brain::tools::content` (daily recap) chama de fora do módulo `chat`.

use rusqlite::params;

use super::chat_db::ChatDb;
use super::history::ChatMessage;

/// Últimas `limit` mensagens da sessão, em ordem cronológica. BOUNDED de propósito: um recap sobre
/// uma sessão longa nunca lê a tabela inteira, só as últimas `limit` linhas.
///
/// `seq DESC, rowid DESC`: `idx_messages_seq` já garante `seq` único POR SESSÃO (`UNIQUE INDEX
/// (session_id, seq)`, `chat_db.rs`), então duas linhas desta sessão nunca empatam em `seq` de
/// verdade — ao contrário de `session_db.rs::history`, que desempata `start_time` (não-único,
/// resolução de milissegundo) por `rowid`. O `rowid DESC` aqui é defesa em profundidade seguindo o
/// mesmo padrão, não a correção de uma colisão possível neste schema.
pub fn recent_messages(db: &ChatDb, session_id: &str, limit: i64) -> Result<Vec<ChatMessage>, String> {
    let conn = db.lock()?;
    let mut stmt = conn
        .prepare("SELECT id, role, content, created_at FROM messages WHERE session_id = ?1 ORDER BY seq DESC, rowid DESC LIMIT ?2")
        .map_err(|e| format!("recent_messages: {e}"))?;
    let rows = stmt
        .query_map(params![session_id, limit], |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                timestamp_ms: row.get::<_, i64>(3)? as u64,
            })
        })
        .map_err(|e| format!("recent_messages: {e}"))?;
    let mut messages = rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("recent_messages: {e}"))?;
    // `DESC` trouxe as mais recentes primeiro; devolve em ordem cronológica, igual ao
    // `.slice(-50)` do TS (que preserva a ordem original do array).
    messages.reverse();
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &str, content: &str) -> ChatMessage {
        ChatMessage { id: format!("{role}-{content}"), role: role.to_string(), content: content.to_string(), timestamp_ms: 1 }
    }

    #[test]
    fn recent_messages_returns_only_the_last_n_in_chronological_order() {
        let db = ChatDb::open_in_memory().unwrap();
        let session_id = db.current_session_id().unwrap();
        for i in 0..5 {
            db.append(&session_id, i, &message("user", &format!("msg-{i}"))).unwrap();
        }

        let recent = recent_messages(&db, &session_id, 3).unwrap();
        assert_eq!(recent.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), vec!["msg-2", "msg-3", "msg-4"]);
    }

    #[test]
    fn recent_messages_returns_everything_when_under_the_limit() {
        let db = ChatDb::open_in_memory().unwrap();
        let session_id = db.current_session_id().unwrap();
        db.append(&session_id, 0, &message("user", "oi")).unwrap();

        let recent = recent_messages(&db, &session_id, 50).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].content, "oi");
    }

    #[test]
    fn recent_messages_is_scoped_to_its_own_session() {
        let db = ChatDb::open_in_memory().unwrap();
        db.append("session-a", 0, &message("user", "de a")).unwrap();
        db.append("session-b", 0, &message("user", "de b")).unwrap();

        let recent = recent_messages(&db, "session-a", 50).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].content, "de a");
    }
}
