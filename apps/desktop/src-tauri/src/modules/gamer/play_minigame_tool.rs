//! Tool `play_minigame` — porte de `tools/play-minigame.ts`. jokenpo/tictactoe/pong são payloads
//! HTML estáticos, sem I/O; truco usa [`super::truco_round::start_truco_round`]. Nenhum dos quatro
//! precisa de config/`AppHandle`, então registra junto de `track_gamer_mood`/`steam` em
//! `gamer::register_tools` (efeito colateral de `modules()`), não em `register_config_bound_tools`.
//!
//! **REMEDIDO 2026-09-22: a fila `miniGameQueue`/`getNextRequestedMiniGame` do TS TEM consumidor,
//! sim.** O cabeçalho anterior deste arquivo dizia "nenhum outro módulo consome" — a premissa
//! estava errada: a RPC `mini-game:requested` de `mika:plugin-rpc` (`super::rpc`, mesmo canal que
//! `renderer/widgets/mini-games.widget.tsx` já chama hoje contra o sidecar Node) É o consumidor.
//! [`next_requested`] fecha essa fila aqui; [`build_payload_data`] é o núcleo puro do payload,
//! compartilhado com a RPC `mini-game:payload` (que devolve só `data`, sem embrulhar de novo — ver
//! `super::rpc`).

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use super::minigame_jokenpo::jokenpo_html;
use super::minigame_pong::pong_html;
use super::minigame_tictactoe::tictactoe_html;
use super::truco_round::start_truco_round;
use crate::modules::tools::{ToolDefinition, ToolHandler};

/// Os 4 `MiniGameId` válidos — mesma lista de `tools/play-minigame.ts::MiniGameId`.
pub(super) const KNOWN_GAME_IDS: [&str; 4] = ["jokenpo", "tictactoe", "pong", "truco"];

/// Porte de `miniGameQueue` — fila de minijogo pedido pela TOOL (LLM), lida pela RPC
/// `mini-game:requested`. Um só por processo, como `crate::modules::tools::registry()`.
static REQUESTED_QUEUE: OnceLock<Mutex<VecDeque<&'static str>>> = OnceLock::new();

fn requested_queue() -> &'static Mutex<VecDeque<&'static str>> {
    REQUESTED_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Porte de `getNextRequestedMiniGame` — consumido por `super::rpc`.
pub(super) fn next_requested() -> Option<&'static str> {
    requested_queue().lock().unwrap_or_else(|e| e.into_inner()).pop_front()
}

/// Núcleo puro do payload de um minijogo CONHECIDO — `None` para um `game` desconhecido, cada
/// chamador (esta tool, ou `super::rpc::MiniGamePayload`) decide o texto do próprio erro, porque o
/// TS usa mensagens DIFERENTES em cada um dos dois call sites.
pub(super) fn build_payload_data(game: &str) -> Option<Value> {
    Some(match game {
        "jokenpo" => json!({ "game": "jokenpo", "html": jokenpo_html(), "message": "Jokenpo game loaded." }),
        "pong" => json!({ "game": "pong", "html": pong_html(), "message": "Game of Pong loaded. Enjoy!" }),
        "tictactoe" => json!({ "game": "tictactoe", "html": tictactoe_html(), "message": "TicTacToe game loaded." }),
        "truco" => {
            let state = start_truco_round();
            json!({
                "game": "truco",
                "kind": "json",
                "state": state,
                "message": "Truco game JSON initialized with dealt cards.",
            })
        }
        _ => return None,
    })
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "play_minigame".into(),
        description: Some("Open a playable mini game against Mika (jokenpo = rock-paper-scissors, pong, tic-tac-toe, truco). Use when the user wants to play something with you right now.".into()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "game": { "type": "string", "enum": ["jokenpo", "pong", "tictactoe", "truco"] },
            },
            "required": ["game"],
        }),
        plugin_id: super::feature::GAMER_COMPANION_PLUGIN_ID.into(),
        requires_window: false,
    }
}

pub struct PlayMinigameHandler;

#[async_trait]
impl ToolHandler for PlayMinigameHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let game = args.get("game").and_then(Value::as_str).unwrap_or("");
        Ok(match build_payload_data(game) {
            Some(data) => {
                // Porte de `miniGameQueue.push(game)` — só para um `game` CONHECIDO, mesma ordem
                // do TS (empurra antes de montar o payload).
                if let Some(&id) = KNOWN_GAME_IDS.iter().find(|&&id| id == game) {
                    requested_queue().lock().unwrap_or_else(|e| e.into_inner()).push_back(id);
                }
                json!({ "success": true, "data": data })
            }
            None => json!({ "success": false, "error": format!("jogo desconhecido: {game}"), "errorCode": "invalid_args" }),
        })
    }
}

/// Serializa os testes (aqui e em `super::rpc`) que empurram/consomem [`REQUESTED_QUEUE`] — ela é
/// um `OnceLock` de processo e `cargo test` roda em paralelo, mesmo padrão de
/// `modules::features::mood::TOOL_REGISTRATION_ORDER`.
#[cfg(test)]
pub(crate) static QUEUE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn jokenpo_returns_html_under_the_game_key() {
        // Estes também empurram na fila global: sem a trava, o teste de "fila vazia" em
        // `super::rpc` via o jogo deles e falhava metade das vezes.
        let _order = QUEUE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = PlayMinigameHandler.call(json!({ "game": "jokenpo" })).await.unwrap();
        while next_requested().is_some() {}
        assert_eq!(result["success"], true);
        assert_eq!(result["data"]["game"], "jokenpo");
        assert!(result["data"]["html"].as_str().unwrap().contains("Pedra, Papel ou Tesoura"));
    }

    #[tokio::test]
    async fn truco_returns_a_json_kind_payload_with_dealt_hands() {
        // Estes também empurram na fila global: sem a trava, o teste de "fila vazia" em
        // `super::rpc` via o jogo deles e falhava metade das vezes.
        let _order = QUEUE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = PlayMinigameHandler.call(json!({ "game": "truco" })).await.unwrap();
        while next_requested().is_some() {}
        assert_eq!(result["success"], true);
        assert_eq!(result["data"]["kind"], "json");
        assert_eq!(result["data"]["game"], "truco");
        assert_eq!(result["data"]["state"]["userHand"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn an_unknown_game_is_a_named_invalid_args_error_not_a_panic() {
        let result = PlayMinigameHandler.call(json!({ "game": "chess" })).await.unwrap();
        assert_eq!(result["success"], false);
        assert_eq!(result["errorCode"], "invalid_args");
    }

    /// Prova a fila `miniGameQueue`/`getNextRequestedMiniGame` — porte remedido nesta rodada.
    #[tokio::test]
    async fn calling_the_tool_enqueues_the_game_for_the_requested_rpc_to_pick_up() {
        let _order = QUEUE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        while next_requested().is_some() {} // drena resíduo de outro teste que rodou antes deste

        assert_eq!(next_requested(), None, "fila deveria estar vazia antes do teste");
        PlayMinigameHandler.call(json!({ "game": "pong" })).await.unwrap();
        assert_eq!(next_requested(), Some("pong"));
        assert_eq!(next_requested(), None, "getNextRequestedMiniGame esvazia a fila (shift)");
    }

    /// NEGATIVO — um `game` desconhecido nunca entra na fila.
    #[tokio::test]
    async fn an_unknown_game_never_reaches_the_requested_queue() {
        let _order = QUEUE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        while next_requested().is_some() {}

        PlayMinigameHandler.call(json!({ "game": "chess" })).await.unwrap();
        assert_eq!(next_requested(), None);
    }
}
