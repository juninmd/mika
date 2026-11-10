use super::*;
use crate::protocol::WindowType;
use serde_json::json;

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-plugin-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> (ConfigStore, TempDir) {
    let dir = TempDir::new();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    (store, dir)
}

fn ctx(window_type: WindowType) -> InvokeContext {
    InvokeContext { window_type, window_label: "test".into(), plugin_id: None }
}

/// Mesma porta morta de `brain/turn.rs`/`gamer/llm_dead_port.rs`: `connect` falha na hora, igual
/// em qualquer máquina, com ou sem Ollama instalado — regra de custo zero deste repositório.
fn point_llm_at_dead_port(store: &ConfigStore) {
    store.set("apiKeys.ollamaBaseUrl", json!("http://127.0.0.1:11500")).unwrap();
}

// --- guarda de superfície -------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_denies_plugin_view_named_by_the_route() {
    let (store, dir) = store();
    let err = dispatch(&store, &ctx(WindowType::PluginView), &dir.0, &json!({ "pluginId": "mika.voice" })).await.unwrap_err();
    assert!(err.contains("pluginTest"), "erro deveria nomear a rota: {err}");
}

#[tokio::test]
async fn dispatch_denies_unknown_window() {
    let (store, dir) = store();
    assert!(dispatch(&store, &ctx(WindowType::Unknown), &dir.0, &json!({})).await.is_err());
}

#[tokio::test]
async fn dispatch_allows_the_three_owner_surfaces() {
    let (store, dir) = store();
    for window_type in [WindowType::Main, WindowType::Settings, WindowType::Wizard] {
        let result = dispatch(&store, &ctx(window_type), &dir.0, &json!({ "pluginId": "mika.embeddings" })).await;
        assert!(result.is_ok(), "{window_type:?} deveria passar a guarda");
    }
}

// --- fallback de pluginId (ipc/settings.ts) -------------------------------------------------------

#[tokio::test]
async fn dispatch_without_plugin_id_or_tab_id_is_a_named_failure_not_a_panic() {
    let (store, dir) = store();
    let result = dispatch(&store, &ctx(WindowType::Main), &dir.0, &json!({})).await.unwrap();
    assert_eq!(result["ok"], json!(false));
    assert_eq!(result["message"], json!("Nenhum plugin especificado para teste."));
}

#[tokio::test]
async fn dispatch_falls_back_to_tab_id_when_plugin_id_is_absent() {
    let (store, dir) = store();
    point_llm_at_dead_port(&store);
    store.set("llm.embeddingProvider", json!("ollama")).unwrap();
    let result = dispatch(&store, &ctx(WindowType::Main), &dir.0, &json!({ "tabId": "mika.embeddings" })).await.unwrap();
    assert!(result["message"].as_str().unwrap().contains("Falha ao validar embeddings"));
}

// --- mika.voice: STT ---------------------------------------------------------------------------

#[tokio::test]
async fn stt_electron_is_real_and_names_the_whisper_model_without_network() {
    let (store, _dir) = store();
    let result = test_stt_provider(&store, &json!({ "asrProvider": "electron" })).await;
    assert_eq!(result["ok"], json!(true));
    assert!(result["message"].as_str().unwrap().contains("Whisper local"));
}

#[tokio::test]
async fn stt_nemotron_is_local_and_the_default_when_nothing_is_saved() {
    let (store, _dir) = store();
    for config in [json!({ "asrProvider": "nemotron" }), json!({})] {
        let result = test_stt_provider(&store, &config).await;
        assert_eq!(result["ok"], json!(true));
        assert!(result["message"].as_str().unwrap().contains("Nemotron"));
    }
}
#[tokio::test]
async fn stt_local_whisper_is_also_treated_as_local() {
    let (store, _dir) = store();
    let result = test_stt_provider(&store, &json!({ "asrProvider": "local-whisper" })).await;
    assert_eq!(result["ok"], json!(true));
}

