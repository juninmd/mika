//! Porte dos provedores de STT em nuvem — `groq-whisper`/`openai-whisper` — de
//! `stt-provider-runner.ts` (ramos `case 'groq-whisper'`/`case 'openai-whisper'`) e
//! `asr.service.ts::ASRService.transcribe`. Chamado por `transcribe::transcribe_pcm` no MESMO
//! ponto de dispatch do Whisper local — não é um caminho paralelo.
//!
//! ## O que cada provedor manda (medido no TS antes de escrever aqui)
//!
//! | | Groq | OpenAI |
//! |---|---|---|
//! | endpoint | `api.groq.com/openai/v1/audio/transcriptions` | `api.openai.com/v1/audio/transcriptions` |
//! | modelo | `whisper-large-v3-turbo` | `whisper-1` |
//! | chave config | `voice.groqApiKey` | `voice.openaiApiKey` |
//! | corpo | `multipart/form-data`: `file`, `model`, `response_format=json`, `language?` |
//! | resposta | JSON `{ text, language? }` — só `text` (trimado) importa aqui |
//!
//! Os dois são IDÊNTICOS na forma (mesma API compatível com OpenAI) — só endpoint/modelo/chave
//! mudam, por isso [`CloudSttProvider`] carrega os três como dado em vez de duplicar a função.
//!
//! ## Upload: PCM16 vira WAV, o resto passa direto
//!
//! Espelha `toUploadable` (`stt-provider-runner.ts`): só quando o `mimeType` é `audio/pcm*` (o
//! caso normal de fala, PCM16 mono 16 kHz na fronteira do Main — `AGENTS.md` §2) o áudio é
//! envolvido em WAV por [`super::pcm_wav::pcm16_to_wav`]; qualquer outro mimeType (webm/ogg do
//! `MediaRecorder`, usado pela tela de homologação) sobe cru, como o TS já fazia. A checagem de
//! PCM16 é a MESMA de `transcribe::is_pcm16_mime` (`pub(super)` ali), não uma segunda cópia.
//!
//! ## Chokepoint de privacidade e segredo
//!
//! `privacy::assert_network_allowed` roda ANTES de montar o corpo da requisição — se o modo
//! offline estiver ligado, a função devolve erro sem nunca chamar `reqwest` (prova em
//! `transcribe_never_touches_the_network_when_offline_mode_is_on`, abaixo). A API key nunca entra
//! em log nem na mensagem de erro devolvida: os erros de HTTP são texto FIXO classificado por
//! status (mesmo espírito de `classifyAsrFailure`), nunca o corpo bruto da resposta do provedor
//! (poderia ecoar algo sensível) nem a URL (a chave vai no header `Authorization`, nunca em query
//! string, então não há URL para vazar).
//!
//! ## Multipart sem a feature `multipart` do reqwest
//!
//! `Cargo.toml` é arquivo compartilhado por várias frentes (ver o comentário no bloco `reqwest`
//! de lá) e está fora do escopo desta tarefa; a feature `multipart` do reqwest não está habilitada
//! no crate. O corpo é montado à mão em [`build_multipart_body`] — formato simples, testado sem
//! rede em vez de depender da feature.

use crate::config::schemas::cloud_stt;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::privacy;

use super::pcm_wav::pcm16_to_wav;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CloudSttProvider {
    Groq,
    OpenAi,
}

impl CloudSttProvider {
    /// Espelha o `switch` de `runSttProvider`: só estes dois valores de `STTProvider` chegam aqui.
    pub(super) fn from_provider_id(id: &str) -> Option<Self> {
        match id {
            "groq-whisper" => Some(Self::Groq),
            "openai-whisper" => Some(Self::OpenAi),
            _ => None,
        }
    }

    fn endpoint(self) -> &'static str {
        match self {
            Self::Groq => cloud_stt::GROQ_TRANSCRIPTIONS_URL,
            Self::OpenAi => cloud_stt::OPENAI_TRANSCRIPTIONS_URL,
        }
    }

    /// Modelos que cada API aceita neste endpoint; o 1º é o default (catálogo em `cloud_stt.rs`).
    fn models(self) -> &'static [&'static str] {
        match self {
            Self::Groq => cloud_stt::GROQ_MODELS,
            Self::OpenAi => cloud_stt::OPENAI_MODELS,
        }
    }

    fn model_path(self) -> &'static str {
        match self {
            Self::Groq => "voice.groqSttModel",
            Self::OpenAi => "voice.openaiSttModel",
        }
    }

    /// Modelo escolhido pelo usuário; valor fora da lista cai no default em vez de virar 400.
    fn model(self, store: &ConfigStore) -> &'static str {
        let chosen = store.get(self.model_path()).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        self.models().iter().copied().find(|m| *m == chosen).unwrap_or(self.models()[0])
    }

    /// Endpoint de validação de credencial (`voice-test-stt.ts`) — listar modelos não consome
    /// cota de transcrição, que é por que o TS escolheu este e não o de áudio.
    fn models_endpoint(self) -> &'static str {
        match self {
            Self::Groq => cloud_stt::GROQ_MODELS_URL,
            Self::OpenAi => cloud_stt::OPENAI_MODELS_URL,
        }
    }

    /// Espelha `'voice.groqApiKey'`/`'voice.openaiApiKey'` (`stt-provider-runner.ts`).
    fn api_key_path(self) -> &'static str {
        match self {
            Self::Groq => "voice.groqApiKey",
            Self::OpenAi => "voice.openaiApiKey",
        }
    }

    /// Rótulo fixo pro chokepoint de privacidade — nunca a URL nem a chave (mesma regra de
    /// `privacy::assert_network_allowed`, que só loga o `provider_tag`).
    fn tag(self) -> &'static str {
        match self {
            Self::Groq => "voice:stt:groq",
            Self::OpenAi => "voice:stt:openai",
        }
    }
}

