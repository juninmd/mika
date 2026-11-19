//! Decision-model pre-filter for the screen watcher (`vision.decisionEnabled`).
//!
//! A decision model (LiquidAI d1-2B served by `llama-server`, `POST /v1/systemone`) answers named
//! questions about one screenshot in a single forward pass, with probabilities and no generated
//! text. The watcher asks it first and only pays for the full vision LLM when the frame is worth it:
//! a game moment or danger right now, or something notable once the normal cadence is due.
//! Two providers (`vision.decisionProvider`): `llamaServer` (d1-2B, `/v1/systemone`) and `ollama`
//! (any local vision model in Ollama, `vision.decisionModel`, structured output).
//! Loopback only: the frame never leaves the machine through this path.

use std::time::Duration;

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::llm::provider_health;

/// Health-registry key: failures show up in the provider banner like any other vision provider.
pub(super) const PROVIDER: &str = "decision";
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080";
const OLLAMA_DEFAULT_BASE_URL: &str = crate::config::schemas::llm_defaults::OLLAMA_BASE_URL;
const DEFAULT_THRESHOLD: f64 = 0.5;
/// A CPU-only box takes seconds per frame; a GPU takes milliseconds.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Loop cadence while a game is in focus: the decision is cheap, the LLM call stays gated.
pub(super) const GAME_INTERVAL_MS: u64 = 3_000;

/// What the decision model says about one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Verdict {
    /// A game moment or a danger: worth an analysis right now, cadence or not.
    pub urgent: bool,
    /// Worth an analysis when the normal cadence is due.
    pub notable: bool,
}

pub(super) fn enabled(store: &ConfigStore) -> bool {
    store.get("vision.decisionEnabled").and_then(|v| v.as_bool()).unwrap_or(false)
}

#[path = "decision_ollama.rs"]
mod ollama;

fn stored(store: &ConfigStore, path: &str) -> Option<String> {
    store.get(path).and_then(|v| v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string))
}

fn uses_ollama(store: &ConfigStore) -> bool {
    stored(store, "vision.decisionProvider").as_deref() == Some("ollama")
}

/// Server the frame goes to: the decision URL for llama-server, the shared Ollama URL otherwise.
pub(super) fn base_url(store: &ConfigStore) -> String {
    if uses_ollama(store) {
        let base = stored(store, "apiKeys.ollamaBaseUrl").unwrap_or_else(|| OLLAMA_DEFAULT_BASE_URL.to_string());
        let trimmed = base.trim_end_matches('/');
        return trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string();
    }
    stored(store, "vision.decisionBaseUrl").unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

fn threshold(store: &ConfigStore) -> f64 {
    store.get("vision.decisionThreshold").and_then(|v| v.as_f64()).unwrap_or(DEFAULT_THRESHOLD).clamp(0.05, 0.95)
}

/// What the watcher does with a frame after asking the decision model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Gate {
    Analyze,
    /// Nothing worth an analysis: the frame counts as seen.
    Boring,
    /// Maybe worth it, but the cadence is not due: the frame stays unseen for a later tick.
    NotDue,
}

/// Pure gate: urgent always passes, notable passes when due, a frame the model calls boring is
/// dropped. Without a verdict (server down, suspended) the plain cadence decides.
pub(super) fn gate(verdict: Option<Verdict>, due: bool) -> Gate {
    match verdict {
        Some(v) if v.urgent || (due && v.notable) => Gate::Analyze,
        Some(v) if !v.notable => Gate::Boring,
        None if due => Gate::Analyze,
        _ => Gate::NotDue,
    }
}

pub(super) fn questions() -> Value {
    json!({
        "game": {"type": "noul", "instructions": "Is a video game being played on this screen (not a menu, launcher or store page)?"},
        "moment": {"type": "choice", "instructions": "Which game moment is on screen right now?", "criteria": {
            "none": "Ordinary gameplay or no game",
            "boss": "A boss fight",
            "death": "The player died or is about to die",
            "victory": "A win, level clear or reward screen",
            "cutscene": "A cutscene or story scene"
        }},
        "danger": {"type": "noul", "instructions": "Is there a visible threat the player may not have noticed: an enemy close by, critically low health or an incoming attack?"},
        "notable": {"type": "noul", "instructions": "Is there something on screen worth a short comment from a friend watching: funny, surprising, exciting or new?"}
    })
}

