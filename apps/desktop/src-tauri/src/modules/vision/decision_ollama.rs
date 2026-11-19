//! Decision provider `ollama`: a small local vision model (any Ollama model with vision) answers
//! the same four questions through structured output, mapped back to the `/v1/systemone` shape so
//! the verdict rules stay one.

use serde_json::{json, Value};

const MOMENTS: [&str; 5] = ["none", "boss", "death", "victory", "cutscene"];

/// JSON schema Ollama constrains the reply to (`format`).
pub(super) fn reply_schema() -> Value {
    let score = json!({"type": "number", "minimum": 0, "maximum": 1});
    json!({
        "type": "object",
        "properties": {
            "game": score, "moment": {"type": "string", "enum": MOMENTS},
            "momentConfidence": score, "danger": score, "notable": score
        },
        "required": ["game", "moment", "momentConfidence", "danger", "notable"]
    })
}

fn prompt() -> String {
    let questions = super::questions();
    let line = |name: &str| questions[name]["instructions"].as_str().unwrap_or_default().to_string();
    format!(
        "Look at the screenshot and answer with JSON only. Scores are probabilities from 0 to 1.\n\
         game: {}\nmoment: {} One of: {}. momentConfidence: how sure you are about moment.\n\
         danger: {}\nnotable: {}",
        line("game"),
        line("moment"),
        MOMENTS.join(", "),
        line("danger"),
        line("notable")
    )
}

/// Ollama wants bare base64, not a data URL.
fn bare_base64(image: &str) -> &str {
    image.split_once(";base64,").map_or(image, |(_, data)| data)
}

pub(super) fn request_body(model: &str, images: &[String]) -> Value {
    let images: Vec<&str> = images.iter().map(|image| bare_base64(image)).collect();
    json!({
        "model": model,
        "stream": false,
        "format": reply_schema(),
        "options": {"temperature": 0},
        "messages": [{"role": "user", "content": prompt(), "images": images}]
    })
}

/// Reads `/api/chat` into `/v1/systemone` answers. `None` when the reply is not the schema.
pub(super) fn answers_from_reply(body: &Value) -> Option<Value> {
    let reply: Value = serde_json::from_str(body.get("message")?.get("content")?.as_str()?).ok()?;
    let score = |name: &str| reply.get(name)?.as_f64().map(|p| p.clamp(0.0, 1.0));
    let moment = reply.get("moment")?.as_str()?;
    Some(json!({
        "game": {"type": "noul", "noul": score("game")?},
        "moment": {"type": "choice", "choice": moment, "confidence": score("momentConfidence")?},
        "danger": {"type": "noul", "noul": score("danger")?},
        "notable": {"type": "noul", "noul": score("notable")?},
    }))
}
