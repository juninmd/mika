//! `credentials:test` — o botão "Testar" de cada credencial em Configurações → Credenciais, para
//! os provedores que NÃO têm outro caminho de verificação já existente (Groq, OpenAI STT, Azure TTS,
//! Google TTS, Steam). Google AI / OpenRouter / OpenAI-compatível / Ollama já se testam por
//! `mika:llm-models`, e ElevenLabs por `mika:elevenlabs-voices` — duplicá-los aqui seria o mesmo
//! GET em dois lugares divergindo com o tempo.
//!
//! Cada teste é UM GET autenticado e barato (lista de modelos/vozes), com prazo curto, passando por
//! `privacy::assert_network_allowed` antes de sair. A chave vem do payload (o valor que está no
//! campo agora) ou, sem ele, da config salva — e NUNCA aparece em log nem na resposta: o erro do
//! `reqwest` carrega a URL (que no Steam leva a chave na query), então ele é traduzido para uma
//! frase fixa em vez de repassado.

use crate::config::schemas::provider_endpoints as endpoints;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::schemas::cloud_stt::{GROQ_MODELS_URL, OPENAI_MODELS_URL};
use crate::config::store::ConfigStore;
use crate::modules::privacy;
use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

const CH_CREDENTIALS_TEST: &str = "credentials:test";
const CHANNELS: &[&str] = &[CH_CREDENTIALS_TEST];

const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_SECRET_LEN: usize = 512;

const GOOGLE_TTS_VOICES_URL: &str = endpoints::GOOGLE_TTS_VOICES_URL;
const STEAM_API_LIST_URL: &str = "https://api.steampowered.com/ISteamWebAPIUtil/GetSupportedAPIList/v1/";

/// Um provedor testável: onde a chave mora na config e como ela vai na requisição.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Provider {
    Groq,
    OpenAiStt,
    Azure,
    GoogleTts,
    Steam,
}

impl Provider {
    fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "groq" => Self::Groq,
            "openaiStt" => Self::OpenAiStt,
            "azure" => Self::Azure,
            "googleTts" => Self::GoogleTts,
            "steam" => Self::Steam,
            _ => return None,
        })
    }

    fn key_path(self) -> &'static str {
        match self {
            Self::Groq => "voice.groqApiKey",
            Self::OpenAiStt => "voice.openaiApiKey",
            Self::Azure => "voice.azureTtsSubscriptionKey",
            Self::GoogleTts => "voice.googleTtsApiKey",
            Self::Steam => "gamer-companion.steamApiKey",
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::Groq => "credentials:groq",
            Self::OpenAiStt => "credentials:openai",
            Self::Azure => "credentials:azure",
            Self::GoogleTts => "credentials:google-tts",
            Self::Steam => "credentials:steam",
        }
    }
}

