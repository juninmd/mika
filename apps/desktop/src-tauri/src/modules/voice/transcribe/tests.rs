use super::*;

// --- lógica pura, sem ConfigStore nem Whisper -------------------------------------------

#[test]
fn parse_payload_requires_non_empty_audio_base64() {
    assert!(parse_payload(&json!({})).is_err());
    assert!(parse_payload(&json!({ "audioBase64": "" })).is_err());
    assert!(parse_payload(&json!({ "audioBase64": 42 })).is_err());
}

#[test]
fn parse_payload_reads_optional_fields_when_present() {
    let req = parse_payload(&json!({
        "audioBase64": "AAA=",
        "mimeType": "audio/pcm;rate=16000",
        "language": "pt",
    }))
    .unwrap();
    assert_eq!(req.audio_base64, "AAA=");
    assert_eq!(req.mime_type.as_deref(), Some("audio/pcm;rate=16000"));
    assert_eq!(req.language.as_deref(), Some("pt"));
}

#[test]
fn parse_payload_tolerates_missing_optionals() {
    let req = parse_payload(&json!({ "audioBase64": "AAA=" })).unwrap();
    assert_eq!(req.mime_type, None);
    assert_eq!(req.language, None);
}

/// NEGATIVO — sem este teto, `mika:voice-transcribe` aceitava `audioBase64` de qualquer tamanho:
/// um payload malformado de dezenas de MB chegava inteiro ao Whisper antes de qualquer validação
/// de conteúdo. `MAX_AUDIO_BASE64_CHARS` é o único teto do canal (ver AGENTS.md §"Isolamento de
/// processo", "unbounded strings").
#[test]
fn parse_payload_rejects_audio_base64_over_the_size_cap() {
    let oversized = "A".repeat(MAX_AUDIO_BASE64_CHARS + 1);
    let error = parse_payload(&json!({ "audioBase64": oversized })).err().expect("devia rejeitar payload acima do teto");
    assert!(error.contains("excede o teto"), "{error}");
}

#[test]
fn parse_payload_accepts_audio_base64_at_the_size_cap() {
    let at_cap = "A".repeat(MAX_AUDIO_BASE64_CHARS);
    assert!(parse_payload(&json!({ "audioBase64": at_cap })).is_ok());
}

#[test]
fn is_local_provider_accepts_only_the_on_device_engines() {
    assert!(is_local_provider("electron"));
    assert!(is_local_provider("local-whisper"));
    assert!(is_local_provider("nemotron"));
    assert!(!is_local_provider("browser"));
    assert!(!is_local_provider("groq-whisper"));
    assert!(!is_local_provider("openai-whisper"));
    assert!(!is_local_provider(""));
}

#[test]
fn caption_annotations_from_noise_are_not_speech() {
    assert_eq!(without_annotations("[Música]"), "");
    assert_eq!(without_annotations(" [SOM DE GOL] ♪ "), "");
    assert_eq!(without_annotations("(risos)"), "");
}

#[test]
fn real_words_around_an_annotation_survive() {
    assert_eq!(without_annotations("Oi Mika [Música] tudo bem?"), "Oi Mika tudo bem?");
    assert_eq!(without_annotations("sem anotação nenhuma"), "sem anotação nenhuma");
}

#[test]
fn unsupported_provider_response_fails_with_the_provider_named() {
    let response = unsupported_provider_response("browser");
    assert_eq!(response["success"], json!(false));
    assert!(response["error"].as_str().unwrap().contains("browser"));
}

#[test]
fn is_pcm16_mime_accepts_only_the_pcm_prefix() {
    assert!(is_pcm16_mime("audio/pcm;rate=16000"));
    assert!(is_pcm16_mime("audio/pcm16"));
    assert!(!is_pcm16_mime("audio/webm"));
    assert!(!is_pcm16_mime("audio/mpeg"));
    assert!(!is_pcm16_mime(""));
}

