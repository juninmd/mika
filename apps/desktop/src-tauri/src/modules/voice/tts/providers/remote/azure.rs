//! Porte de `providers/azure.provider.ts` (`generateAzureAudio`, `assertAzureRegion`). O formato de
//! saída fica fixo (o player espera mp3); o idioma do SSML sai do locale da própria voz
//! (`pt-BR-FranciscaNeural` → `pt-BR`), e velocidade/tom vão num `<prosody>` só quando mudados.

use crate::config::schemas::voice_voices::{default_voice, AZURE_VOICES};
use crate::config::store::ConfigStore;
use crate::modules::voice::tts::abort_registry::CancelHandle;

use super::http::{guard, race_cancel, redact, request_client};

const FALLBACK_LANGUAGE: &str = "pt-BR";
const FALLBACK_VOICE: &str = "pt-BR-FranciscaNeural";
const OUTPUT_FORMAT: &str = "audio-16khz-128kbitrate-mono-mp3";

#[derive(Clone)]
pub struct AzureTtsOptions {
    pub subscription_key: String,
    pub region: String,
    pub voice: String,
    /// `voice.azureTtsRate`/`voice.azureTtsPitch`: variação relativa em %, 0 = padrão da voz.
    pub rate_pct: f32,
    pub pitch_pct: f32,
    pub timeout_ms: u32,
    /// `voice.azureTtsTextLimit` — ver o comentário equivalente em `elevenlabs.rs`.
    pub text_limit: usize,
}

impl Default for AzureTtsOptions {
    fn default() -> Self {
        Self {
            subscription_key: String::new(),
            region: String::new(),
            voice: FALLBACK_VOICE.into(),
            rate_pct: 0.0,
            pitch_pct: 0.0,
            timeout_ms: 25_000,
            text_limit: 3_000,
        }
    }
}

/// Voz neural padrão da Azure por `app.locale`: a primeira da tabela do idioma (`voice_voices.rs`).
fn locale_voice(locale: &str) -> &'static str {
    default_voice(AZURE_VOICES, locale).unwrap_or(FALLBACK_VOICE)
}

impl AzureTtsOptions {
    pub fn from_config(store: &ConfigStore) -> Self {
        let d = Self::default();
        let locale = crate::config::locale::app_locale(store);
        Self {
            subscription_key: store.get("voice.azureTtsSubscriptionKey").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
            region: store.get("voice.azureTtsRegion").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
            voice: crate::config::locale::follow_locale(str_or(store, "voice.azureTtsVoice", &d.voice), FALLBACK_VOICE, &locale, |l| locale_voice(l).to_string()),
            rate_pct: f32_or(store, "voice.azureTtsRate", d.rate_pct).clamp(-50.0, 100.0),
            pitch_pct: f32_or(store, "voice.azureTtsPitch", d.pitch_pct).clamp(-50.0, 50.0),
            timeout_ms: f32_or(store, "voice.azureTtsTimeoutMs", d.timeout_ms as f32) as u32,
            text_limit: f32_or(store, "voice.azureTtsTextLimit", d.text_limit as f32) as usize,
        }
    }
}

