//! Módulo `llm` em Rust — port PARCIAL do backend TypeScript correspondente.
//!
//! Só catálogo de modelos, perfil de modelo, saúde de provedor e quota (6 canais): `mika:llm-models`,
//! `mika:elevenlabs-voices`, `llm:getProviderHealth`, `llm:resetProviderCircuit`,
//! `llm:getOpenRouterQuota`, `llm:getModelProfile`. Chat/streaming (`mika:chat-send` e companhia) FICA no sidecar Node
//! nesta fase — é outra frente. Ver `docs/tauri-rust-migration.md`.
//!
//! `rig-core` não entra aqui: listar modelos é GET simples por provedor, não o laço de
//! completions que `rig` resolve (ver `providers::*` do crate). Uso o `reqwest::Client` do app —
//! o mesmo que `brain::llm_provider` entrega ao `rig-core` como transporte.
//!
//! RISCO ABERTO (ver relatório da rodada): enquanto o chat continuar no sidecar, é ELE quem
//! observa falha real de provedor e mantém o circuit breaker (`packages/llm/src/llm-circuit-
//! state.ts`). Reivindicar `llm:getProviderHealth`/`llm:resetProviderCircuit` aqui tira esses 2
//! canais do sidecar PARA SEMPRE (`native::NativeRegistry` não tem fallback parcial) — o painel de
//! saúde do provedor fica cego ao estado real até o chat também migrar para Rust.
//!
//! `feature.rs`/`feature_fields.rs` são um registro DIFERENTE: publicam `LLM_SETTINGS` (a aba de
//! settings da tela, não os 5 canais IPC deste arquivo) no `feature_registry::shared()`. Ver o
//! cabeçalho de `feature.rs` para por que sob `mika.llm` e não `mika.brain`.

mod feature;
mod feature_fields;
pub(crate) mod model_profile;
mod profile_route;
pub(crate) mod provider_health;
pub(crate) mod pull;
pub(crate) mod status;
pub(crate) mod turn_metrics;

use crate::config::schemas::provider_endpoints as endpoints;
use crate::config::schemas::llm_defaults;
use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::modules::privacy;
use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

pub use feature::register_feature;

const CH_LLM_MODELS: &str = "mika:llm-models";
const CH_ELEVENLABS_VOICES: &str = "mika:elevenlabs-voices";
const CH_PROVIDER_HEALTH: &str = "llm:getProviderHealth";
const CH_RESET_CIRCUIT: &str = "llm:resetProviderCircuit";
const CH_OPENROUTER_QUOTA: &str = "llm:getOpenRouterQuota";
const CH_MODEL_PROFILE: &str = "llm:getModelProfile";
const CH_STATUS: &str = "llm:getStatus";
const CH_PULL_MODEL: &str = "llm:pullModel";
const CH_CANCEL_PULL: &str = "llm:cancelPull";
const CHANNELS: &[&str] = &[
    CH_LLM_MODELS,
    CH_ELEVENLABS_VOICES,
    CH_PROVIDER_HEALTH,
    CH_RESET_CIRCUIT,
    CH_OPENROUTER_QUOTA,
    CH_MODEL_PROFILE,
    CH_STATUS,
    CH_PULL_MODEL,
    CH_CANCEL_PULL,
];

// Caminhos de `ConfigKey` em `packages/sdk/src/contracts/schemas/config-keys.enum.ts`, copiados
// como literal: o enum é TypeScript, não há como importar o símbolo daqui.
const CFG_GOOGLE_AI: &str = "apiKeys.googleAI";
const CFG_OPENROUTER: &str = "apiKeys.openrouter";
const CFG_OLLAMA_BASE_URL: &str = "apiKeys.ollamaBaseUrl";
const CFG_OLLAMA_API_KEY: &str = "apiKeys.ollamaApiKey";
const CFG_OPENAI_COMPAT_BASE_URL: &str = "apiKeys.openaiCompatibleBaseUrl";
const CFG_OPENAI_COMPAT_API_KEY: &str = "apiKeys.openaiCompatibleApiKey";

const CFG_UNSLOTH_BASE_URL: &str = "apiKeys.unslothBaseUrl";
const CFG_UNSLOTH_API_KEY: &str = "apiKeys.unslothApiKey";

const OLLAMA_DEFAULT_BASE_URL: &str = llm_defaults::OLLAMA_BASE_URL;
const CATALOG_TIMEOUT: Duration = Duration::from_secs(15);
const QUOTA_TIMEOUT: Duration = Duration::from_secs(8);

/*
 * Os endereços externos, nomeados em vez de embutidos na chamada.
 *
 * Não é arrumação: a URL agora é usada DUAS vezes — uma no gate de modo offline, outra na
 * requisição. Se as duas fossem literais soltos, dava para gatear um endereço e chamar outro, e o
 * gate passaria a aprovar uma requisição que não é a que sai. O par endereço↔decisão já divergiu
 * neste repo antes, e some sem erro nenhum.
 */
const GOOGLE_MODELS_URL: &str = endpoints::GOOGLE_AI_MODELS_URL;
const OPENROUTER_MODELS_URL: &str = endpoints::OPENROUTER_ZDR_MODELS_URL;
const OPENROUTER_QUOTA_URL: &str = endpoints::OPENROUTER_KEY_URL;
const ELEVENLABS_VOICES_URL: &str = endpoints::ELEVENLABS_VOICES_URL;

static HTTP: OnceLock<reqwest::Client> = OnceLock::new();

/// O cliente HTTP do processo para LLM e embeddings (catálogo, turno de chat via `rig`, embedding,
/// aquecimento do Ollama): um pool só, então a conexão (e o handshake TLS de um provedor remoto)
/// sobrevive de um turno para o outro. Um `reqwest::Client` novo por turno — o default do `rig` —
/// pagava TCP + TLS de novo a cada mensagem. Cada chamador continua passando pelo próprio
/// `assert_network_allowed` antes de despachar.
pub(crate) fn http_client() -> &'static reqwest::Client {
    HTTP.get_or_init(|| {
        // Sem timeout, um provedor que aceita a conexão e cala prende o turno para sempre.
        // `read_timeout` é o intervalo ENTRE chunks (não o total): o streaming do chat segue
        // vivo enquanto chegar token; cada chamada de catálogo ainda põe o próprio `.timeout()`.
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(120))
            .build()
            .expect("cliente http de llm")
    })
}