#[test]
fn non_pcm_response_names_the_offending_mime_type() {
    let response = non_pcm_response("audio/webm");
    assert_eq!(response["success"], json!(false));
    assert!(response["error"].as_str().unwrap().contains("audio/webm"));

    let response = non_pcm_response("");
    assert!(response["error"].as_str().unwrap().contains("sem mimeType"));
}

#[test]
fn pcm_sample_rate_reads_the_rate_parameter() {
    assert_eq!(pcm_sample_rate("audio/pcm;rate=16000"), 16_000);
    assert_eq!(pcm_sample_rate("audio/pcm;rate=8000"), 8_000);
}

/// NEGATIVO — sem parâmetro ou com um valor que não faz sentido, cai no default em vez de
/// entrar em pânico ou silenciosamente aceitar um número absurdo sem checagem nenhuma.
/// `inference::whisper::transcribe` continua sendo quem recusa o valor de verdade.
#[test]
fn pcm_sample_rate_defaults_when_the_parameter_is_absent_or_malformed() {
    assert_eq!(pcm_sample_rate("audio/pcm"), DEFAULT_PCM_SAMPLE_RATE);
    assert_eq!(pcm_sample_rate("audio/pcm;rate=abc"), DEFAULT_PCM_SAMPLE_RATE);
    assert_eq!(pcm_sample_rate(""), DEFAULT_PCM_SAMPLE_RATE);
}

// --- resolução de config: mesmo padrão de `desktop.rs::resolve_ollama_endpoint` ---------

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-voice-transcribe-test-{}-{unique}", std::process::id()));
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

#[test]
fn resolve_provider_defaults_to_nemotron_without_explicit_config() {
    // A fresh install must not start on Whisper, which turns room noise into "[Música]" messages.
    let (store, _dir) = test_store();
    assert_eq!(resolve_provider(&store), "nemotron");
}

#[test]
fn resolve_provider_prefers_saved_config() {
    let (store, _dir) = test_store();
    store.set(CFG_ASR_PROVIDER, json!("groq-whisper")).unwrap();
    assert_eq!(resolve_provider(&store), "groq-whisper");
}

#[test]
fn resolve_language_prefers_the_request_over_config() {
    let (store, _dir) = test_store();
    store.set(CFG_ASR_LANGUAGE, json!("en")).unwrap();
    assert_eq!(resolve_language(&store, Some("pt".to_string())).as_deref(), Some("pt"));
    assert_eq!(resolve_language(&store, None).as_deref(), Some("en"));
}

/// Sem `asrLanguage` salvo a fala segue o idioma da interface: antes o default era "pt" fixo e um
/// usuário em coreano era transcrito como português. Escolha explícita continua valendo.
#[test]
fn configured_language_follows_app_locale_unless_the_user_chose_one() {
    let (store, _dir) = test_store();
    assert_eq!(configured_language(&store).as_deref(), Some("pt"));
    store.set("app.locale", json!("ko-KR")).unwrap();
    assert_eq!(configured_language(&store).as_deref(), Some("ko"));
    store.set(CFG_ASR_LANGUAGE, json!("en")).unwrap();
    assert_eq!(configured_language(&store).as_deref(), Some("en"));
}

// --- dispatch: provider de nuvem sai pelo MESMO ponto, não um caminho paralelo -----------

