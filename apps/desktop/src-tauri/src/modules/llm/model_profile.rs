//! O que cada MODELO aceita, perguntado ao próprio provedor — nunca deduzido do nome.
//!
//! Antes disto o turno mandava os mesmos parâmetros para qualquer modelo: o Ollama sempre recebia
//! `num_ctx` 32 768 (acima da janela de treino de modelos de 8k, abaixo da de modelos de 128k), e
//! `topP`/`enableThinking`/`thinkingMaxTokens` não chegavam a Google/OpenRouter. Mandar `reasoning`
//! a um modelo OpenRouter sem raciocínio é 400, não parâmetro ignorado — por isso cada campo aqui é
//! `Option`: `None` = "o provedor não disse", e quem monta o request não envia nada nesse caso.
//!
//! Fontes: OpenRouter `GET /api/v1/models` (`context_length`, `top_provider.max_completion_tokens`,
//! `supported_parameters`); Google `GET v1beta/models/{id}` (`inputTokenLimit`, `outputTokenLimit`,
//! `thinking`); Ollama `POST /api/show` (`model_info.*.context_length`, `capabilities`); llama-server
//! `GET /props` na RAIZ (`default_generation_settings.n_ctx` — o contexto CARREGADO, não o de treino).

use crate::config::schemas::provider_endpoints as endpoints;
use crate::config::schemas::llm_defaults;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use reqwest::Client as ReqwestClient;
use serde_json::Value;

use crate::config::store::ConfigStore;
use crate::modules::privacy;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelProfile {
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub reasoning: Option<bool>,
    pub top_p: Option<bool>,
}

const GOOGLE_MODEL_BASE: &str = endpoints::GOOGLE_AI_MODEL_BASE;
const OPENROUTER_MODELS_URL: &str = endpoints::OPENROUTER_MODELS_URL;
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);
/// Falha também fica em cache (curta): offline ou chave errada não pode custar uma requisição por turno.
const HIT_TTL: Duration = Duration::from_secs(30 * 60);
const MISS_TTL: Duration = Duration::from_secs(2 * 60);
const CACHE_CAP: usize = 256;

fn u64_at(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(Value::as_u64).filter(|&n| n > 0)
}

pub fn parse_openrouter(body: &Value, model: &str) -> Option<ModelProfile> {
    let entry = body.get("data")?.as_array()?.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(model))?;
    let params: Vec<&str> =
        entry.get("supported_parameters").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    Some(ModelProfile {
        context_window: u64_at(entry, "/context_length").or_else(|| u64_at(entry, "/top_provider/context_length")),
        max_output_tokens: u64_at(entry, "/top_provider/max_completion_tokens"),
        reasoning: Some(params.contains(&"reasoning")),
        top_p: Some(params.contains(&"top_p")),
    })
}

pub fn parse_google(body: &Value) -> ModelProfile {
    ModelProfile {
        context_window: u64_at(body, "/inputTokenLimit"),
        max_output_tokens: u64_at(body, "/outputTokenLimit"),
        reasoning: body.get("thinking").and_then(Value::as_bool).or(Some(false)),
        top_p: Some(true),
    }
}

pub fn parse_ollama_show(body: &Value) -> ModelProfile {
    let context_window = body
        .get("model_info")
        .and_then(Value::as_object)
        .and_then(|info| info.iter().find_map(|(k, v)| k.ends_with(".context_length").then(|| v.as_u64()).flatten()))
        .filter(|&n| n > 0);
    let thinking = body.get("capabilities").and_then(Value::as_array).map(|caps| caps.iter().any(|c| c.as_str() == Some("thinking")));
    ModelProfile { context_window, max_output_tokens: None, reasoning: thinking, top_p: Some(true) }
}

pub fn parse_llama_props(body: &Value) -> ModelProfile {
    ModelProfile { context_window: u64_at(body, "/default_generation_settings/n_ctx"), max_output_tokens: None, reasoning: None, top_p: Some(true) }
}

/// Id de modelo Google vai no PATH da URL — só o alfabeto dos ids reais passa.
fn is_safe_google_id(model: &str) -> bool {
    !model.is_empty() && model.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
}

fn server_root(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string()
}

