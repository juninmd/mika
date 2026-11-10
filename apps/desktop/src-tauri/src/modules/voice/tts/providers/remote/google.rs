//! Porte de `providers/google.provider.ts` (`generateGoogleAudio`). Único dos 4 que manda a
//! credencial por query string (`?key=...`), não header — é exatamente o caso que `http::redact`
//! existe para cobrir: `reqwest::Error` embute a URL da requisição no `Display`, query incluída.

use crate::config::schemas::provider_endpoints as endpoints;
use crate::config::schemas::voice_voices::{default_voice, GOOGLE_VOICES};
use crate::config::store::ConfigStore;
use crate::modules::voice::tts::abort_registry::CancelHandle;

use super::http::{guard, race_cancel, redact, request_client};

const BASE_URL: &str = endpoints::GOOGLE_TTS_ORIGIN;

#[derive(Clone)]
pub struct GoogleTtsOptions {
    pub api_key: String,
    pub voice: String,
    pub language_code: String,
    pub speaking_rate: f32,
    pub pitch: f32,
    pub timeout_ms: u32,
    /// `voice.googleTtsTextLimit` — ver o comentário equivalente em `elevenlabs.rs`.
    pub text_limit: usize,
}

impl Default for GoogleTtsOptions {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            voice: "pt-BR-Wavenet-A".into(),
            language_code: "pt-BR".into(),
            speaking_rate: 1.0,
            pitch: 0.0,
            timeout_ms: 25_000,
            text_limit: 3_000,
        }
    }
}

/// Voz padrão e código de idioma do Google por `app.locale` (zh-CN é `cmn-CN` no Google): a
/// primeira WaveNet do idioma na tabela (`voice_voices.rs`).
fn locale_voice(locale: &str) -> (&'static str, &'static str) {
    let language = if locale == "zh-CN" { "cmn-CN" } else { locale };
    let voice = default_voice(GOOGLE_VOICES, language).unwrap_or("pt-BR-Wavenet-A");
    // O código de idioma é o prefixo da própria voz (`ja-JP-Wavenet-A` -> `ja-JP`).
    let end = voice.match_indices('-').nth(1).map_or(voice.len(), |(index, _)| index);
    (voice, &voice[..end])
}

impl GoogleTtsOptions {
    pub fn from_config(store: &ConfigStore) -> Self {
        use crate::config::locale::{app_locale, follow_locale};
        let d = Self::default();
        let locale = app_locale(store);
        Self {
            api_key: store.get("voice.googleTtsApiKey").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
            voice: follow_locale(str_or(store, "voice.googleTtsVoice", &d.voice), &d.voice, &locale, |l| locale_voice(l).0.to_string()),
            language_code: follow_locale(str_or(store, "voice.googleTtsLanguageCode", &d.language_code), &d.language_code, &locale, |l| {
                locale_voice(l).1.to_string()
            }),
            speaking_rate: f32_or(store, "voice.googleTtsSpeakingRate", d.speaking_rate),
            pitch: f32_or(store, "voice.googleTtsPitch", d.pitch),
            timeout_ms: f32_or(store, "voice.googleTtsTimeoutMs", d.timeout_ms as f32) as u32,
            text_limit: f32_or(store, "voice.googleTtsTextLimit", d.text_limit as f32) as usize,
        }
    }
}

fn str_or(store: &ConfigStore, path: &str, fallback: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
}
fn f32_or(store: &ConfigStore, path: &str, fallback: f32) -> f32 {
    store.get(path).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(fallback)
}

/// Texto da causa no idioma da interface (`voice.tts.google.*` no dicionário).
fn classify(store: &ConfigStore, status: u16) -> String {
    let key = match status {
        401 | 403 => "voice.tts.google.authFailed",
        429 => "voice.tts.google.quota",
        400 => "voice.tts.google.badRequest",
        _ => "voice.tts.google.failed",
    };
    crate::modules::settings::t(key, &crate::config::locale::app_locale(store))
}

