use super::*;

// --- roteamento de provider id ------------------------------------------------------------

#[test]
fn from_provider_id_resolves_exactly_the_two_cloud_ids() {
    assert_eq!(CloudSttProvider::from_provider_id("groq-whisper"), Some(CloudSttProvider::Groq));
    assert_eq!(CloudSttProvider::from_provider_id("openai-whisper"), Some(CloudSttProvider::OpenAi));
    assert_eq!(CloudSttProvider::from_provider_id("electron"), None);
    assert_eq!(CloudSttProvider::from_provider_id("local-whisper"), None);
    assert_eq!(CloudSttProvider::from_provider_id("browser"), None);
    assert_eq!(CloudSttProvider::from_provider_id(""), None);
}

#[test]
fn each_provider_points_at_its_own_config_path_model_and_endpoint() {
    assert_eq!(CloudSttProvider::Groq.api_key_path(), "voice.groqApiKey");
    assert_eq!(CloudSttProvider::OpenAi.api_key_path(), "voice.openaiApiKey");
    assert!(CloudSttProvider::Groq.endpoint().starts_with("https://api.groq.com"));
    assert!(CloudSttProvider::OpenAi.endpoint().starts_with("https://api.openai.com"));
    assert_eq!(CloudSttProvider::Groq.models()[0], "whisper-large-v3-turbo");
    assert_eq!(CloudSttProvider::OpenAi.models()[0], "gpt-transcribe");
}

#[test]
fn the_chosen_model_is_sent_and_an_unknown_one_never_reaches_the_request() {
    let (store, _dir) = test_store();
    assert_eq!(CloudSttProvider::OpenAi.model(&store), "gpt-transcribe");
    store.set("voice.openaiSttModel", json!("gpt-4o-transcribe")).unwrap();
    assert_eq!(CloudSttProvider::OpenAi.model(&store), "gpt-4o-transcribe");
    assert!(store.set("voice.groqSttModel", json!("modelo-que-nao-existe")).is_err(), "schema aceitou modelo fora da lista");
    assert_eq!(CloudSttProvider::Groq.model(&store), "whisper-large-v3-turbo");
}

// --- classificação de erro: texto fixo por classe, nunca o corpo/segredo cru --------------

#[test]
fn classify_http_error_returns_fixed_text_per_status_class() {
    let (store, _dir) = test_store();
    assert!(classify_http_error(&store, 401, CloudSttProvider::Groq).contains("Credencial inválida"));
    assert!(classify_http_error(&store, 403, CloudSttProvider::OpenAi).contains("Credencial inválida"));
    assert!(classify_http_error(&store, 429, CloudSttProvider::Groq).contains("Limite de uso"));
    assert!(classify_http_error(&store, 503, CloudSttProvider::OpenAi).contains("indisponível"));
    assert!(classify_http_error(&store, 400, CloudSttProvider::Groq).contains("HTTP 400"));
}

/// A mensagem chega à UI no idioma da interface, com o provedor e o status preenchidos, e nunca
/// como a chave crua do dicionário (chave faltando num locale cai no inglês, não vira `voice.…`).
#[test]
fn error_messages_follow_the_ui_language_in_every_locale() {
    let (store, _dir) = test_store();
    for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
        store.set("app.locale", json!(locale)).unwrap();
        for status in [401, 429, 503, 418] {
            let text = classify_http_error(&store, status, CloudSttProvider::Groq);
            assert!(text.contains("Groq") && !text.contains("{{") && !text.starts_with("voice."), "{locale} {status}: {text}");
        }
        let missing = missing_key_response(&store, CloudSttProvider::OpenAi)["error"].as_str().unwrap().to_string();
        assert!(missing.contains("voice.openaiApiKey") || missing.contains("apiKeys."), "{locale}: {missing}");
    }
    store.set("app.locale", json!("en-US")).unwrap();
    let english = classify_http_error(&store, 503, CloudSttProvider::Groq);
    store.set("app.locale", json!("ko-KR")).unwrap();
    assert_ne!(classify_http_error(&store, 503, CloudSttProvider::Groq), english, "ko-KR devolveu o texto em inglês");
}

// --- upload: só PCM16 vira WAV --------------------------------------------------------------

#[test]
fn build_upload_wraps_pcm16_in_wav() {
    let (bytes, mime) = build_upload(vec![1, 0, 2, 0], "audio/pcm;rate=16000", 16_000);
    assert_eq!(mime, "audio/wav");
    assert_eq!(&bytes[0..4], b"RIFF");
}

#[test]
fn build_upload_passes_non_pcm_through_unchanged() {
    let (bytes, mime) = build_upload(vec![9, 9, 9], "audio/webm", 16_000);
    assert_eq!(mime, "audio/webm");
    assert_eq!(bytes, vec![9, 9, 9]);
}

