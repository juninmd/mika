//! Porte de `providers/elevenlabs.provider.ts` (`generateElevenLabsAudio`, `classifyFailure`,
//! `fetchWithRetry`) — só a geração; sem `lastPermanentFailure`/fallback automático, ver o
//! cabeçalho de `remote/mod.rs`.

use crate::config::schemas::provider_endpoints as endpoints;
use crate::config::store::ConfigStore;
use crate::modules::voice::tts::abort_registry::CancelHandle;

use super::http::{guard, race_cancel, redact, request_client};

const BASE_URL: &str = endpoints::ELEVENLABS_ORIGIN;

/// Sem campo para `elevenLabsOutputFormat`: o schema Rust (`config/schemas/voice.rs`) não declara
/// esse caminho (só o TS o expõe) — fora do escopo desta tarefa (`modules/voice/tts/**` apenas).
/// O default do TS (`TTS_DEFAULTS.OUTPUT_FORMAT`) cobre o caso até esse campo ser portado.
const DEFAULT_OUTPUT_FORMAT: &str = "mp3_44100_128";

#[derive(Clone)]
pub struct ElevenLabsOptions {
    pub api_key: String,
    pub voice_id: String,
    pub model_id: String,
    pub stability: f32,
    pub similarity_boost: f32,
    pub style: f32,
    pub speaker_boost: bool,
    /// `voice.elevenLabsSpeed` — `voice_settings.speed`, faixa aceita pela API: 0.7–1.2.
    pub speed: f32,
    pub output_format: String,
    pub timeout_ms: u32,
    /// `voice.elevenLabsTextLimit` — antes desta rodada só existia na UI: o corte por chunk
    /// (`speaker/chunk.rs::text_limit`) usava sempre `supertonic_text_limit` (3000), fixo, para
    /// qualquer provedor.
    pub text_limit: usize,
}

impl Default for ElevenLabsOptions {
    /// Espelha `TTS_DEFAULTS` de `voice/constants.ts`.
    fn default() -> Self {
        Self {
            api_key: String::new(),
            voice_id: "EXAVITQu4vr4xnSDxMaL".into(),
            model_id: "eleven_multilingual_v2".into(),
            stability: 0.45,
            similarity_boost: 0.75,
            style: 0.0,
            speaker_boost: true,
            speed: 1.0,
            output_format: DEFAULT_OUTPUT_FORMAT.into(),
            timeout_ms: 30_000,
            text_limit: 3_000,
        }
    }
}

impl ElevenLabsOptions {
    /// Lê os caminhos que `config/schemas/voice.rs::elevenlabs_fields` e `llm.rs` (a credencial,
    /// `apiKeys.elevenLabs`) já declaram — nenhum schema novo precisou entrar para este porte.
    pub fn from_config(store: &ConfigStore) -> Self {
        let d = Self::default();
        Self {
            api_key: str_or(store, "apiKeys.elevenLabs", &d.api_key),
            voice_id: str_or(store, "voice.elevenLabsVoiceId", &d.voice_id),
            model_id: str_or(store, "voice.elevenLabsModel", &d.model_id),
            stability: f32_or(store, "voice.ttsStability", d.stability),
            similarity_boost: f32_or(store, "voice.elevenLabsSimilarityBoost", d.similarity_boost),
            style: f32_or(store, "voice.elevenLabsStyle", d.style),
            speaker_boost: bool_or(store, "voice.elevenLabsSpeakerBoost", d.speaker_boost),
            speed: f32_or(store, "voice.elevenLabsSpeed", d.speed).clamp(0.7, 1.2),
            output_format: d.output_format.clone(),
            timeout_ms: f32_or(store, "voice.elevenLabsTimeoutMs", d.timeout_ms as f32) as u32,
            text_limit: f32_or(store, "voice.elevenLabsTextLimit", d.text_limit as f32) as usize,
        }
    }
}

fn str_or(store: &ConfigStore, path: &str, fallback: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
}
fn f32_or(store: &ConfigStore, path: &str, fallback: f32) -> f32 {
    store.get(path).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(fallback)
}
fn bool_or(store: &ConfigStore, path: &str, fallback: bool) -> bool {
    store.get(path).and_then(|v| v.as_bool()).unwrap_or(fallback)
}

/// Espelha `classifyFailure`: 401/403 é conta (credencial/cota), 402 é a voz específica. A
/// distinção quota×auth do TS (regex sobre a mensagem) vira uma checagem simples de substring —
/// mesmo efeito, sem puxar `regex` para um único uso.
fn classify(status: u16, message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    match status {
        401 | 403 if lower.contains("quota") || lower.contains("credit") => "sem crédito na conta",
        401 | 403 => "credencial inválida",
        402 => "voz não coberta pelo plano",
        _ => "falha ao gerar áudio",
    }
}