/// Guard do catálogo de modelos Ollama — não do chat, que já usa só `assert_network_allowed`
/// (`turn::run_turn_llm` → `resolve_ollama_config`, ver `brain/turn.rs`).
///
/// `url` é o endpoint que a chamada realmente vai usar (já resolvido: explícito do payload OU o
/// salvo); `stored_base_url` é sempre o que está no `ConfigStore`, nunca o payload. Duas perguntas,
/// nunca misturadas:
/// - `url` É o endereço salvo (`same_endpoint`) → mesmo trust boundary do chat: quem confia nisso
///   é o próprio usuário que salvou (Ollama Cloud incluso), então só o gate de modo offline decide.
/// - `url` é DIFERENTE do salvo → veio do payload da tela de configuração sondando um endpoint
///   ainda não salvo: passa pela guarda de SSRF (`privacy::assert_user_endpoint_allowed`) — e a key
///   salva nunca acompanha essa sondagem (`key_for_endpoint` já corta isso antes daqui).
fn assert_ollama_catalog_endpoint_allowed(store: &ConfigStore, url: &str, stored_base_url: &str) -> Result<(), String> {
    if same_endpoint(url, stored_base_url) {
        privacy::assert_network_allowed(store, url, "llm:ollama-catalog")
    } else {
        privacy::assert_user_endpoint_allowed(store, url, "ollama")
    }
}

/// Provedor que fala o protocolo OpenAI contra uma base URL digitada pelo usuário: o `openaiCompatible`
/// e o `unsloth`. Um resolver só, duas identidades — `unsloth` é preset (URL padrão, chave e modelo
/// próprios), não protocolo novo (ver `specs/02-providers.md`). Os caminhos de config são cópia
/// literal de `config-keys.enum.ts`, como as demais constantes `CFG_*`.
pub(crate) struct OpenAiShaped {
    pub id: &'static str,
    /// Nome nas mensagens de erro e no log.
    pub label: &'static str,
    pub base_url_path: &'static str,
    pub api_key_path: &'static str,
    pub model_path: &'static str,
    pub model_default: &'static str,
    /// Preenche `baseUrl` quando nada foi salvo — vazio = o usuário precisa informar.
    pub base_url_default: &'static str,
}

pub(crate) const OPENAI_SHAPED: [OpenAiShaped; 2] = [
    OpenAiShaped {
        id: "openaiCompatible",
        label: "OpenAI-compatible",
        base_url_path: CFG_OPENAI_COMPAT_BASE_URL,
        api_key_path: CFG_OPENAI_COMPAT_API_KEY,
        model_path: "llm.openaiCompatibleModel",
        model_default: llm_defaults::OPENAI_COMPATIBLE_MODEL,
        base_url_default: "",
    },
    OpenAiShaped {
        id: "unsloth",
        label: "Unsloth",
        base_url_path: CFG_UNSLOTH_BASE_URL,
        api_key_path: CFG_UNSLOTH_API_KEY,
        model_path: "llm.unslothModel",
        model_default: "",
        base_url_default: UNSLOTH_DEFAULT_BASE_URL,
    },
];

pub(crate) const UNSLOTH_DEFAULT_BASE_URL: &str = "http://localhost:8888/v1";

/// `None` para qualquer id que não seja um dos provedores no formato OpenAI.
pub(crate) fn openai_shaped(provider: &str) -> Option<&'static OpenAiShaped> {
    OPENAI_SHAPED.iter().find(|shaped| shaped.id == provider)
}

/// Espelha `isLLMProviderId`/`DEFAULT_LLM_PROVIDER_ID` de `llm-catalog-data.ts`. Só 5 provedores
/// existem hoje; qualquer string desconhecida (ou ausente) cai no default real do app: `ollama`.
fn normalize_provider(raw: &str) -> String {
    match raw {
        "google" | "openrouter" | "ollama" | "openaiCompatible" | "unsloth" => raw.to_string(),
        _ => "ollama".to_string(),
    }
}

/// Chave explícita > credencial salva > variável de ambiente — mesma ordem de `loadLLMModels`.
fn resolve_key(explicit: Option<&str>, store: &ConfigStore, config_path: &str, env_var: &str) -> String {
    if let Some(key) = explicit.filter(|k| !k.is_empty()) {
        return key.to_string();
    }
    if let Some(stored) = store
        .get(config_path)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
    {
        return stored;
    }
    std::env::var(env_var).unwrap_or_default()
}

fn strip_trailing_slash_and_v1(v: &str) -> String {
    let no_slash = v.trim().trim_end_matches('/');
    no_slash.strip_suffix("/v1").unwrap_or(no_slash).to_string()
}

fn same_endpoint(a: &str, b: &str) -> bool {
    strip_trailing_slash_and_v1(&a.to_ascii_lowercase()) == strip_trailing_slash_and_v1(&b.to_ascii_lowercase())
}

/// Mesma regra de `keyForEndpoint` em `llm-catalog.ts`: a credencial salva só é reaproveitada
/// quando o `baseUrl` pedido é o MESMO já configurado — outro endpoint não herda a chave.
fn key_for_endpoint(explicit: Option<&str>, requested_base_url: Option<&str>, stored_base_url: &str, stored_key: &str) -> String {
    if let Some(key) = explicit.filter(|k| !k.is_empty()) {
        return key.to_string();
    }
    if let Some(requested) = requested_base_url {
        if !same_endpoint(requested, stored_base_url) {
            return String::new();
        }
    }
    stored_key.to_string()
}

/// Preço pode vir número ou string da API (ex. OpenRouter); sempre normalizado para string, igual
/// ao `.toString()` do TS.
fn price_str(pricing: &Value, key: &str) -> Option<String> {
    match pricing.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn sort_by_name(models: &mut [Value]) {
    models.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .cmp(b["name"].as_str().unwrap_or(""))
    });
}

// --- Catálogo por provedor -----------------------------------------------------------------