#[tokio::test]
async fn stt_browser_is_real_without_network() {
    let (store, _dir) = store();
    let result = test_stt_provider(&store, &json!({ "asrProvider": "browser" })).await;
    assert_eq!(result["ok"], json!(true));
    assert!(result["message"].as_str().unwrap().contains("navegador"));
}

/// Era `stt_cloud_providers_degrade_named_instead_of_calling_the_network`, e cobrava que groq/openai
/// respondessem "ainda não foi portado". Portaram (`voice/stt_cloud.rs`), então o que se cobra agora
/// é o ramo que prova custo zero: **sem chave configurada a validação nem chega à rede**.
///
/// O caminho com chave não tem teste automatizado de propósito — ele é um GET real a provedor pago,
/// e a regra da casa é que nenhuma suíte o dispare. Quem o exercita é o usuário, no botão.
#[tokio::test]
async fn stt_cloud_providers_report_the_missing_key_without_touching_the_network() {
    let (store, _dir) = store();
    for (provider, expected) in [("groq-whisper", "Groq"), ("openai-whisper", "OpenAI"), ("groq", "Groq"), ("openai", "OpenAI")] {
        let result = test_stt_provider(&store, &json!({ "asrProvider": provider })).await;
        assert_eq!(result["ok"], json!(false), "sem chave não pode dar ok para {provider}");
        let message = result["message"].as_str().unwrap();
        assert!(message.contains(expected), "mensagem deveria nomear o provedor: {message}");
        assert!(message.contains("não configurada"), "deveria apontar a chave faltando: {message}");
    }
}

#[tokio::test]
async fn stt_unknown_provider_is_a_named_failure() {
    let (store, _dir) = store();
    let result = test_stt_provider(&store, &json!({ "asrProvider": "carrier-pigeon" })).await;
    assert_eq!(result["ok"], json!(false));
    assert!(result["message"].as_str().unwrap().contains("não reconhecido"));
}

// --- mika.voice: TTS -----------------------------------------------------------------------------

#[tokio::test]
async fn tts_system_provider_is_real_ok_true_without_generating_audio() {
    let (store, dir) = store();
    let result = test_tts_connection(&store, &dir.0, &json!({ "voice.ttsProvider": "system" })).await;
    assert_eq!(result["ok"], json!(true));
    assert!(result["audio"].is_null(), "modo sistema não gera áudio");
}

#[tokio::test]
async fn tts_supertonic_with_offline_mode_degrades_named_without_touching_the_network() {
    let (store, dir) = store();
    store.set("privacy.offlineMode", json!(true)).unwrap();
    let result = test_tts_connection(&store, &dir.0, &json!({ "voice.ttsProvider": "supertonic" })).await;
    assert_eq!(result["ok"], json!(false));
    let message = result["message"].as_str().unwrap();
    assert!(message.to_lowercase().contains("offline"), "deveria nomear o bloqueio de rede: {message}");
}

/// Cobrava `elevenlabs`, depois `piper` — os dois quebraram quando viraram provedor real, um de
/// cada vez, e o teste documentava o corte, não o defeito. Agora cobra `kokoro`, o único que
/// **ainda** está em `CUT_PROVIDERS` (ver `voice/tts/dispatch.rs`); a lista de quem está cortado é
/// responsabilidade de `plugins/mod.rs`, que a lê por valor.
#[tokio::test]
async fn tts_cut_provider_degrades_named_without_touching_the_network() {
    let (store, dir) = store();
    let result = test_tts_connection(&store, &dir.0, &json!({ "voice.ttsProvider": "kokoro" })).await;
    assert_eq!(result["ok"], json!(false));
    assert!(result["message"].as_str().unwrap().contains("kokoro"), "mensagem deveria nomear o provedor cortado");
}

