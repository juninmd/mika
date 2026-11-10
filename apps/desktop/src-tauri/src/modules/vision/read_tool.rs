//! `vision_read` — as 3 actions despacham de verdade nesta rodada, porte funcional de
//! `handlers/analyze-image.ts`/`handlers/ocr-screen.ts`/`handlers/scan-people.ts`.
//!
//! `describe` recebe a imagem do CHAMADOR (`imageBase64`/`images`) — sem captura de tela, só o
//! chokepoint de rede (`vision.enabled` + `privacy::assert_network_allowed` dentro de
//! [`super::llm_client::analyze`]). `ocr`/`people` capturam a tela primeiro, gateadas pelo MESMO
//! chokepoint de [`super::screen_capture::capture_screen_frames`] que `capture_tool` usa.
//!
//! `people` (parse de JSON estruturado por prompt, sem tipo Zod — ver o cabeçalho de
//! [`super::people_tool`]) mora em arquivo próprio só pelo limite de 300 linhas.
//! Sem cache de 15s (`OCR_CACHE_TTL_MS`, TS) em `ocr` — otimização, não corretude.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::llm_client;
use super::people_tool::people;
use super::responses::{label_screen_output, llm_error_result};
use super::screen_capture::capture_screen_frames;
use crate::config::store::ConfigStore;
use crate::modules::tools::ToolHandler;
use std::sync::Arc;

/// Cópia literal de `visionReadTool.description` (`tools/vision-tools.ts`).
pub(super) const DESCRIPTION: &str = "Vision on text, people and shared images. action=ocr: read on-screen text — subtitles, dialogue, errors, code (`region` narrows it). action=people: identify real people or fictional characters on screen (\"who is this?\", pt-BR \"quem é essa pessoa?\"). action=describe: answer about an image the user shared (`imageBase64`/`images`). For a general look at the screen use vision_capture.";

fn is_vision_enabled(store: &ConfigStore) -> bool {
    store.get("vision.enabled").and_then(|value| value.as_bool()).unwrap_or(false)
}

/// Mesmo alias-mapping de `capture_tool::canonical_action` (smoke item 7: Gemma mandou um `action`
/// fora de `["describe", "ocr", "people"]`) — sinônimo óbvio cai no valor certo antes do `match` de
/// despacho; canônico ou não mapeado passa direto.
fn canonical_action(action: &str) -> &str {
    match action {
        "read" | "text" | "transcribe" => "ocr",
        "look" | "see" | "image" | "analyze" | "analyse" => "describe",
        "who" | "identify" | "person" | "character" => "people",
        other => other,
    }
}