fn http() -> &'static ReqwestClient {
    privacy::fixed_endpoint_client()
}

async fn json_of(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let response = request.timeout(FETCH_TIMEOUT).send().await.map_err(|err| format!("unreachable ({err})"))?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    response.json::<Value>().await.map_err(|_| "resposta inválida".to_string())
}

fn stored(store: &ConfigStore, path: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn with_bearer(request: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    if key.is_empty() { request } else { request.bearer_auth(key) }
}

async fn fetch(store: &ConfigStore, provider: &str, model: &str) -> Result<ModelProfile, String> {
    match provider {
        "openrouter" => {
            privacy::assert_network_allowed(store, OPENROUTER_MODELS_URL, "llm:model-profile")?;
            let body = json_of(http().get(OPENROUTER_MODELS_URL)).await?;
            parse_openrouter(&body, model).ok_or_else(|| format!("modelo {model} ausente do catálogo"))
        }
        "google" => {
            let key = stored(store, "apiKeys.googleAI");
            if key.is_empty() || !is_safe_google_id(model) {
                return Err("sem chave ou id inválido".to_string());
            }
            let url = format!("{GOOGLE_MODEL_BASE}{model}");
            privacy::assert_network_allowed(store, &url, "llm:model-profile")?;
            Ok(parse_google(&json_of(http().get(&url).header("x-goog-api-key", key)).await?))
        }
        "ollama" => {
            let base = stored(store, "apiKeys.ollamaBaseUrl");
            let root = server_root(if base.is_empty() { llm_defaults::OLLAMA_BASE_URL } else { &base });
            privacy::assert_network_allowed(store, &root, "llm:model-profile")?;
            let request = http().post(format!("{root}/api/show")).json(&serde_json::json!({ "model": model }));
            Ok(parse_ollama_show(&json_of(with_bearer(request, &stored(store, "apiKeys.ollamaApiKey"))).await?))
        }
        id => {
            let shaped = super::openai_shaped(id).ok_or_else(|| format!("provedor sem perfil: {id}"))?;
            let url = props_url(shaped.id, &stored(store, shaped.base_url_path));
            if url.is_empty() {
                return Err("baseUrl não configurado".to_string());
            }
            privacy::assert_user_endpoint_allowed(store, &url, shaped.id)?;
            let request = http().get(url);
            Ok(parse_llama_props(&json_of(with_bearer(request, &stored(store, shaped.api_key_path))).await?))
        }
    }
}

/// Onde mora o `props` do llama-server: na raiz do servidor para `openaiCompatible` (`/props`,
/// fora do `/v1`), e sob o `/v1` no Unsloth (`GET /v1/props`, `unsloth.ai/docs/basics/api`).
fn props_url(provider: &str, base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    match provider {
        "unsloth" => format!("{trimmed}/props"),
        _ => format!("{}/props", server_root(trimmed)),
    }
}

type Cache = Mutex<HashMap<String, (Instant, Duration, ModelProfile)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Perfil do modelo, com cache por (provedor, modelo). Falha vira perfil vazio — o turno segue com
/// o comportamento de antes (nenhum parâmetro condicional enviado), e a falha fica no log.
pub async fn resolve(store: &ConfigStore, provider: &str, model: &str) -> ModelProfile {
    let key = format!("{provider}|{model}");
    if let Some((at, ttl, profile)) = cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        if at.elapsed() < ttl {
            return profile;
        }
    }
    let (profile, ttl) = match fetch(store, provider, model).await {
        Ok(profile) => (profile, HIT_TTL),
        Err(reason) => {
            log::warn!("llm model profile unavailable: provider={provider} model={model} reason={reason}");
            (ModelProfile::default(), MISS_TTL)
        }
    };
    if let Ok(mut c) = cache().lock() {
        if c.len() >= CACHE_CAP {
            c.clear();
        }
        c.insert(key, (Instant::now(), ttl, profile.clone()));
    }
    profile
}