async fn fetch_google_models(store: &ConfigStore, explicit_key: Option<&str>, kind: &str) -> Result<Vec<Value>, String> {
    let api_key = resolve_key(explicit_key, store, CFG_GOOGLE_AI, "GOOGLE_API_KEY");
    if api_key.is_empty() {
        // Catálogo estático (fallback sem credencial) não foi portado — duplicar a tabela de preços
        // curada de `llm-catalog-data.ts` aqui divergiria da fonte real a cada atualização de preço.
        return Err("Google AI: chave de API não configurada".into());
    }

    privacy::assert_network_allowed(store, GOOGLE_MODELS_URL, "llm:google")?;

    let response = http_client()
        .get(GOOGLE_MODELS_URL)
        .header("x-goog-api-key", &api_key)
        .timeout(CATALOG_TIMEOUT)
        .send()
        .await
        .map_err(|_| "Google AI unreachable".to_string())?;

    if !response.status().is_success() {
        return Err(format!("Google AI models failed ({})", response.status().as_u16()));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|_| "Google AI: resposta inválida".to_string())?;

    let required_method = if kind == "embedding" { "embedContent" } else { "generateContent" };

    let mut models: Vec<Value> = body
        .get("models")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|m| {
            m.get("supportedGenerationMethods")
                .and_then(Value::as_array)
                .is_some_and(|methods| methods.iter().any(|method| method.as_str() == Some(required_method)))
        })
        .map(|m| {
            let raw_name = m.get("name").and_then(Value::as_str).unwrap_or_default();
            let id = raw_name.strip_prefix("models/").unwrap_or(raw_name).to_string();
            let name = m
                .get("displayName")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| id.clone());
            json!({
                "id": id,
                "name": name,
                "provider": "google",
                "source": "remote",
                "vendor": "google",
                "description": m.get("description"),
                "contextWindow": m.get("inputTokenLimit"),
                "maxCompletionTokens": m.get("outputTokenLimit"),
            })
        })
        .collect();

    sort_by_name(&mut models);
    Ok(models)
}

async fn fetch_openrouter_models(store: &ConfigStore, explicit_key: Option<&str>, kind: &str) -> Result<Vec<Value>, String> {
    let api_key = resolve_key(explicit_key, store, CFG_OPENROUTER, "OPENROUTER_API_KEY");
    if api_key.is_empty() {
        return Err("OpenRouter: chave de API não configurada".into());
    }

    // zdr=true: sem o filtro, o catálogo oferece modelo sem endpoint ZDR e a primeira mensagem cai
    // em 404 — o par deste filtro é o `provider: {zdr, data_collection}` do corpo do chat (`brain/provider_params.rs`).
    privacy::assert_network_allowed(store, OPENROUTER_MODELS_URL, "llm:openrouter")?;

    let response = http_client()
        .get(OPENROUTER_MODELS_URL)
        .bearer_auth(&api_key)
        .timeout(CATALOG_TIMEOUT)
        .send()
        .await
        .map_err(|_| "OpenRouter unreachable".to_string())?;

    if !response.status().is_success() {
        return Err(format!("OpenRouter models failed ({})", response.status().as_u16()));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|_| "OpenRouter: resposta inválida".to_string())?;

    let mut models: Vec<Value> = body
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|m| {
            if kind != "embedding" {
                return true;
            }
            m.get("architecture")
                .and_then(|a| a.get("output_modalities"))
                .and_then(Value::as_array)
                .is_some_and(|mods| mods.iter().any(|x| x.as_str() == Some("embedding")))
        })
        .map(|m| {
            let id = m.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
            let name = m
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| id.clone());
            let input_modalities: Vec<Value> = m
                .get("architecture")
                .and_then(|a| a.get("input_modalities"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let output_modalities: Vec<Value> = m
                .get("architecture")
                .and_then(|a| a.get("output_modalities"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let pricing = m.get("pricing").cloned().unwrap_or(Value::Null);
            let prompt_price = price_str(&pricing, "prompt");
            let completion_price = price_str(&pricing, "completion");
            let supports_tools = m
                .get("supported_parameters")
                .and_then(Value::as_array)
                .is_some_and(|params| params.iter().any(|p| p.as_str().is_some_and(|s| s.contains("tool"))));
            let supports_vision = input_modalities.iter().any(|v| v.as_str() == Some("image") || v.as_str() == Some("file"));
            let is_free = prompt_price.as_deref().unwrap_or("0").parse::<f64>().unwrap_or(0.0) == 0.0
                && completion_price.as_deref().unwrap_or("0").parse::<f64>().unwrap_or(0.0) == 0.0;

            json!({
                "id": id,
                "name": name,
                "provider": "openrouter",
                "source": "remote",
                "vendor": id.split('/').next().unwrap_or_default(),
                "description": m.get("description"),
                "contextWindow": m.get("context_length").or_else(|| m.get("top_provider").and_then(|t| t.get("context_length"))),
                "promptPrice": prompt_price,
                "completionPrice": completion_price,
                "inputCacheReadPrice": price_str(&pricing, "input_cache_read"),
                "supportsTools": supports_tools,
                "supportsVision": supports_vision,
                "isFree": is_free,
                "knowledgeCutoff": m.get("knowledge_cutoff"),
                "created": m.get("created"),
                "ended": m.get("ended"),
                "maxCompletionTokens": m.get("top_provider").and_then(|t| t.get("max_completion_tokens")),
                "inputModalities": input_modalities,
                "outputModalities": output_modalities,
            })
        })
        .collect();

    sort_by_name(&mut models);
    super::token_usage::remember_openrouter_prices(&models);
    Ok(models)
}

/// Só enche o cache de preços de `token_usage` — o chamador não quer a lista, quer o custo do
/// próximo turno. Falha (sem chave, offline) só loga: o custo segue desconhecido, que é a verdade.
pub(crate) async fn refresh_openrouter_prices(store: &ConfigStore) {
    if let Err(error) = fetch_openrouter_models(store, None, "chat").await {
        log::debug!("preços do OpenRouter não atualizados: {error}");
    }
}

/// Família/nome de modelo que só gera vetor, para Ollama antigo que não devolve `capabilities` em
/// `/api/show`. A lista de famílias vem dos modelos de embedding da biblioteca oficial do Ollama.
pub(crate) fn looks_like_embedding(name: &str, family: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let family = family.to_ascii_lowercase();
    name.contains("embed")
        || name.contains("minilm")
        || name.starts_with("bge-")
        || matches!(family.as_str(), "bert" | "nomic-bert" | "xlm-roberta" | "jina-bert" | "jina-bert-v2")
}

/// Ollama lista chat e embedding no mesmo `/api/tags`. `capabilities` (`/api/show`) decide quando
/// existe; sem ela, a heurística de nome/família. Antes o seletor de embedding oferecia `llama3.2`
/// (não gera vetor) e o de chat oferecia `nomic-embed-text` (não conversa).
fn ollama_model_matches_kind(capabilities: &[&str], name: &str, family: &str, kind: &str) -> bool {
    let is_embedding = if capabilities.is_empty() {
        looks_like_embedding(name, family)
    } else {
        capabilities.contains(&"embedding") && !capabilities.contains(&"completion")
    };
    (kind == "embedding") == is_embedding
}

/// `/api/show` por modelo é uma requisição a mais por item a cada abertura da tela; o `digest` de
/// `/api/tags` muda quando o modelo muda, então `url|nome|digest` é chave suficiente.
static OLLAMA_SHOW_CACHE: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, Value>>> =
    std::sync::LazyLock::new(Default::default);

async fn cached_ollama_capabilities(url: &str, api_key: &str, name: &str, digest: &str) -> Value {
    let key = format!("{url}|{name}|{digest}");
    if let Some(hit) = OLLAMA_SHOW_CACHE.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let fresh = fetch_ollama_capabilities(url, api_key, name).await;
    if fresh.get("capabilities").and_then(Value::as_array).is_some_and(|c| !c.is_empty()) {
        if let Ok(mut cache) = OLLAMA_SHOW_CACHE.lock() {
            cache.insert(key, fresh.clone());
        }
    }
    fresh
}

/// `/api/show` só é consultado depois de `/api/tags` — mesma ordem de `fetchOllamaModels`.
async fn fetch_ollama_models(store: &ConfigStore, explicit_key: Option<&str>, explicit_base_url: Option<&str>, kind: &str) -> Result<Vec<Value>, String> {
    let stored_base_url = store
        .get(CFG_OLLAMA_BASE_URL)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| OLLAMA_DEFAULT_BASE_URL.to_string());
    let requested_base_url = explicit_base_url.unwrap_or(&stored_base_url);
    let url = strip_trailing_slash_and_v1(requested_base_url);
    assert_ollama_catalog_endpoint_allowed(store, &url, &stored_base_url)?;

    let stored_key = store
        .get(CFG_OLLAMA_API_KEY)
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let api_key = key_for_endpoint(explicit_key, explicit_base_url, &stored_base_url, &stored_key);

    let mut request = http_client().get(format!("{url}/api/tags")).timeout(CATALOG_TIMEOUT);
    if !api_key.is_empty() {
        request = request.bearer_auth(&api_key);
    }
    let response = request.send().await.map_err(|_| "Ollama unreachable".to_string())?;
    if !response.status().is_success() {
        return Err(format!("Ollama models failed ({})", response.status().as_u16()));
    }

    let body: Value = response.json().await.map_err(|_| "Ollama: resposta inválida".to_string())?;
    let raw_models: Vec<Value> = body.get("models").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut models = Vec::with_capacity(raw_models.len());
    for m in raw_models {
        let Some(name) = m.get("name").and_then(Value::as_str) else { continue };
        let family = m.get("details").and_then(|d| d.get("family")).and_then(Value::as_str).unwrap_or("");
        let digest = m.get("digest").and_then(Value::as_str).unwrap_or("");

        let capabilities = cached_ollama_capabilities(&url, &api_key, name, digest).await;
        let caps: Vec<&str> = capabilities
            .get("capabilities")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if !ollama_model_matches_kind(&caps, name, family, kind) {
            continue;
        }
        let vendor = if family.is_empty() { "local" } else { family };
        let mut item = json!({
            "id": name, "name": name, "provider": "ollama", "source": "remote", "vendor": vendor,
        });
        if let Some(obj) = item.as_object_mut() {
            for field in ["supportsTools", "supportsVision", "contextWindow"] {
                if let Some(value) = capabilities.get(field) {
                    obj.insert(field.to_string(), value.clone());
                }
            }
        }
        models.push(item);
    }

    sort_by_name(&mut models);
    Ok(models)
}

/// As capabilities (`tools`, `vision`) e o `contextWindow` só existem em `/api/show`; `/api/tags`
/// NUNCA os traz. Já foi defeito real aqui — o app concluía, olhando só `/api/tags`, que o modelo
/// local não suportava ferramentas.
async fn fetch_ollama_capabilities(url: &str, api_key: &str, model_name: &str) -> Value {
    let mut request = http_client()
        .post(format!("{url}/api/show"))
        .json(&json!({ "model": model_name }))
        .timeout(CATALOG_TIMEOUT);
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }

    let Ok(response) = request.send().await else {
        return json!({});
    };
    if !response.status().is_success() {
        return json!({});
    }
    let Ok(body) = response.json::<Value>().await else {
        return json!({});
    };

    let capabilities: Vec<&str> = body
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    // A janela de contexto vem sob uma chave cujo prefixo varia por arquitetura (`qwen3.context_length`,
    // `llama.context_length`, ...) — mesma pegadinha de `contextLengthFrom` no TS.
    let context_window = body
        .get("model_info")
        .and_then(Value::as_object)
        .and_then(|info| info.iter().find(|(k, v)| k.ends_with(".context_length") && v.as_u64().is_some_and(|n| n > 0)))
        .and_then(|(_, v)| v.as_u64());

    json!({
        "capabilities": capabilities,
        "supportsTools": capabilities.contains(&"tools"),
        "supportsVision": capabilities.contains(&"vision"),
        "contextWindow": context_window,
    })
}

