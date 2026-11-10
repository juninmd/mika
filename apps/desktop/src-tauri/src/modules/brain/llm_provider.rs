//! Resolução MULTI-PROVEDOR de LLM — port de `resolveLanguageModelForProvider`
//! (`packages/llm/src/llm-provider.ts`) sobre `rig-core`, para os 5 provedores reais do catálogo
//! (`packages/sdk/src/llm/llm-catalog-data.ts`): `google` (Gemini), `openrouter`, `ollama`,
//! `openaiCompatible` e `unsloth` (preset do `openaiCompatible`: mesmo cliente, endereço/chave/modelo
//! próprios — ver [`crate::modules::llm::OpenAiShaped`]). Desde a `rig-core` 0.43 não há mais um `Client` por provedor: cada um é uma
//! `Model<Wire, Transport>` (o *wire* diz o que enviar e como ler a resposta; o transporte, por onde),
//! e `DynModel<Completion>` apaga o par — é `Clone`, `Send + Sync` e `dyn`-compatível, então o enum de
//! despacho que existia aqui (`ProviderModel`, porque o antigo `CompletionModel` usava RPITIT) e o
//! genérico `run_turn<M>` deixaram de ser necessários. Nenhum provedor do catálogo TS ficou de fora:
//! não há degrade nomeado a reportar aqui.
//!
//! O transporte é SEMPRE o `reqwest::Client` do app (`llm::http_client`, com timeouts e pool próprios)
//! embrulhado em `rig_reqwest::ReqwestClient`: o transporte embutido do `rig-core` (feature `reqwest`)
//! usaria um pool separado, sem os timeouts daqui.
//!
//! ## `openaiCompatible` usa a Completions API, não a Responses API
//!
//! `OpenAI::completion()` (`rig-core`) segue a rota do dialeto — Responses API (`/v1/responses`) para
//! OpenAI, endpoint exclusivo dela que LiteLLM/vLLM/NVIDIA NIM/o próprio Ollama (`/v1`) não
//! implementam. `OpenAI::chat()` fixa Chat Completions (`/v1/chat/completions`), o protocolo que todo
//! endpoint "compatível" fala — mesmo que `resolveOpenAiCompatibleModel` no TS assume ao chamar
//! `createOpenAI({ baseURL, apiKey })(modelId)`.
//!
//! ## Duas perguntas independentes, mesma distinção de `modules/llm.rs`
//!
//! `privacy::assert_network_allowed` pergunta se o modo offline permite SAIR (todo provedor remoto);
//! a guarda de SSRF pergunta se o ENDEREÇO é confiável — só para endpoint DIGITADO PELO USUÁRIO
//! (`openaiCompatible`), que por isso vai por `privacy::assert_user_endpoint_allowed` (as duas, o
//! mesmo chokepoint do catálogo). Google e OpenRouter têm URL FIXA: só o gate de modo offline.
//!
//! ## Fora do escopo desta rodada (reportado, não aplicado)
//!
//! - **`thinking`/reasoning por provedor** (`enableThinking`/`thinkingMaxTokens` do schema `llm`,
//!   que o TS aplica via `additional_params` no OpenRouter e `think:false` no Ollama) não é
//!   aplicado aos 3 provedores remotos aqui — mesma classe de corte já documentada no cabeçalho de
//!   `turn.rs` ("Sem custo/roteamento/reflexão"). `temperature`/`maxTokens`/`maxToolSteps` (campos
//!   compartilhados do MESMO namespace `llm`) SÃO aplicados a todos os 4.

use crate::config::schemas::provider_endpoints as endpoints;
use crate::config::schemas::llm_defaults;
use rig_core::operation::Completion;
use rig_core::providers::gemini::GeminiConfig;
use rig_core::providers::ollama::OllamaConfig;
use rig_core::providers::openai::wire::OPENROUTER;
use rig_core::providers::openai::OpenAIConfig;
use rig_core::DynModel;

use crate::config::store::ConfigStore;
use crate::modules::privacy;

