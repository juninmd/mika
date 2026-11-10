//! `content` — porte de `contentTool` + `ContentPipelineService`
//! (`apps/desktop/src/main/services/brain/tools/content.ts` +
//! `apps/desktop/src/main/services/brain/services/content-pipeline.service.ts`).
//!
//! ## Tabela TS × Rust
//!
//! | Método TS | Depende de | Rust | Por quê |
//! |---|---|---|---|
//! | `craftSocialPost(event)` | `ChatService.generateRaw` (LLM), `prompts/social-post.md` | **portado, paridade real** | Só LLM — sem `HistoryService`. |
//! | `generateDailyRecap()` | idem + `HistoryService.getAll().slice(-50)` | **portado, paridade real** (esta rodada) | O histórico JÁ existe em Rust: `modules::chat::recent_messages` lê o MESMO `chat.db` que `HistoryStore` persiste (ver `modules/chat/mod.rs`). A premissa do cabeçalho anterior ("HistoryService não existe em Rust") estava errada — só a LEITURA de fora do módulo `chat` não existia ainda. |
//!
//! `craftSocialPost` mede 100% LLM (`turn::run_turn_llm_raw`, tool-free, `social-post.md` como
//! `userContent` — TS não passa `systemPrompt` nesta chamada, então o prompt de sistema é só a
//! persona, igual ao turno principal). `generateDailyRecap` faz o mesmo, mais uma leitura BOUNDED
//! (últimas 50 mensagens da sessão "atual", `ORDER BY seq DESC ... LIMIT 50` — ver
//! `chat_db.rs::recent_messages`) formatada como `[role]: content`, igual ao `.map(m =>
//! \`[${m.role}]: ${m.content}\`).join('\n')` de `content-pipeline.service.ts`.
//!
//! **Gap nomeado, não escondido:** `chat_db_path: Option<PathBuf>` vem de
//! `register_config_bound_tools` (resolvido no boot, o único ponto com `AppHandle`) — se a
//! resolução falhar lá, `daily_recap` degrada `unavailable` nomeando ESSE motivo específico, não o
//! genérico antigo.
//!
//! Evento: TS emite `Events.ASSISTANT_MESSAGE` (`chat:assistant-message`) após o recap
//! (`content-pipeline.service.ts::generateDailyRecap`). `EventKey::AssistantMessage` passou a
//! existir nesta rodada (`events/keys.rs`) — `generate_daily_recap` publica no barramento de
//! processo via `assistant_message::publish_recap` (mesmo payload `{ content, timestamp }`, sem
//! `isCollaboration`), e `modules::chat::spawn_assistant_message_bridge` encaminha isso à janela
//! (ver o cabeçalho de `modules/chat/assistant_bridge.rs`). `craft_social_post` continua sem emitir
//! — o TS também não publica nada para `social_post`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortHandle, TurnError};
use crate::modules::tools::ToolHandler;

use super::prompts;

/// Cópia literal de `contentTool.description` (`tools/content.ts`).
pub(super) const DESCRIPTION: &str = "Write content in your voice, only when asked. action=daily_recap: fun recap of today's conversations. action=social_post: short X/Twitter post draft about `event`.";

/// Porte de `.slice(-50)` em `ContentPipelineService.generateDailyRecap`.
const RECAP_HISTORY_LIMIT: i64 = 50;

fn fail(message: impl Into<String>, code: &str) -> Value {
    json!({ "success": false, "error": message.into(), "errorCode": code })
}

fn ok(data: Value) -> Value {
    json!({ "success": true, "data": data })
}

fn turn_error_message(error: TurnError) -> String {
    format!("Falha ao gerar conteúdo: {error}")
}

pub(super) struct ContentHandler {
    store: Arc<ConfigStore>,
    chat_db_path: Option<PathBuf>,
}

impl ContentHandler {
    pub(super) fn new(store: Arc<ConfigStore>, chat_db_path: Option<PathBuf>) -> Self {
        Self { store, chat_db_path }
    }

    /// Porte de `craftSocialPost`: `generateRaw(loadPrompt('social-post', {eventDescription}))`,
    /// sem `systemPrompt` (só a persona) — o template renderizado É o `userContent`.
    async fn craft_social_post(&self, event: &str) -> Result<String, String> {
        let user_content = prompts::render(prompts::SOCIAL_POST, &[("eventDescription", event)]);
        let (_handle, signal) = TurnAbortHandle::new();

        turn::run_turn_llm_raw(&self.store, None, &user_content, signal, &NullObserver)
            .await
            .map(|result| result.text)
            .map_err(turn_error_message)
    }

    /// Porte de `ContentPipelineService.generateDailyRecap`: lê as últimas
    /// [`RECAP_HISTORY_LIMIT`] mensagens da sessão "atual", monta o contexto `[role]: content` e
    /// pede o recap ao LLM. Como o TS, não passa `systemPrompt` (só a persona).
    ///
    /// Log: só o TAMANHO do recap gerado, nunca o histórico ou o texto — `content-pipeline.
    /// service.ts` só loga `{ length: recap.length }`, e histórico de conversa é conteúdo privado
    /// do usuário (ver o header do relatório desta rodada).
    async fn generate_daily_recap(&self) -> Result<String, String> {
        let path = self
            .chat_db_path
            .as_ref()
            .ok_or_else(|| "chat.db: caminho não resolvido no boot (app_data_dir indisponível)".to_string())?;
        let history = crate::modules::chat::recent_messages(path, RECAP_HISTORY_LIMIT)?;

        log::info!("content: gerando daily recap ({} mensagens de contexto)", history.len());
        let context = build_recap_context(&history);
        let user_content = prompts::render(prompts::DAILY_RECAP, &[("context", &context)]);
        let (_handle, signal) = TurnAbortHandle::new();

        let recap = turn::run_turn_llm_raw(&self.store, None, &user_content, signal, &NullObserver)
            .await
            .map(|result| result.text)
            .map_err(turn_error_message)?;
        log::info!("content: daily recap gerado ({} chars)", recap.len());
        super::assistant_message::publish_recap(crate::events::bus(), &recap);
        Ok(recap)
    }
}