async fn fetch_openai_compatible_models(store: &ConfigStore, shaped: &OpenAiShaped, explicit_key: Option<&str>, explicit_base_url: Option<&str>) -> Result<Vec<Value>, String> {
    let stored_base_url = store
        .get(shaped.base_url_path)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| shaped.base_url_default.to_string());
    let base_url = explicit_base_url.unwrap_or(&stored_base_url);
    if base_url.is_empty() {
        return Err(format!("{}: baseUrl não configurado", shaped.label));
    }
    let url = base_url.trim().trim_end_matches('/').to_string();
    privacy::assert_user_endpoint_allowed(store, &url, shaped.id)?;

    let stored_key = store
        .get(shaped.api_key_path)
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let api_key = key_for_endpoint(explicit_key, explicit_base_url, &stored_base_url, &stored_key);

    let mut request = http_client().get(format!("{url}/models")).timeout(CATALOG_TIMEOUT);
    if !api_key.is_empty() {
        request = request.bearer_auth(&api_key);
    }
    let response = request.send().await.map_err(|_| format!("{} unreachable", shaped.label))?;
    if !response.status().is_success() {
        return Err(format!("{} models failed ({})", shaped.label, response.status().as_u16()));
    }

    let body: Value = response.json().await.map_err(|_| format!("{}: resposta inválida", shaped.label))?;

    let mut models: Vec<Value> = body
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(Value::as_str)?.to_string();
            let vendor = m.get("owned_by").and_then(Value::as_str).unwrap_or(shaped.id).to_string();
            Some(json!({
                "id": id.clone(), "name": id, "provider": shaped.id, "source": "remote",
                "vendor": vendor, "created": m.get("created"),
            }))
        })
        .collect();

    sort_by_name(&mut models);
    Ok(models)
}