#[test]
fn build_upload_defaults_empty_mime_to_wav_without_converting() {
    let (bytes, mime) = build_upload(vec![7, 7], "", 16_000);
    assert_eq!(mime, "audio/wav");
    assert_eq!(bytes, vec![7, 7], "sem mimeType não é PCM16 — não deve ganhar cabeçalho RIFF");
}

// --- filename --------------------------------------------------------------------------

#[test]
fn filename_for_normalizes_known_mime_types() {
    assert_eq!(filename_for("audio/wav"), "audio.wav");
    assert_eq!(filename_for("audio/mpeg"), "audio.mp3");
    assert_eq!(filename_for("audio/x-wav;codec=1"), "audio.wav");
    assert_eq!(filename_for("audio/ogg"), "audio.ogg");
}

// --- multipart body: forma, não transporte ----------------------------------------------

#[test]
fn multipart_body_contains_every_field_and_closes_with_the_final_boundary() {
    let body = build_multipart_body("BOUND", b"RIFF...", "audio.wav", "audio/wav", "whisper-1", Some("pt"));
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("name=\"model\"\r\n\r\nwhisper-1"));
    assert!(text.contains("name=\"response_format\"\r\n\r\njson"));
    assert!(text.contains("name=\"language\"\r\n\r\npt"));
    assert!(text.contains("filename=\"audio.wav\""));
    assert!(text.contains("Content-Type: audio/wav"));
    assert!(text.ends_with("--BOUND--\r\n"));
}

#[test]
fn multipart_body_omits_language_field_when_absent() {
    let body = build_multipart_body("BOUND", b"x", "audio.wav", "audio/wav", "whisper-1", None);
    let text = String::from_utf8_lossy(&body);
    assert!(!text.contains("name=\"language\""));
}

#[test]
fn parse_success_text_trims_whitespace() {
    assert_eq!(parse_success_text(br#"{"text":"  oi mika  "}"#).unwrap(), "oi mika");
}

#[test]
fn parse_success_text_rejects_malformed_json() {
    assert!(parse_success_text(b"not json").is_err());
}

// --- custo zero: prova de que nenhum caminho de teste toca rede real ----------------------

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-stt-cloud-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn test_store() -> (ConfigStore, TempDir) {
    let dir = TempDir::new();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    (store, dir)
}

/// Sem chave configurada, a função devolve erro ANTES de resolver `assert_network_allowed` ou
/// tocar `reqwest` — não há `ConfigStore` com chave nenhuma aqui, então uma falha neste teste
/// que chegasse a montar requisição real seria óbvia (a chave usada no header seria vazia).
#[tokio::test]
async fn transcribe_fails_without_touching_the_network_when_api_key_is_missing() {
    let (store, _dir) = test_store();
    let result = transcribe(&store, CloudSttProvider::Groq, vec![0; 10], "audio/pcm;rate=16000", 16_000, None).await;
    assert_eq!(result["success"], json!(false));
    assert!(result["error"].as_str().unwrap().contains("Groq API key"));
}

/// PROVA DE CUSTO ZERO: com a chave presente (fixture sintética, nunca uma chave real) mas modo
/// offline ligado, `assert_network_allowed` recusa ANTES do corpo multipart ser montado ou de
/// `reqwest::Client::post` ser chamado — a função nunca chega a `send()`. Sem esse gate rodar
/// primeiro, este teste despacharia uma requisição HTTPS real pro Groq.
#[tokio::test]
async fn transcribe_never_touches_the_network_when_offline_mode_is_on() {
    let (store, _dir) = test_store();
    store.set(CloudSttProvider::Groq.api_key_path(), json!("gsk-fixture-sintetica-nao-real")).unwrap();
    store.set(privacy::OFFLINE_MODE_PATH, json!(true)).unwrap();

    let result = transcribe(&store, CloudSttProvider::Groq, vec![0; 10], "audio/pcm;rate=16000", 16_000, None).await;
    assert_eq!(result["success"], json!(false));
    assert!(result["error"].as_str().unwrap().contains("Modo offline ligado"));
}

/// OpenAI/Groq pedem ISO-639-1 (`pt`): `voice.asrLanguage` salvo como `pt-BR`/`pt_BR` virava HTTP 400
/// em toda fala. O Nemotron distingue variantes regionais, então a normalização é só do upload.
#[test]
fn the_language_sent_to_the_cloud_is_the_bare_iso_code() {
    assert_eq!(iso_language("pt-BR"), "pt");
    assert_eq!(iso_language("pt_BR"), "pt");
    assert_eq!(iso_language(" EN "), "en");
    assert_eq!(iso_language("ja"), "ja");
}