/// A requisição que o teste vai fazer, decidida sem rede — é o que os testes olham.
#[derive(Debug, PartialEq)]
struct Probe {
    url: String,
    headers: Vec<(&'static str, String)>,
    /// Campo JSON cujo array vira a contagem mostrada na UI ("42 vozes").
    count_field: Option<&'static str>,
}

/// Região do Azure vira HOST (`{region}.tts.speech.microsoft.com`): só `[a-z0-9]` passa, senão
/// um valor como `evil.com/x?` apontaria a chave para outro servidor.
fn is_valid_azure_region(region: &str) -> bool {
    !region.is_empty() && region.len() <= 40 && region.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn plan(provider: Provider, key: &str, region: &str) -> Result<Probe, String> {
    let bearer = || vec![("Authorization", format!("Bearer {key}"))];
    Ok(match provider {
        Provider::Groq => Probe { url: GROQ_MODELS_URL.into(), headers: bearer(), count_field: Some("data") },
        Provider::OpenAiStt => Probe { url: OPENAI_MODELS_URL.into(), headers: bearer(), count_field: Some("data") },
        Provider::GoogleTts => {
            Probe { url: GOOGLE_TTS_VOICES_URL.into(), headers: vec![("X-Goog-Api-Key", key.to_string())], count_field: Some("voices") }
        }
        Provider::Azure => {
            if !is_valid_azure_region(region) {
                return Err("Região do Azure inválida — use o id curto, ex.: brazilsouth".into());
            }
            Probe {
                url: format!("https://{region}.tts.speech.microsoft.com/cognitiveservices/voices/list"),
                headers: vec![("Ocp-Apim-Subscription-Key", key.to_string())],
                count_field: Some(""),
            }
        }
        // A Web API da Steam só aceita a chave na query. Por isso nenhuma mensagem daqui repassa
        // o erro do reqwest (que imprime a URL).
        Provider::Steam => Probe { url: format!("{STEAM_API_LIST_URL}?key={key}"), headers: vec![], count_field: None },
    })
}

fn explicit_or_stored(payload: &Value, field: &str, store: &ConfigStore, path: &str) -> String {
    let explicit = payload.get(field).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    match explicit {
        Some(value) => value.to_string(),
        None => store.get(path).and_then(|v| v.as_str().map(|s| s.trim().to_string())).unwrap_or_default(),
    }
}

/// Status HTTP → frase para a pessoa. Sem corpo da resposta (pode ecoar a chave) e sem URL.
fn status_message(status: u16) -> String {
    match status {
        401 | 403 => "Chave recusada pelo provedor (401/403) — confira se copiou inteira".into(),
        404 => "Endpoint não encontrado (404) — região ou conta erradas?".into(),
        429 => "Limite de requisições atingido (429) — a chave existe, tente de novo em instantes".into(),
        other => format!("O provedor respondeu HTTP {other}"),
    }
}

fn count_in(body: &Value, field: Option<&str>) -> Option<usize> {
    match field? {
        "" => body.as_array().map(Vec::len),
        name => body.get(name).and_then(Value::as_array).map(Vec::len),
    }
}

async fn run_probe(store: &ConfigStore, provider: Provider, probe: Probe) -> Value {
    // Gate com a URL SEM query: a da Steam carrega a chave, e o gate só precisa do host.
    let gate_url = probe.url.split('?').next().unwrap_or_default();
    if let Err(error) = privacy::assert_network_allowed(store, gate_url, provider.tag()) {
        return json!({ "ok": false, "error": error });
    }
    let started = Instant::now();
    let mut request = crate::modules::llm::http_client().get(&probe.url).timeout(PROBE_TIMEOUT);
    for (name, value) in &probe.headers {
        request = request.header(*name, value);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            let reason = if error.is_timeout() { "tempo esgotado" } else { "sem conexão" };
            log::warn!("teste de credencial falhou: provider={} ({reason})", provider.tag());
            return json!({ "ok": false, "error": format!("Provedor inacessível ({reason})") });
        }
    };
    let latency_ms = started.elapsed().as_millis() as u64;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        log::info!("teste de credencial: provider={} status={status}", provider.tag());
        return json!({ "ok": false, "status": status, "latencyMs": latency_ms, "error": status_message(status) });
    }
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    json!({ "ok": true, "status": status, "latencyMs": latency_ms, "count": count_in(&body, probe.count_field) })
}