async fn list_llm_models(store: &ConfigStore, payload: &Value) -> Value {
    let provider = payload
        .get("provider")
        .and_then(Value::as_str)
        .map(normalize_provider)
        .unwrap_or_else(|| "ollama".to_string());
    let kind = payload.get("kind").and_then(Value::as_str).unwrap_or("chat");
    let explicit_key = payload.get("apiKey").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let explicit_base_url = payload.get("baseUrl").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());

    let result = match provider.as_str() {
        "google" => fetch_google_models(store, explicit_key, kind).await,
        id if openai_shaped(id).is_some() => {
            fetch_openai_compatible_models(store, openai_shaped(id).expect("checado no guard"), explicit_key, explicit_base_url).await
        }
        "ollama" => fetch_ollama_models(store, explicit_key, explicit_base_url, kind).await,
        _ => fetch_openrouter_models(store, explicit_key, kind).await,
    };

    match result {
        Ok(models) => json!({ "models": models }),
        Err(error) => {
            log::warn!("llm model catalog fetch failed: provider={provider} error={error}");
            json!({ "models": [], "error": error })
        }
    }
}

// --- ElevenLabs -------------------------------------------------------------------------------

const PLAN_RESTRICTED_CATEGORY: &str = "professional";
const ALWAYS_INCLUDED_CATEGORIES: [&str; 2] = ["premade", "cloned"];

/// Espelha `classifyElevenLabsVoice` em `elevenlabs-voice-availability.ts`.
fn classify_elevenlabs_voice(voice: &Value) -> (bool, &'static str, &'static str) {
    let is_library_voice = voice.get("sharing").is_some_and(|v| !v.is_null());
    let category = voice.get("category").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();

    if is_library_voice || category == PLAN_RESTRICTED_CATEGORY {
        return (is_library_voice, "plan-restricted", "requer plano pago");
    }
    if ALWAYS_INCLUDED_CATEGORIES.contains(&category.as_str()) {
        return (is_library_voice, "included", "incluída no seu plano");
    }
    (is_library_voice, "unknown", "disponibilidade não confirmada")
}

/// Espelha `annotateElevenLabsVoices`: anota E ordena (included < unknown < plan-restricted), pois
/// o seletor de voz do renderer depende dessa ordem — devolver a lista crua sem isso reordenaria a
/// UI em silêncio.
fn annotate_elevenlabs_voices(voices: Vec<Value>) -> Vec<Value> {
    fn rank(availability: &str) -> u8 {
        match availability {
            "included" => 0,
            "unknown" => 1,
            _ => 2,
        }
    }

    let mut annotated: Vec<Value> = voices
        .into_iter()
        .map(|mut voice| {
            let (is_library_voice, availability, label) = classify_elevenlabs_voice(&voice);
            if let Some(obj) = voice.as_object_mut() {
                obj.insert("isLibraryVoice".to_string(), json!(is_library_voice));
                obj.insert("availability".to_string(), json!(availability));
                obj.insert("availabilityLabel".to_string(), json!(label));
            }
            voice
        })
        .collect();

    annotated.sort_by_key(|v| rank(v.get("availability").and_then(Value::as_str).unwrap_or("unknown")));
    annotated
}

async fn list_elevenlabs_voices(store: &ConfigStore, payload: &Value) -> Value {
    let Some(api_key) = payload.get("apiKey").and_then(Value::as_str).filter(|k| !k.is_empty()) else {
        return json!({ "ok": false, "error": "apiKey ausente" });
    };

    if let Err(error) = privacy::assert_network_allowed(store, ELEVENLABS_VOICES_URL, "voice:elevenlabs") {
        return json!({ "ok": false, "error": error });
    }

    let response = match http_client()
        .get(ELEVENLABS_VOICES_URL)
        .header("xi-api-key", api_key)
        .timeout(CATALOG_TIMEOUT)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return json!({ "ok": false, "error": "ElevenLabs unreachable" }),
    };

    if !response.status().is_success() {
        return json!({ "ok": false, "error": format!("HTTP {}", response.status().as_u16()) });
    }

    match response.json::<Value>().await {
        Ok(body) => {
            let raw_voices: Vec<Value> = body.get("voices").and_then(Value::as_array).cloned().unwrap_or_default();
            json!({ "ok": true, "voices": annotate_elevenlabs_voices(raw_voices) })
        }
        Err(_) => json!({ "ok": false, "error": "ElevenLabs: resposta inválida" }),
    }
}

// --- Saúde, circuito e quota --------------------------------------------------------------------

/// Registro REAL, atualizado a cada turno de chat de verdade (`chat::send::handle_chat_send` via
/// `provider_health::record_success`/`record_failure`) — ver o cabeçalho de `provider_health.rs`
/// para o porquê deste módulo deixou de devolver `[]` sempre.
fn get_provider_health() -> Value {
    json!({ "health": provider_health::snapshot() })
}

/// Reseta a entrada de verdade agora que existe uma (ver `provider_health.rs`). `ok: false`
/// continua honesto quando não havia nada para resetar — nunca fingir um reset que não aconteceu
/// (classe de defeito já vista neste repositório: "resposta que significa não fiz").
fn reset_provider_circuit(payload: &Value) -> Value {
    let provider = payload.get("provider").and_then(Value::as_str).unwrap_or("");
    let model = payload.get("model").and_then(Value::as_str).unwrap_or("");
    let ok = provider_health::reset(provider, model).is_some();
    json!({ "ok": ok })
}

async fn get_openrouter_quota(store: &ConfigStore) -> Value {
    let api_key = store
        .get(CFG_OPENROUTER)
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return json!({ "ok": false, "error": "OpenRouter API key is missing. Please configure it in Settings." });
    }

    if let Err(error) = privacy::assert_network_allowed(store, OPENROUTER_QUOTA_URL, "llm:openrouter") {
        return json!({ "ok": false, "error": error });
    }

    let response = match http_client()
        .get(OPENROUTER_QUOTA_URL)
        .bearer_auth(trimmed)
        .timeout(QUOTA_TIMEOUT)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return json!({ "ok": false, "error": "OpenRouter unreachable" }),
    };

    if !response.status().is_success() {
        let status = response.status().as_u16();
        return json!({ "ok": false, "error": format!("OpenRouter respondeu {status} ao consultar a quota da chave.") });
    }

    let body: Value = match response.json().await {
        Ok(b) => b,
        Err(_) => return json!({ "ok": false, "error": "OpenRouter: resposta inválida" }),
    };

    let data = body.get("data").cloned().unwrap_or_else(|| json!({}));
    let limit = data.get("limit").and_then(Value::as_f64);
    let limit_remaining = data.get("limit_remaining").and_then(Value::as_f64);
    let exhausted = matches!((limit, limit_remaining), (Some(_), Some(remaining)) if remaining <= 0.0);

    let quota = json!({
        "label": data.get("label").and_then(Value::as_str).unwrap_or(""),
        "limit": limit,
        "limitRemaining": limit_remaining,
        "limitReset": data.get("limit_reset"),
        "usage": data.get("usage").and_then(Value::as_f64).unwrap_or(0.0),
        "usageDaily": data.get("usage_daily").and_then(Value::as_f64).unwrap_or(0.0),
        "usageMonthly": data.get("usage_monthly").and_then(Value::as_f64).unwrap_or(0.0),
        "isFreeTier": data.get("is_free_tier").and_then(Value::as_bool).unwrap_or(false),
        "exhausted": exhausted,
    });

    json!({ "ok": true, "quota": quota })
}