/// O modelo de chat/completion de qualquer um dos 4 provedores, com wire e transporte apagados.
/// `Debug` só imprime o nome do provedor e o id do modelo (nunca a configuração, que carrega a
/// chave), e `Clone` é barato (`Arc`).
pub type ChatModel = DynModel<Completion>;

/// URLs fixas do app, nunca digitadas pelo usuário — nomeadas em vez de embutidas na checagem, pelo
/// mesmo motivo documentado em `modules/llm.rs`: usadas só para o gate de modo offline, e só o HOST
/// importa ali (`privacy::is_loopback_url` olha só o host da URL).
const GOOGLE_CHAT_ENDPOINT: &str = endpoints::GOOGLE_AI_ORIGIN;
const OPENROUTER_CHAT_ENDPOINT: &str = endpoints::OPENROUTER_ORIGIN;

/// O `reqwest::Client` do app como transporte do `rig-core`. Clonar é barato (o pool é compartilhado).
fn transport() -> rig_reqwest::ReqwestClient {
    rig_reqwest::ReqwestClient::from(crate::modules::llm::http_client().clone())
}

/// **Teste-only, dupla trava — nunca uma chave de `ConfigStore`.** Google e OpenRouter têm URL FIXA
/// (ver o cabeçalho), então o E2E não consegue provar o fluxo desses provedores sem chamar — e
/// cobrar — a nuvem. Só no binário `e2e` e com `MIKA_E2E=1` E `MIKA_E2E_CLOUD_BASE_URL` (loopback)
/// as duas setadas, o turno vai para esse servidor local. No binário distribuído a função nem
/// existe: as variáveis no ambiente do usuário não redirecionam o prompt para lugar nenhum.
#[cfg(feature = "e2e")]
fn e2e_cloud_base() -> Option<String> {
    e2e_cloud_base_with(std::env::var("MIKA_E2E").ok().as_deref(), std::env::var("MIKA_E2E_CLOUD_BASE_URL").ok().as_deref())
}

#[cfg(not(feature = "e2e"))]
fn e2e_cloud_base() -> Option<String> {
    None
}

/// Núcleo puro de [`e2e_cloud_base`]: o env é lido pelo chamador (mutar `std::env` num teste vazaria
/// para os outros, que rodam no mesmo processo).
#[cfg(any(test, feature = "e2e"))]
fn e2e_cloud_base_with(mika_e2e: Option<&str>, base: Option<&str>) -> Option<String> {
    if mika_e2e != Some("1") {
        return None;
    }
    let base = base.map(str::trim).filter(|b| !b.is_empty() && privacy::is_loopback_url(b))?;
    Some(base.trim_end_matches('/').to_string())
}

/// Ollama pelo `/api/chat` NATIVO (e não pela rota OpenAI-compatível `/v1`): só ele aceita `think`,
/// `keep_alive` e `options.num_ctx`, que `turn::build_ollama_additional_params` envia. Sem chave
/// configurada o cabeçalho `Authorization` simplesmente não vai.
pub fn build_ollama_model(base_url: &str, api_key: &str, model: &str) -> ChatModel {
    OllamaConfig::new().with_base_url(base_url).with_api_key(api_key).connect(transport()).native_completion(model).erase()
}

/// Espelha `isLLMProviderId`/`DEFAULT_LLM_PROVIDER_ID` (`llm-catalog-data.ts`) — mesmo fallback de
/// `normalize_provider` em `modules/llm.rs` (privada àquele arquivo, fora do meu escopo; duplicada
/// aqui pelo mesmo precedente já citado no cabeçalho).
pub fn resolve_provider_id(store: &ConfigStore) -> String {
    match store.get("llm.provider").and_then(|v| v.as_str().map(str::to_string)) {
        Some(id) if id == "google" || id == "openrouter" || crate::modules::llm::openai_shaped(&id).is_some() => id,
        _ => "ollama".to_string(),
    }
}

pub struct RemoteLlmConfig {
    pub model: String,
    pub api_key: String,
    /// Só preenchido para os provedores no formato OpenAI (`openaiCompatible`, `unsloth`) — Google e
    /// OpenRouter usam a URL fixa do provedor.
    pub base_url: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
    pub max_tool_steps: u32,
}

