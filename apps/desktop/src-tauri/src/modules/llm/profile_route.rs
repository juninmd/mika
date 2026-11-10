//! `llm:getModelProfile` — o mesmo [`ModelProfile`] que o turno usa, entregue à aba LLM.
//!
//! A tela limitava o contexto por um catálogo próprio (só Ollama) e mostrava `topP`/raciocínio para
//! qualquer modelo, enquanto o turno descartava esses campos quando o perfil negava suporte: o
//! usuário mexia num controle que o modelo escolhido ignorava. Ler o perfil daqui mantém uma fonte só.

use serde_json::{json, Value};

use super::model_profile::ModelProfile;
use crate::modules::brain::provider_params::thinking_budget_cap;

const PROVIDERS: &[&str] = &["google", "openrouter", "ollama", "openaiCompatible", "unsloth"];
const MAX_MODEL_ID_LEN: usize = 256;

/// Provedor por lista fechada; o modelo só vira chave de cache e argumento das buscas já gateadas
/// de `model_profile::fetch` (id Google ainda passa por `is_safe_google_id`).
pub fn parse_request(payload: &Value) -> Result<(String, String), String> {
    let provider = payload.get("provider").and_then(Value::as_str).filter(|p| PROVIDERS.contains(p));
    let model = payload.get("model").and_then(Value::as_str).map(str::trim).filter(|m| !m.is_empty() && m.len() <= MAX_MODEL_ID_LEN);
    match (provider, model) {
        (Some(provider), Some(model)) => Ok((provider.to_string(), model.to_string())),
        _ => Err("llm:getModelProfile exige provider conhecido e model não vazio".to_string()),
    }
}

/// `None` do perfil vira campo ausente: "o provedor não disse" não é "não suporta".
pub fn to_response(model: &str, profile: &ModelProfile) -> Value {
    let cap = thinking_budget_cap(model, profile);
    json!({
        "contextWindow": profile.context_window,
        "maxOutputTokens": profile.max_output_tokens,
        "reasoning": profile.reasoning,
        "topP": profile.top_p,
        "thinkingBudgetMax": (cap != u64::MAX).then_some(cap),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_needs_a_known_provider_and_a_bounded_model_id() {
        assert_eq!(parse_request(&json!({ "provider": "ollama", "model": " qwen3:8b " })), Ok(("ollama".into(), "qwen3:8b".into())));
        for bad in [
            json!({ "provider": "anthropic", "model": "x" }),
            json!({ "provider": "google", "model": "  " }),
            json!({ "provider": "google" }),
            json!({ "provider": "google", "model": "a".repeat(MAX_MODEL_ID_LEN + 1) }),
        ] {
            assert!(parse_request(&bad).is_err(), "{bad}");
        }
    }

    /// Gemini 2.5 Flash aceita 65 536 de saída, mas o orçamento de raciocínio para em 24 576 — o
    /// slider não pode oferecer o que o turno corta em silêncio.
    #[test]
    fn response_caps_the_thinking_budget_at_the_family_limit_and_keeps_unknowns_null() {
        let flash = ModelProfile { context_window: Some(1_048_576), max_output_tokens: Some(65_536), reasoning: Some(true), top_p: Some(true) };
        let body = to_response("gemini-2.5-flash", &flash);
        assert_eq!(body["contextWindow"], json!(1_048_576));
        assert_eq!(body["thinkingBudgetMax"], json!(24_576));

        let unknown = to_response("qwen3:8b", &ModelProfile::default());
        assert!(unknown["contextWindow"].is_null() && unknown["reasoning"].is_null() && unknown["thinkingBudgetMax"].is_null());
    }
}
