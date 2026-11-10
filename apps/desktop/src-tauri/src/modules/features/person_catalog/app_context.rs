//! Jogo em uso para a extração de menções — porte do `game.current` de
//! `person-catalog-lifecycle.ts` (`GAME_CHANGED`/`GAME_STOPPED`) e do `appContext` de
//! `person-catalog-mention-detect.ts`.

use serde_json::Value;

use crate::events::EventKey;

/// Atualiza o jogo corrente a partir de um evento do barramento; devolve `true` se o evento era de jogo.
pub(super) fn track_game(current: &mut Option<String>, key: EventKey, payload: &Value) -> bool {
    match key {
        EventKey::GameChanged => *current = payload.get("game").and_then(Value::as_str).map(str::to_string),
        EventKey::GameStopped => *current = None,
        _ => return false,
    }
    true
}

/// Texto que o TS interpolava no prompt: dá ao modelo a pista para preencher `role` ("duo in X").
pub(super) fn extraction_context(game: &str) -> String {
    format!(
        "the user was playing \"{game}\" when they said this. If the text suggests the mentioned person plays with them, \
         that can inform \"role\" (e.g. \"duo in {game}\")."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Sem isto a extração nunca sabia o jogo: `app_context` era sempre `None` e a menção sem `appName`.
    #[test]
    fn game_changed_sets_and_game_stopped_clears_the_current_game() {
        let mut current = None;
        assert!(track_game(&mut current, EventKey::GameChanged, &json!({ "game": "Valorant", "initial": false })));
        assert_eq!(current.as_deref(), Some("Valorant"));
        assert!(!track_game(&mut current, EventKey::ToolResult, &json!({})));
        assert_eq!(current.as_deref(), Some("Valorant"));
        assert!(track_game(&mut current, EventKey::GameStopped, &json!({})));
        assert_eq!(current, None);
    }

    #[test]
    fn extraction_context_names_the_game_for_the_role_hint() {
        assert!(extraction_context("Valorant").contains("duo in Valorant"));
    }
}