// --- Download de modelo Ollama -------------------------------------------------------------------

/// Uma linha por fatia seria centenas de eventos por segundo; a tela só precisa de ~5 por segundo
/// e de toda troca de fase (`pulling manifest` → `pulling <digest>` → `verifying` → `success`).
const PULL_EVENT_INTERVAL: Duration = Duration::from_millis(200);

fn emit_pull_event(app: &AppHandle, payload: &Value) {
    use tauri::{Emitter, Manager};
    for label in app.webview_windows().keys() {
        if is_owner_surface(crate::protocol::WindowType::from_label(label)) {
            let _ = app.emit_to(label.as_str(), pull::PROGRESS_EVENT, payload.clone());
        }
    }
}

async fn pull_model_with_progress(app: &AppHandle, store: &ConfigStore, payload: &Value) -> Value {
    let model = payload.get("model").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let endpoint = crate::modules::desktop::resolve_ollama_endpoint(store);
    let mut last: Option<(std::time::Instant, String)> = None;
    let on_progress = |p: &pull::Progress| {
        let due = last.as_ref().is_none_or(|(at, status)| at.elapsed() >= PULL_EVENT_INTERVAL || *status != p.status);
        if due {
            last = Some((std::time::Instant::now(), p.status.clone()));
            emit_pull_event(app, &json!({ "model": model, "status": p.status, "completed": p.completed, "total": p.total, "done": false }));
        }
    };
    let result = pull::pull_model(store, &endpoint.base_url, endpoint.api_key.as_deref(), &model, "llm:ollama-pull", on_progress).await;
    let error = result.as_ref().err().cloned();
    emit_pull_event(app, &json!({ "model": model, "done": true, "ok": result.is_ok(), "error": error }));
    if result.is_ok() {
        // Modelo novo instalado: a lista de "instalados" da tela tem de mudar sem o usuário clicar.
        status::get_status(app, store).await;
    } else {
        log::warn!("download de modelo Ollama falhou: model={model} error={}", error.clone().unwrap_or_default());
    }
    json!({ "ok": result.is_ok(), "error": error })
}

// --- Módulo -------------------------------------------------------------------------------------

pub struct LlmModule;