/// Reads `/v1/systemone` answers. Moment and danger only count inside a game, so a code editor
/// misread as a "victory screen" never forces an analysis. `None` when an answer is missing.
pub(super) fn verdict_from(answers: &Value, threshold: f64) -> Option<Verdict> {
    let noul = |name: &str| answers.get(name)?.get("noul")?.as_f64();
    let moment = answers.get("moment")?;
    let moment_hit = moment.get("choice")?.as_str()? != "none" && moment.get("confidence")?.as_f64()? >= threshold;
    let in_game = noul("game")? >= threshold;
    let urgent = in_game && (moment_hit || noul("danger")? >= threshold);
    Some(Verdict { urgent, notable: urgent || noul("notable")? >= threshold })
}

/// Its own client: no redirects and no system proxy, so the frame cannot leave the loopback
/// address the caller checked.
fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("decision http client")
    })
}

fn as_data_url(image: &str) -> String {
    if image.starts_with("data:") {
        image.to_string()
    } else {
        format!("data:image/jpeg;base64,{image}")
    }
}

async fn post_json(url: String, body: &Value) -> Result<Value, String> {
    let response = http_client().post(url).timeout(REQUEST_TIMEOUT).json(body).send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    response.json::<Value>().await.map_err(|err| err.to_string())
}

const MISSING_ANSWERS: &str = "resposta sem as respostas esperadas";

/// `/v1/systemone` answers from whichever provider is configured.
async fn ask(store: &ConfigStore, url: &str, images: &[String]) -> Result<Value, String> {
    if uses_ollama(store) {
        let model = stored(store, "vision.decisionModel").ok_or("nenhum modelo de decisão escolhido")?;
        let body = post_json(format!("{url}/api/chat"), &ollama::request_body(&model, images)).await?;
        return ollama::answers_from_reply(&body).ok_or_else(|| MISSING_ANSWERS.to_string());
    }
    let images: Vec<String> = images.iter().map(|image| as_data_url(image)).collect();
    let body = json!({ "state": Value::Null, "images": images, "questions": questions() });
    let mut reply = post_json(format!("{}/v1/systemone", url.trim_end_matches('/')), &body).await?;
    Ok(reply.get_mut("answers").map(Value::take).unwrap_or(Value::Null))
}

/// Asks the decision model about one frame (every monitor the analysis mode keeps). `None` = no usable verdict (non-loopback URL,
/// suspended after repeated failures, server error): the caller falls back to the plain cadence.
pub(super) async fn decide(store: &ConfigStore, images: &[String]) -> Option<Verdict> {
    if images.is_empty() {
        return None;
    }
    let url = base_url(store);
    if !crate::modules::privacy::is_loopback_url(&url) {
        log::warn!("vision decision: {url} is not loopback, decision model skipped");
        return None;
    }
    if crate::modules::privacy::assert_network_allowed(store, &url, PROVIDER).is_err() {
        return None;
    }
    if provider_health::is_blocked(PROVIDER, &url).is_some() {
        return None;
    }
    let started = std::time::Instant::now();
    let verdict = ask(store, &url, images)
        .await
        .and_then(|answers| verdict_from(&answers, threshold(store)).ok_or_else(|| MISSING_ANSWERS.to_string()));
    match verdict {
        Ok(v) => {
            provider_health::record_success(PROVIDER, &url);
            log::debug!("vision decision: {v:?} in {} ms", started.elapsed().as_millis());
            Some(v)
        }
        Err(reason) => {
            log::debug!("vision decision: {reason}");
            provider_health::record_failure_for_surface(PROVIDER, &url, &reason, provider_health::extract_http_status(&reason), "vision");
            None
        }
    }
}

#[cfg(test)]
#[path = "decision_tests.rs"]
mod tests;
