//! `match_effect` — porte COMPLETO de `matchEffectCommandTool`
//! (`apps/desktop/src/main/services/brain/tools/effect-commands.ts`): função pura, sem LLM nem
//! `ConfigStore`, a ÚNICA das 5 tools de `brain` com paridade total nesta rodada (ver a tabela em
//! `tools/mod.rs`).

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::modules::tools::ToolHandler;

pub(super) const DESCRIPTION: &str = "Full-screen visual effect from a command: /matrix, /sakura, /cosmos, /synthwave, /fireworks (aliases like /party, /retro), or \"surpreenda-me\" for a random one. Use when the user types one or asks for an effect; pass their text.";

/// Espelha `EFFECT_TRIGGERS` — chave já em minúsculas, igual ao `lower` que o TS compara.
const TRIGGERS: &[(&str, &str)] = &[
    ("/matrix", "matrix"),
    ("/hack", "matrix"),
    ("wake up neo", "matrix"),
    ("/neural", "constellation"),
    ("/cosmos", "constellation"),
    ("/connect", "constellation"),
    ("/sakura", "sakura"),
    ("/dream", "sakura"),
    ("/zen", "sakura"),
    ("/sonho", "sakura"),
    ("/synthwave", "synthwave"),
    ("/retro", "synthwave"),
    ("/80s", "synthwave"),
    ("/fireworks", "fireworks"),
    ("/celebrate", "fireworks"),
    ("/party", "fireworks"),
    ("/comemorar", "fireworks"),
];

const RANDOM_EFFECTS: [&str; 5] = ["matrix", "constellation", "sakura", "synthwave", "fireworks"];
const SURPRISE_KEYWORDS: [&str; 2] = ["surpreenda-me", "surpreenda me"];

/// Índice pseudo-aleatório determinístico por chamada — não precisa ser criptográfico, só
/// distribuir entre os 5 efeitos como `Math.floor(Math.random() * n)` faz no TS. Deriva do relógio
/// em vez de trazer uma dependência de RNG só para isto.
fn random_index(len: usize) -> usize {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos as usize) % len
}

pub(super) fn run(text: &str) -> Value {
    let lower = text.trim().to_lowercase();

    if let Some((_, effect)) = TRIGGERS.iter().find(|(trigger, _)| *trigger == lower) {
        return json!({ "success": true, "data": { "type": "toggle", "effect": effect, "triggered": true } });
    }

    if SURPRISE_KEYWORDS.contains(&lower.as_str()) {
        let effect = RANDOM_EFFECTS[random_index(RANDOM_EFFECTS.len())];
        return json!({ "success": true, "data": { "type": "random", "effect": effect, "triggered": true } });
    }

    json!({ "success": true, "data": { "triggered": false } })
}

pub(super) struct MatchEffectHandler;

#[async_trait]
impl ToolHandler for MatchEffectHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        Ok(run(args["text"].as_str().unwrap_or_default()))
    }
}

/// Espelha `matchEffectCommandTool.inputSchema` (`z.object({ text: z.string() })`).
pub(super) fn schema() -> Value {
    json!({ "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_a_trigger_case_insensitively_and_trims() {
        assert_eq!(
            run("  /MATRIX  "),
            json!({ "success": true, "data": { "type": "toggle", "effect": "matrix", "triggered": true } })
        );
        assert_eq!(
            run("Wake Up Neo"),
            json!({ "success": true, "data": { "type": "toggle", "effect": "matrix", "triggered": true } })
        );
    }

    #[test]
    fn every_documented_trigger_resolves_to_its_effect() {
        for (trigger, effect) in TRIGGERS {
            assert_eq!(run(trigger)["data"]["effect"], json!(*effect), "trigger {trigger}");
        }
    }

    #[test]
    fn surprise_keyword_returns_a_random_effect_from_the_documented_set() {
        let result = run("surpreenda-me");
        assert_eq!(result["data"]["type"], json!("random"));
        let effect = result["data"]["effect"].as_str().unwrap();
        assert!(RANDOM_EFFECTS.contains(&effect), "effect {effect} fora do conjunto");
    }

    #[test]
    fn unmatched_text_is_a_success_with_triggered_false_not_an_error() {
        assert_eq!(run("oi mika, tudo bem?"), json!({ "success": true, "data": { "triggered": false } }));
    }

    #[test]
    fn empty_text_does_not_panic_and_is_not_triggered() {
        assert_eq!(run(""), json!({ "success": true, "data": { "triggered": false } }));
    }

    #[tokio::test]
    async fn handler_reads_text_from_args_and_defaults_missing_text_to_empty() {
        let handler = MatchEffectHandler;
        let result = handler.call(json!({ "text": "/sakura" })).await.unwrap();
        assert_eq!(result["data"]["effect"], json!("sakura"));

        let missing = handler.call(json!({})).await.unwrap();
        assert_eq!(missing, json!({ "success": true, "data": { "triggered": false } }));
    }
}