/// Mesmos campos compartilhados de `temperature`/`maxTokens`/`maxToolSteps` que
/// `turn::resolve_ollama_config` já lê (namespace `llm` único, ver `config/schemas/llm.rs`) — só o
/// modelo/credencial/baseUrl variam por provedor.
pub fn resolve_remote_config(store: &ConfigStore, provider: &str) -> RemoteLlmConfig {
    let get_str = |path: &str| store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty());

    let shaped = crate::modules::llm::openai_shaped(provider);
    let (model_path, model_default, key_path) = match (provider, shaped) {
        ("google", _) => ("llm.googleModel", llm_defaults::GOOGLE_MODEL, "apiKeys.googleAI"),
        ("openrouter", _) => ("llm.openrouterModel", llm_defaults::OPENROUTER_MODEL, "apiKeys.openrouter"),
        (_, Some(shaped)) => (shaped.model_path, shaped.model_default, shaped.api_key_path),
        _ => ("llm.openaiCompatibleModel", llm_defaults::OPENAI_COMPATIBLE_MODEL, "apiKeys.openaiCompatibleApiKey"),
    };

    RemoteLlmConfig {
        model: get_str(model_path).unwrap_or_else(|| model_default.to_string()),
        api_key: get_str(key_path).unwrap_or_default(),
        base_url: shaped.map(|shaped| get_str(shaped.base_url_path).unwrap_or_else(|| shaped.base_url_default.to_string())),
        temperature: store.get("llm.temperature").and_then(|v| v.as_f64()),
        max_tokens: store.get("llm.maxTokens").and_then(|v| v.as_f64()).map(|n| n as u64),
        max_tool_steps: store
            .get("llm.maxToolSteps")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .filter(|&n| n > 0)
            .unwrap_or(15),
    }
}