/// O par positivo: um provedor remoto portado NÃO pode mais responder "não foi portado". Sem chave
/// configurada, ElevenLabs não falha mais — cai para a voz local (Supertonic, ver
/// `dispatch.rs::generate_elevenlabs_with_fallback`, item 3 da rodada que portou o fallback). Modo
/// offline aqui não é o que está sob teste: é o jeito determinístico de provar que o caminho
/// tentado foi mesmo o Supertonic (bloqueado por rede, não pelo `apiKeys.elevenLabs` ausente), sem
/// baixar os pesos do modelo de verdade.
#[tokio::test]
async fn a_ported_remote_provider_falls_back_to_the_local_voice_instead_of_a_missing_port() {
    let (store, dir) = store();
    store.set("privacy.offlineMode", json!(true)).unwrap();
    let result = test_tts_connection(&store, &dir.0, &json!({ "voice.ttsProvider": "elevenlabs" })).await;
    assert_eq!(result["ok"], json!(false));
    let message = result["message"].as_str().unwrap();
    assert!(!message.contains("não foi portado"), "elevenlabs está portado; a mensagem envelheceu: {message}");
    assert!(!message.contains("apiKeys.elevenLabs"), "não devia ter chegado no erro nomeado do ElevenLabs: {message}");
    assert!(message.to_lowercase().contains("offline"), "não tentou o Supertonic (rota de fallback): {message}");
}

#[tokio::test]
async fn voice_branch_routes_stt_by_tab_id_and_tts_otherwise() {
    let (store, dir) = store();
    let stt_result = test_voice_connection(&store, &dir.0, Some("stt"), &json!({ "asrProvider": "browser" })).await;
    assert_eq!(stt_result["ok"], json!(true));
    assert!(stt_result["message"].as_str().unwrap().contains("navegador"));

    let tts_result = test_voice_connection(&store, &dir.0, Some("voice"), &json!({ "voice.ttsProvider": "system" })).await;
    assert_eq!(tts_result["ok"], json!(true));
}

// --- llm -----------------------------------------------------------------------------------------

/// Cobrava que provedor != `ollama` respondesse "sem cliente nativo ainda". Os três remotos foram
/// portados (`brain/llm_provider.rs`), então o que se cobra agora é a mesma distinção do TTS: sem
/// credencial ele falha **nomeando a credencial**, não a ausência da feature.
///
/// O caminho com chave não tem teste automatizado — é chamada real a provedor pago, e a regra da
/// casa é que nenhuma suíte a dispare.
#[tokio::test]
async fn a_ported_remote_llm_provider_names_the_missing_credential_not_a_missing_port() {
    let (store, _dir) = store();
    for (provider, expected) in [("google", "google"), ("openrouter", "openrouter")] {
        store.set("llm.provider", json!(provider)).unwrap();
        let result = test_llm_connection(&store, None, &json!({})).await;
        assert_eq!(result["ok"], json!(false), "sem chave não pode dar ok para {provider}");
        let message = result["message"].as_str().unwrap();
        assert!(message.contains(expected), "mensagem deveria nomear o provedor: {message}");
        assert!(!message.contains("sem cliente nativo"), "{provider} está portado; a mensagem envelheceu: {message}");
        assert!(message.contains("não configurada"), "deveria apontar a credencial faltando: {message}");
    }
}

#[tokio::test]
async fn llm_ollama_without_a_live_server_degrades_named_not_a_panic() {
    let (store, _dir) = store();
    point_llm_at_dead_port(&store);
    let result = test_llm_connection(&store, None, &json!({})).await;
    assert_eq!(result["ok"], json!(false));
    assert!(result["message"].as_str().unwrap().contains("Falha ao validar LLM"));
}

/// A redação em si tem suíte própria em `test_redaction.rs` (o que é cortado, o que sobrevive,
/// e o `?` de prosa que não é query). O que ESTE teste guarda é outra coisa, e é a que quebraria
/// sem barulho: que o ramo `llm` de fato PASSA o erro por ela antes de devolver. Redação escrita,
/// testada e não chamada é exatamente a classe de defeito que esta árvore já catalogou.
#[test]
fn the_llm_branch_routes_its_error_through_the_redaction_layer() {
    let source = include_str!("plugin_test.rs");
    let production: String = source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or_default()
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        production.contains("mask_secret("),
        "o ramo llm parou de passar o erro do provedor por mask_secret — a chave volta a poder chegar à tela"
    );
}

