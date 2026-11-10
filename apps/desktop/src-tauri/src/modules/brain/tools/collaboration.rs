//! `collaboration` — porte de `collaborationTool` + `CollaborationService`
//! (`apps/desktop/src/main/services/brain/tools/collaboration.ts` +
//! `apps/desktop/src/main/services/brain/services/collaboration.service.ts`): um debate multi-
//! turno entre dois "agentes" de IA (dois nomes de persona), cada turno gerado pelo mesmo LLM que
//! o resto de `brain` já usa (`turn::run_turn_llm_raw`), alternando quem "fala" a cada volta.
//!
//! **Dispatcha de verdade nesta rodada** — o mecanismo inteiro (validação, sessão, abertura,
//! turnos alternados até o teto, pausa entre turnos, `stop` cancelando a geração em voo) está em
//! [`super::collaboration_session`]; este arquivo só é a borda de `ToolHandler` (validação de
//! entrada, schema, o texto de retorno imediato). Ver o cabeçalho de `collaboration_session.rs`
//! para o desenho da sessão e a tabela de paridade em `tools/mod.rs`.
//!
//! **Os dois gaps herdados foram fechados nesta rodada** (`EventKey::AssistantMessage` passou a
//! existir, ver `events/keys.rs`):
//! - Emissão no EventBus por turno (`super::assistant_message::publish_debate_turn`, mesmo
//!   `[agente]: fala` do TS) — `modules::chat::spawn_assistant_message_bridge` encaminha à janela.
//! - Gravação no `chat.db` compartilhado (`super::collaboration_history::persist_debate_turn`, via
//!   `modules::chat::append_message` — o chokepoint de ESCRITA que essa rodada abriu, sem alargar o
//!   campo privado `HistoryStore` de `ChatModule`). `chat_db_path` chega aqui do mesmo jeito que já
//!   chega em `content.rs` (resolvido uma vez em `modules/mod.rs::register_config_bound_tools`).

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::store::ConfigStore;
use crate::modules::tools::ToolHandler;

use super::collaboration_session as session;

/// Cópia literal de `collaborationTool.description` (`tools/collaboration.ts`).
pub(super) const DESCRIPTION: &str = "Start or stop a spoken debate between two AI characters (`agentA`, `agentB`) about a `topic`. Only when the user asks for a debate.";

fn fail(message: impl Into<String>) -> Value {
    json!({ "success": false, "error": message.into(), "errorCode": "invalid_args" })
}

fn ok(data: Value) -> Value {
    json!({ "success": true, "data": data })
}

/// `args[key]` não-vazio — mesma checagem "falsy" do `if (!agentA || !agentB || !topic)` do TS
/// (string vazia falha, string só de espaço PASSA: JS só considera `''` falsy, não
/// `.trim().is_empty()`). Sem `trim()` de propósito, para não validar mais do que o TS validava.
fn non_empty(args: &Value, key: &str) -> Option<String> {
    args[key].as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

pub(super) struct CollaborationHandler {
    store: Arc<ConfigStore>,
    chat_db_path: Option<PathBuf>,
}

impl CollaborationHandler {
    pub(super) fn new(store: Arc<ConfigStore>, chat_db_path: Option<PathBuf>) -> Self {
        Self { store, chat_db_path }
    }
}

#[async_trait]
impl ToolHandler for CollaborationHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        match args["action"].as_str().unwrap_or_default() {
            "stop" => {
                session::stop();
                Ok(ok(json!({ "message": "Colaboração encerrada." })))
            }
            "start" => {
                let (Some(agent_a), Some(agent_b), Some(topic)) =
                    (non_empty(&args, "agentA"), non_empty(&args, "agentB"), non_empty(&args, "topic"))
                else {
                    return Ok(fail("agentA, agentB e topic são obrigatórios para action=start."));
                };

                // Mesmo contrato fire-and-forget do TS (`void service.startDebate(...)`): a tool
                // devolve a confirmação NA HORA, o debate roda em segundo plano.
                let message = format!("Debate entre {agent_a} e {agent_b} iniciado sobre: {topic}");
                session::start(Arc::clone(&self.store), self.chat_db_path.clone(), agent_a, agent_b, topic);
                Ok(ok(json!({ "message": message })))
            }
            other => Ok(fail(format!("collaboration: ação desconhecida \"{other}\""))),
        }
    }
}

/// Espelha `collaborationTool.inputSchema`.
pub(super) fn schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "action": { "type": "string", "enum": ["start", "stop"] },
            "agentA": { "type": "string" },
            "agentB": { "type": "string" },
            "topic": { "type": "string" },
        },
        "required": ["action"],
    })
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
            let path = std::env::temp_dir().join(format!("mika-brain-collaboration-tool-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn handler() -> (CollaborationHandler, TempDir) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        // `chat_db_path: None` nestes testes: nenhum deles chega a rodar um turno de verdade (sem
        // Ollama no ambiente), então não há gravação de histórico para observar aqui — ver
        // `collaboration_history.rs` para o teste dedicado à persistência.
        (CollaborationHandler::new(Arc::new(store), None), dir)
    }

    #[tokio::test]
    async fn stop_always_succeeds_even_with_no_active_debate() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "stop" })).await.unwrap();
        assert_eq!(result, json!({ "success": true, "data": { "message": "Colaboração encerrada." } }));
    }

    #[tokio::test]
    async fn start_without_any_of_the_three_required_fields_is_a_named_invalid_args_error() {
        let (handler, _dir) = handler();
        for args in [json!({ "action": "start" }), json!({ "action": "start", "agentA": "Mika" }), json!({ "action": "start", "agentA": "Mika", "agentB": "Luna" })]
        {
            let result = handler.call(args).await.unwrap();
            assert_eq!(result["success"], json!(false));
            assert_eq!(result["errorCode"], json!("invalid_args"));
        }
    }

    /// Espelha o TS: só `''` é falsy em JS, `'   '` não é — validação sem `trim()`.
    #[tokio::test]
    async fn start_treats_a_blank_but_non_empty_field_as_present_same_as_the_ts_falsy_check() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "start", "agentA": "Mika", "agentB": "Luna", "topic": "   " })).await.unwrap();
        assert_eq!(result["success"], json!(true), "espaço em branco é truthy em JS — não deveria falhar a validação");
    }

    #[tokio::test]
    async fn start_with_an_empty_string_field_is_the_same_named_invalid_args_error() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "start", "agentA": "Mika", "agentB": "Luna", "topic": "" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    /// A prova central da rodada: `start` com os 3 campos DISPACHA de verdade (não mais
    /// `CollaborationService indisponível`) e devolve a mesma mensagem de sucesso que o TS monta.
    #[tokio::test]
    async fn start_with_all_three_fields_dispatches_for_real_and_returns_the_ts_success_message() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "start", "agentA": "Mika", "agentB": "Luna", "topic": "RPG de mesa" })).await.unwrap();
        assert_eq!(result, json!({ "success": true, "data": { "message": "Debate entre Mika e Luna iniciado sobre: RPG de mesa" } }));

        // Encerra a sessão que este teste acabou de iniciar (o debate roda em background sobre o
        // slot GLOBAL de `collaboration_session.rs`) para não vazar estado para outro teste.
        session::stop();
    }

    #[tokio::test]
    async fn unknown_action_is_a_named_invalid_args_error() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "nope" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }
}