/// Janela que o turno realmente usou: no Ollama é o `llm.contextWindow` pedido, cortado no máximo
/// do modelo (mesma regra de `brain::turn::fit_to_model`); nos remotos é a do próprio modelo.
pub async fn effective_context_window(store: &ConfigStore, provider: &str, model: &str) -> Option<u64> {
    let window = resolve(store, provider, model).await.context_window;
    if provider != "ollama" {
        return window;
    }
    let requested = store.get("llm.contextWindow").and_then(|v| v.as_u64()).filter(|&n| n > 0)?;
    Some(window.map_or(requested, |w| requested.min(w)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Shape real de `GET /api/v1/models` (2026-09-27): um modelo com raciocínio, outro sem.
    #[test]
    fn openrouter_profile_reads_limits_and_supported_parameters_of_the_requested_model_only() {
        let body = json!({ "data": [
            { "id": "google/gemini-2.5-flash", "context_length": 1048576,
              "top_provider": { "context_length": 1048576, "max_completion_tokens": 65535 },
              "supported_parameters": ["max_tokens", "reasoning", "temperature", "tools", "top_p"] },
            { "id": "google/gemma-3-27b-it", "context_length": 131072,
              "top_provider": { "max_completion_tokens": 117964 },
              "supported_parameters": ["max_tokens", "temperature", "tools"] },
        ]});
        let flash = parse_openrouter(&body, "google/gemini-2.5-flash").unwrap();
        assert_eq!((flash.context_window, flash.max_output_tokens, flash.reasoning, flash.top_p), (Some(1048576), Some(65535), Some(true), Some(true)));
        let gemma = parse_openrouter(&body, "google/gemma-3-27b-it").unwrap();
        assert_eq!((gemma.reasoning, gemma.top_p), (Some(false), Some(false)));
        assert_eq!(parse_openrouter(&body, "vendor/unknown"), None);
    }

    #[test]
    fn google_profile_reads_token_limits_and_the_thinking_flag() {
        let profile = parse_google(&json!({ "inputTokenLimit": 1048576, "outputTokenLimit": 65536, "thinking": true }));
        assert_eq!((profile.context_window, profile.max_output_tokens, profile.reasoning), (Some(1048576), Some(65536), Some(true)));
        assert_eq!(parse_google(&json!({ "inputTokenLimit": 32768 })).reasoning, Some(false));
    }

    /// A chave de contexto do Ollama tem prefixo por arquitetura (`gemma3.`, `qwen3.`...).
    #[test]
    fn ollama_profile_finds_the_context_length_under_any_architecture_prefix() {
        let profile = parse_ollama_show(&json!({
            "model_info": { "general.architecture": "qwen3", "qwen3.context_length": 40960 },
            "capabilities": ["completion", "tools", "thinking"],
        }));
        assert_eq!((profile.context_window, profile.reasoning), (Some(40960), Some(true)));
        assert_eq!(parse_ollama_show(&json!({ "capabilities": ["completion"] })).reasoning, Some(false));
        assert_eq!(parse_ollama_show(&json!({})).context_window, None);
    }

    #[test]
    fn llama_server_profile_uses_the_loaded_context_not_the_training_one() {
        let profile = parse_llama_props(&json!({ "default_generation_settings": { "n_ctx": 16384 }, "total_slots": 2 }));
        assert_eq!(profile.context_window, Some(16384));
        assert_eq!(parse_llama_props(&json!({})).context_window, None);
    }

    #[test]
    fn google_model_id_never_carries_path_or_query_characters() {
        assert!(is_safe_google_id("gemini-2.5-flash"));
        for bad in ["", "../keys", "gemini?key=x", "a/b", "gemini flash"] {
            assert!(!is_safe_google_id(bad), "{bad}");
        }
    }

    #[test]
    fn props_lives_at_the_server_root_except_on_unsloth_where_it_sits_under_v1() {
        assert_eq!(props_url("openaiCompatible", "http://127.0.0.1:8089/v1/"), "http://127.0.0.1:8089/props");
        assert_eq!(props_url("unsloth", "http://localhost:8888/v1"), "http://localhost:8888/v1/props");
        assert_eq!(props_url("unsloth", "  "), "");
    }

    #[test]
    fn server_root_drops_the_v1_suffix_so_props_hits_the_root() {
        assert_eq!(server_root("http://127.0.0.1:8089/v1/"), "http://127.0.0.1:8089");
        assert_eq!(server_root("http://localhost:11434"), "http://localhost:11434");
    }
}
