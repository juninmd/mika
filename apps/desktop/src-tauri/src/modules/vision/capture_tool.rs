//! `vision_capture` — as 3 actions despacham de verdade nesta rodada. `frame` (`handlers/capture-
//! screen.ts`) só captura, sem análise. `screenshot`/`watch_now` capturam PIXELS reais
//! ([`super::screen_capture`]) e chamam o Ollama local ([`super::llm_client`]) — porte funcional de
//! `handlers/take-screenshot.ts`/`handlers/watch-now.ts` (`generateText` com imagem), sem streaming
//! e sem o contexto de app em primeiro plano (capacidade nova, não portada — ver `mod.rs`).
//!
//! Toda captura passa pelo MESMO chokepoint de privacidade, gateado no adaptador
//! ([`super::screen_capture::capture_screen_frames`]), nunca pelo chamador: `vision.enabled` →
//! `privacy::is_screen_capture_allowed` → blocklist de janela visível. A ida à rede (mandar a
//! imagem para o Ollama) passa por `privacy::assert_network_allowed` dentro de
//! [`super::llm_client::analyze`] — modo offline com endpoint remoto barra ANTES do base64 sair.
//! Ver o cabeçalho de `mod.rs` (§3) para por que [`VisionCaptureHandler`] recebe o `ConfigStore` no
//! construtor em vez de resolvê-lo sozinha.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{llm_client, observation_events};
use super::responses::{capture_unavailable_result, disabled_result, label_screen_output, llm_error_result};
use super::screen_capture::{any_visible_window_blocked, capture_screen_frames, is_vision_enabled};
use crate::config::store::ConfigStore;
use crate::modules::privacy;
use crate::modules::tools::ToolHandler;

/// Cópia literal de `visionCaptureTool.description` (`tools/vision-tools.ts`).
pub(super) const DESCRIPTION: &str = "Look at the user's screen right now (all monitors). action=screenshot: capture AND describe — \"what am I looking at?\" (pt-BR \"olha a minha tela\", \"o que tem na minha tela?\"). action=watch_now: react/comment in first person — \"what do you think of this?\" (pt-BR \"comenta isso\", \"o que você acha?\"). action=frame: capture only. Reading text or naming people: vision_read.";

/// Alias -> valor canônico do enum (smoke item 7: Gemma mandou um `action` fora de
/// `["screenshot", "frame", "watch_now"]` duas vezes na mesma sessão). Sinônimos óbvios que o
/// modelo já usou em vez do literal exato caem no valor certo antes do `match` de despacho; um
/// valor já canônico, ou qualquer outra coisa não mapeada, passa direto (o `match` de despacho
/// continua sendo quem recusa o que sobrar).
fn canonical_action(action: &str) -> &str {
    match action {
        "capture" | "look" | "see" | "photo" | "picture" | "screen" => "screenshot",
        "grab" | "snapshot" => "frame",
        "react" | "comment" | "narrate" => "watch_now",
        other => other,
    }
}

fn quality_from(args: &Value) -> u8 {
    args["quality"].as_u64().map(|q| q.clamp(1, 100) as u8).unwrap_or(80)
}

/// `action=frame`. Dimensão de thumbnail fixa (o TS pede o tamanho real do monitor via
/// `desktopCapturer`; aqui pedimos um teto generoso porque `action=frame` NUNCA devolve os pixels —
/// só confirma quantos monitores foram capturados de verdade, igual ao TS).
fn frame(store: &ConfigStore, args: &Value) -> Value {
    if !is_vision_enabled(store) {
        return disabled_result();
    }
    // O CHOKEPOINT: checado aqui, no adaptador, antes de qualquer captura — nunca pelo chamador.
    if !privacy::is_screen_capture_allowed(store) {
        return capture_unavailable_result();
    }
    if any_visible_window_blocked(store) {
        return capture_unavailable_result();
    }

    let quality = quality_from(args);
    let payload = json!({
        "types": ["screen"],
        "thumbnailWidth": 1920,
        "thumbnailHeight": 1080,
        "thumbnailQuality": quality,
    });
    let Ok(Value::Array(sources)) = crate::capture::get_sources(&payload) else {
        return capture_unavailable_result();
    };

    let captured: Vec<Value> = sources
        .into_iter()
        .filter(|source| !source["thumbnailEmpty"].as_bool().unwrap_or(true))
        .map(|source| json!({ "sourceId": source["id"], "name": source["name"] }))
        .collect();

    if captured.is_empty() {
        return capture_unavailable_result();
    }

    json!({ "success": true, "data": { "monitorCount": captured.len(), "sources": captured, "quality": quality } })
}

