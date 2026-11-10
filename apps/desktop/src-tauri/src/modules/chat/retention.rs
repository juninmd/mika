//! `privacy.conversationRetention` — apaga do `chat.db` as mensagens mais velhas que a janela
//! escolhida. Até 2026-09-27 o controle existia na tela de Privacidade e nenhum código Rust o lia: o
//! usuário escolhia "1 dia" e a conversa inteira continuava em disco para sempre.
//!
//! Roda na abertura do banco (`HistoryStore::ensure_persistence`), antes de restaurar a sessão — o
//! que sai do disco nunca chega à memória do processo. O resumo rolante da sessão podada sai junto:
//! ele descreve justamente as mensagens apagadas, e o índice `summarized_through` deixaria de
//! apontar para as mensagens certas.

use rusqlite::params;

use super::chat_db::ChatDb;
use crate::config::store::ConfigStore;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// Instante (ms) antes do qual uma mensagem expira; `None` para "forever" ou valor desconhecido —
/// um valor que não se entende nunca apaga histórico.
pub(super) fn cutoff_ms(choice: &str, now_ms: u64) -> Option<u64> {
    let days = match choice {
        "1day" => 1,
        "7days" => 7,
        "30days" => 30,
        "90days" => 90,
        _ => return None,
    };
    Some(now_ms.saturating_sub(days * DAY_MS))
}

/// Remove as mensagens com `created_at < cutoff` e o resumo rolante das sessões afetadas, numa só
/// transação. Devolve quantas mensagens saíram.
pub(super) fn prune_before(db: &ChatDb, cutoff: u64) -> Result<usize, String> {
    let mut conn = db.lock()?;
    let tx = conn.transaction().map_err(|e| format!("retention: {e}"))?;
    tx.execute(
        "DELETE FROM chat_rolling_summary WHERE session_id IN \
         (SELECT DISTINCT session_id FROM messages WHERE created_at < ?1)",
        params![cutoff as i64],
    )
    .map_err(|e| format!("retention: {e}"))?;
    let removed = tx
        .execute("DELETE FROM messages WHERE created_at < ?1", params![cutoff as i64])
        .map_err(|e| format!("retention: {e}"))?;
    tx.commit().map_err(|e| format!("retention: {e}"))?;
    Ok(removed)
}

pub(super) fn apply(store: &ConfigStore, db: &ChatDb, now_ms: u64) -> Result<usize, String> {
    let choice = store.get("privacy.conversationRetention").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    match cutoff_ms(&choice, now_ms) {
        Some(cutoff) => prune_before(db, cutoff),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::history::ChatMessage;

    fn at(id: &str, timestamp_ms: u64) -> ChatMessage {
        ChatMessage { id: id.to_string(), role: "user".to_string(), content: id.to_string(), timestamp_ms }
    }

    #[test]
    fn forever_and_unknown_values_never_expire_anything() {
        assert_eq!(cutoff_ms("forever", 10 * DAY_MS), None);
        assert_eq!(cutoff_ms("", 10 * DAY_MS), None);
        assert_eq!(cutoff_ms("2weeks", 10 * DAY_MS), None);
    }

    #[test]
    fn cutoff_is_the_chosen_window_before_now_and_never_underflows() {
        assert_eq!(cutoff_ms("7days", 10 * DAY_MS), Some(3 * DAY_MS));
        assert_eq!(cutoff_ms("90days", DAY_MS), Some(0));
    }

    #[test]
    fn prune_removes_only_old_messages_and_the_summary_that_described_them() {
        let db = ChatDb::open_in_memory().unwrap();
        db.append("old", 0, &at("velha", DAY_MS)).unwrap();
        db.append("old", 1, &at("nova", 5 * DAY_MS)).unwrap();
        db.append("fresh", 0, &at("recente", 5 * DAY_MS)).unwrap();
        db.set_rolling_summary("old", "resumo da velha", 1, 5 * DAY_MS).unwrap();
        db.set_rolling_summary("fresh", "resumo intacto", 1, 5 * DAY_MS).unwrap();

        assert_eq!(prune_before(&db, 3 * DAY_MS).unwrap(), 1);

        let kept: Vec<String> = db.restore("old").unwrap().0.into_iter().map(|m| m.content).collect();
        assert_eq!(kept, vec!["nova"]);
        assert_eq!(db.rolling_summary("old").unwrap(), None);
        assert_eq!(db.rolling_summary("fresh").unwrap().map(|(s, _)| s), Some("resumo intacto".to_string()));
    }

    #[test]
    fn apply_reads_the_privacy_setting_and_defaults_to_keeping_everything() {
        let dir = std::env::temp_dir().join(format!("mika-retention-{}", std::process::id()));
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        let db = ChatDb::open_in_memory().unwrap();
        db.append("s", 0, &at("antiga", 0)).unwrap();

        assert_eq!(apply(&store, &db, 100 * DAY_MS).unwrap(), 0, "default 'forever' não apaga nada");
        store.set("privacy.conversationRetention", serde_json::json!("1day")).unwrap();
        assert_eq!(apply(&store, &db, 100 * DAY_MS).unwrap(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
