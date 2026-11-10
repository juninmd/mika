//! Porte de `providers/openai.provider.ts` (`generateOpenAIAudio`) — servidor compatível com a API
//! `audio/speech` da OpenAI (Kokoro-FastAPI local é o default, `TTS_DEFAULTS.OPENAI_BASE_URL`).
//!
//! **SSRF.** `isOpenAIBaseUrlAllowed` do TS: loopback direto, o resto por `validateExternalUrl` —
//! aqui `privacy::assert_user_endpoint_allowed` (SSRF + modo offline, o mesmo chokepoint do LLM).

use crate::config::store::ConfigStore;
use crate::modules::privacy::assert_user_endpoint_allowed;
use crate::modules::voice::tts::abort_registry::CancelHandle;

use super::http::{race_cancel, redact, request_client};

#[derive(Clone)]
pub struct OpenAiTtsOptions {
    pub base_url: String,
    pub voice: String,
    pub model: String,
    /// `voice.openaiTtsSpeed` — `speed` da API `audio/speech` (0.25–4.0).
    pub speed: f32,
    pub api_key: String,
    pub timeout_ms: u32,
}

impl Default for OpenAiTtsOptions {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8880/v1".into(),
            voice: "af_heart".into(),
            model: "kokoro".into(),
            speed: 1.0,
            api_key: String::new(),
            // Sem campo de schema para `openaiTtsTextLimit`/`openaiTtsTimeoutMs` — mesma lacuna que
            // `state.rs` já documenta para `supertonic_text_limit`. `TTS_TIMEOUTS.OPENAI`.
            timeout_ms: 20_000,
        }
    }
}

impl OpenAiTtsOptions {
    pub fn from_config(store: &ConfigStore) -> Self {
        let d = Self::default();
        Self {
            base_url: str_or(store, "voice.openaiTtsBaseUrl", &d.base_url),
            voice: str_or(store, "voice.openaiTtsVoice", &d.voice),
            model: str_or(store, "voice.openaiTtsModel", &d.model),
            speed: store.get("voice.openaiTtsSpeed").and_then(|v| v.as_f64()).map_or(d.speed, |v| v.clamp(0.25, 4.0) as f32),
            api_key: store.get("voice.openaiTtsApiKey").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
            timeout_ms: d.timeout_ms,
        }
    }
}

fn str_or(store: &ConfigStore, path: &str, fallback: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
}

fn classify(status: u16) -> &'static str {
    match status {
        401 | 403 => "chave de API inválida ou ausente",
        429 => "cota/limite de uso excedido",
        400 => "voz ou modelo inválido para esse servidor",
        _ => "falha ao gerar áudio",
    }
}

pub async fn generate(
    text: &str,
    opts: &OpenAiTtsOptions,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Vec<u8>, String> {
    generate_at(text, opts, store, cancel).await
}

async fn generate_at(text: &str, opts: &OpenAiTtsOptions, store: &ConfigStore, cancel: &CancelHandle) -> Result<Vec<u8>, String> {
    let url = format!("{}/audio/speech", opts.base_url.trim_end_matches('/'));
    assert_user_endpoint_allowed(store, &url, "tts:openai").map_err(redact)?;
    let client = request_client(opts.timeout_ms);

    let body = serde_json::json!({ "model": opts.model, "input": text, "voice": opts.voice, "response_format": "mp3", "speed": opts.speed });
    let mut request = client.post(&url).header("Content-Type", "application/json").json(&body);
    if !opts.api_key.is_empty() {
        request = request.bearer_auth(&opts.api_key);
    }

    let res = race_cancel(request.send(), cancel)
        .await
        .ok_or_else(|| "cancelado".to_string())?
        .map_err(|e| redact(format!("OpenAI TTS falhou: {e}")))?;

    if !res.status().is_success() {
        let status = res.status().as_u16();
        let body_text = res.text().await.unwrap_or_default();
        return Err(redact(format!("OpenAI TTS: {} (HTTP {status}): {body_text}", classify(status))));
    }

    let bytes = res.bytes().await.map_err(|e| redact(format!("OpenAI TTS: leitura do áudio falhou: {e}")))?;
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;
    use crate::modules::voice::tts::abort_registry::AbortRegistry;
    use super::super::http::test_support;

    fn store() -> ConfigStore {
        let dir = std::env::temp_dir().join(format!("mika-openai-tts-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", false)] }]);
        ConfigStore::open(&dir, schema).unwrap()
    }

    fn cancel() -> CancelHandle {
        let registry = Box::leak(Box::new(AbortRegistry::new()));
        registry.register_active().handle()
    }

    #[tokio::test]
    async fn a_successful_response_returns_the_audio_bytes_and_hits_audio_speech() {
        let mock = test_support::spawn(200, "audio/mpeg", vec![9, 9, 9]);
        let opts = OpenAiTtsOptions { base_url: mock.base_url(), api_key: "sk-abc".into(), ..OpenAiTtsOptions::default() };

        let audio = generate_at("oi", &opts, &store(), &cancel()).await.unwrap();
        assert_eq!(audio, vec![9, 9, 9]);
        let req = mock.request_text();
        assert!(req.starts_with("POST /audio/speech"), "endpoint errado: {req}");
        assert!(req.contains("authorization: Bearer sk-abc"), "faltou o Authorization: {req}");
    }

    #[tokio::test]
    async fn the_configured_speed_reaches_the_request_body() {
        let mock = test_support::spawn(200, "audio/mpeg", vec![1]);
        let opts = OpenAiTtsOptions { base_url: mock.base_url(), speed: 1.25, ..OpenAiTtsOptions::default() };
        generate_at("oi", &opts, &store(), &cancel()).await.unwrap();
        assert!(mock.request_text().contains("\"speed\":1.25"), "speed não enviado: {}", mock.request_text());
    }

    #[tokio::test]
    async fn a_missing_api_key_still_sends_the_request_without_an_authorization_header() {
        let mock = test_support::spawn(200, "audio/mpeg", vec![1]);
        let opts = OpenAiTtsOptions { base_url: mock.base_url(), api_key: String::new(), ..OpenAiTtsOptions::default() };

        generate_at("oi", &opts, &store(), &cancel()).await.unwrap();
        let req = mock.request_text();
        assert!(!req.to_lowercase().contains("authorization"), "mandou Authorization sem chave: {req}");
    }

    #[tokio::test]
    async fn a_private_network_base_url_is_blocked_before_any_network_call() {
        let opts = OpenAiTtsOptions { base_url: "http://192.168.0.10:8880/v1".into(), ..OpenAiTtsOptions::default() };
        let err = generate_at("oi", &opts, &store(), &cancel()).await.unwrap_err();
        assert!(err.contains("SSRF"), "não bloqueou por ssrf: {err}");
    }

    #[tokio::test]
    async fn a_400_is_classified_as_an_invalid_voice() {
        let mock = test_support::spawn(400, "application/json", b"{}".to_vec());
        let opts = OpenAiTtsOptions { base_url: mock.base_url(), ..OpenAiTtsOptions::default() };
        let err = generate_at("oi", &opts, &store(), &cancel()).await.unwrap_err();
        assert!(err.contains("voz ou modelo inválido"), "não classificou: {err}");
    }
}
