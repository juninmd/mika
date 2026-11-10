//! O único pedaço puro de `handleScreenshotShortcut` (`vision-proactive.ts`): o payload de
//! `Events.BRAIN_PROACTIVE_THOUGHT` que o atalho de captura publica depois que
//! `vision_capture(action=screenshot)` devolve a análise. Vive em `modules::vision`, e não em
//! `shortcuts.rs` (que só despacha), para não empurrar esse arquivo (já perto do teto de 300
//! linhas de `AGENTS.md`) para cima — o chamador real é `crate::shortcuts::dispatch_screenshot_shortcut`.

use serde_json::{json, Value};

/// Porte do corpo de `ctx.events.emit(Events.BRAIN_PROACTIVE_THOUGHT, {...})` em
/// `handleScreenshotShortcut`.
pub(crate) fn proactive_thought_payload_for_screenshot(analysis: &str) -> Value {
    json!({
        "source": "screenshot-shortcut",
        "context": format!("The user pressed the screenshot shortcut and wants you to look at the screen now. Capture analysis:\n{analysis}"),
        "priority": 5,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O payload que o atalho de captura publica bate com o shape que
    /// `modules::brain::thought::handle_proactive_event` espera (`source`/`context`/`priority`),
    /// mesma tripla que `handleScreenshotShortcut` (TS) emitia via `Events.BRAIN_PROACTIVE_THOUGHT`.
    #[test]
    fn screenshot_shortcut_payload_matches_the_ts_proactive_thought_shape() {
        let payload = proactive_thought_payload_for_screenshot("tem um bug na linha 42");
        assert_eq!(payload["source"], json!("screenshot-shortcut"));
        assert_eq!(payload["priority"], json!(5));
        let context = payload["context"].as_str().unwrap();
        assert!(context.starts_with("The user pressed the screenshot shortcut"));
        assert!(context.contains("tem um bug na linha 42"));
    }
}