pub async fn generate(
    text: &str,
    opts: &GoogleTtsOptions,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Vec<u8>, String> {
    generate_at(BASE_URL, text, opts, store, cancel).await
}

async fn generate_at(
    base_url: &str,
    text: &str,
    opts: &GoogleTtsOptions,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Vec<u8>, String> {
    if opts.api_key.is_empty() {
        return Err(crate::modules::settings::t("voice.tts.google.missingKey", &crate::config::locale::app_locale(store)));
    }

    let url = format!("{base_url}/v1/text:synthesize");
    // A guarda de privacidade checa só o HOST — a query com a chave entra depois, no `.query()`
    // da requisição de verdade, nunca no argumento logável.
    guard(store, &url, "tts:google")?;
    let client = request_client(opts.timeout_ms);

    let body = serde_json::json!({
        "input": { "text": text },
        "voice": { "languageCode": opts.language_code, "name": opts.voice },
        "audioConfig": { "audioEncoding": "MP3", "speakingRate": opts.speaking_rate, "pitch": opts.pitch },
    });

    let request = client.post(&url).query(&[("key", opts.api_key.as_str())]).header("Content-Type", "application/json").json(&body);

    let res = race_cancel(request.send(), cancel)
        .await
        .ok_or_else(|| "cancelado".to_string())?
        .map_err(|e| redact(format!("Google TTS falhou: {e}")))?;

    if !res.status().is_success() {
        let status = res.status().as_u16();
        let body_text = res.text().await.unwrap_or_default();
        return Err(redact(format!("Google TTS: {} (HTTP {status}): {body_text}", classify(store, status))));
    }

    let payload: serde_json::Value = res.json().await.map_err(|e| redact(format!("Google TTS: invalid response: {e}")))?;
    let audio_content = payload.get("audioContent").and_then(|v| v.as_str()).ok_or_else(|| "Google TTS: response without audioContent".to_string())?;

    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(audio_content).map_err(|e| format!("Google TTS: audioContent is not valid base64: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;
    use crate::modules::voice::tts::abort_registry::AbortRegistry;
    use super::super::http::test_support;

    fn store() -> ConfigStore {
        let dir = std::env::temp_dir().join(format!("mika-google-tts-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", false)] }]);
        ConfigStore::open(&dir, schema).unwrap()
    }

    fn cancel() -> CancelHandle {
        let registry = Box::leak(Box::new(AbortRegistry::new()));
        registry.register_active().handle()
    }

    fn opts() -> GoogleTtsOptions {
        GoogleTtsOptions { api_key: "AIza-teste".into(), ..GoogleTtsOptions::default() }
    }

    /// Voz e código do Google ainda no default português seguem a interface; voz escolhida fica.
    #[test]
    fn untouched_voice_defaults_follow_the_ui_language() {
        let dir = std::env::temp_dir().join(format!("mika-google-tts-locale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        assert_eq!(GoogleTtsOptions::from_config(&store).language_code, "pt-BR");

        store.set("app.locale", serde_json::json!("zh-CN")).unwrap();
        let zh = GoogleTtsOptions::from_config(&store);
        assert_eq!((zh.voice.as_str(), zh.language_code.as_str()), ("cmn-CN-Wavenet-A", "cmn-CN"));

        store.set("voice.googleTtsVoice", serde_json::json!("cmn-CN-Wavenet-B")).unwrap();
        assert_eq!(GoogleTtsOptions::from_config(&store).voice, "cmn-CN-Wavenet-B");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_successful_response_decodes_the_base64_audio_content_and_hits_synthesize() {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode([5, 6, 7]);
        let mock = test_support::spawn(200, "application/json", format!("{{\"audioContent\":\"{encoded}\"}}").into_bytes());

        let audio = generate_at(&mock.base_url(), "oi", &opts(), &store(), &cancel()).await.unwrap();
        assert_eq!(audio, vec![5, 6, 7]);
        let req = mock.request_text();
        assert!(req.starts_with("POST /v1/text:synthesize?key=AIza-teste"), "endpoint/query errado: {req}");
    }

    #[tokio::test]
    async fn a_response_without_audio_content_is_a_named_error() {
        let mock = test_support::spawn(200, "application/json", b"{}".to_vec());
        let err = generate_at(&mock.base_url(), "oi", &opts(), &store(), &cancel()).await.unwrap_err();
        assert!(err.contains("without audioContent"));
    }

    #[tokio::test]
    async fn missing_api_key_fails_before_touching_the_network() {
        let opts = GoogleTtsOptions { api_key: String::new(), ..GoogleTtsOptions::default() };
        let err = generate_at("http://127.0.0.1:1", "oi", &opts, &store(), &cancel()).await.unwrap_err();
        assert!(err.contains("googleTtsApiKey"));
    }

    /// A prova central deste provedor: mesmo quando o transporte falha (host inexistente, sem
    /// listener), a chave que ia na query NUNCA aparece na mensagem — é o `reqwest::Error` que
    /// embutiria a URL inteira sem a redação em `http::redact`.
    #[tokio::test]
    async fn a_transport_failure_never_leaks_the_api_key_from_the_query_string() {
        let err = generate_at("http://127.0.0.1:1", "oi", &opts(), &store(), &cancel()).await.unwrap_err();
        assert!(!err.contains("AIza-teste"), "a chave vazou na mensagem de erro: {err}");
        assert!(err.contains("[REDACTED]"), "a query não foi redigida: {err}");
    }

    /// Defeito real achado ao conferir a documentação do Google: os defaults de es-ES/fr-FR/it-IT/
    /// de-DE apontavam para WaveNets que a lista oficial não traz mais (`es-ES-Wavenet-C`, ...).
    #[test]
    fn every_ui_locale_default_is_a_listed_voice_whose_prefix_is_its_language_code() {
        for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
            let (voice, code) = locale_voice(locale);
            assert!(GOOGLE_VOICES.iter().any(|option| option.value == voice), "{locale}: {voice} fora da tabela");
            assert!(voice.starts_with(&format!("{code}-")), "{locale}: {voice} x {code}");
        }
    }

    #[test]
    fn user_facing_errors_resolve_in_every_locale() {
        let dir = std::env::temp_dir().join(format!("mika-google-tts-errors-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
            store.set("app.locale", serde_json::json!(locale)).unwrap();
            let texts = [401, 429, 400, 500].map(|status| classify(&store, status));
            for text in &texts {
                assert!(!text.starts_with("voice.") && !text.contains("{{"), "{locale}: {text}");
            }
            assert_eq!(texts.iter().collect::<std::collections::HashSet<_>>().len(), 4, "{locale}: classes iguais");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