/// Resolve credencial, passa pelo chokepoint certo (offline vs. SSRF — ver cabeçalho) e constrói o
/// modelo `rig-core` do provedor remoto pedido. Nunca chamado para `"ollama"` — esse caminho
/// continua em [`build_ollama_model`].
pub fn build_remote_model(store: &ConfigStore, provider: &str, config: &RemoteLlmConfig) -> Result<ChatModel, String> {
    match provider {
        "google" => {
            if config.api_key.is_empty() {
                return Err("google: chave de API não configurada".to_string());
            }
            privacy::assert_network_allowed(store, GOOGLE_CHAT_ENDPOINT, "brain:turn:google")?;
            let mut provider = GeminiConfig::new(config.api_key.as_str());
            if let Some(base) = e2e_cloud_base() {
                provider = provider.with_base_url(base);
            }
            Ok(provider.connect(transport()).completion(config.model.as_str()).erase())
        }
        "openrouter" => {
            if config.api_key.is_empty() {
                return Err("openrouter: chave de API não configurada".to_string());
            }
            privacy::assert_network_allowed(store, OPENROUTER_CHAT_ENDPOINT, "brain:turn:openrouter")?;
            let mut provider = OpenAIConfig::with_key(&OPENROUTER, config.api_key.as_str());
            if let Some(base) = e2e_cloud_base() {
                provider = provider.with_base_url(format!("{base}/api/v1"));
            }
            Ok(provider.connect(transport()).chat(config.model.as_str()).erase())
        }
        id => {
            // Qualquer id que não seja Google/OpenRouter chega aqui como `openaiCompatible` (o
            // resolver nunca devolve outro): o `unwrap_or` só cobre um id que não existe.
            let shaped = crate::modules::llm::openai_shaped(id).unwrap_or(&crate::modules::llm::OPENAI_SHAPED[0]);
            let base_url = config.base_url.as_deref().unwrap_or_default();
            if base_url.is_empty() {
                return Err(format!("{}: baseUrl não configurado", shaped.id));
            }
            if config.model.is_empty() {
                return Err(format!("{}: modelo não configurado — escolha o modelo carregado no servidor", shaped.id));
            }
            privacy::assert_user_endpoint_allowed(store, base_url, shaped.id)?;
            Ok(OpenAIConfig::new(config.api_key.as_str()).with_base_url(base_url).connect(transport()).chat(config.model.as_str()).erase())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-llm-provider-test-{}-{unique}", std::process::id()));
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

    /// `llm.provider` é `choice` no schema (`config/schemas/llm.rs::PROVIDER_IDS`) — o próprio
    /// `ConfigStore::set` já recusa um valor fora do enum, então "desconhecido" nunca chega a ser
    /// gravado. O fallback de [`resolve_provider_id`] cobre só a ausência real (config novo, sem
    /// `llm.provider` ainda gravado) — mesmo default do schema (`"ollama"`).
    #[test]
    fn resolve_provider_id_falls_back_to_ollama_when_absent() {
        let (store, _dir) = store();
        assert_eq!(resolve_provider_id(&store), "ollama");
    }

    #[test]
    fn resolve_provider_id_accepts_the_five_real_ids() {
        let (store, _dir) = store();
        for id in ["google", "openrouter", "ollama", "openaiCompatible", "unsloth"] {
            store.set("llm.provider", json!(id)).unwrap();
            assert_eq!(resolve_provider_id(&store), id);
        }
    }

    #[test]
    fn resolve_remote_config_uses_the_documented_defaults_per_provider() {
        let (store, _dir) = store();
        assert_eq!(resolve_remote_config(&store, "google").model, "gemini-3.8-flash");
        assert_eq!(resolve_remote_config(&store, "openrouter").model, "google/gemini-3.8-flash");
        assert_eq!(resolve_remote_config(&store, "openaiCompatible").model, "gpt-5.4-mini");
        assert_eq!(resolve_remote_config(&store, "openaiCompatible").base_url, Some(String::new()));
        assert_eq!(resolve_remote_config(&store, "google").base_url, None);
    }

    /// O Unsloth tem endereço padrão e chave/modelo PRÓPRIOS: a chave do `openaiCompatible` nunca
    /// serve a ele (vazaria uma credencial para outro servidor), nem a dele ao proxy.
    #[test]
    fn resolve_remote_config_for_unsloth_uses_its_own_url_key_and_model() {
        let (store, _dir) = store();
        store.set("apiKeys.openaiCompatibleApiKey", json!("chave-do-proxy")).unwrap();
        store.set("apiKeys.unslothApiKey", json!("sk-unsloth-teste")).unwrap();
        store.set("llm.unslothModel", json!("gemma-4-26B-A4B-it-GGUF")).unwrap();
        let config = resolve_remote_config(&store, "unsloth");
        assert_eq!(config.api_key, "sk-unsloth-teste");
        assert_eq!(config.model, "gemma-4-26B-A4B-it-GGUF");
        assert_eq!(config.base_url.as_deref(), Some("http://localhost:8888/v1"));
        assert_eq!(resolve_remote_config(&store, "openaiCompatible").api_key, "chave-do-proxy");
    }

    #[test]
    fn e2e_cloud_base_needs_both_locks_and_a_loopback_url() {
        assert_eq!(e2e_cloud_base_with(Some("1"), Some("http://127.0.0.1:4010/")), Some("http://127.0.0.1:4010".to_string()));
        assert_eq!(e2e_cloud_base_with(None, Some("http://127.0.0.1:4010")), None, "sem MIKA_E2E=1 nada redireciona");
        assert_eq!(e2e_cloud_base_with(Some("0"), Some("http://127.0.0.1:4010")), None);
        assert_eq!(e2e_cloud_base_with(Some("1"), None), None);
        assert_eq!(e2e_cloud_base_with(Some("1"), Some("https://evil.example.com")), None, "só loopback");
    }

    #[test]
    fn build_remote_model_unsloth_needs_a_model_and_accepts_the_loopback_default() {
        let (store, _dir) = store();
        let config = resolve_remote_config(&store, "unsloth");
        let error = build_remote_model(&store, "unsloth", &config).err().unwrap();
        assert!(error.contains("modelo"), "{error}");

        store.set("llm.unslothModel", json!("qwen3")).unwrap();
        let config = resolve_remote_config(&store, "unsloth");
        assert!(build_remote_model(&store, "unsloth", &config).is_ok());

        store.set("privacy.offlineMode", json!(true)).unwrap();
        assert!(build_remote_model(&store, "unsloth", &config).is_ok(), "loopback segue permitido offline");
    }

    /// Sem credencial: erro nomeado, nunca uma tentativa de rede — e a chave nunca é lida como
    /// texto vazio silenciosamente aceito.
    #[test]
    fn build_remote_model_without_credential_is_a_named_error_never_a_network_attempt() {
        let (store, _dir) = store();
        let config = resolve_remote_config(&store, "google");
        let error = build_remote_model(&store, "google", &config).unwrap_err();
        assert!(error.contains("chave de API"));
        assert!(!error.contains("http"), "erro sem credencial não deveria nem chegar a montar URL");
    }

    #[test]
    fn build_remote_model_openai_compatible_without_base_url_is_a_named_error() {
        let (store, _dir) = store();
        let config = resolve_remote_config(&store, "openaiCompatible");
        let error = build_remote_model(&store, "openaiCompatible", &config).unwrap_err();
        assert!(error.contains("baseUrl"));
    }

    /// Endpoint de rede interna é recusado pela guarda de SSRF antes de montar cliente, e a chave
    /// nunca aparece na mensagem de erro.
    #[test]
    fn build_remote_model_openai_compatible_blocks_private_endpoint_without_leaking_the_key() {
        let (store, _dir) = store();
        let config = RemoteLlmConfig {
            model: "gpt-4o-mini".to_string(),
            api_key: "sk-chave-secreta-de-teste".to_string(),
            base_url: Some("http://10.0.0.7:4000".to_string()),
            temperature: None,
            max_tokens: None,
            max_tool_steps: 15,
        };
        let error = build_remote_model(&store, "openaiCompatible", &config).unwrap_err();
        assert!(error.contains("SSRF"), "{error}");
        assert!(!error.contains("sk-chave-secreta-de-teste"));
    }

    /// Loopback (LiteLLM/vLLM/llama-server locais) e provedor remoto PÚBLICO montam cliente — o port
    /// loopback-only recusava o segundo, que o TS aceitava; modo offline continua bloqueando o remoto.
    #[test]
    fn build_remote_model_openai_compatible_accepts_loopback_and_public_endpoints() {
        let (store, _dir) = store();
        let config = |base_url: &str| RemoteLlmConfig {
            model: "m".to_string(),
            api_key: "k".to_string(),
            base_url: Some(base_url.to_string()),
            temperature: None,
            max_tokens: None,
            max_tool_steps: 15,
        };
        assert!(build_remote_model(&store, "openaiCompatible", &config("http://127.0.0.1:8089/v1")).is_ok());
        assert!(build_remote_model(&store, "openaiCompatible", &config("https://8.8.8.8/v1")).is_ok());

        store.set("privacy.offlineMode", json!(true)).unwrap();
        let error = build_remote_model(&store, "openaiCompatible", &config("https://8.8.8.8/v1")).err().unwrap();
        assert!(error.contains("offline"), "{error}");
        assert!(build_remote_model(&store, "openaiCompatible", &config("http://127.0.0.1:8089/v1")).is_ok());
    }

    /// Modo offline ligado bloqueia Google/OpenRouter (URL fixa, remota) ANTES de qualquer
    /// tentativa de rede — mesmo chokepoint que já cobre Ollama em `turn.rs`.
    #[tokio::test]
    async fn google_and_openrouter_refuse_the_call_under_offline_mode_before_touching_the_network() {
        let (store, _dir) = store();
        store.set("privacy.offlineMode", json!(true)).unwrap();
        store.set("apiKeys.googleAI", json!("chave-de-teste")).unwrap();
        store.set("apiKeys.openrouter", json!("chave-de-teste")).unwrap();

        let google_config = resolve_remote_config(&store, "google");
        let google_error = build_remote_model(&store, "google", &google_config).unwrap_err();
        assert!(google_error.contains("privacy.offlineMode"));

        let openrouter_config = resolve_remote_config(&store, "openrouter");
        let openrouter_error = build_remote_model(&store, "openrouter", &openrouter_config).unwrap_err();
        assert!(openrouter_error.contains("privacy.offlineMode"));
    }
}