#[async_trait]
impl NativeModule for LlmModule {
    fn name(&self) -> &'static str {
        "llm"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(&self, app: &AppHandle, ctx: &InvokeContext, channel: &str, payload: Value) -> Result<Value, String> {
        // Mesma guarda de `assertOwnerSurface`: os 5 canais deste módulo exigem janela dona
        // (main/settings/wizard) do lado TS (`voice/ipc.ts` e `ipc/system-metrics.ts`).
        if !is_owner_surface(ctx.window_type) {
            return Err(format!("{channel} recusado para {:?}", ctx.window_type));
        }

        // `ConfigStore` é UM SÓ por processo (`state::config_store`), não um `OnceLock` próprio
        // deste módulo: duas instâncias são dois caches do mesmo `config.json`, e a segunda a
        // salvar apaga a escrita da primeira sem erro. `single_config_store_per_process` é o gate.
        let store = crate::state::config_store(app)?;
        status::arm(app);

        match channel {
            CH_LLM_MODELS => Ok(list_llm_models(&store, &payload).await),
            CH_ELEVENLABS_VOICES => Ok(list_elevenlabs_voices(&store, &payload).await),
            CH_PROVIDER_HEALTH => Ok(get_provider_health()),
            CH_RESET_CIRCUIT => Ok(reset_provider_circuit(&payload)),
            CH_OPENROUTER_QUOTA => Ok(get_openrouter_quota(&store).await),
            CH_MODEL_PROFILE => {
                let (provider, model) = profile_route::parse_request(&payload)?;
                Ok(profile_route::to_response(&model, &model_profile::resolve(&store, &provider, &model).await))
            }
            CH_STATUS => Ok(status::get_status(app, &store).await),
            CH_PULL_MODEL => Ok(pull_model_with_progress(app, &store, &payload).await),
            CH_CANCEL_PULL => Ok(json!({ "ok": pull::cancel() })),
            other => Err(format!("llm: canal '{other}' declarado mas não tratado")),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(LlmModule)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WindowType;

    /// Toda saída externa deste módulo passa pelo gate de modo offline.
    ///
    /// Lê o próprio fonte porque a falha é de OMISSÃO: quem acrescentar o próximo provedor escreve
    /// um `.get("https://...")` e pronto — nada quebra, nenhum teste fica vermelho, e o toggle de
    /// modo offline simplesmente não vale para aquele provedor. Foi exatamente assim que Google,
    /// OpenRouter e ElevenLabs saíram para a internet com o modo offline ligado.
    ///
    /// Endpoint configurável vai por `privacy::assert_user_endpoint_allowed` (SSRF + offline); URL
    /// fixa só precisa do gate de modo offline.
    #[test]
    fn every_external_url_goes_through_the_offline_gate() {
        let source = include_str!("llm.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();

        for url_const in [
            "GOOGLE_MODELS_URL",
            "OPENROUTER_MODELS_URL",
            "OPENROUTER_QUOTA_URL",
            "ELEVENLABS_VOICES_URL",
        ] {
            assert!(
                production.contains(&format!("assert_network_allowed(store, {url_const}")),
                "{url_const} é usada sem passar por privacy::assert_network_allowed"
            );
        }

        // Piso de alcance: se os `const` sumirem, o laço acima não tem o que conferir.
        assert!(production.contains("const GOOGLE_MODELS_URL"), "leitura do fonte quebrou");

        // Nenhuma URL externa pode ser literal na chamada: literal é o que escapa do laço acima.
        // `https://` dentro de comentário ou doc não conta — só o que está numa chamada HTTP.
        for line in production.lines() {
            let code = line.split("//").next().unwrap_or_default();
            assert!(
                !(code.contains(".get(\"https://") || code.contains(".post(\"https://")),
                "URL externa literal na chamada: {}. Declare um `const` e gateie-o.",
                line.trim()
            );
        }
    }

    fn ctx(window_type: WindowType) -> InvokeContext {
        InvokeContext {
            window_type,
            window_label: "test".into(),
            plugin_id: None,
        }
    }

    #[test]
    fn channels_match_the_ts_contract() {
        assert_eq!(
            CHANNELS,
            &[
                "mika:llm-models",
                "mika:elevenlabs-voices",
                "llm:getProviderHealth",
                "llm:resetProviderCircuit",
                "llm:getOpenRouterQuota",
                "llm:getModelProfile",
                "llm:getStatus",
                "llm:pullModel",
                "llm:cancelPull",
            ]
        );
    }

    #[test]
    fn owner_surface_matches_ts_policy() {
        assert!(is_owner_surface(ctx(WindowType::Main).window_type));
        assert!(is_owner_surface(ctx(WindowType::Settings).window_type));
        assert!(is_owner_surface(ctx(WindowType::Wizard).window_type));
        assert!(!is_owner_surface(ctx(WindowType::PluginView).window_type));
        assert!(!is_owner_surface(ctx(WindowType::Unknown).window_type));
    }

    /// Registro `static` de processo: nomes de provider/model exclusivos deste teste evitam cruzar
    /// com o que `provider_health::tests` deixa para trás (mesmo cuidado que aquele arquivo já
    /// documenta) e provam a fiação real — não mais o `[]` fabricado que este teste checava antes
    /// de `get_provider_health` passar a refletir `provider_health::snapshot()`.
    #[test]
    fn provider_health_reflects_a_real_recorded_failure_not_a_fabricated_empty_list() {
        let provider = "test-llm-mod-provider-health";
        let model = "test-model";
        provider_health::record_failure(provider, model, "erro de teste", Some(500));

        let health = get_provider_health()["health"].as_array().cloned().unwrap_or_default();
        assert!(health.iter().any(|v| v["provider"] == provider && v["model"] == model && v["state"] == "degraded"));

        provider_health::reset(provider, model);
    }

    /// `ok: true` aqui seria a classe de defeito "resposta que significa não fiz" se nada tivesse
    /// sido resetado de verdade — agora que `provider_health` guarda estado real, o positivo
    /// (entrada existia e foi limpa) e o negativo (nada para resetar) continuam ambos honestos.
    #[test]
    fn reset_provider_circuit_is_honest_about_doing_nothing_and_resets_a_real_entry() {
        let provider = "test-llm-mod-reset-provider";
        let model = "test-model";

        let nothing_to_reset = reset_provider_circuit(&json!({ "provider": provider, "model": model }));
        assert_eq!(nothing_to_reset, json!({ "ok": false }));

        provider_health::record_failure(provider, model, "erro", None);
        let reset_response = reset_provider_circuit(&json!({ "provider": provider, "model": model }));
        assert_eq!(reset_response, json!({ "ok": true }));
    }

    #[test]
    fn reset_provider_circuit_response_never_echoes_a_credential() {
        let response = reset_provider_circuit(&json!({ "provider": "openrouter", "apiKey": "sk-secreto-de-teste" }));
        assert!(!response.to_string().contains("sk-secreto-de-teste"));
    }

    #[test]
    fn normalize_provider_falls_back_to_ollama() {
        assert_eq!(normalize_provider("google"), "google");
        assert_eq!(normalize_provider("openrouter"), "openrouter");
        assert_eq!(normalize_provider("ollama"), "ollama");
        assert_eq!(normalize_provider("openaiCompatible"), "openaiCompatible");
        assert_eq!(normalize_provider("unsloth"), "unsloth");
        assert_eq!(normalize_provider("anthropic-direto-que-nao-existe"), "ollama");
        assert_eq!(normalize_provider(""), "ollama");
    }

    /// Defeito real: o card de embedding do Ollama listava `llama3.2` (não gera vetor) e o seletor
    /// de chat listava `nomic-embed-text` (não conversa) — `/api/tags` mistura os dois.
    #[test]
    fn ollama_catalog_splits_chat_and_embedding_models_by_capability() {
        let chat_caps = ["completion", "tools"];
        let embed_caps = ["embedding"];
        assert!(ollama_model_matches_kind(&embed_caps, "nomic-embed-text", "nomic-bert", "embedding"));
        assert!(!ollama_model_matches_kind(&chat_caps, "llama3.2", "llama", "embedding"));
        assert!(ollama_model_matches_kind(&chat_caps, "llama3.2", "llama", "chat"));
        assert!(!ollama_model_matches_kind(&embed_caps, "nomic-embed-text", "nomic-bert", "chat"));
    }

    /// Ollama antigo não devolve `capabilities`: nome/família decidem.
    #[test]
    fn ollama_catalog_without_capabilities_falls_back_to_name_and_family() {
        for (name, family) in [("mxbai-embed-large", "bert"), ("all-minilm:l6-v2", "bert"), ("bge-m3", "xlm-roberta"), ("custom", "nomic-bert")] {
            assert!(ollama_model_matches_kind(&[], name, family, "embedding"), "{name}");
            assert!(!ollama_model_matches_kind(&[], name, family, "chat"), "{name}");
        }
        assert!(ollama_model_matches_kind(&[], "qwen3:8b", "qwen3", "chat"));
        assert!(!ollama_model_matches_kind(&[], "qwen3:8b", "qwen3", "embedding"));
    }

    #[test]
    fn same_endpoint_ignores_case_trailing_slash_and_v1_suffix() {
        assert!(same_endpoint("http://localhost:11434/", "http://LOCALHOST:11434"));
        assert!(same_endpoint("https://api.openai.com/v1", "https://api.openai.com"));
        assert!(!same_endpoint("http://localhost:11434", "http://outro-host:11434"));
    }

    /// A credencial salva NUNCA acompanha um `baseUrl` diferente do configurado — senão um valor
    /// digitado no wizard vazaria a chave salva para um endpoint arbitrário.
    /// O endereço já salvo (inclusive Ollama Cloud, não-loopback) passa pelo mesmo gate do chat —
    /// modo offline desligado libera, ligado bloqueia.
    #[test]
    fn stored_ollama_cloud_endpoint_uses_the_offline_gate_like_chat_does() {
        let (store, _dir) = store();
        assert!(assert_ollama_catalog_endpoint_allowed(&store, "https://ollama.com", "https://ollama.com").is_ok());

        store.set("privacy.offlineMode", json!(true)).unwrap();
        let err = assert_ollama_catalog_endpoint_allowed(&store, "https://ollama.com", "https://ollama.com").unwrap_err();
        assert!(err.contains("offline"), "esperava erro de modo offline, veio: {err}");
    }

    /// Endpoint DIFERENTE do salvo (sondagem da tela de configuração antes de salvar) passa pela
    /// guarda de SSRF e depois pelo modo offline — IP literal para o teste não depender de DNS.
    #[test]
    fn probed_endpoint_different_from_stored_goes_through_ssrf_then_offline() {
        let (store, _dir) = store();
        assert!(assert_ollama_catalog_endpoint_allowed(&store, "http://localhost:11434", "https://ollama.com").is_ok());
        assert!(assert_ollama_catalog_endpoint_allowed(&store, "http://8.8.8.8:11434", "https://ollama.com").is_ok());

        let err = assert_ollama_catalog_endpoint_allowed(&store, "http://192.168.1.50:11434", "https://ollama.com").unwrap_err();
        assert!(err.contains("SSRF"), "esperava erro de SSRF, veio: {err}");

        store.set("privacy.offlineMode", json!(true)).unwrap();
        let err = assert_ollama_catalog_endpoint_allowed(&store, "http://8.8.8.8:11434", "https://ollama.com").unwrap_err();
        assert!(err.contains("offline"), "esperava erro de modo offline, veio: {err}");
    }

    /// `unsloth` e `openaiCompatible` dividem o resolver, mas NUNCA a credencial nem o endereço: a
    /// chave `sk-unsloth-…` não pode ir para um proxy qualquer, nem a chave do proxy para o Unsloth.
    #[test]
    fn openai_shaped_providers_share_the_resolver_but_never_credentials_or_address() {
        let unsloth = openai_shaped("unsloth").expect("unsloth é OpenAI-shaped");
        let compatible = openai_shaped("openaiCompatible").expect("openaiCompatible é OpenAI-shaped");
        assert_eq!(unsloth.api_key_path, "apiKeys.unslothApiKey");
        assert_eq!(unsloth.base_url_path, "apiKeys.unslothBaseUrl");
        assert_eq!(unsloth.base_url_default, "http://localhost:8888/v1");
        assert_ne!(unsloth.api_key_path, compatible.api_key_path);
        assert_ne!(unsloth.base_url_path, compatible.base_url_path);
        assert_ne!(unsloth.model_path, compatible.model_path);
        for other in ["google", "openrouter", "ollama", ""] {
            assert!(openai_shaped(other).is_none(), "{other}");
        }
    }

    #[test]
    fn key_for_endpoint_does_not_leak_the_stored_key_to_a_different_host() {
        let leaked = key_for_endpoint(None, Some("http://host-diferente:11434"), "http://localhost:11434", "chave-secreta-armazenada");
        assert_eq!(leaked, "");

        let reused = key_for_endpoint(None, Some("http://localhost:11434"), "http://localhost:11434", "chave-secreta-armazenada");
        assert_eq!(reused, "chave-secreta-armazenada");

        let explicit = key_for_endpoint(Some("chave-explicita"), Some("http://qualquer-host"), "http://localhost:11434", "chave-secreta-armazenada");
        assert_eq!(explicit, "chave-explicita");
    }

    #[test]
    fn price_str_normalizes_number_and_string_pricing() {
        assert_eq!(price_str(&json!({ "prompt": 0.15 }), "prompt"), Some("0.15".to_string()));
        assert_eq!(price_str(&json!({ "prompt": "0.15" }), "prompt"), Some("0.15".to_string()));
        assert_eq!(price_str(&json!({}), "prompt"), None);
    }

    #[test]
    fn classify_marks_library_voices_as_plan_restricted_even_with_included_category() {
        let (is_library, availability, _) = classify_elevenlabs_voice(&json!({ "category": "premade", "sharing": { "status": "enabled" } }));
        assert!(is_library);
        assert_eq!(availability, "plan-restricted");
    }

    #[test]
    fn classify_included_and_unknown_categories() {
        assert_eq!(classify_elevenlabs_voice(&json!({ "category": "premade" })).1, "included");
        assert_eq!(classify_elevenlabs_voice(&json!({ "category": "cloned" })).1, "included");
        assert_eq!(classify_elevenlabs_voice(&json!({ "category": "professional" })).1, "plan-restricted");
        assert_eq!(classify_elevenlabs_voice(&json!({})).1, "unknown");
    }

    /// A ordem (included < unknown < plan-restricted) é o que o seletor de voz do renderer
    /// depende — devolver fora de ordem reordena a UI em silêncio.
    #[test]
    fn annotate_sorts_included_before_unknown_before_plan_restricted() {
        let voices = vec![
            json!({ "voice_id": "a", "category": "professional" }),
            json!({ "voice_id": "b", "category": "premade" }),
            json!({ "voice_id": "c" }),
        ];
        let annotated = annotate_elevenlabs_voices(voices);
        let ids: Vec<&str> = annotated.iter().map(|v| v["voice_id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-llm-module-test-{}-{unique}", std::process::id()));
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

    /// Sem credencial configurada, os 4 provedores remotos falham de forma legível — nunca
    /// devolvem um catálogo inventado (o fallback estático do TS não foi portado, de propósito;
    /// ver o comentário no topo do arquivo).
    #[tokio::test]
    async fn models_without_credential_return_an_honest_error_not_a_fake_catalog() {
        let (store, _dir) = store();

        for provider in ["google", "openrouter"] {
            let response = list_llm_models(&store, &json!({ "provider": provider })).await;
            assert_eq!(response["models"], json!([]));
            assert!(response["error"].as_str().is_some(), "provider={provider} sem error");
        }
    }

    /// `openaiCompatible` sem `baseUrl` nenhum (nem no payload, nem salvo) é o mesmo caso: erro
    /// legível, catálogo vazio.
    #[tokio::test]
    async fn openai_compatible_without_base_url_returns_an_honest_error() {
        let (store, _dir) = store();
        let response = list_llm_models(&store, &json!({ "provider": "openaiCompatible" })).await;
        assert_eq!(response["models"], json!([]));
        assert!(response["error"].as_str().unwrap().contains("baseUrl"));
    }

    /// A chave nunca aparece na mensagem de erro — nem quando a requisição falha por endpoint
    /// bloqueado (SSRF) para um `baseUrl` remoto explícito.
    #[tokio::test]
    async fn blocked_remote_base_url_error_never_echoes_the_api_key() {
        let (store, _dir) = store();
        let response = list_llm_models(
            &store,
            &json!({ "provider": "ollama", "baseUrl": "http://exemplo-remoto.invalid:11434", "apiKey": "chave-secreta-de-teste" }),
        )
        .await;
        assert_eq!(response["models"], json!([]));
        assert!(!response.to_string().contains("chave-secreta-de-teste"));
    }
}