fn str_or(store: &ConfigStore, path: &str, fallback: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
}
fn f32_or(store: &ConfigStore, path: &str, fallback: f32) -> f32 {
    store.get(path).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(fallback)
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Locale da voz (`pt-BR-FranciscaNeural` → `pt-BR`): `xml:lang` fixo em pt-BR errava toda voz de outro idioma.
fn voice_language(voice: &str) -> String {
    let mut parts = voice.splitn(3, '-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(lang), Some(region), Some(_)) if lang.len() == 2 && region.len() == 2 => format!("{lang}-{region}"),
        _ => FALLBACK_LANGUAGE.to_string(),
    }
}

/// Texto escapado: `&`/`<` cru na fala tornava o SSML inválido e a Azure respondia 400.
fn build_ssml(text: &str, opts: &AzureTtsOptions) -> String {
    let voice = escape_xml(&opts.voice);
    let mut body = escape_xml(text);
    if opts.rate_pct != 0.0 || opts.pitch_pct != 0.0 {
        body = format!("<prosody rate=\"{:+}%\" pitch=\"{:+}%\">{body}</prosody>", opts.rate_pct.round(), opts.pitch_pct.round());
    }
    format!("<speak version=\"1.0\" xml:lang=\"{}\"><voice name=\"{voice}\">{body}</voice></speak>", voice_language(&opts.voice))
}

/// Espelha `AZURE_REGION = /^[a-z0-9-]+$/`.
fn is_valid_region(region: &str) -> bool {
    !region.is_empty() && region.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Mensagem da UI no idioma de `app.locale` (`voice.tts.azure.*` no dicionário).
fn message(store: &ConfigStore, key: &str, vars: &[(&str, &str)]) -> String {
    crate::modules::settings::tf(key, &crate::config::locale::app_locale(store), vars)
}

fn classify(store: &ConfigStore, status: u16) -> String {
    let key = match status {
        401 | 403 => "voice.tts.azure.authFailed",
        429 => "voice.tts.azure.quota",
        400 => "voice.tts.azure.badRequest",
        _ => "voice.tts.azure.failed",
    };
    message(store, key, &[])
}

pub async fn generate(
    text: &str,
    opts: &AzureTtsOptions,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Vec<u8>, String> {
    generate_at(None, text, opts, store, cancel).await
}

/// `host_override` existe só para os testes apontarem ao mock loopback — a URL real da Azure é
/// derivada da região (`https://{region}.tts.speech.microsoft.com`), então não há uma constante
/// `BASE_URL` única como nos outros 3 provedores.
async fn generate_at(
    host_override: Option<&str>,
    text: &str,
    opts: &AzureTtsOptions,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Vec<u8>, String> {
    if opts.subscription_key.is_empty() || opts.region.is_empty() {
        return Err(message(store, "voice.tts.azure.missingConfig", &[]));
    }
    if !is_valid_region(&opts.region) {
        return Err(message(store, "voice.tts.azure.invalidRegion", &[("region", &opts.region)]));
    }

    let url = match host_override {
        Some(host) => format!("{host}/cognitiveservices/v1"),
        None => format!("https://{}.tts.speech.microsoft.com/cognitiveservices/v1", opts.region),
    };
    guard(store, &url, "tts:azure")?;
    let client = request_client(opts.timeout_ms);

    let ssml = build_ssml(text, opts);

    let request = client
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", &opts.subscription_key)
        .header("Content-Type", "application/ssml+xml")
        .header("X-Microsoft-OutputFormat", OUTPUT_FORMAT)
        .header("User-Agent", "Mika-TTS")
        .body(ssml);

    let res = race_cancel(request.send(), cancel)
        .await
        .ok_or_else(|| "cancelado".to_string())?
        .map_err(|e| redact(format!("Azure TTS request failed: {e}")))?;

    if !res.status().is_success() {
        let status = res.status().as_u16();
        let body_text = res.text().await.unwrap_or_default();
        return Err(redact(format!("Azure TTS: {} (HTTP {status}): {body_text}", classify(store, status))));
    }

    let bytes = res.bytes().await.map_err(|e| redact(format!("Azure TTS: reading the audio failed: {e}")))?;
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
        let dir = std::env::temp_dir().join(format!("mika-azure-tts-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", false)] }]);
        ConfigStore::open(&dir, schema).unwrap()
    }

    fn cancel() -> CancelHandle {
        let registry = Box::leak(Box::new(AbortRegistry::new()));
        registry.register_active().handle()
    }

    fn opts() -> AzureTtsOptions {
        AzureTtsOptions { subscription_key: "sub-123".into(), region: "brazilsouth".into(), ..AzureTtsOptions::default() }
    }

    #[test]
    fn an_untouched_voice_follows_the_ui_language_and_a_chosen_one_stays() {
        let dir = std::env::temp_dir().join(format!("mika-azure-tts-locale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        assert_eq!(AzureTtsOptions::from_config(&store).voice, "pt-BR-FranciscaNeural");

        store.set("app.locale", serde_json::json!("de-DE")).unwrap();
        assert_eq!(AzureTtsOptions::from_config(&store).voice, "de-DE-KatjaNeural");

        store.set("voice.azureTtsVoice", serde_json::json!("de-DE-ConradNeural")).unwrap();
        assert_eq!(AzureTtsOptions::from_config(&store).voice, "de-DE-ConradNeural");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_successful_response_returns_the_audio_bytes_and_hits_cognitiveservices() {
        let mock = test_support::spawn(200, "audio/mpeg", vec![7, 7]);
        let audio = generate_at(Some(&mock.base_url()), "oi", &opts(), &store(), &cancel()).await.unwrap();

        assert_eq!(audio, vec![7, 7]);
        let req = mock.request_text();
        assert!(req.starts_with("POST /cognitiveservices/v1"), "endpoint errado: {req}");
        assert!(req.contains("ocp-apim-subscription-key: sub-123"), "faltou a chave: {req}");
    }

    #[test]
    fn ssml_escapes_the_spoken_text_and_takes_the_language_from_the_voice() {
        let ssml = build_ssml("Tom & Jerry <3", &AzureTtsOptions { voice: "en-US-JennyNeural".into(), ..opts() });
        assert!(ssml.contains("xml:lang=\"en-US\""), "{ssml}");
        assert!(ssml.contains(">Tom &amp; Jerry &lt;3</voice>"), "{ssml}");
        assert!(!ssml.contains("prosody"), "prosody sem ajuste muda a voz padrão: {ssml}");
    }

    #[test]
    fn ssml_carries_rate_and_pitch_only_when_the_user_changed_them() {
        let ssml = build_ssml("oi", &AzureTtsOptions { rate_pct: 15.0, pitch_pct: -10.0, ..opts() });
        assert!(ssml.contains("<prosody rate=\"+15%\" pitch=\"-10%\">oi</prosody>"), "{ssml}");
    }

    #[tokio::test]
    async fn an_invalid_region_is_rejected_before_any_network_call() {
        let opts = AzureTtsOptions { region: "Brazil South!".into(), ..opts() };
        let err = generate_at(Some("http://127.0.0.1:1"), "oi", &opts, &store(), &cancel()).await.unwrap_err();
        assert!(err.contains("Região da Azure inválida"));
    }

    /// As quatro classes de erro e as duas de configuração saem no idioma da interface — nunca a
    /// chave crua do dicionário nem `{{region}}` sem preencher.
    #[test]
    fn user_facing_errors_resolve_in_every_locale() {
        let dir = std::env::temp_dir().join(format!("mika-azure-tts-errors-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
            store.set("app.locale", serde_json::json!(locale)).unwrap();
            let texts = [401, 429, 400, 500].map(|status| classify(&store, status));
            for text in texts.iter().chain([&message(&store, "voice.tts.azure.invalidRegion", &[("region", "x!")])]) {
                assert!(!text.starts_with("voice.") && !text.contains("{{"), "{locale}: {text}");
            }
            assert_eq!(texts.iter().collect::<std::collections::HashSet<_>>().len(), 4, "{locale}: classes iguais");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn missing_subscription_key_or_region_fails_before_touching_the_network() {
        let opts = AzureTtsOptions { subscription_key: String::new(), ..opts() };
        let err = generate_at(Some("http://127.0.0.1:1"), "oi", &opts, &store(), &cancel()).await.unwrap_err();
        assert!(err.contains("assinatura ou região"));
    }

    #[tokio::test]
    async fn a_429_is_classified_as_quota_exhaustion() {
        let mock = test_support::spawn(429, "text/plain", b"quota".to_vec());
        let err = generate_at(Some(&mock.base_url()), "oi", &opts(), &store(), &cancel()).await.unwrap_err();
        assert!(err.contains("cota/limite"), "não classificou: {err}");
    }

    /// O default de cada idioma tem de ser uma voz que a tabela (e portanto a UI) lista — um nome
    /// fora dela é uma voz que o usuário nem consegue escolher de volta.
    #[test]
    fn every_ui_locale_has_a_listed_default_voice_with_a_matching_language_tag() {
        for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
            let voice = locale_voice(locale);
            assert!(AZURE_VOICES.iter().any(|option| option.value == voice), "{locale}: {voice} fora da tabela");
            assert_eq!(voice_language(voice), locale, "{locale}: xml:lang da voz padrão");
        }
    }
}