/// Prova de fiação: `resolve_provider` -> `groq-whisper` -> `transcribe_pcm` reconhece o id via
/// `stt_cloud::CloudSttProvider::from_provider_id` e chama `stt_cloud::transcribe` — sem chave
/// configurada, a função volta o erro de `stt_cloud` (não mais o `unsupported_provider_response`
/// de antes do porte). Não toca rede: a ausência de chave é o primeiro `return` de `stt_cloud::
/// transcribe`.
#[tokio::test]
async fn transcribe_pcm_routes_cloud_providers_to_stt_cloud_instead_of_unsupported() {
    let (store, _dir) = test_store();
    store.set(CFG_ASR_PROVIDER, json!("groq-whisper")).unwrap();

    let req = TranscribeRequest { audio_base64: "AAA=".to_string(), mime_type: Some("audio/pcm;rate=16000".to_string()), language: None };
    let result = transcribe_pcm(&store, std::env::temp_dir(), req).await;

    assert_eq!(result["success"], json!(false));
    let error = result["error"].as_str().unwrap();
    assert!(error.contains("Groq API key"), "esperava o erro de stt_cloud (chave ausente), veio: {error}");
    assert!(!error.contains("não é suportado"), "groq-whisper não deve mais cair em unsupported_provider_response");
}
/// The barge-in path called Whisper directly whatever `asrProvider` said, so the engine that
/// hallucinates "[Música]" from TTS echo kept transcribing interruptions after Nemotron became the
/// default. Both paths now share `transcribe_local`; without the model, the error names Nemotron.
#[tokio::test]
async fn transcribe_local_routes_nemotron_to_nemotron_not_whisper() {
    let missing = std::env::temp_dir().join("mika-transcribe-local-no-models");
    let pcm = vec![0u8; 16_000 * 2];
    let error = transcribe_local("nemotron", whisper::DEFAULT_WHISPER_MODEL_ID, missing.clone(), pcm.clone(), 16_000, None).await.unwrap_err();
    assert!(error.contains("Nemotron"), "esperava o erro do Nemotron, veio: {error}");
    let error = transcribe_local("local-whisper", whisper::DEFAULT_WHISPER_MODEL_ID, missing, pcm, 16_000, None).await.unwrap_err();
    assert!(!error.contains("Nemotron"), "Whisper não deve cair no Nemotron: {error}");
}

#[test]
fn resolve_whisper_model_defaults_to_base_without_explicit_config() {
    let (store, _dir) = test_store();
    assert_eq!(resolve_whisper_model(&store), whisper::DEFAULT_WHISPER_MODEL_ID);
}

#[test]
fn resolve_whisper_model_prefers_saved_config() {
    let (store, _dir) = test_store();
    store.set(CFG_WHISPER_MODEL, json!("onnx-community/whisper-small")).unwrap();
    assert_eq!(resolve_whisper_model(&store), "onnx-community/whisper-small");
}

/// O motor `silero` só descarta quando o Silero RODOU e não ouviu voz na janela; sem veredito (VAD
/// mudo, modelo ausente) a fala segue — descartar ali calaria o STT inteiro.
#[test]
fn neural_vad_rejects_only_a_silero_verdict_of_no_voice() {
    assert!(neural_vad_rejects("silero", false, Some(false), false));
    assert!(!neural_vad_rejects("silero", false, Some(true), false));
    assert!(!neural_vad_rejects("silero", true, None, true), "sem veredito falha aberto");
    assert!(!neural_vad_rejects("energy", false, Some(false), true), "motor de energia sem `vadIgnoreTyping`: nunca descarta");
}

/// `voice.vadIgnoreTyping` no motor de energia: descarta só o trecho em que o VAD VIU tecla e não
/// viu voz. Ventoinha/ruído sem tecla continua passando — é a semântica do motor de energia.
#[test]
fn energy_engine_drops_a_window_that_was_only_typing_when_ignore_typing_is_on() {
    assert!(neural_vad_rejects("energy", true, Some(false), true), "só digitação: descarta");
    assert!(!neural_vad_rejects("energy", true, Some(true), true), "voz por cima da digitação: segue");
    assert!(!neural_vad_rejects("energy", true, Some(false), false), "sem tecla vista: energia decide");
}

/// 16 kHz PCM16 mono: 32_000 bytes = 1 s = 42_668 chars de base64 (arredondado para cima).
#[test]
fn pcm_window_covers_the_utterance_plus_slack() {
    assert_eq!(pcm_window_ms(42_668), 1_000 + NEURAL_VAD_SLACK_MS);
    assert_eq!(pcm_window_ms(0), NEURAL_VAD_SLACK_MS);
}