fn display_name(provider: CloudSttProvider) -> &'static str {
    match provider {
        CloudSttProvider::Groq => "Groq",
        CloudSttProvider::OpenAi => "OpenAI",
    }
}

/// Mensagem da UI no idioma de `app.locale` (chaves em `voice.stt.cloud.*`).
fn message(store: &ConfigStore, key: &str, provider: CloudSttProvider, extra: &[(&str, &str)]) -> String {
    let mut vars = vec![("provider", display_name(provider))];
    vars.extend_from_slice(extra);
    crate::modules::settings::tf(key, &crate::config::locale::app_locale(store), &vars)
}

fn missing_key_response(store: &ConfigStore, provider: CloudSttProvider) -> Value {
    let error = message(store, "voice.stt.cloud.missingKey", provider, &[("path", provider.api_key_path())]);
    json!({ "success": false, "error": error })
}

/// Espelha `classifyAsrFailure`: texto fixo por classe de status, nunca o corpo bruto da resposta
/// do provedor (poderia ecoar algo sensível de volta pra tela).
fn classify_http_error(store: &ConfigStore, status: u16, provider: CloudSttProvider) -> String {
    match status {
        401 | 403 => message(store, "voice.stt.cloud.invalidCredential", provider, &[]),
        429 => message(store, "voice.stt.cloud.rateLimited", provider, &[]),
        s if s >= 500 => message(store, "voice.stt.cloud.unavailable", provider, &[]),
        s => message(store, "voice.stt.cloud.failed", provider, &[("status", &s.to_string())]),
    }
}

/// Espelha `toUploadable`: só PCM16 vira WAV; qualquer outro mimeType sobe cru.
fn build_upload(bytes: Vec<u8>, mime_type: &str, sample_rate: u32) -> (Vec<u8>, String) {
    if super::transcribe::is_pcm16_mime(mime_type) {
        (pcm16_to_wav(&bytes, sample_rate), "audio/wav".to_string())
    } else {
        let resolved = if mime_type.is_empty() { "audio/wav" } else { mime_type };
        (bytes, resolved.to_string())
    }
}

/// Espelha `ASRService.filenameFor`.
fn filename_for(mime_type: &str) -> String {
    let ext = mime_type.split('/').nth(1).and_then(|s| s.split(';').next()).unwrap_or("mp3");
    let safe = match ext {
        "mpeg" => "mp3",
        "x-wav" => "wav",
        "ogg" => "ogg",
        other => other,
    };
    format!("audio.{safe}")
}

/// Código ISO-639-1 que as APIs pedem: `pt-BR`/`pt_BR`/` PT ` -> `pt`.
fn iso_language(language: &str) -> String {
    language.trim().split(['-', '_']).next().unwrap_or_default().to_lowercase()
}

/// Monta o corpo `multipart/form-data` à mão (ver o cabeçalho do arquivo pro porquê). Campos na
/// MESMA ordem de `ASRService.transcribe`: `model`, `response_format`, `language?`, `file`.
fn build_multipart_body(
    boundary: &str,
    file_bytes: &[u8],
    filename: &str,
    file_mime_type: &str,
    model: &str,
    language: Option<&str>,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(file_bytes.len() + 512);

    let push_field = |body: &mut Vec<u8>, name: &str, value: &str| {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes());
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    };

    push_field(&mut body, "model", model);
    push_field(&mut body, "response_format", "json");
    if let Some(lang) = language {
        push_field(&mut body, "language", lang);
    }

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n").as_bytes());
    body.extend_from_slice(format!("Content-Type: {file_mime_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(b"\r\n");

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// Espelha o corpo de sucesso `{ text, language? }` — só `text` (trimado) importa pro chamador.
fn parse_success_text(body: &[u8]) -> Result<String, String> {
    let parsed: Value = serde_json::from_slice(body).map_err(|_| "invalid response from the provider".to_string())?;
    let text = parsed.get("text").and_then(Value::as_str).unwrap_or_default();
    Ok(text.trim().to_string())
}

static HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn http_client() -> &'static reqwest::Client {
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("cliente http de stt")
    })
}