// --- mika.embeddings -------------------------------------------------------------------------------

/// Premissa remedida nesta rodada: `google`/`openrouter`/`openaiCompatible` deixaram de ser "sem
/// provedor portado" (`memory::episodic::embedding.rs::resolve_embedding_target` cobre os 4 desde
/// 2026-09-24) — sem credencial configurada, o erro agora é o HONESTO do provedor real (credencial
/// ausente), nunca mais o genérico "ainda sem cliente Rust". Sem rede real: `GoogleEmbeddingFetcher`
/// recusa ANTES de qualquer chamada quando `api_key` está vazia.
#[tokio::test]
async fn embeddings_google_without_a_credential_fails_named_not_as_unported() {
    let (store, _dir) = store();
    store.set("llm.embeddingProvider", json!("google")).unwrap();
    let result = test_embedding_connection(&store, Some("teste"), &json!({})).await;
    assert_eq!(result["ok"], json!(false));
    let message = result["message"].as_str().unwrap();
    assert!(message.contains("API key"), "esperava erro de credencial ausente, veio: {message}");
    assert!(!message.contains("não reconhecido"), "google é um dos 4 provedores portados");
}

/// Um 5º nome de provider (fora dos 4 de [02](../../../../../../specs/02-providers.md)) continua
/// recusado nomeado — cobertura do ramo `other` de `test_embedding_connection`. Vem pela config
/// TRANSIENTE (tela ainda não salvou), não por `store.set`: o schema do `ConfigStore` já recusaria
/// um enum inválido antes de chegar aqui — o único jeito de exercitar o ramo é um valor que a tela
/// ainda não persistiu.
#[tokio::test]
async fn embeddings_unknown_provider_is_named_not_silently_ignored() {
    let (store, _dir) = store();
    let result = test_embedding_connection(&store, Some("teste"), &json!({ "llm.embeddingProvider": "bedrock" })).await;
    assert_eq!(result["ok"], json!(false));
    assert!(result["message"].as_str().unwrap().contains("\"bedrock\""));
}

/// Unsloth sem modelo de embedding: erro NOMEADO antes de qualquer rede (não há modelo padrão
/// inventado) — e o provedor é reconhecido, não cai no ramo `other`.
#[tokio::test]
async fn embeddings_unsloth_without_a_model_fails_named_before_the_network() {
    let (store, _dir) = store();
    store.set("llm.embeddingProvider", json!("unsloth")).unwrap();
    let result = test_embedding_connection(&store, None, &json!({})).await;
    assert_eq!(result["ok"], json!(false));
    let message = result["message"].as_str().unwrap();
    assert!(message.contains("modelo de embedding"), "{message}");
    assert!(!message.contains("não reconhecido"), "{message}");
}

/// Mesma porta morta de `llm_ollama_without_a_live_server_degrades_named_not_a_panic` — chega até a
/// tentativa de rede real (prova que o ramo `ollama` não degrada mais sem tentar) e falha nomeado,
/// nunca panica.
#[tokio::test]
async fn embeddings_ollama_without_a_live_server_degrades_named_not_a_panic() {
    let (store, _dir) = store();
    point_llm_at_dead_port(&store);
    store.set("llm.embeddingProvider", json!("ollama")).unwrap();
    let result = test_embedding_connection(&store, Some("teste"), &json!({})).await;
    assert_eq!(result["ok"], json!(false));
    assert!(result["message"].as_str().unwrap().contains("Falha ao validar embeddings"));
}

