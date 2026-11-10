//! Grava cada turno do debate na sessão "atual" de `chat.db` — porte da escrita que
//! `chat-generate.runtime.ts::runGenerateRaw` faz sempre que `saveToHistory !== false`
//! (`collaboration.service.ts` passa isso em TODA chamada de `generateRaw`: a abertura e cada
//! turno). O par (`role: "user"`, `role: "assistant"`) é exatamente o que aquela função TS grava —
//! ver `modules::chat::append_message` para o chokepoint de escrita em si, fora deste módulo, por
//! trás do limite de `modules::chat` (mesmo padrão de `modules::chat::recent_messages`, que
//! `content.rs` já usa do lado de LEITURA).
//!
//! **Falha em gravar NÃO derruba o debate.** Ao contrário de `daily_recap` (que precisa do
//! histórico como INSUMO do recap — sem ele, não há o que resumir), aqui a gravação é um efeito
//! colateral do turno que o LLM já gerou. Um erro de disco vira `log::warn!` (nunca o texto do
//! turno — ver a disciplina de log no cabeçalho de `collaboration_session.rs`) e o debate segue.

use std::path::PathBuf;

/// `chat_db_path: None` (boot não resolveu `chat.db`) é o mesmo caso já nomeado em `content.rs` —
/// aqui só o turno não é persistido, o debate continua rodando e aparecendo no chat via
/// `assistant_message::publish_debate_turn`.
pub(super) fn persist_debate_turn(chat_db_path: &Option<PathBuf>, user_content: &str, assistant_content: &str) {
    let Some(path) = chat_db_path else {
        log::warn!("collaboration: chat.db não resolvido no boot — turno não persistido no histórico");
        return;
    };
    if let Err(error) = crate::modules::chat::append_message(path, "user", user_content) {
        log::warn!("collaboration: falha ao gravar turno (user) no histórico: {error}");
    }
    if let Err(error) = crate::modules::chat::append_message(path, "assistant", assistant_content) {
        log::warn!("collaboration: falha ao gravar turno (assistant) no histórico: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-collab-history-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn without_a_resolved_chat_db_path_it_warns_and_does_not_panic() {
        persist_debate_turn(&None, "topico", "fala de abertura");
    }

    /// A prova central: os DOIS papéis chegam no `chat.db` compartilhado, na ordem em que o TS os
    /// grava — lido de volta pelo MESMO chokepoint que `content::daily_recap` usaria para montar o
    /// próximo recap.
    #[test]
    fn writes_both_roles_to_the_shared_chat_db_and_recent_messages_reads_them_back() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");

        persist_debate_turn(&Some(path.clone()), "topico do debate", "fala de abertura");

        let history = crate::modules::chat::recent_messages(&path, 50).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, "user");
        assert_eq!(history[0].content, "topico do debate");
        assert_eq!(history[1].role, "assistant");
        assert_eq!(history[1].content, "fala de abertura");
    }

    #[test]
    fn a_failed_role_write_is_logged_but_does_not_panic_the_caller() {
        // Path que existe como DIRETÓRIO, não arquivo: `ChatDb::open` falha ao abrir a conexão —
        // prova que um erro de disco vira log, não pânico, sem inventar um segundo modo de falha.
        let dir = TempDir::new();
        persist_debate_turn(&Some(dir.0.clone()), "topico", "fala");
    }
}
