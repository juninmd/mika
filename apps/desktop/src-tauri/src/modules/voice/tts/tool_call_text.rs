//! Tool calls a model writes as text instead of calling natively ("ela lê as tools"): none of them
//! is speech, so they leave before synthesis. Runs first in `sanitize_text`, before the bracket and
//! angle-bracket passes would strip only the markers and leave the JSON body to be read aloud.

use std::sync::LazyLock;

use regex::Regex;

static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // `<tool_call>{...}</tool_call>`, `<function_call>`, `<tool_code>` spans, body included.
        r"(?s)<(tool_call|function_call|tool_code)>.*?</(?:tool_call|function_call|tool_code)>",
        // Gemma: `<|tool_call>call:name{...}<tool_call|>` (and `tool_response`), body included.
        r"(?s)<\|tool_(?:call|response)>.*?<tool_(?:call|response)\|>",
        // Gemma/Mistral/Qwen markers and a lone marker left when a span was split across sentences.
        r"(?i)</?(?:tool_call|function_call|tool_code)>|<\|?[a-z_]*tool[a-z_]*\|>?|\[TOOL_CALLS\]",
        // `vision_capture({...})` / `desktop(action="launch")`: a snake_case name with a call.
        r"\b[a-z][a-z0-9]*(?:_[a-z0-9]+)+\s*\([^()]*\)",
        // `call:desktop{...}`.
        r"\bcall:[a-z_][a-z0-9_]*\s*\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}",
    ]
    .iter()
    .map(|pattern| Regex::new(pattern).expect("padrão de tool call textual inválido"))
    .collect()
});

/// A JSON object (one nesting level): the body a call marker leaves behind, `{"name": ...}`.
static JSON_BODY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\{\s*"[^"]*"\s*:[^{}]*(?:\{[^{}]*\}[^{}]*)*\}"#).expect("padrão de corpo JSON inválido"));

/// The markers a model writes in front of a call body. A bare JSON object without one is a
/// structured reply (facts, people, reflection), so [`JSON_BODY`] only runs when one is present.
static CALL_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)</?(?:tool_call|function_call|tool_code)>|<\|?[a-z_]*tool[a-z_]*\|>?|\[TOOL_CALLS\]|\bcall:[a-z_]").expect("padrão de marcador inválido")
});

pub(crate) fn strip_tool_call_text(text: &str) -> String {
    let marked = CALL_MARKER.is_match(text);
    let stripped = PATTERNS.iter().fold(text.to_string(), |acc, re| re.replace_all(&acc, " ").into_owned());
    if marked { JSON_BODY.replace_all(&stripped, " ").into_owned() } else { stripped }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spoken(text: &str) -> String {
        strip_tool_call_text(text).split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn a_tagged_tool_call_is_dropped_with_its_json_body() {
        assert_eq!(spoken(r#"Pode deixar. <tool_call>{"name": "vision_capture", "arguments": {}}</tool_call> Pronto!"#), "Pode deixar. Pronto!");
    }

    /// Smoke 2026-09-27: a fired reminder came back as Gemma's own call syntax and was shown.
    #[test]
    fn gemma_call_syntax_is_dropped_whole() {
        assert_eq!(spoken(r#"<|tool_call>call:reminder_manager{action: "get_reminders"}<tool_call|>"#), "");
        assert_eq!(spoken(r#"Ei! <|tool_call>call:x{a: 1}<tool_call|> Bebe água."#), "Ei! Bebe água.");
    }

    #[test]
    fn marker_tokens_and_a_following_json_body_are_dropped() {
        assert_eq!(spoken(r#"<|tool_call|>{"name": "avatar", "arguments": {"pose": "sitting"}}"#), "");
        assert_eq!(spoken(r#"[TOOL_CALLS] {"name": "desktop"}"#), "");
    }

    #[test]
    fn a_function_call_shape_and_a_call_prefix_are_dropped() {
        assert_eq!(spoken(r#"Olha só: vision_capture({"action": "screenshot"}) já vi."#), "Olha só: já vi.");
        assert_eq!(spoken(r#"call:desktop{"x":100,"y":200} feito"#), "feito");
    }

    #[test]
    fn normal_speech_with_parentheses_and_braces_in_prose_survives() {
        let line = "Sabe o Senbonzakura (a espada do Byakuya)? Chave {de casa} tá na mesa.";
        assert_eq!(spoken(line), line);
    }

    // Facts/people/reflection replies are JSON the caller parses: with no call marker in the text
    // the objects are data, and deleting them turned a correct reply into "" or "[ ]".
    #[test]
    fn a_structured_json_reply_without_a_call_marker_is_left_intact() {
        for reply in [
            r#"{"facts": []}"#,
            r#"{"facts": [{"fact": "nome", "value": "Ana"}]}"#,
            r#"[{"fact": "nome", "value": "Ana"}]"#,
            r#"{"people": [{"name": "Ana", "relation": "amiga"}]}"#,
            "```json\n{\"facts\": []}\n```",
        ] {
            assert_eq!(strip_tool_call_text(reply), reply, "{reply}");
        }
    }
}