/// Teste negativo obrigatório: modo offline barra ANTES de qualquer tentativa de rede, mesmo padrão
/// de `tts_supertonic_with_offline_mode_degrades_named_without_touching_the_network`.
#[tokio::test]
async fn embeddings_offline_mode_blocks_a_non_loopback_ollama() {
    let (store, _dir) = store();
    store.set("privacy.offlineMode", json!(true)).unwrap();
    store.set("llm.embeddingProvider", json!("ollama")).unwrap();
    store.set("apiKeys.ollamaBaseUrl", json!("http://ollama-remoto.invalid:11434")).unwrap();
    let result = test_embedding_connection(&store, Some("teste"), &json!({})).await;
    assert_eq!(result["ok"], json!(false));
    assert!(result["message"].to_string().to_lowercase().contains("offline"), "deveria nomear o bloqueio de rede: {result}");
}

/// `local` is the default provider: the test button must run it (in-process, so offline mode does
/// not block it) and, without the model, fail naming the embedding — not "unknown provider".
#[tokio::test]
async fn embeddings_local_runs_even_offline_and_names_the_missing_model() {
    let (store, _dir) = store();
    store.set("privacy.offlineMode", json!(true)).unwrap();
    let result = test_embedding_connection(&store, Some("teste"), &json!({})).await;
    assert_eq!(result["ok"], json!(false));
    assert!(result["message"].as_str().unwrap().contains("Falha ao validar embeddings"), "{result}");
}

// --- registro do ramo genérico ----------------------------------------------------------------------

struct FixedTestConnection(Value);

#[async_trait]
impl TestConnection for FixedTestConnection {
    async fn call(&self, _prompt: Option<&str>) -> Value {
        self.0.clone()
    }
}

#[tokio::test]
async fn dispatch_reaches_a_registered_generic_handler() {
    let (store, dir) = store();
    registry().register("mika.zzz-test-plugin", Arc::new(FixedTestConnection(json!({ "ok": true, "message": "oi" }))));

    let result = dispatch(&store, &ctx(WindowType::Main), &dir.0, &json!({ "pluginId": "mika.zzz-test-plugin" })).await.unwrap();
    assert_eq!(result, json!({ "ok": true, "message": "oi" }));
}

#[tokio::test]
async fn dispatch_names_the_gap_for_an_unregistered_generic_plugin() {
    let (store, dir) = store();
    let result = dispatch(&store, &ctx(WindowType::Main), &dir.0, &json!({ "pluginId": "mika.not-registered-anywhere" })).await.unwrap();
    assert_eq!(result["ok"], json!(false));
    assert!(result["message"].as_str().unwrap().contains("mika.not-registered-anywhere"));
}
/// Par assimétrico: um endereço vindo da tela (config transitória) nunca leva a chave guardada —
/// senão quem forja o payload recebe `apiKeys.ollamaApiKey` no primeiro POST.
#[test]
fn a_transient_endpoint_never_carries_the_stored_credential() {
    let (store, _dir) = store();
    store.set("apiKeys.ollamaBaseUrl", json!("http://localhost:11434")).unwrap();
    store.set("apiKeys.ollamaApiKey", json!("sk-guardada")).unwrap();
    let forged = json!({ "apiKeys.ollamaBaseUrl": "http://atacante.example" });
    let (url, key) = endpoint_and_key(&forged, &store, "apiKeys.ollamaBaseUrl", "apiKeys.ollamaApiKey");
    assert_eq!(url.as_deref(), Some("http://atacante.example"));
    assert_eq!(key, "");

    let both = json!({ "apiKeys.ollamaBaseUrl": "http://outro:11434", "apiKeys.ollamaApiKey": "sk-da-tela" });
    assert_eq!(endpoint_and_key(&both, &store, "apiKeys.ollamaBaseUrl", "apiKeys.ollamaApiKey").1, "sk-da-tela");

    let (url, key) = endpoint_and_key(&json!({}), &store, "apiKeys.ollamaBaseUrl", "apiKeys.ollamaApiKey");
    assert_eq!((url.as_deref(), key.as_str()), (Some("http://localhost:11434"), "sk-guardada"));
}