#[async_trait]
impl ToolHandler for ContentHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        match args["action"].as_str().unwrap_or_default() {
            "daily_recap" => match self.generate_daily_recap().await {
                Ok(recap) => Ok(ok(json!({ "recap": recap }))),
                Err(message) => Ok(fail(message, "unavailable")),
            },
            "social_post" => {
                let event = args["event"].as_str().map(str::trim).unwrap_or_default();
                if event.is_empty() {
                    return Ok(fail("event é obrigatório para action=social_post.", "invalid_args"));
                }
                match self.craft_social_post(event).await {
                    Ok(post) => Ok(ok(json!({ "post": post }))),
                    Err(message) => Ok(fail(message, "unavailable")),
                }
            }
            other => Ok(fail(format!("content: ação desconhecida \"{other}\""), "invalid_args")),
        }
    }
}

/// Porte de `history.map(m => \`[${m.role}]: ${m.content}\`).join('\n')`
/// (`content-pipeline.service.ts::generateDailyRecap`) — função pura, testável sem I/O nem rede.
fn build_recap_context(history: &[crate::modules::chat::ChatMessage]) -> String {
    history.iter().map(|m| format!("[{}]: {}", m.role, m.content)).collect::<Vec<_>>().join("\n")
}

/// Espelha `contentTool.inputSchema`.
pub(super) fn schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "action": { "type": "string", "enum": ["daily_recap", "social_post"] },
            "event": { "type": "string" },
        },
        "required": ["action"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-brain-content-tool-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn handler() -> (ContentHandler, TempDir) {
        let (handler, dir, _) = handler_with_chat_db();
        (handler, dir)
    }

    /// `chat_db_path` aponta para um arquivo que ainda NÃO existe dentro do próprio `TempDir` —
    /// `ChatDb::open`/`recent_messages` criam o arquivo e o schema na hora, mesmo comportamento de
    /// `chat_db.rs::open` em produção (primeira leitura antes de qualquer `chat-send`).
    fn handler_with_chat_db() -> (ContentHandler, TempDir, PathBuf) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        let chat_db_path = dir.0.join("chat.db");
        (ContentHandler::new(Arc::new(store), Some(chat_db_path.clone())), dir, chat_db_path)
    }

    #[tokio::test]
    async fn daily_recap_without_a_resolved_chat_db_path_degrades_named_by_that_not_the_old_generic_guard() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        let handler = ContentHandler::new(Arc::new(store), None);

        let result = handler.call(json!({ "action": "daily_recap" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("unavailable"));
        let message = result["error"].as_str().unwrap();
        assert!(message.contains("chat.db"), "mensagem deveria nomear a dependência real que falta: {message}");
        assert!(!message.contains("ContentPipelineService indisponível"), "não deveria mais usar a mensagem genérica antiga: {message}");
    }

    /// Sem servidor Ollama real no ambiente de teste, `generate_daily_recap` degrada honestamente a
    /// `unavailable` — mas só DEPOIS de ler o histórico com sucesso (prova que o caminho de leitura
    /// foi alcançado, não pulado).
    #[tokio::test]
    async fn daily_recap_with_a_resolved_chat_db_path_reads_history_and_degrades_unavailable_without_a_live_provider() {
        let (handler, _dir, _path) = handler_with_chat_db();
        let result = handler.call(json!({ "action": "daily_recap" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("unavailable"));
        let message = result["error"].as_str().unwrap();
        assert!(!message.contains("chat.db: caminho não resolvido"), "deveria ter passado da checagem de caminho: {message}");
        assert!(!message.contains("ContentPipelineService indisponível"), "não deveria mais usar a mensagem genérica antiga: {message}");
    }

    #[test]
    fn build_recap_context_matches_the_ts_role_bracket_format() {
        let history = vec![
            crate::modules::chat::ChatMessage { id: "1".into(), role: "user".into(), content: "oi".into(), timestamp_ms: 1 },
            crate::modules::chat::ChatMessage { id: "2".into(), role: "assistant".into(), content: "tudo bem?".into(), timestamp_ms: 2 },
        ];
        assert_eq!(build_recap_context(&history), "[user]: oi\n[assistant]: tudo bem?");
    }

    #[test]
    fn build_recap_context_of_empty_history_is_an_empty_string() {
        assert_eq!(build_recap_context(&[]), "");
    }

    #[tokio::test]
    async fn social_post_without_event_is_a_named_invalid_args_error_not_a_panic() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "social_post" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    #[tokio::test]
    async fn social_post_with_a_blank_event_is_the_same_named_invalid_args_error() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "social_post", "event": "   " })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    /// Sem servidor Ollama real no ambiente de teste, `craft_social_post` degrada honestamente a
    /// `unavailable` — prova que a validação de argumento passou e o caminho de geração foi
    /// alcançado de verdade (não a mensagem antiga "ContentPipelineService indisponível").
    #[tokio::test]
    async fn social_post_with_a_valid_event_reaches_generation_and_degrades_unavailable_without_a_live_provider() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "social_post", "event": "1000 seguidores" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("unavailable"));
        let message = result["error"].as_str().unwrap();
        assert!(!message.contains("ContentPipelineService indisponível"), "não deveria mais usar a mensagem genérica antiga: {message}");
    }

    #[tokio::test]
    async fn unknown_action_is_a_named_invalid_args_error() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "nope" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }
}