/// `action=describe`. Aceita `imageBase64` (uma imagem) OU `images` (várias, cada item com
/// `imageBase64`) — mesma leitura de `getVisionImages` (TS), entradas vazias filtradas fora.
fn images_from_args(args: &Value) -> Vec<String> {
    if let Some(items) = args["images"].as_array() {
        return items
            .iter()
            .filter_map(|item| item["imageBase64"].as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    }
    args["imageBase64"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| vec![s.to_string()])
        .unwrap_or_default()
}

async fn describe(store: &ConfigStore, app: Option<&tauri::AppHandle>, args: &Value) -> Value {
    if !is_vision_enabled(store) {
        return json!({ "success": false, "error": "Vision plugin is disabled in settings.", "errorCode": "not_configured" });
    }
    let images = images_from_args(args);
    if images.is_empty() {
        return json!({ "success": false, "error": "action=describe requires imageBase64 or images.", "errorCode": "invalid_args" });
    }
    let image_count = images.len();
    // WP9 item 4: mesma recompressão que `capture_tool`/`watcher` já rodam antes de
    // `analyze_with_usage` — ver o cabeçalho de `llm_client::prepare_images`.
    let images = match llm_client::prepare_images(store, &images).await {
        Ok(images) => images,
        Err(err) => return llm_error_result(&err),
    };

    let prompt = args["prompt"].as_str().filter(|p| !p.trim().is_empty()).unwrap_or("Describe what you see in this image in detail.");

    match llm_client::analyze_with_usage(store, "vision:read:describe", prompt, &images).await {
        Ok(outcome) => {
            llm_client::record_usage(store, app, &outcome);
            json!({ "success": true, "data": { "analysis": label_screen_output(&outcome.text), "imageCount": image_count } })
        }
        Err(err) => llm_error_result(&err),
    }
}

/// Inline prompt (the unwired `prompts/vision/ocr-screen.md` copy was deleted 2026-09-28).
fn ocr_prompt(region: &str, context: Option<&str>) -> String {
    let context_line = context.map(|c| format!(" Context: {c}.")).unwrap_or_default();
    format!(
        "Extract ALL relevant text from the \"{region}\" region of the screen.{context_line} Return only the transcribed text, organized in blocks if needed. For game dialogue or subtitles, keep the order."
    )
}

/// `action=ocr`. Mesma qualidade de captura do TS (90). Sem o cache de 15s (`OCR_CACHE_TTL_MS`) —
/// otimização, não corretude; cada chamada aqui analisa de novo, honesto por ora.
async fn ocr(store: &ConfigStore, app: Option<&tauri::AppHandle>, args: &Value) -> Value {
    let frames = match capture_screen_frames(store) {
        Ok(frames) => frames,
        Err(result) => return result,
    };
    let images = match llm_client::prepare_frames(store, frames, 90).await {
        Ok(images) => images,
        Err(err) => return llm_error_result(&err),
    };

    let region = args["region"].as_str().unwrap_or("full");
    let context = args["context"].as_str().filter(|c| !c.trim().is_empty());
    let prompt = ocr_prompt(region, context);

    match llm_client::analyze_with_usage(store, "vision:read:ocr", &prompt, &images).await {
        Ok(outcome) => {
            llm_client::record_usage(store, app, &outcome);
            json!({ "success": true, "data": { "text": label_screen_output(&outcome.text), "region": region } })
        }
        Err(err) => llm_error_result(&err),
    }
}

pub(super) struct VisionReadHandler {
    store: Arc<ConfigStore>,
    /// WP9 item 4 — ver `mod.rs::register`/`llm_client::record_usage`. `None` em todo teste deste
    /// arquivo (mesmo motivo de `capture_tool::VisionCaptureHandler`).
    app: Option<tauri::AppHandle>,
}

impl VisionReadHandler {
    pub(super) fn new(store: Arc<ConfigStore>, app: Option<tauri::AppHandle>) -> Self {
        Self { store, app }
    }
}

#[async_trait]
impl ToolHandler for VisionReadHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        match canonical_action(args["action"].as_str().unwrap_or_default()) {
            "describe" => Ok(describe(&self.store, self.app.as_ref(), &args).await),
            "ocr" => Ok(ocr(&self.store, self.app.as_ref(), &args).await),
            "people" => Ok(people(&self.store, self.app.as_ref(), &args).await),
            other => Err(format!("vision_read: ação desconhecida \"{other}\"")),
        }
    }
}

/// Espelha `visionReadTool.inputSchema` (`READ_ACTIONS` do TS).
pub(super) fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["describe", "ocr", "people"],
                "description": "Exactly one of: describe (answer questions about a provided image), ocr (read/transcribe on-screen text), people (identify who/what is on screen).",
            },
            "imageBase64": { "type": "string" },
            "images": { "type": "array" },
            "prompt": { "type": "string" },
            "region": { "type": "string", "enum": ["full", "bottom", "top", "center"], "description": "action=ocr: screen area (bottom = subtitles/dialogue)." },
            "context": { "type": "string" },
            "focus": { "type": "string" },
        },
        "required": ["action"],
    })
}

#[cfg(test)]
#[path = "read_tool_tests.rs"]
mod tests;