/// Fachada real: resolve a chave -> chokepoint de privacidade -> monta multipart -> POST ->
/// classifica erro/parseia sucesso. Retorna o MESMO contrato `{success, error}`/
/// `{success, data:{text}}` de `transcribe::transcribe_pcm`, para o chamador não distinguir
/// local de nuvem.
pub(super) async fn transcribe(
    store: &ConfigStore,
    provider: CloudSttProvider,
    audio: Vec<u8>,
    mime_type: &str,
    sample_rate: u32,
    language: Option<String>,
) -> Value {
    let api_key = store
        .get(provider.api_key_path())
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty());
    let Some(api_key) = api_key else {
        return missing_key_response(store, provider);
    };

    if let Err(e) = privacy::assert_network_allowed(store, provider.endpoint(), provider.tag()) {
        return json!({ "success": false, "error": e });
    }

    let (upload_bytes, upload_mime) = build_upload(audio, mime_type, sample_rate);
    let filename = filename_for(&upload_mime);
    let boundary = format!("mika-stt-{}", uuid::Uuid::new_v4());
    let language = language.as_deref().map(iso_language);
    let body = build_multipart_body(&boundary, &upload_bytes, &filename, &upload_mime, provider.model(store), language.as_deref());

    let response = match http_client()
        .post(provider.endpoint())
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        // Nunca ecoa o erro cru do reqwest: pode conter a URL/endpoint.
        Err(_) => return json!({ "success": false, "error": message(store, "voice.stt.cloud.network", provider, &[]) }),
    };

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return json!({ "success": false, "error": classify_http_error(store, status, provider) });
    }

    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(_) => return json!({ "success": false, "error": message(store, "voice.stt.cloud.network", provider, &[]) }),
    };

    match parse_success_text(&bytes) {
        Ok(text) => json!({ "success": true, "data": { "text": text } }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// O botão "testar conexão" de STT em Configurações — porte de `voice-test-stt.ts`.
///
/// Mora AQUI e não em `settings/` de propósito: endpoint, caminho da chave e rótulo de cada
/// provedor já são deste arquivo, e duplicá-los na tela de settings criaria a segunda cópia que
/// diverge sozinha. O `settings` só pergunta "esta credencial serve?".
///
/// Usa `/v1/models` (GET), o mesmo do TS: valida a credencial sem gastar transcrição. Ainda assim
/// é chamada de rede a provedor pago, então passa pelo chokepoint de privacidade como qualquer
/// outra e NENHUM teste automatizado a executa — os testes cobrem os dois ramos que retornam antes
/// do `.send()` (sem chave, e modo offline).
pub(crate) async fn validate_credentials(store: &ConfigStore, provider_id: &str) -> Option<Value> {
    // `voice-test-stt.ts` aceita os apelidos curtos (`'groq'`/`'openai'`) além dos ids de
    // `STTProvider`; `runSttProvider` não. O apelido vive só neste caminho, então é aqui que ele é
    // normalizado — `from_provider_id` continua estrito, para o dispatch de transcrição não passar
    // a aceitar id que o TS nunca mandaria.
    let provider = match provider_id {
        "groq" => Some(CloudSttProvider::Groq),
        "openai" => Some(CloudSttProvider::OpenAi),
        other => CloudSttProvider::from_provider_id(other),
    }?;
    let api_key = store.get(provider.api_key_path()).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty());
    let Some(api_key) = api_key else {
        return Some(json!({ "ok": false, "message": message(store, "voice.stt.cloud.missingKey", provider, &[("path", provider.api_key_path())]) }));
    };

    let endpoint = provider.models_endpoint();
    if let Err(e) = privacy::assert_network_allowed(store, endpoint, provider.tag()) {
        return Some(json!({ "ok": false, "message": e }));
    }

    let response = http_client().get(endpoint).header("Authorization", format!("Bearer {api_key}")).send().await;
    Some(match response {
        // O TS devolve o CORPO da resposta de erro aqui; este porte devolve a classe do status.
        // A diferença é deliberada: corpo de erro de provedor já veio com a chave ecoada dentro
        // ("Incorrect API key provided: sk-..."), e esta mensagem vai direto para a tela.
        Ok(r) if r.status().is_success() => {
            json!({ "ok": true, "message": message(store, "voice.stt.cloud.testOk", provider, &[]) })
        }
        Ok(r) => json!({ "ok": false, "message": classify_http_error(store, r.status().as_u16(), provider) }),
        Err(_) => json!({ "ok": false, "message": message(store, "voice.stt.cloud.network", provider, &[]) }),
    })
}

#[cfg(test)]
mod tests;