async fn test_credential(store: &ConfigStore, payload: &Value) -> Value {
    let Some(provider) = payload.get("provider").and_then(Value::as_str).and_then(Provider::parse) else {
        return json!({ "ok": false, "error": "Provedor sem teste de credencial" });
    };
    let key = explicit_or_stored(payload, "apiKey", store, provider.key_path());
    if key.is_empty() {
        return json!({ "ok": false, "error": "Preencha a chave antes de testar" });
    }
    if key.len() > MAX_SECRET_LEN || key.chars().any(char::is_control) {
        return json!({ "ok": false, "error": "Chave com formato inválido" });
    }
    let region = explicit_or_stored(payload, "region", store, "voice.azureTtsRegion");
    match plan(provider, &key, &region) {
        Ok(probe) => run_probe(store, provider, probe).await,
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

pub struct CredentialsModule;

#[async_trait]
impl NativeModule for CredentialsModule {
    fn name(&self) -> &'static str {
        "credentials"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(&self, app: &AppHandle, ctx: &InvokeContext, channel: &str, payload: Value) -> Result<Value, String> {
        if !is_owner_surface(ctx.window_type) {
            return Err(format!("{channel} recusado para {:?}", ctx.window_type));
        }
        let store: Arc<ConfigStore> = crate::state::config_store(app)?;
        match channel {
            CH_CREDENTIALS_TEST => Ok(test_credential(&store, &payload).await),
            other => Err(format!("credentials: canal '{other}' declarado mas não tratado")),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(CredentialsModule)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::test_support::ScratchDir;

    const KEY: &str = "gsk_SECRET123";

    fn store() -> (ConfigStore, ScratchDir) {
        let dir = ScratchDir::new("credentials-test");
        (ConfigStore::open(dir.path(), crate::config::schemas::registry()).unwrap(), dir)
    }

    #[test]
    fn the_channel_is_declared_in_the_typescript_contract() {
        let contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc.ts");
        assert!(contract.contains(&format!("channel: '{CH_CREDENTIALS_TEST}'")));
    }

    #[test]
    fn key_goes_in_a_header_for_every_provider_that_accepts_one() {
        for provider in [Provider::Groq, Provider::OpenAiStt, Provider::GoogleTts, Provider::Azure] {
            let probe = plan(provider, KEY, "brazilsouth").unwrap();
            assert!(!probe.url.contains(KEY), "{provider:?} vazou a chave na URL");
            assert!(probe.headers.iter().any(|(_, v)| v.contains(KEY)), "{provider:?} não mandou a chave");
        }
    }

    /// Região vira host: um valor com `.`/`/` apontaria a chave para outro servidor.
    #[test]
    fn azure_region_that_is_not_a_bare_id_is_refused_before_any_request() {
        for bad in ["", "evil.com/x?", "brazil south", "BRAZILSOUTH", "a-b"] {
            assert!(plan(Provider::Azure, KEY, bad).is_err(), "região {bad:?} deveria ser recusada");
        }
        assert!(plan(Provider::Azure, KEY, "brazilsouth").unwrap().url.starts_with("https://brazilsouth.tts.speech"));
    }

    #[test]
    fn status_messages_never_carry_the_key_or_the_url() {
        for status in [401, 403, 404, 429, 500] {
            let message = status_message(status);
            assert!(!message.contains("http") && !message.contains(KEY), "{message}");
        }
    }

    #[test]
    fn count_reads_the_array_the_provider_lists() {
        assert_eq!(count_in(&json!({ "data": [1, 2, 3] }), Some("data")), Some(3));
        assert_eq!(count_in(&json!([1, 2]), Some("")), Some(2));
        assert_eq!(count_in(&json!({ "voices": [] }), None), None);
    }

    #[tokio::test]
    async fn unknown_provider_and_empty_key_answer_without_touching_the_network() {
        let (store, _dir) = store();
        let unknown = test_credential(&store, &json!({ "provider": "openrouter" })).await;
        assert_eq!(unknown["ok"], json!(false));
        let empty = test_credential(&store, &json!({ "provider": "groq" })).await;
        assert_eq!(empty["error"], json!("Preencha a chave antes de testar"));
    }

    #[tokio::test]
    async fn offline_mode_blocks_the_probe_and_the_answer_does_not_echo_the_key() {
        let (store, _dir) = store();
        store.set("privacy.offlineMode", json!(true)).unwrap();
        store.set("gamer-companion.steamApiKey", json!(KEY)).unwrap();
        let result = test_credential(&store, &json!({ "provider": "steam" })).await;
        assert_eq!(result["ok"], json!(false));
        assert!(result["error"].as_str().unwrap().contains("Modo offline"));
        assert!(!result.to_string().contains(KEY));
    }

    #[test]
    fn every_external_url_goes_through_the_offline_gate() {
        let source = include_str!("mod.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        assert!(production.contains("privacy::assert_network_allowed(store, gate_url"));
        assert_eq!(production.matches(".get(&probe.url)").count(), 1, "toda saída passa por run_probe");
    }
}
