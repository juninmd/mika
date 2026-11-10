//! Histórico de chat — porte de `HistoryState`/`HistoryService`
//! (`apps/desktop/src/main/services/chat/history-state.ts` + `history.service.ts`). O array
//! `this.messages` do TypeScript é a fonte de verdade EM MEMÓRIA de uma sessão; o banco é
//! persistência por trás dela. Este arquivo cobre só o tipo `ChatMessage` e os dois handlers de
//! IPC que leem/apagam o histórico — a store em si (`HistoryStore`) mora em `history_store.rs`, e
//! a persistência SQLite em `chat_db.rs`/`session_adoption.rs` (ver o cabeçalho de cada um).
//!
//! **Por que não `modules::memory::MemoryStore` (rusqlite) para o histórico.** Aquele store guarda
//! memória EPISÓDICA — fatos e resumos que sobrevivem entre sessões, schema que a fase 3 da
//! migração já decidiu para esse propósito. Histórico de conversa é outra coisa: as mensagens
//! brutas do turno atual, no formato que `mika:chat-send`/`mika:chat-history` trocam com o
//! renderer. Gravar nas tabelas de memória episódica misturaria dois formatos que a fase 3 manteve
//! deliberadamente separados — por isso `chat_db.rs` abre seu PRÓPRIO arquivo (`chat.db`, o mesmo
//! caminho que o TypeScript já usa hoje), não `memory.db`.
//!
//! **Escopo menor que o TypeScript, herdado e não escondido:** `session-manager.ts` (múltiplas
//! sessões NOMEADAS, trocáveis pela UI) não foi portado — `HistoryStore` continua com uma única
//! sessão "atual" por processo. O que ESTA rodada resolve é a regressão registrada no cabeçalho de
//! `scripts/checks/sidecar-attrition.mts`: aquela sessão única agora sobrevive ao restart (ver
//! `history_store.rs::ensure_persistence`) em vez de nascer vazia a cada boot.

use rig_core::completion::Message as RigMessage;
use serde_json::{json, Value};

use super::history_store::HistoryStore;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

/// Espelha `ChatMessage` de `packages/sdk/src/contracts/types/runtime-types.ts`, só os 4 campos
/// que `desktopChatHistoryResponseSchema` expõe (`id`, `role`, `content`, `timestamp`). Anexos,
/// `toolCallId`/`toolName`/`usage` não têm produtor nesta rodada — `chat_db.rs` já reproduz as
/// colunas inteiras do schema de `messages-table.ts`, mas essas quatro continuam `NULL` do lado
/// Rust até um produtor existir.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp_ms: u64,
}

impl ChatMessage {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "role": self.role,
            "content": self.content,
            "timestamp": self.timestamp_ms,
        })
    }

    /// Porte do filtro em `HistoryStore::to_turn_history`: `system` aqui é sempre um turno que
    /// FALHOU (o `content` é a mensagem de erro, convenção de `send.rs`) — dar isso ao modelo como
    /// fala real confundiria o próximo turno sobre o que de fato foi dito.
    pub(super) fn to_rig(&self) -> Option<RigMessage> {
        match self.role.as_str() {
            "user" => Some(RigMessage::user(&self.content)),
            "assistant" => Some(RigMessage::assistant(&self.content)),
            _ => None,
        }
    }
}

/// `mika:chat-history`: devolve a forma de `desktopChatHistoryResponseSchema` a partir do store.
pub fn handle_chat_history(ctx: &InvokeContext, history: &HistoryStore) -> Result<Value, String> {
    if !is_owner_surface(ctx.window_type) {
        return Err(format!("mika:chat-history recusado para {:?}", ctx.window_type));
    }
    let messages: Vec<Value> = history.all()?.iter().map(ChatMessage::to_json).collect();
    Ok(json!({ "messages": messages }))
}

/// `mika:chat-clear`: esvazia o store (memória + disco) — mesma resposta `{ success: true }` de
/// `chat/ipc.ts::registerChatIPC`. `history.clear()` propaga erro se o delete no disco falhar —
/// nunca devolve sucesso para um apagar que não aconteceu (ver o cabeçalho de `history_store.rs`).
pub fn handle_chat_clear(ctx: &InvokeContext, history: &HistoryStore) -> Result<Value, String> {
    if !is_owner_surface(ctx.window_type) {
        return Err(format!("mika:chat-clear recusado para {:?}", ctx.window_type));
    }
    history.clear()?;
    Ok(json!({ "success": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WindowType;

    fn ctx(window_type: WindowType) -> InvokeContext {
        InvokeContext { window_type, window_label: "main".to_string(), plugin_id: None }
    }

    fn message(role: &str, content: &str) -> ChatMessage {
        ChatMessage { id: format!("{role}-1"), role: role.to_string(), content: content.to_string(), timestamp_ms: 1 }
    }

    #[test]
    fn to_json_carries_the_four_response_fields() {
        let json = message("user", "oi").to_json();
        assert_eq!(json["id"], "user-1");
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "oi");
        assert_eq!(json["timestamp"], 1);
    }

    #[test]
    fn to_rig_keeps_only_user_and_assistant() {
        assert!(message("user", "oi").to_rig().is_some());
        assert!(message("assistant", "oi").to_rig().is_some());
        assert!(message("system", "erro de um turno anterior").to_rig().is_none());
    }

    #[test]
    fn handle_chat_history_reflects_what_was_pushed_to_the_store() {
        let store = HistoryStore::new();
        store.push(message("user", "oi")).unwrap();
        store.push(message("assistant", "oi, tudo bem?")).unwrap();

        let response = handle_chat_history(&ctx(WindowType::Main), &store).unwrap();
        let messages = response["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["content"], "oi");
        assert_eq!(messages[1]["role"], "assistant");
    }

    #[test]
    fn handle_chat_history_refuses_a_non_owner_surface() {
        let store = HistoryStore::new();
        assert!(handle_chat_history(&ctx(WindowType::PluginView), &store).is_err());
    }

    #[test]
    fn handle_chat_clear_empties_the_store_and_reports_success() {
        let store = HistoryStore::new();
        store.push(message("user", "oi")).unwrap();

        let response = handle_chat_clear(&ctx(WindowType::Main), &store).unwrap();
        assert_eq!(response, json!({ "success": true }));
        assert!(store.all().unwrap().is_empty());
    }

    #[test]
    fn handle_chat_clear_refuses_a_non_owner_surface() {
        let store = HistoryStore::new();
        assert!(handle_chat_clear(&ctx(WindowType::PluginView), &store).is_err());
    }
}
