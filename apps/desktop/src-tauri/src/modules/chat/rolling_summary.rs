//! Resumo rolante do histórico de chat — 04-memory.md §"Curto prazo", a MUST que faltava: "acima
//! do limite, o excedente é resumido, não truncado" + "o resumo rolante é gerado por LLM e
//! guardado, não recalculado a cada turno".
//!
//! **Mesmo desenho do resumo de sessão de jogo** (`gamer::session_memory::summarize_with_llm`):
//! síncrono nunca bloqueia no LLM. `send::handle_chat_send` grava a mensagem do turno (síncrono),
//! calcula se sobrou excedente novo (`HistoryStore::overflow_for_summary`) e só ENTÃO dispara este
//! resumo em segundo plano (`tauri::async_runtime::spawn`, fora do caminho de resposta — nunca
//! atrasa o turno atual). Uma falha do LLM (offline, provedor fora) simplesmente deixa o excedente
//! sem resumo desta vez; a próxima chamada de `overflow_for_summary` tenta de novo com o mesmo
//! ponto de partida (`summarized_through` só avança quando `set_rolling_summary` roda com sucesso).
//!
//! **Incremental, não do zero.** O resumo anterior (se houver) entra no prompt como contexto —
//! resumir 4 mensagens novas ignorando o resumo já guardado perderia tudo que ele cobria antes.

use crate::config::store::ConfigStore;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortSignal};

use super::history::ChatMessage;

/// Teto de mensagens por chamada de resumo — mesmo valor de
/// `gamer::session_memory::summary_prompt::MAX_EVENTS`, para não estourar o contexto do modelo
/// quando o excedente novo for grande (ex.: `historyLimit` reduzido a posteriori).
pub(super) const MAX_MESSAGES: usize = 40;

fn summary_prompt(existing_summary: Option<&str>, overflow: &[ChatMessage]) -> String {
    let start = overflow.len().saturating_sub(MAX_MESSAGES);
    let lines: Vec<String> = overflow[start..].iter().map(|m| format!("{}: {}", m.role, m.content)).collect();
    let previous = existing_summary.map(|s| format!("Previous summary: {s}\n")).unwrap_or_default();
    format!(
        "Update the rolling summary of this conversation with the new turns below, same language as \
         the turns, one short paragraph (max ~500 characters), keeping facts from the previous summary \
         that still matter.\n{previous}New turns:\n{}",
        lines.join("\n")
    )
}

/// `llm.enableSummarization` — a chave da aba LLM que desliga o resumo rolante. Até 2026-09-27 ela
/// não era lida e o resumo rodava sempre; o default do schema virou `true` para manter isso.
pub fn summarization_enabled(store: &ConfigStore) -> bool {
    store.get("llm.enableSummarization").and_then(|v| v.as_bool()).unwrap_or(true)
}

/// Resume o excedente (a MUST do §"Curto prazo"). `existing_summary` é o que já está guardado, para
/// o resultado incorporar o que veio antes em vez de só as mensagens novas. Nunca toca provedor
/// pago/rede real em teste — mesma convenção `llm_dead_port` de `gamer::llm_dead_port`.
pub async fn summarize_overflow_with_llm(
    store: &ConfigStore,
    existing_summary: Option<&str>,
    overflow: &[ChatMessage],
    abort: TurnAbortSignal,
) -> Result<String, String> {
    let prompt = summary_prompt(existing_summary, overflow);
    turn::run_turn_llm_raw(store, None, &prompt, abort, &NullObserver).await.map(|r| r.text.trim().to_string()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &str, content: &str) -> ChatMessage {
        ChatMessage { id: format!("{role}-{content}"), role: role.to_string(), content: content.to_string(), timestamp_ms: 1 }
    }

    #[test]
    fn summary_prompt_includes_the_new_turns() {
        let overflow = vec![message("user", "joga Valorant"), message("assistant", "que rank?")];
        let prompt = summary_prompt(None, &overflow);
        assert!(prompt.contains("user: joga Valorant"), "{prompt}");
        assert!(prompt.contains("assistant: que rank?"), "{prompt}");
        assert!(!prompt.contains("Previous summary"), "{prompt}");
    }

    #[test]
    fn summary_prompt_carries_the_previous_summary_forward() {
        let overflow = vec![message("user", "parou de jogar Valorant")];
        let prompt = summary_prompt(Some("Jogava Valorant."), &overflow);
        assert!(prompt.contains("Previous summary: Jogava Valorant."), "{prompt}");
    }

    #[test]
    fn summary_prompt_caps_at_the_message_ceiling() {
        let overflow: Vec<ChatMessage> = (0..50).map(|i| message("user", &format!("msg{i}"))).collect();
        let prompt = summary_prompt(None, &overflow);
        assert!(!prompt.contains("msg0\n"), "mensagem mais antiga não deveria caber no teto: {prompt}");
        assert!(prompt.contains("msg49"), "{prompt}");
    }

    #[test]
    fn summarization_is_on_by_default_and_the_settings_switch_turns_it_off() {
        let dir = std::env::temp_dir().join(format!("mika-chat-rolling-summary-switch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        assert!(summarization_enabled(&store), "sem valor salvo o resumo continua rodando como antes");
        store.set("llm.enableSummarization", serde_json::json!(false)).unwrap();
        assert!(!summarization_enabled(&store));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 04-memory.md §"Curto prazo": uma falha do LLM (porta morta, nunca provedor real em teste)
    /// devolve erro nomeado — quem chama decide não avançar `summarized_through`.
    #[tokio::test]
    async fn summarize_overflow_with_llm_fails_named_without_touching_a_real_provider_when_unreachable() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mika-chat-rolling-summary-llm-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        // Mesma convenção de `gamer::llm_dead_port` — porta sem listener, nunca provedor real.
        store.set("apiKeys.ollamaBaseUrl", serde_json::json!("http://127.0.0.1:11500")).unwrap();

        let overflow = vec![message("user", "oi")];
        let (_handle, signal) = turn::TurnAbortHandle::new();
        let result = summarize_overflow_with_llm(&store, None, &overflow, signal).await;
        assert!(result.is_err(), "sem provedor no ar, o resumo por LLM deveria falhar nomeado");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
