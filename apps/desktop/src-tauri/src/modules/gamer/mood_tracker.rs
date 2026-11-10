//! Port de `tools/mood-tracker.ts` — tool `track_gamer_mood`, pura e sem I/O: conta
//! palavras-gatilho nas mensagens recentes e devolve SINAIS estruturados (mood, tiltScore,
//! matchedSignals). A frase de acolhimento continua sendo gerada pelo LLM a partir desses sinais —
//! este handler nunca fala pela Mika (`check:no-hardcoded-speech` cobraria exatamente isso).

use crate::modules::tools::{ToolDefinition, ToolHandler};
use async_trait::async_trait;
use serde_json::{json, Value};

const TILT_KEYWORDS: &[&str] = &[
    "hate", "quit", "bullshit", "rage", "stress", "unfair", "lag", "trash", "lixo", "odeio", "desisto", "injusto",
    "merda", "caralho", "porra", "afk", "troll",
];

/// Uma regex de palavra inteira por gatilho, compilada uma vez: `analyze` compilava as 17 a cada
/// chamada da tool, e a compilação era praticamente todo o custo dela.
fn tilt_patterns() -> &'static [(&'static str, regex::Regex)] {
    static PATTERNS: std::sync::OnceLock<Vec<(&'static str, regex::Regex)>> = std::sync::OnceLock::new();
    PATTERNS.get_or_init(|| {
        TILT_KEYWORDS
            .iter()
            .filter_map(|keyword| {
                let pattern = format!(r"\b{}\b", regex::escape(keyword));
                regex::Regex::new(&pattern).ok().map(|re| (*keyword, re))
            })
            .collect()
    })
}

fn analyze(recent_messages: &[String], game_outcome: &str) -> Value {
    let text = recent_messages.join(" ").to_lowercase();
    let matched: Vec<&str> = tilt_patterns().iter().filter(|(_, re)| re.is_match(&text)).map(|(kw, _)| *kw).collect();

    let mut tilt_score = matched.len() as i64;
    if game_outcome == "loss" {
        tilt_score += 1;
    }
    let is_tilted = tilt_score >= 2;

    json!({
        "success": true,
        "mood": if is_tilted { "tilted" } else { "calm" },
        "tiltScore": tilt_score,
        "matchedSignals": matched,
        "gameOutcome": if game_outcome.is_empty() { "unknown" } else { game_outcome },
        "recommendedAction": if is_tilted { "suggest_short_break" } else { "encourage" },
    })
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "track_gamer_mood".into(),
        description: Some(
            "Detect whether the user is tilted (stressed/angry) from their recent game chat or voice lines — when they sound frustrated mid-match or after losses. Returns signals (mood, tiltScore, matches): write YOUR OWN calming or hype line; never read them back."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "recentMessages": { "type": "array", "items": { "type": "string" }, "description": "The user's latest lines, oldest first." },
                "gameOutcome": { "type": "string", "enum": ["win", "loss", "draw", "ongoing"] },
            },
            "required": ["recentMessages"],
        }),
        plugin_id: super::feature::GAMER_COMPANION_PLUGIN_ID.into(),
        requires_window: false,
    }
}

pub struct MoodTrackerHandler;

#[async_trait]
impl ToolHandler for MoodTrackerHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let recent_messages: Vec<String> = args
            .get("recentMessages")
            .and_then(Value::as_array)
            .ok_or("recentMessages é obrigatório e deve ser um array de strings")?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let game_outcome = args.get("gameOutcome").and_then(Value::as_str).unwrap_or("");

        Ok(analyze(&recent_messages, game_outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn calm_with_no_tilt_keywords() {
        let result = analyze(&messages(&["boa partida, vamos de novo"]), "");
        assert_eq!(result["mood"], "calm");
        assert_eq!(result["tiltScore"], 0);
        assert_eq!(result["gameOutcome"], "unknown");
    }

    #[test]
    fn two_matched_keywords_cross_the_tilt_threshold() {
        let result = analyze(&messages(&["que lag desgraçado, hate isso"]), "ongoing");
        assert_eq!(result["mood"], "tilted");
        assert_eq!(result["tiltScore"], 2);
        assert_eq!(result["recommendedAction"], "suggest_short_break");
    }

    #[test]
    fn a_loss_adds_one_to_the_tilt_score() {
        let result = analyze(&messages(&["que lag"]), "loss");
        assert_eq!(result["tiltScore"], 2);
        assert_eq!(result["mood"], "tilted");
    }

    #[test]
    fn word_boundary_prevents_substring_false_positives() {
        // "afk" não deveria casar dentro de "cafka" (nome fictício de jogador).
        let result = analyze(&messages(&["cafka acabou de entrar"]), "");
        assert_eq!(result["tiltScore"], 0);
    }

    #[test]
    fn matched_signals_preserve_keyword_declaration_order_not_message_order() {
        let result = analyze(&messages(&["stress e depois hate"]), "");
        assert_eq!(result["matchedSignals"], json!(["hate", "stress"]));
    }

    #[tokio::test]
    async fn handler_rejects_missing_recent_messages() {
        let err = MoodTrackerHandler.call(json!({})).await.unwrap_err();
        assert!(err.contains("recentMessages"));
    }

    #[tokio::test]
    async fn handler_round_trips_through_the_tool_registry_contract() {
        let result = MoodTrackerHandler.call(json!({ "recentMessages": ["odeio isso", "que merda"] })).await.unwrap();
        assert_eq!(result["mood"], "tilted");
    }
}