const DEFAULT_WATCH_NOW_PROMPT: &str =
    "Describe what is happening on the screen as a natural first-person comment from someone watching beside them. 3 sentences at most.";

/// `action=screenshot`. Porte funcional de `handlers/take-screenshot.ts`: captura + `generateText` +
/// `emitScreenObservation` ([`super::observation_events::emit_screen_observation`]), com a janela
/// ativa do watcher de `system_signals`. `watch_now` (`handlers/watch-now.ts`) NÃO emite este evento
/// no TS — só `screenshot` publica.
async fn screenshot(store: &ConfigStore, app: Option<&tauri::AppHandle>, args: &Value) -> Value {
    let frames = match capture_screen_frames(store) {
        Ok(frames) => frames,
        Err(result) => return result,
    };
    let images = match llm_client::prepare_frames(store, frames, quality_from(args)).await {
        Ok(images) => images,
        Err(err) => return llm_error_result(&err),
    };

    let window = crate::modules::system_signals::active_window::last_known();
    let prompt = args["prompt"].as_str().filter(|p| !p.trim().is_empty()).map_or_else(|| observation_events::screenshot_prompt(window.as_ref()), str::to_string);

    match llm_client::analyze_with_usage(store, "vision:capture:screenshot", &prompt, &images).await {
        Ok(outcome) => {
            llm_client::record_usage(store, app, &outcome);
            observation_events::emit_screen_observation(store, &outcome.text, images.len(), window.as_ref());
            let foreground_app = observation_events::foreground_json(window.as_ref());
            json!({
                "success": true,
                "data": { "screenshotCaptured": true, "monitorCount": images.len(), "analysis": label_screen_output(&outcome.text), "foregroundApp": foreground_app }
            })
        }
        Err(err) => llm_error_result(&err),
    }
}

/// `action=watch_now`. Porte funcional de `handlers/watch-now.ts`: mesma captura, `generateText`
/// sem tools, resposta em primeira pessoa. Qualidade 85 (mesmo default do TS).
async fn watch_now(store: &ConfigStore, app: Option<&tauri::AppHandle>, args: &Value) -> Value {
    let frames = match capture_screen_frames(store) {
        Ok(frames) => frames,
        Err(result) => return result,
    };
    let images = match llm_client::prepare_frames(store, frames, 85).await {
        Ok(images) => images,
        Err(err) => return llm_error_result(&err),
    };

    let question = args["question"].as_str().filter(|q| !q.trim().is_empty());
    let prompt = match question {
        Some(q) => format!(
            "The user asks: \"{q}\". Look at the images and answer in first person, as Mika reacting to their screen. 3 sentences at most."
        ),
        None => DEFAULT_WATCH_NOW_PROMPT.to_string(),
    };

    match llm_client::analyze_with_usage(store, "vision:capture:watch_now", &prompt, &images).await {
        Ok(outcome) => {
            llm_client::record_usage(store, app, &outcome);
            json!({ "success": true, "data": { "response": label_screen_output(&outcome.text) } })
        }
        Err(err) => llm_error_result(&err),
    }
}

pub(super) struct VisionCaptureHandler {
    store: Arc<ConfigStore>,
    /// WP9 item 4 — ver `register`/`llm_client::record_usage`. `None` em todo teste deste arquivo.
    app: Option<tauri::AppHandle>,
}

impl VisionCaptureHandler {
    pub(super) fn new(store: Arc<ConfigStore>, app: Option<tauri::AppHandle>) -> Self {
        Self { store, app }
    }
}

#[async_trait]
impl ToolHandler for VisionCaptureHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        match canonical_action(args["action"].as_str().unwrap_or_default()) {
            "frame" => Ok(frame(&self.store, &args)),
            "screenshot" => Ok(screenshot(&self.store, self.app.as_ref(), &args).await),
            "watch_now" => Ok(watch_now(&self.store, self.app.as_ref(), &args).await),
            other => Err(format!("vision_capture: ação desconhecida \"{other}\"")),
        }
    }
}

/// Espelha `visionCaptureTool.inputSchema` (`z.object` → JSON Schema, `CAPTURE_ACTIONS` do TS).
pub(super) fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["screenshot", "frame", "watch_now"],
                "description": "Exactly one of: screenshot (capture AND describe what's visible), frame (capture only, no analysis), watch_now (react/comment/narrate in first person).",
            },
            "quality": { "type": "number", "minimum": 1, "maximum": 100 },
            "prompt": { "type": "string" },
            "question": { "type": "string", "description": "Specific question about the screen." },
        },
        "required": ["action"],
    })
}

#[cfg(test)]
#[path = "capture_tool_tests.rs"]
mod tests;