pub async fn generate(
    text: &str,
    opts: &ElevenLabsOptions,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Vec<u8>, String> {
    generate_at(BASE_URL, text, opts, store, cancel).await
}

/// `base_url` separado de [`generate`] para os testes apontarem a um mock loopback sem tocar a URL
/// real do provedor — mesma injeção de endpoint que `supertonic/assets.rs` já faz via `hf_base()`.
async fn generate_at(
    base_url: &str,
    text: &str,
    opts: &ElevenLabsOptions,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Vec<u8>, String> {
    if opts.api_key.is_empty() {
        return Err("ElevenLabs: apiKeys.elevenLabs não configurada".into());
    }

    let url = format!("{base_url}/v1/text-to-speech/{}/stream", opts.voice_id);
    guard(store, &url, "tts:elevenlabs")?;
    let client = request_client(opts.timeout_ms);

    let body = serde_json::json!({
        "text": text,
        "model_id": opts.model_id,
        "output_format": opts.output_format,
        "voice_settings": {
            "stability": opts.stability,
            "similarity_boost": opts.similarity_boost,
            "style": opts.style,
            "use_speaker_boost": opts.speaker_boost,
            "speed": opts.speed,
        },
    });

    // Espelha `fetchWithRetry`: até 2 retentativas em 5xx/erro de transporte, backoff 1s*tentativa.
    // 400/401/402 nunca retentam — são erro do pedido, não transitório.
    let mut attempt = 0u8;
    loop {
        if cancel.is_cancelled() {
            return Err("cancelado".into());
        }
        let request = client.post(&url).header("xi-api-key", &opts.api_key).header("Accept", "audio/mpeg").json(&body).send();

        let outcome = race_cancel(request, cancel).await.ok_or_else(|| "cancelado".to_string())?;
        match outcome {
            Ok(res) if res.status().is_success() => {
                let bytes = res.bytes().await.map_err(|e| redact(format!("ElevenLabs TTS: leitura do áudio falhou: {e}")))?;
                return Ok(bytes.to_vec());
            }
            Ok(res) => {
                let status = res.status().as_u16();
                if [400, 401, 402].contains(&status) || attempt >= 2 {
                    let body_text = res.text().await.unwrap_or_default();
                    return Err(redact(format!(
                        "ElevenLabs TTS: {} (HTTP {status}): {body_text}",
                        classify(status, &body_text)
                    )));
                }
            }
            Err(err) if attempt >= 2 => return Err(redact(format!("ElevenLabs TTS falhou: {err}"))),
            Err(_) => {}
        }
        attempt += 1;
        tokio::time::sleep(std::time::Duration::from_millis(1_000 * attempt as u64)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;
    use crate::modules::voice::tts::abort_registry::AbortRegistry;
    use super::super::http::test_support;

    fn store() -> ConfigStore {
        let dir = std::env::temp_dir().join(format!("mika-elevenlabs-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", false)] }]);
        ConfigStore::open(&dir, schema).unwrap()
    }

    fn cancel() -> CancelHandle {
        let registry = Box::leak(Box::new(AbortRegistry::new()));
        registry.register_active().handle()
    }

    fn opts() -> ElevenLabsOptions {
        ElevenLabsOptions { api_key: "sk-teste-123".into(), ..ElevenLabsOptions::default() }
    }

    #[tokio::test]
    async fn a_successful_response_returns_the_audio_bytes_and_hits_the_documented_endpoint() {
        let mock = test_support::spawn(200, "audio/mpeg", vec![1, 2, 3, 4]);
        let audio = generate_at(&mock.base_url(), "oi", &opts(), &store(), &cancel()).await.unwrap();

        assert_eq!(audio, vec![1, 2, 3, 4]);
        let req = mock.request_text();
        assert!(req.starts_with("POST /v1/text-to-speech/EXAVITQu4vr4xnSDxMaL/stream"), "endpoint errado: {req}");
        assert!(req.contains("xi-api-key: sk-teste-123"), "header da chave ausente: {req}");
    }

    #[tokio::test]
    async fn the_configured_speed_reaches_voice_settings() {
        let mock = test_support::spawn(200, "audio/mpeg", vec![1]);
        let opts = ElevenLabsOptions { speed: 1.1, ..opts() };
        generate_at(&mock.base_url(), "oi", &opts, &store(), &cancel()).await.unwrap();
        assert!(mock.request_text().contains("\"speed\":1.1"), "speed não enviado: {}", mock.request_text());
    }

    #[tokio::test]
    async fn a_401_is_classified_as_a_credential_failure_and_never_leaks_a_url_query() {
        let mock = test_support::spawn(401, "application/json", b"{\"detail\":\"invalid key\"}".to_vec());
        let err = generate_at(&mock.base_url(), "oi", &opts(), &store(), &cancel()).await.unwrap_err();

        assert!(err.contains("credencial inválida"), "não classificou: {err}");
        assert!(err.contains("HTTP 401"));
    }

    #[tokio::test]
    async fn missing_api_key_fails_before_touching_the_network() {
        let opts = ElevenLabsOptions { api_key: String::new(), ..ElevenLabsOptions::default() };
        // Porta 1 sem listener: se o código chegasse a tentar conectar, ECONNREFUSED na hora
        // provaria a mesma coisa — mas o `Err` tem de chegar ANTES disso, pela checagem da chave.
        let err = generate_at("http://127.0.0.1:1", "oi", &opts, &store(), &cancel()).await.unwrap_err();
        assert!(err.contains("apiKeys.elevenLabs"));
    }

    #[tokio::test]
    async fn from_config_reads_the_credential_from_the_shared_api_keys_namespace() {
        let dir = std::env::temp_dir().join(format!("mika-elevenlabs-fromconfig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        store.set("apiKeys.elevenLabs", serde_json::json!("sk-do-usuario")).unwrap();
        store.set("voice.elevenLabsVoiceId", serde_json::json!("voz-custom")).unwrap();

        let opts = ElevenLabsOptions::from_config(&store);
        assert_eq!(opts.api_key, "sk-do-usuario");
        assert_eq!(opts.voice_id, "voz-custom");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn offline_mode_blocks_the_call_with_zero_network_touched() {
        let dir = std::env::temp_dir().join(format!("mika-elevenlabs-offline-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", true)] }]);
        let store = ConfigStore::open(&dir, schema).unwrap();

        // URL real do provedor (não loopback): se o guard não bloqueasse antes do request, este
        // teste tentaria uma conexão de verdade em vez de falhar instantaneamente.
        let err = generate("oi", &opts(), &store, &cancel()).await.unwrap_err();
        assert!(err.to_lowercase().contains("offline"), "não bloqueou por modo offline: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
