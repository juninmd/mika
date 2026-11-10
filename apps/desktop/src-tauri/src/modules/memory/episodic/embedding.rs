//! Geração e invalidação de embedding — porte de `episodic-embedding.ts`.
//!
//! **Os quatro provedores têm cliente real** (`generate_embedding` — ver
//! `memory::mod::EMBEDDING_SETTINGS`/`embedding_settings_reader`, o mesmo `OnceLock` cacheado no
//! boot que `EPISODIC_LIST_LIMIT`/`RECORDING_ALLOWED` já usam, porque esta função não tem
//! `ConfigStore` no próprio ponto de chamada): `ollama` (`{ollamaBaseUrl}/api/embeddings`), `google`
//! (`generativelanguage.googleapis.com/v1beta/models/{model}:embedContent`, key por query string,
//! mesma forma que `packages/llm/src/llm-embedding-model.ts` usava antes da migração), `openrouter`
//! (`openrouter.ai/api/v1/embeddings`, forma OpenAI) e `openaiCompatible` (`{baseUrl}/embeddings`,
//! mesma forma, `baseUrl` configurável pelo usuário). Nunca lança, nunca inventa vetor: falha de
//! rede ou provedor sem credencial degrada para `None` (busca por palavra-chave). A busca semântica
//! (`search_tier`, `find_duplicate_memory`, `invalidate_*`) continua a mesma: só precisa de um vetor
//! de entrada já calculado.
//!
//! **Dois caminhos, um cache.** O turno de chat (`persona::compose_system_prompt`) já é `async` e
//! usa [`generate_embedding_async`]: o fetch é `await`ado no runtime de quem chama, em paralelo com
//! o resto da montagem do prompt. Os chamadores síncronos (`write.rs`, pós-turno) usam
//! [`generate_embedding`], que despacha o MESMO fetch num runtime de ponte do processo e espera o
//! resultado — `Handle::current().block_on(..)` na thread de um worker panicaria ("Cannot start a
//! runtime from within a runtime"). Antes, cada chamada síncrona subia uma `std::thread` e um runtime
//! novos, e o turno de chat bloqueava o worker async nessa ponte antes de chamar o LLM. Os dois
//! caminhos consultam [`QUERY_CACHE`]: o mesmo texto no mesmo modelo não volta ao provedor.
//!
//! **Rede passa pelo guarda de privacidade**: `assert_network_allowed`-equivalente inline (mesma
//! regra — `is_loopback_url` sempre permite, senão respeita `privacy.offlineMode` cacheado em
//! `EmbeddingSettings::offline_mode`) roda ANTES de qualquer requisição, igual ao resto do processo
//! (`createLoggedFetch`/`assert_network_allowed`, ver `privacy/mod.rs`).

use crate::config::schemas::provider_endpoints as endpoints;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use super::store::load_tier;
use super::search::cosine_similarity;
use super::types::{index_key, key, EMBEDDING_META_KEY, EMBEDDING_SCAN_LIMIT};
use super::types::MemoryTier;
use crate::modules::memory::{EmbeddingSettings, MemoryStore};
use crate::modules::privacy::is_loopback_url;

const EMBEDDING_TIMEOUT: Duration = Duration::from_secs(20);

/// URL fixa do endpoint de embedding do Google — a key vai por query string (mesma forma que
/// `@ai-sdk/google` usava antes da migração), nunca por header.
const GOOGLE_EMBED_API_BASE: &str = endpoints::GOOGLE_AI_V1BETA;
const GOOGLE_DEFAULT_EMBED_MODEL: &str = "gemini-embedding-001";
/// URL fixa do endpoint de embedding do OpenRouter — forma OpenAI (`/embeddings`), key por Bearer.
const OPENROUTER_EMBED_BASE: &str = endpoints::OPENROUTER_API_V1;
const OPENROUTER_DEFAULT_EMBED_MODEL: &str = "google/gemini-embedding-001";
const OPENAI_COMPATIBLE_DEFAULT_EMBED_MODEL: &str = "text-embedding-3-small";

/// Porta estreita de transporte — o mesmo padrão de `AchievementsFetcher`
/// (`gamer/steam_achievements`): produção chama o Ollama de verdade, teste injeta um dublê e prova
/// que o gate de rede/offline roda ANTES de qualquer chamada, sem precisar de um servidor de
/// verdade no ar.
#[async_trait]
pub(crate) trait EmbeddingFetcher: Send + Sync {
    async fn embed(&self, base_url: &str, api_key: &str, model: &str, content: &str) -> Result<Vec<f64>, String>;
}

struct OllamaEmbeddingFetcher;

#[async_trait]
impl EmbeddingFetcher for OllamaEmbeddingFetcher {
    async fn embed(&self, base_url: &str, api_key: &str, model: &str, content: &str) -> Result<Vec<f64>, String> {
        let mut request = crate::modules::llm::http_client()
            .post(format!("{base_url}/api/embeddings"))
            .timeout(EMBEDDING_TIMEOUT)
            .json(&json!({ "model": model, "prompt": content }));
        if !api_key.is_empty() {
            request = request.bearer_auth(api_key);
        }
        let response = request.send().await.map_err(|e| format!("Ollama embeddings unreachable: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("Ollama embeddings failed ({})", response.status().as_u16()));
        }
        let body: serde_json::Value = response.json().await.map_err(|_| "Ollama embeddings: resposta inválida".to_string())?;
        body.get("embedding")
            .and_then(serde_json::Value::as_array)
            .map(|arr| arr.iter().filter_map(serde_json::Value::as_f64).collect())
            .ok_or_else(|| "Ollama embeddings: resposta sem 'embedding'".to_string())
    }
}

/// Forma OpenAI (`POST {base_url}/embeddings`, `{"model","input"}`, resposta `data[0].embedding`) —
/// compartilhada por OpenRouter e OpenAI-compatible, que só diferem na URL base e no default de
/// modelo (ver [`OpenRouterEmbeddingFetcher`]/[`OpenAiCompatibleEmbeddingFetcher`]).
async fn fetch_openai_shaped_embedding(base_url: &str, api_key: &str, model: &str, content: &str, (provider, provider_tag): (&str, &str)) -> Result<Vec<f64>, String> {
    let mut request = crate::modules::llm::http_client()
        .post(format!("{base_url}/embeddings"))
        .timeout(EMBEDDING_TIMEOUT)
        .json(&json!({ "model": model, "input": content }));
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }
    let response = request.send().await.map_err(|_| format!("{provider_tag} embeddings unreachable"))?;
    if !response.status().is_success() {
        return Err(format!("{provider_tag} embeddings failed ({})", response.status().as_u16()));
    }
    let body: serde_json::Value = response.json().await.map_err(|_| format!("{provider_tag} embeddings: resposta inválida"))?;
    if let Some(tokens) = body.pointer("/usage/prompt_tokens").and_then(serde_json::Value::as_u64) {
        crate::modules::token_usage::record_embedding(provider, model, tokens);
    }
    body.get("data")
        .and_then(serde_json::Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("embedding"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(serde_json::Value::as_f64).collect())
        .ok_or_else(|| format!("{provider_tag} embeddings: resposta sem 'data[0].embedding'"))
}

/// In-process e5 (`inference::embedder`): no server, no key, works offline. `Err` until the model is
/// downloaded, which degrades to keyword search like any other provider failure.
struct LocalEmbeddingFetcher;

#[async_trait]
impl EmbeddingFetcher for LocalEmbeddingFetcher {
    async fn embed(&self, _base_url: &str, _api_key: &str, _model: &str, content: &str) -> Result<Vec<f64>, String> {
        // ONNX síncrono: roda no pool de bloqueio, nunca no worker async do turno.
        let content = content.to_string();
        tokio::task::spawn_blocking(move || crate::modules::inference::embedder::embed(&content))
            .await
            .map_err(|e| format!("embedding local não completou: {e}"))?
            .map(|v| v.into_iter().map(f64::from).collect())
    }
}

/// `taskType`/`outputDimensionality` are optional in `embedContent`; empty/0 means the API default.
fn google_embed_body(content: &str, task_type: &str, dimensions: i64) -> serde_json::Value {
    let mut body = json!({ "content": { "parts": [{ "text": content }] } });
    if !task_type.is_empty() {
        body["taskType"] = json!(task_type);
    }
    if dimensions > 0 {
        body["outputDimensionality"] = json!(dimensions);
    }
    body
}

struct GoogleEmbeddingFetcher {
    task_type: String,
    dimensions: i64,
}

#[async_trait]
impl EmbeddingFetcher for GoogleEmbeddingFetcher {
    /// `base_url` é ignorado de propósito — Google não tem endpoint configurável para embedding, só
    /// a key (por query string, nunca por header — mesma forma que o cliente anterior usava).
    async fn embed(&self, _base_url: &str, api_key: &str, model: &str, content: &str) -> Result<Vec<f64>, String> {
        if api_key.is_empty() {
            return Err("Google AI: API key ausente".to_string());
        }
        let model = if model.is_empty() { GOOGLE_DEFAULT_EMBED_MODEL } else { model };
        let response = crate::modules::llm::http_client()
            .post(format!("{GOOGLE_EMBED_API_BASE}/models/{model}:embedContent?key={api_key}"))
            .timeout(EMBEDDING_TIMEOUT)
            .json(&google_embed_body(content, &self.task_type, self.dimensions))
            .send()
            .await
            .map_err(|_| "Google embeddings unreachable".to_string())?;
        if !response.status().is_success() {
            return Err(format!("Google embeddings failed ({})", response.status().as_u16()));
        }
        let body: serde_json::Value = response.json().await.map_err(|_| "Google embeddings: resposta inválida".to_string())?;
        body.get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(serde_json::Value::as_array)
            .map(|arr| arr.iter().filter_map(serde_json::Value::as_f64).collect())
            .ok_or_else(|| "Google embeddings: resposta sem 'embedding.values'".to_string())
    }
}

struct OpenRouterEmbeddingFetcher;

#[async_trait]
impl EmbeddingFetcher for OpenRouterEmbeddingFetcher {
    /// `base_url` é ignorado de propósito — OpenRouter é sempre `OPENROUTER_EMBED_BASE`, nunca um
    /// endereço vindo de config (mesma forma que `openrouterEmbeddingFetch` fixava antes).
    async fn embed(&self, _base_url: &str, api_key: &str, model: &str, content: &str) -> Result<Vec<f64>, String> {
        if api_key.is_empty() {
            return Err("OpenRouter: API key ausente".to_string());
        }
        let model = if model.is_empty() { OPENROUTER_DEFAULT_EMBED_MODEL } else { model };
        fetch_openai_shaped_embedding(OPENROUTER_EMBED_BASE, api_key, model, content, ("openrouter", "OpenRouter")).await
    }
}

struct OpenAiCompatibleEmbeddingFetcher;

#[async_trait]
impl EmbeddingFetcher for OpenAiCompatibleEmbeddingFetcher {
    async fn embed(&self, base_url: &str, api_key: &str, model: &str, content: &str) -> Result<Vec<f64>, String> {
        if base_url.is_empty() {
            return Err("OpenAI-compatible: baseUrl não configurado".to_string());
        }
        let model = if model.is_empty() { OPENAI_COMPATIBLE_DEFAULT_EMBED_MODEL } else { model };
        fetch_openai_shaped_embedding(base_url.trim_end_matches('/'), api_key, model, content, ("openaiCompatible", "OpenAI-compatible")).await
    }
}

/// Unsloth (`POST /v1/embeddings`, `unsloth.ai/docs/basics/api`): mesmo formato OpenAI, mas SEM
/// modelo padrão — o servidor serve o embedding GGUF carregado, e um nome inventado aqui (como o
/// `text-embedding-3-small` do `openaiCompatible`) não existe nele.
struct UnslothEmbeddingFetcher;

#[async_trait]
impl EmbeddingFetcher for UnslothEmbeddingFetcher {
    async fn embed(&self, base_url: &str, api_key: &str, model: &str, content: &str) -> Result<Vec<f64>, String> {
        if base_url.is_empty() {
            return Err("Unsloth: baseUrl não configurado".to_string());
        }
        if model.is_empty() {
            return Err("Unsloth: modelo de embedding não configurado".to_string());
        }
        fetch_openai_shaped_embedding(base_url.trim_end_matches('/'), api_key, model, content, ("unsloth", "Unsloth")).await
    }
}

/// Runtime de ponte do processo (um worker): onde o caminho síncrono despacha o fetch. Persistente,
/// então a conexão que ele abriu no pool compartilhado continua viva para o próximo pedido.
fn bridge_runtime() -> Option<&'static tokio::runtime::Runtime> {
    static RUNTIME: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| tokio::runtime::Builder::new_multi_thread().worker_threads(1).thread_name("mika-embedding").enable_all().build().ok())
        .as_ref()
}

/// Ponte síncrona -> assíncrona — ver o cabeçalho do módulo. Bloqueia só quem chama.
fn fetch_embedding_blocking(fetcher: Arc<dyn EmbeddingFetcher>, base_url: String, api_key: String, model: String, content: String) -> Option<Vec<f64>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    bridge_runtime()?.spawn(async move {
        let _ = tx.send(fetcher.embed(&base_url, &api_key, &model, &content).await.ok());
    });
    rx.recv().ok().flatten()
}

/// Últimos vetores por (provedor, modelo, endpoint, parâmetros, texto) — LRU pequeno: o que se
/// repete de verdade é a mensagem do turno (reenvio, failover que refaz o turno, gravação pós-turno
/// do mesmo texto), não um histórico longo.
pub(crate) struct EmbeddingCache(Mutex<VecDeque<(String, Vec<f64>)>>, usize);

const EMBEDDING_CACHE_CAP: usize = 32;

impl EmbeddingCache {
    pub(crate) const fn new() -> Self {
        Self::with_capacity(EMBEDDING_CACHE_CAP)
    }

    const fn with_capacity(cap: usize) -> Self {
        Self(Mutex::new(VecDeque::new()), cap)
    }

    fn get(&self, key: &str) -> Option<Vec<f64>> {
        let mut entries = self.0.lock().ok()?;
        let position = entries.iter().position(|(k, _)| k == key)?;
        let entry = entries.remove(position)?;
        let vector = entry.1.clone();
        entries.push_back(entry);
        Some(vector)
    }

    fn clear(&self) {
        if let Ok(mut entries) = self.0.lock() {
            entries.clear();
        }
    }

    fn put(&self, key: String, vector: Vec<f64>) {
        let Ok(mut entries) = self.0.lock() else { return };
        if entries.len() >= self.1 {
            entries.pop_front();
        }
        entries.push_back((key, vector));
    }
}

static QUERY_CACHE: EmbeddingCache = EmbeddingCache::new();
/// The user's long-term facts, embedded in the background ([`warm_fact_vectors`]) so the chat turn
/// can rank them by meaning without waiting on the provider (`brain::persona::facts_block`).
static FACT_CACHE: EmbeddingCache = EmbeddingCache::with_capacity(256);

/// Esvazia os dois caches em RAM (consulta e fatos).
pub(crate) fn clear_caches() {
    QUERY_CACHE.clear();
    FACT_CACHE.clear();
}

#[cfg(test)]
pub(crate) fn cache_put_for_test(key: &str, vector: Vec<f64>) {
    QUERY_CACHE.put(key.to_string(), vector);
}

#[cfg(test)]
pub(crate) fn cache_has_for_test(key: &str) -> bool {
    QUERY_CACHE.get(key).is_some()
}

/// Cache-only: the vector already fetched for `content`, never a network call.
fn cached_vector(content: &str, cache: &EmbeddingCache) -> Option<Vec<f64>> {
    let settings = crate::modules::memory::embedding_settings_reader()?;
    let (_, base_url, _, _) = resolve_embedding_target(&settings)?;
    cache.get(&cache_key(&settings, &base_url, content))
}

/// Similarity of a fact to `message` from vectors already in hand: the message's (episodic recall
/// fetched it this turn) and the fact's ([`warm_fact_vectors`]). `None` when either is missing.
pub(crate) fn fact_similarity_to(message: &str) -> impl Fn(&str) -> Option<f64> {
    let query = cached_vector(message, &QUERY_CACHE);
    move |fact| Some(cosine_similarity(query.as_ref()?, &cached_vector(fact, &FACT_CACHE)?))
}

/// Embeds the facts not cached yet, in the background and one pass at a time — the next turns
/// rank by them. Same provider, cache key and offline gate as every other embedding here.
pub(crate) fn warm_fact_vectors(texts: Vec<String>) {
    static WARMING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let missing: Vec<String> = texts.into_iter().filter(|text| cached_vector(text, &FACT_CACHE).is_none()).collect();
    let (Some(store), Some(settings)) = (crate::modules::memory::memory_store_reader(), crate::modules::memory::embedding_settings_reader()) else { return };
    if missing.is_empty() || WARMING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        for text in missing {
            generate_embedding_via_async(resolve_embedding_target, &settings, &text, store, &FACT_CACHE).await;
        }
        WARMING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// Chamado por `settings::plugin_test::test_embedding_connection` (botão "Validar" da aba
/// `mika.embeddings`) — mesmo transporte que [`resolve_embedding_target`] usa em produção, mas
/// chamado direto num contexto JÁ assíncrono (o dispatcher de plugin-test é `async fn`), sem a
/// ponte de thread que o caminho síncrono de `write.rs`/`service.rs` precisa. Erro sobe honesto para
/// a UI mostrar, em vez de degradar para `None` como o caminho de produção faz. `endpoint_and_key`
/// (`plugin_test.rs`) já resolve `base_url`/`api_key` pelo par correto do provider ANTES de chamar
/// esta função — aqui só resta escolher o fetcher.
pub(crate) async fn test_embedding(provider: &str, base_url: &str, api_key: &str, model: &str, content: &str) -> Result<Vec<f64>, String> {
    let fetcher: Arc<dyn EmbeddingFetcher> = match provider {
        "ollama" => Arc::new(OllamaEmbeddingFetcher),
        "local" => Arc::new(LocalEmbeddingFetcher),
        "google" => Arc::new(GoogleEmbeddingFetcher { task_type: String::new(), dimensions: 0 }),
        "openrouter" => Arc::new(OpenRouterEmbeddingFetcher),
        "openaiCompatible" => Arc::new(OpenAiCompatibleEmbeddingFetcher),
        "unsloth" => Arc::new(UnslothEmbeddingFetcher),
        other => return Err(format!("Provedor de embeddings \"{other}\" não reconhecido.")),
    };
    fetcher.embed(base_url, api_key, model, content).await
}

/// Um alvo de embedding já resolvido: o fetcher a chamar, o `base_url`/`api_key` que ele recebe, e o
/// endereço usado SÓ para o gate de modo offline (`guard_url` — para Google/OpenRouter é a URL fixa
/// do provedor, nunca loopback; para Ollama/OpenAI-compatible é o mesmo `base_url` configurável).
type EmbeddingTarget = (Arc<dyn EmbeddingFetcher>, String, String, String);

/// Mesma tabela usada por produção (`generate_embedding`) e pelo botão "Validar"
/// (`settings::plugin_test::test_embedding_connection`, via [`test_embedding`]) — as duas nunca
/// podem divergir sobre qual credencial/endpoint um provider usa (classe de defeito "par
/// assimétrico endereço↔credencial", já vista neste repositório).
fn resolve_embedding_target(settings: &EmbeddingSettings) -> Option<EmbeddingTarget> {
    match settings.provider.as_str() {
        // In-process: the guard URL is loopback so offline mode never blocks it.
        "local" => Some((Arc::new(LocalEmbeddingFetcher), String::new(), String::new(), "http://127.0.0.1".to_string())),
        "ollama" => Some((Arc::new(OllamaEmbeddingFetcher), settings.base_url.clone(), settings.api_key.clone(), settings.base_url.clone())),
        "google" => Some((
            Arc::new(GoogleEmbeddingFetcher { task_type: settings.task_type.clone(), dimensions: settings.dimensions }),
            String::new(),
            settings.google_api_key.clone(),
            GOOGLE_EMBED_API_BASE.to_string(),
        )),
        "openrouter" => Some((Arc::new(OpenRouterEmbeddingFetcher), String::new(), settings.openrouter_api_key.clone(), OPENROUTER_EMBED_BASE.to_string())),
        "openaiCompatible" => Some((
            Arc::new(OpenAiCompatibleEmbeddingFetcher),
            settings.openai_compat_base_url.clone(),
            settings.openai_compat_api_key.clone(),
            settings.openai_compat_base_url.clone(),
        )),
        "unsloth" => Some((Arc::new(UnslothEmbeddingFetcher), settings.unsloth_base_url.clone(), settings.unsloth_api_key.clone(), settings.unsloth_base_url.clone())),
        _ => None,
    }
}

/// = `generateEmbedding`: chama o provedor configurado (os 5 — [02](../../../../../../specs/02-providers.md))
/// e degrada para `None` — nunca lança — quando o provedor é desconhecido, a rede está bloqueada por
/// modo offline, ou a chamada falha. Caminho SÍNCRONO (pós-turno); o turno de chat usa
/// [`generate_embedding_async`].
pub fn generate_embedding(store: &MemoryStore, content: &str, embedding_enabled: bool) -> Option<Vec<f64>> {
    if !embedding_enabled {
        return None;
    }
    let settings = crate::modules::memory::embedding_settings_reader()?;
    generate_embedding_via(resolve_embedding_target, &settings, content, store, &QUERY_CACHE)
}

/// Mesmo contrato de [`generate_embedding`], `await`ado no runtime de quem chama.
pub async fn generate_embedding_async(store: &MemoryStore, content: &str, embedding_enabled: bool) -> Option<Vec<f64>> {
    if !embedding_enabled {
        return None;
    }
    let settings = crate::modules::memory::embedding_settings_reader()?;
    generate_embedding_via_async(resolve_embedding_target, &settings, content, store, &QUERY_CACHE).await
}

/// Um pedido que passou pelo resolvedor e pelo gate de modo offline, com a chave de cache já montada.
struct Dispatch {
    fetcher: Arc<dyn EmbeddingFetcher>,
    base_url: String,
    api_key: String,
    cache_key: String,
}

/// Resolvedor + gate de rede — o gate roda ANTES de qualquer fetch nos dois caminhos.
fn prepare(resolve: impl Fn(&EmbeddingSettings) -> Option<EmbeddingTarget>, settings: &EmbeddingSettings, content: &str) -> Option<Dispatch> {
    let Some((fetcher, base_url, api_key, guard_url)) = resolve(settings) else {
        log::debug!("memória episódica: provedor de embedding '{}' sem cliente Rust — busca por palavra-chave", settings.provider);
        return None;
    };
    if settings.offline_mode && !is_loopback_url(&guard_url) {
        log::warn!("modo offline: embedding bloqueado (provider={}, url={guard_url})", settings.provider);
        return None;
    }
    let cache_key = cache_key(settings, &base_url, content);
    Some(Dispatch { fetcher, base_url, api_key, cache_key })
}

fn cache_key(settings: &EmbeddingSettings, base_url: &str, content: &str) -> String {
    format!("{}\u{1f}{}\u{1f}{base_url}\u{1f}{}\u{1f}{}\u{1f}{content}", settings.provider, settings.model, settings.task_type, settings.dimensions)
}

/// **Invalidação de assinatura amarrada aqui, não em `write.rs`.** É o único lugar com os TRÊS
/// ingredientes de `"{provider}:{model}:{dim}"` ao mesmo tempo — `settings.provider`/`settings.model`
/// já estão em escopo, e `vector.len()` só existe depois que o fetch responde. `store` nunca dispara
/// rede sozinho: `invalidate_index_if_embedding_changed` é leitura/escrita local no `MemoryStore`.
fn finish(store: &MemoryStore, settings: &EmbeddingSettings, cache: &EmbeddingCache, cache_key: String, fetched: Option<Vec<f64>>) -> Option<Vec<f64>> {
    let Some(vector) = fetched else {
        log::warn!("memória episódica: falha ao gerar embedding via {} — caindo para busca por palavra-chave", settings.provider);
        return None;
    };
    let signature = format!("{}:{}:{}", settings.provider, settings.model, vector.len());
    if let Err(error) = invalidate_index_if_embedding_changed(store, None, &signature) {
        log::warn!("memória episódica: falha ao checar assinatura de embedding: {error}");
    }
    cache.put(cache_key, vector.clone());
    Some(vector)
}

/// Corpo testável de [`generate_embedding`] (resolvedor e cache injetados, sem servidor real no ar).
fn generate_embedding_via(
    resolve: impl Fn(&EmbeddingSettings) -> Option<EmbeddingTarget>,
    settings: &EmbeddingSettings,
    content: &str,
    store: &MemoryStore,
    cache: &EmbeddingCache,
) -> Option<Vec<f64>> {
    let dispatch = prepare(resolve, settings, content)?;
    if let Some(hit) = cache.get(&dispatch.cache_key) {
        return Some(hit);
    }
    let fetched = fetch_embedding_blocking(dispatch.fetcher, dispatch.base_url, dispatch.api_key, settings.model.clone(), content.to_string());
    finish(store, settings, cache, dispatch.cache_key, fetched)
}

/// Corpo testável de [`generate_embedding_async`].
async fn generate_embedding_via_async(
    resolve: impl Fn(&EmbeddingSettings) -> Option<EmbeddingTarget>,
    settings: &EmbeddingSettings,
    content: &str,
    store: &MemoryStore,
    cache: &EmbeddingCache,
) -> Option<Vec<f64>> {
    let dispatch = prepare(resolve, settings, content)?;
    if let Some(hit) = cache.get(&dispatch.cache_key) {
        return Some(hit);
    }
    let fetched = dispatch.fetcher.embed(&dispatch.base_url, &dispatch.api_key, &settings.model, content).await.ok();
    finish(store, settings, cache, dispatch.cache_key, fetched)
}

/// = `invalidateIndexIfEmbeddingChanged`: tira de circulação o vetor de TODAS as entradas quando o
/// modelo de embedding trocou de assinatura (provider/modelo/dimensão) — nunca compara vetores de
/// espaços diferentes como se fossem compatíveis.
///
/// **Não apaga:** o vetor antigo é guardado no mesmo hash sob `embedding@<assinatura antiga>` e o da
/// assinatura nova, se já existir guardado, volta a `embedding`. Antes era `h_del` — trocar de modelo
/// (ou testar um e voltar) destruía irreversivelmente a memória semântica. O que não tem vetor da
/// assinatura nova segue na busca por palavra-chave até ser regravado; reembeddar em segundo plano
/// fica como evolução (custa chamadas ao provedor, que o usuário não pediu).
pub fn invalidate_index_if_embedding_changed(
    store: &MemoryStore,
    current_signature: Option<&str>,
    signature: &str,
) -> Result<(), String> {
    if current_signature == Some(signature) {
        return Ok(());
    }
    let stored = store.h_get_all(EMBEDDING_META_KEY)?;
    let stored_signature = stored.get("signature").cloned();
    if stored_signature.as_deref() != Some(signature) {
        if let Some(prev) = &stored_signature {
            let keep = recent_stashed_signatures(&stored, prev, signature);
            let mut stashed = 0usize;
            for tier in [MemoryTier::Short, MemoryTier::Long] {
                let ids = store.z_range_by_score(
                    &index_key(tier),
                    crate::modules::memory::ScoreBound::NegInf,
                    crate::modules::memory::ScoreBound::PosInf,
                    crate::modules::memory::ZRangeOpts::default(),
                )?;
                for id in ids {
                    stashed += swap_stashed_embedding(store, &key(tier, &id), prev, signature, &keep)?;
                }
            }
            store.h_set(EMBEDDING_META_KEY, STASHED_META_FIELD, &serde_json::to_string(&keep).unwrap_or_default())?;
            log::warn!("modelo de embedding mudou ({prev} -> {signature}); {stashed} vetores guardados sob a assinatura antiga");
        }
        store.h_set(EMBEDDING_META_KEY, "signature", signature)?;
    }
    Ok(())
}

/// Quantas assinaturas antigas mantêm vetor guardado por entrada. Cada vetor pesa dimensão×8 bytes ×
/// toda entrada: sem teto, quem experimenta modelos acumulava um vetor por modelo, para sempre.
const MAX_STASHED_SIGNATURES: usize = 2;
const STASHED_META_FIELD: &str = "stashed";

/// As assinaturas que continuam guardadas depois de trocar de `prev` para `next`: `prev` entra na
/// frente (a mais recente), `next` sai (volta a ser a ativa) e o excedente de
/// [`MAX_STASHED_SIGNATURES`] é cortado pelo fim. A ordem vem do campo `stashed` do meta-hash.
fn recent_stashed_signatures(meta: &std::collections::HashMap<String, String>, prev: &str, next: &str) -> Vec<String> {
    let known: Vec<String> = meta.get(STASHED_META_FIELD).and_then(|raw| serde_json::from_str(raw).ok()).unwrap_or_default();
    std::iter::once(prev.to_string())
        .chain(known.into_iter().filter(|signature| signature != prev && signature != next))
        .take(MAX_STASHED_SIGNATURES)
        .collect()
}

/// Campo do hash que guarda o vetor de uma assinatura que não está em uso.
fn stash_field(signature: &str) -> String {
    format!("embedding@{signature}")
}

/// Guarda o `embedding` atual sob `prev`, traz de volta o que estava guardado sob `next` e apaga o
/// vetor de toda assinatura fora de `keep`. Devolve 1 se havia vetor para guardar.
fn swap_stashed_embedding(store: &MemoryStore, hash_key: &str, prev: &str, next: &str, keep: &[String]) -> Result<usize, String> {
    let hash = store.h_get_all(hash_key)?;
    let mut stashed = 0;
    if let Some(vector) = hash.get("embedding") {
        store.h_set(hash_key, &stash_field(prev), vector)?;
        store.h_del(hash_key, "embedding")?;
        stashed = 1;
    }
    if let Some(saved) = hash.get(&stash_field(next)) {
        store.h_set(hash_key, "embedding", saved)?;
        store.h_del(hash_key, &stash_field(next))?;
    }
    for field in hash.keys().filter_map(|f| f.strip_prefix("embedding@")) {
        if !keep.iter().any(|signature| signature == field) {
            store.h_del(hash_key, &stash_field(field))?;
        }
    }
    Ok(stashed)
}

/// = `invalidateStaleEmbedding`: sem embedding novo mas o conteúdo mudou, o vetor antigo não
/// corresponde mais ao texto — apaga em vez de deixar um vetor mentiroso na busca semântica.
pub fn invalidate_stale_embedding(
    store: &MemoryStore,
    tier: MemoryTier,
    id: &str,
    previous_content: Option<&str>,
    new_content: &str,
    new_embedding: Option<&[f64]>,
) -> Result<(), String> {
    let content_changed = previous_content.is_some() && previous_content != Some(new_content);
    if !content_changed {
        return Ok(());
    }
    let hash_key = key(tier, id);
    if new_embedding.is_none() {
        store.h_del(&hash_key, "embedding")?;
    }
    // Os vetores guardados sob outras assinaturas descrevem o texto ANTIGO: voltar a esse modelo
    // não pode ressuscitar um vetor que não corresponde mais ao conteúdo.
    for field in store.h_get_all(&hash_key)?.into_keys().filter(|f| f.starts_with("embedding@")) {
        store.h_del(&hash_key, &field)?;
    }
    Ok(())
}

/// = `findDuplicateMemory`: varre os dois tiers (teto `EMBEDDING_SCAN_LIMIT`) atrás do vizinho MAIS
/// PRÓXIMO acima do limiar de deduplicação (antes: o primeiro que passasse, na ordem da varredura).
/// Só funde entradas do MESMO tipo — uma conversa nunca reescreve um resumo do tier longo — e nunca
/// uma afirmação com a própria negação ("gosta de gatos" / "não gosta de gatos" têm cosseno alto).
pub fn find_duplicate_memory(store: &MemoryStore, emb: &[f64], dedup_threshold: f64, entry_type: &str, content: &str) -> Result<Option<String>, String> {
    let mut best: Option<(String, f64)> = None;
    for tier in [MemoryTier::Short, MemoryTier::Long] {
        for entry in load_tier(store, tier, Some(EMBEDDING_SCAN_LIMIT))? {
            let Some(candidate_emb) = &entry.embedding else { continue };
            if candidate_emb.len() != emb.len() || entry.r#type != entry_type || negation_differs(&entry.content, content) {
                continue;
            }
            let sim = cosine_similarity(emb, candidate_emb);
            if sim >= dedup_threshold && best.as_ref().is_none_or(|(_, top)| sim > *top) {
                best = Some((entry.id, sim));
            }
        }
    }
    Ok(best.map(|(id, _)| id))
}

/// Só UM dos dois textos nega ("não", "nunca", "parou de", "not", "never"...): a afirmação e a
/// negação são memórias diferentes, mesmo quando o vetor quase não as distingue.
fn negation_differs(a: &str, b: &str) -> bool {
    fn negates(text: &str) -> bool {
        let norm = crate::modules::memory::text::strip_diacritics(text).to_lowercase();
        norm.split(|c: char| !c.is_alphanumeric()).any(|w| matches!(w, "nao" | "nunca" | "jamais" | "not" | "never" | "no" | "parou" | "deixou" | "stopped" | "quit"))
    }
    negates(a) != negates(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::write_entry;

    #[test]
    fn generate_embedding_degrades_to_none_honestly() {
        let store = MemoryStore::open_in_memory().unwrap();
        assert_eq!(generate_embedding(&store, "qualquer coisa", true), None);
        assert_eq!(generate_embedding(&store, "qualquer coisa", false), None);
    }

    fn settings(provider: &str, offline: bool, base_url: &str) -> EmbeddingSettings {
        EmbeddingSettings {
            provider: provider.to_string(),
            model: "nomic-embed-text".to_string(),
            base_url: base_url.to_string(),
            api_key: String::new(),
            google_api_key: String::new(),
            openrouter_api_key: String::new(),
            openai_compat_base_url: String::new(),
            openai_compat_api_key: String::new(),
            unsloth_base_url: String::new(),
            unsloth_api_key: String::new(),
            task_type: String::new(),
            dimensions: 0,
            offline_mode: offline,
        }
    }

    /// Dublê que grava se foi chamado — prova POSITIVAMENTE que um caminho bloqueado (provedor
    /// diferente, modo offline) nunca despacha, e não só que o resultado final bate.
    struct FakeFetcher {
        result: Result<Vec<f64>, String>,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl FakeFetcher {
        fn ok(vector: Vec<f64>) -> Self {
            Self { result: Ok(vector), calls: Default::default() }
        }

        fn err(message: &str) -> Self {
            Self { result: Err(message.to_string()), calls: Default::default() }
        }

        fn calls(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn was_called(&self) -> bool {
            self.calls() > 0
        }
    }

    #[async_trait]
    impl EmbeddingFetcher for FakeFetcher {
        async fn embed(&self, _base_url: &str, _api_key: &str, _model: &str, _content: &str) -> Result<Vec<f64>, String> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.result.clone()
        }
    }

    /// Cada teste com o próprio cache: o do processo ([`QUERY_CACHE`]) vazaria vetor de um teste
    /// para outro que usa o mesmo texto.
    fn via(resolve: impl Fn(&EmbeddingSettings) -> Option<EmbeddingTarget>, cfg: &EmbeddingSettings, content: &str, store: &MemoryStore) -> Option<Vec<f64>> {
        generate_embedding_via(resolve, cfg, content, store, &EmbeddingCache::new())
    }

    /// O turno de chat embeda a mensagem do usuário ANTES de chamar o LLM; a mesma mensagem de novo
    /// (reenvio, failover que refaz o turno, gravação pós-turno do mesmo texto) não pode pagar uma
    /// segunda ida ao provedor.
    #[test]
    fn a_repeated_query_is_served_from_the_cache_without_a_second_fetch() {
        let store = MemoryStore::open_in_memory().unwrap();
        let fetcher = Arc::new(FakeFetcher::ok(vec![0.4, 0.6]));
        let dyn_fetcher: Arc<dyn EmbeddingFetcher> = fetcher.clone();
        let cfg = settings("ollama", false, "http://localhost:11434");
        let cache = EmbeddingCache::new();
        let resolve = fake_resolver(dyn_fetcher);

        assert_eq!(generate_embedding_via(&resolve, &cfg, "oi mika", &store, &cache), Some(vec![0.4, 0.6]));
        assert_eq!(generate_embedding_via(&resolve, &cfg, "oi mika", &store, &cache), Some(vec![0.4, 0.6]));
        assert_eq!(fetcher.calls(), 1);

        let mut other_model = cfg.clone();
        other_model.model = "mxbai-embed-large".to_string();
        generate_embedding_via(&resolve, &other_model, "oi mika", &store, &cache);
        assert_eq!(fetcher.calls(), 2, "vetor de outro modelo nunca serve de resposta");
    }

    #[test]
    fn a_failed_fetch_is_never_cached() {
        let store = MemoryStore::open_in_memory().unwrap();
        let fetcher = Arc::new(FakeFetcher::err("fora do ar"));
        let dyn_fetcher: Arc<dyn EmbeddingFetcher> = fetcher.clone();
        let cfg = settings("ollama", false, "http://localhost:11434");
        let cache = EmbeddingCache::new();
        let resolve = fake_resolver(dyn_fetcher);
        generate_embedding_via(&resolve, &cfg, "oi mika", &store, &cache);
        generate_embedding_via(&resolve, &cfg, "oi mika", &store, &cache);
        assert_eq!(fetcher.calls(), 2);
    }

    /// Caminho do turno de chat: `await` direto no runtime de quem chama — nada de thread nova nem
    /// runtime novo por turno. `current_thread` é o caso que a ponte síncrona não suportaria.
    #[tokio::test(flavor = "current_thread")]
    async fn the_async_path_awaits_the_fetch_on_the_callers_runtime_and_shares_the_cache() {
        let store = MemoryStore::open_in_memory().unwrap();
        let fetcher = Arc::new(FakeFetcher::ok(vec![0.7]));
        let dyn_fetcher: Arc<dyn EmbeddingFetcher> = fetcher.clone();
        let cfg = settings("ollama", false, "http://localhost:11434");
        let cache = EmbeddingCache::new();
        let resolve = fake_resolver(dyn_fetcher);

        assert_eq!(generate_embedding_via_async(&resolve, &cfg, "oi mika", &store, &cache).await, Some(vec![0.7]));
        assert_eq!(generate_embedding_via(&resolve, &cfg, "oi mika", &store, &cache), Some(vec![0.7]));
        assert_eq!(fetcher.calls(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_async_path_keeps_the_offline_gate() {
        let store = MemoryStore::open_in_memory().unwrap();
        let fetcher = Arc::new(FakeFetcher::ok(vec![1.0]));
        let dyn_fetcher: Arc<dyn EmbeddingFetcher> = fetcher.clone();
        let cfg = settings("ollama", true, "http://ollama-remoto.invalid:11434");
        assert_eq!(generate_embedding_via_async(fake_resolver(dyn_fetcher), &cfg, "oi mika", &store, &EmbeddingCache::new()).await, None);
        assert!(!fetcher.was_called());
    }

    /// Resolvedor de teste: injeta um [`FakeFetcher`] fixo qualquer que seja o provider pedido, com
    /// `guard_url`/`base_url` iguais ao `base_url` da config — suficiente para testar o dispatch e o
    /// gate de modo offline sem servidor real no ar.
    fn fake_resolver(fetcher: Arc<dyn EmbeddingFetcher>) -> impl Fn(&EmbeddingSettings) -> Option<EmbeddingTarget> {
        move |s: &EmbeddingSettings| Some((Arc::clone(&fetcher), s.base_url.clone(), s.api_key.clone(), s.base_url.clone()))
    }

    #[test]
    fn generate_embedding_via_returns_the_vector_from_a_configured_ollama_provider() {
        let store = MemoryStore::open_in_memory().unwrap();
        let fetcher: Arc<dyn EmbeddingFetcher> = Arc::new(FakeFetcher::ok(vec![0.1, 0.2, 0.3]));
        let cfg = settings("ollama", false, "http://localhost:11434");
        assert_eq!(via(fake_resolver(fetcher), &cfg, "oi mika", &store), Some(vec![0.1, 0.2, 0.3]));
    }

    /// Provider desconhecido (nenhum dos 4) não pode despachar — `resolve_embedding_target` devolve
    /// `None` e o dispatch nem chega a resolver fetcher algum.
    /// `local` runs in-process: offline mode must not block it, and it never needs a key or URL.
    #[test]
    fn the_local_provider_resolves_without_credentials_and_passes_the_offline_gate() {
        let (_, base_url, api_key, guard_url) = resolve_embedding_target(&settings("local", true, "")).expect("local resolve");
        assert!(base_url.is_empty() && api_key.is_empty());
        assert!(is_loopback_url(&guard_url), "{guard_url}");
    }

    // `llm.embeddingTaskType`/`llm.embeddingOutputDimensionality` were shown for Google and never
    // sent (dead controls, 2026-09-27 sweep). A new dimension re-indexes by itself (signature).
    #[test]
    fn the_google_request_carries_the_configured_task_type_and_dimensions() {
        let body = google_embed_body("oi", "RETRIEVAL_QUERY", 768);
        assert_eq!(body["taskType"], "RETRIEVAL_QUERY");
        assert_eq!(body["outputDimensionality"], 768);
        assert_eq!(body["content"]["parts"][0]["text"], "oi");
        let defaults = google_embed_body("oi", "", 0);
        assert!(defaults.get("taskType").is_none() && defaults.get("outputDimensionality").is_none());
    }

    #[test]
    fn generate_embedding_via_never_dispatches_for_an_unknown_provider() {
        let store = MemoryStore::open_in_memory().unwrap();
        let cfg = settings("bing-embeddings-inventado", false, "http://localhost:11434");
        assert_eq!(via(|_| None, &cfg, "oi mika", &store), None);
    }

    /// Teste negativo obrigatório: modo offline barra a chamada mesmo com provedor `ollama`
    /// configurado, quando a baseUrl não é loopback — mesma ordem que
    /// `steam_achievements::offline_mode_blocks_the_call_even_with_valid_credentials` prova.
    #[test]
    fn generate_embedding_via_offline_mode_blocks_a_non_loopback_ollama() {
        let store = MemoryStore::open_in_memory().unwrap();
        let fetcher = Arc::new(FakeFetcher::ok(vec![1.0]));
        let dyn_fetcher: Arc<dyn EmbeddingFetcher> = fetcher.clone();
        let cfg = settings("ollama", true, "http://ollama-remoto.invalid:11434");
        assert_eq!(via(fake_resolver(dyn_fetcher), &cfg, "oi mika", &store), None);
        assert!(!fetcher.was_called(), "modo offline não pode chegar a despachar rede");
    }

    #[test]
    fn generate_embedding_via_offline_mode_still_allows_a_loopback_ollama() {
        let store = MemoryStore::open_in_memory().unwrap();
        let fetcher: Arc<dyn EmbeddingFetcher> = Arc::new(FakeFetcher::ok(vec![9.0]));
        let cfg = settings("ollama", true, "http://localhost:11434");
        assert_eq!(via(fake_resolver(fetcher), &cfg, "oi mika", &store), Some(vec![9.0]));
    }

    #[test]
    fn generate_embedding_via_degrades_to_none_when_the_fetch_fails() {
        let store = MemoryStore::open_in_memory().unwrap();
        let fetcher: Arc<dyn EmbeddingFetcher> = Arc::new(FakeFetcher::err("Ollama embeddings unreachable"));
        let cfg = settings("ollama", false, "http://localhost:11434");
        assert_eq!(via(fake_resolver(fetcher), &cfg, "oi mika", &store), None);
    }

    /// **O teste do item 3**: uma entrada já indexada sob uma assinatura ANTIGA é invalidada assim
    /// que um embedding de verdade é gerado sob uma assinatura NOVA (provider/modelo/dimensão
    /// diferentes) — sem isto, `find_duplicate_memory`/busca semântica comparariam vetores de
    /// espaços incompatíveis como se fossem do mesmo modelo (defects.md "memory-relationship" P1:
    /// `invalidate_index_if_embedding_changed` existia, testada, mas sem chamador de produção).
    #[test]
    fn generate_embedding_via_invalidates_the_index_when_the_signature_changes() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Short, "a", "conteudo antigo", "conversation", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();
        store.h_set(EMBEDDING_META_KEY, "signature", "google:text-embedding-004:2").unwrap();

        let fetcher: Arc<dyn EmbeddingFetcher> = Arc::new(FakeFetcher::ok(vec![0.1, 0.2, 0.3]));
        let cfg = settings("ollama", false, "http://localhost:11434"); // model = "nomic-embed-text" (helper `settings()`)
        let vector = via(fake_resolver(fetcher), &cfg, "oi mika", &store);
        assert_eq!(vector, Some(vec![0.1, 0.2, 0.3]));

        let hash = store.h_get_all(&key(MemoryTier::Short, "a")).unwrap();
        assert!(!hash.contains_key("embedding"), "assinatura mudou (provider e dimensão) — vetor antigo tinha de ser derrubado");
        let meta = store.h_get_all(EMBEDDING_META_KEY).unwrap();
        assert_eq!(meta.get("signature"), Some(&"ollama:nomic-embed-text:3".to_string()));
    }

    /// NEGATIVO — mesma assinatura (mesmo provider/modelo/dimensão): o índice sobrevive.
    #[test]
    fn generate_embedding_via_keeps_the_index_when_the_signature_is_unchanged() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Short, "a", "conteudo", "conversation", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();
        store.h_set(EMBEDDING_META_KEY, "signature", "ollama:nomic-embed-text:2").unwrap();

        let fetcher: Arc<dyn EmbeddingFetcher> = Arc::new(FakeFetcher::ok(vec![0.5, 0.5]));
        let cfg = settings("ollama", false, "http://localhost:11434");
        via(fake_resolver(fetcher), &cfg, "oi de novo", &store);

        let hash = store.h_get_all(&key(MemoryTier::Short, "a")).unwrap();
        assert!(hash.contains_key("embedding"), "mesma assinatura: índice preservado");
    }

    // --- resolve_embedding_target: os 4 provedores, par correto endereço↔credencial -------------

    fn full_settings(provider: &str) -> EmbeddingSettings {
        EmbeddingSettings {
            provider: provider.to_string(),
            model: String::new(),
            base_url: "http://localhost:11434".to_string(),
            api_key: "ollama-key".to_string(),
            google_api_key: "google-key".to_string(),
            openrouter_api_key: "openrouter-key".to_string(),
            openai_compat_base_url: "http://localhost:4000".to_string(),
            openai_compat_api_key: "openai-compatible-key".to_string(),
            unsloth_base_url: "http://localhost:8888/v1".to_string(),
            unsloth_api_key: "sk-unsloth-key".to_string(),
            task_type: String::new(),
            dimensions: 0,
            offline_mode: false,
        }
    }

    #[test]
    fn resolve_embedding_target_covers_all_five_providers_from_02_providers() {
        for provider in ["ollama", "google", "openrouter", "openaiCompatible", "unsloth"] {
            assert!(resolve_embedding_target(&full_settings(provider)).is_some(), "provider={provider} deveria resolver");
        }
        assert!(resolve_embedding_target(&full_settings("bedrock")).is_none(), "provider fora dos 5 não deve resolver");
    }

    /// Cada provider usa SÓ a própria credencial — nunca a de outro (o Google não pode herdar a key
    /// do Ollama, por exemplo). Prova a tabela inteira de uma vez, não campo a campo.
    #[test]
    fn resolve_embedding_target_never_mixes_up_credentials_across_providers() {
        let cfg = full_settings("google");
        let (_, _, api_key, _) = resolve_embedding_target(&cfg).unwrap();
        assert_eq!(api_key, "google-key");

        let cfg = full_settings("openrouter");
        let (_, _, api_key, _) = resolve_embedding_target(&cfg).unwrap();
        assert_eq!(api_key, "openrouter-key");

        let cfg = full_settings("openaiCompatible");
        let (_, base_url, api_key, guard_url) = resolve_embedding_target(&cfg).unwrap();
        assert_eq!(base_url, "http://localhost:4000");
        assert_eq!(api_key, "openai-compatible-key");
        assert_eq!(guard_url, "http://localhost:4000");

        let cfg = full_settings("unsloth");
        let (_, base_url, api_key, guard_url) = resolve_embedding_target(&cfg).unwrap();
        assert_eq!(base_url, "http://localhost:8888/v1");
        assert_eq!(api_key, "sk-unsloth-key", "a chave do Unsloth nunca é a do openaiCompatible");
        assert_eq!(guard_url, "http://localhost:8888/v1");
    }

    /// Google/OpenRouter têm endereço FIXO (nunca loopback) — modo offline sempre bloqueia os dois,
    /// mesma garantia que `assert_network_allowed` dá para as URLs fixas de `modules/llm.rs`.
    #[test]
    fn resolve_embedding_target_fixed_endpoints_are_never_loopback() {
        for provider in ["google", "openrouter"] {
            let (_, _, _, guard_url) = resolve_embedding_target(&full_settings(provider)).unwrap();
            assert!(!is_loopback_url(&guard_url), "provider={provider} tem endereço fixo e não pode virar loopback");
        }
    }

    /// Fim a fim: com o resolvedor REAL (não um dublê), modo offline bloqueia Google mesmo com
    /// credencial válida configurada — nunca chega a montar o fetcher de rede.
    #[test]
    fn generate_embedding_via_offline_mode_blocks_google_even_with_a_valid_key() {
        let store = MemoryStore::open_in_memory().unwrap();
        let mut cfg = full_settings("google");
        cfg.offline_mode = true;
        assert_eq!(via(resolve_embedding_target, &cfg, "oi mika", &store), None);
    }

    #[test]
    fn find_duplicate_memory_matches_above_threshold_and_ignores_dimension_mismatch() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Short, "a", "x", "conversation", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();
        write_entry(&store, MemoryTier::Short, "b", "y", "conversation", 2, "{}", 3600, Some(&[1.0, 0.0, 0.0])).unwrap();

        let dup = find_duplicate_memory(&store, &[1.0, 0.0], 0.95, "conversation", "x").unwrap();
        assert_eq!(dup, Some("a".to_string()));
    }

    #[test]
    fn find_duplicate_memory_picks_the_closest_neighbour_not_the_first_one_above_the_threshold() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Short, "perto", "a", "conversation", 1, "{}", 3600, Some(&[1.0, 0.1])).unwrap();
        write_entry(&store, MemoryTier::Short, "exato", "b", "conversation", 2, "{}", 3600, Some(&[1.0, 0.0])).unwrap();
        // "perto" vem antes na varredura (mais antigo não, mais novo primeiro: "exato"); troque a ordem
        // gravando o melhor por último não muda o resultado — vale a maior similaridade.
        let dup = find_duplicate_memory(&store, &[1.0, 0.0], 0.9, "conversation", "novo").unwrap();
        assert_eq!(dup, Some("exato".to_string()));
    }

    #[test]
    fn find_duplicate_memory_never_overwrites_an_entry_of_another_type() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Long, "resumo", "resumo consolidado de semanas", "summary", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();
        let dup = find_duplicate_memory(&store, &[1.0, 0.0], 0.95, "conversation", "User: oi").unwrap();
        assert_eq!(dup, None, "uma conversa não reescreve um resumo do tier longo");
    }

    #[test]
    fn find_duplicate_memory_does_not_merge_a_statement_with_its_negation() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Short, "a", "gosta de gatos", "conversation", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();
        assert_eq!(find_duplicate_memory(&store, &[1.0, 0.0], 0.95, "conversation", "não gosta de gatos").unwrap(), None);
        assert_eq!(find_duplicate_memory(&store, &[1.0, 0.0], 0.95, "conversation", "gosta MUITO de gatos").unwrap(), Some("a".to_string()));
    }

    #[test]
    fn invalidate_index_if_embedding_changed_drops_only_on_real_signature_change() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Short, "a", "x", "conversation", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();

        invalidate_index_if_embedding_changed(&store, None, "google:text-embedding-004:2").unwrap();
        let hash = store.h_get_all(&key(MemoryTier::Short, "a")).unwrap();
        assert!(hash.contains_key("embedding"), "primeira gravação de assinatura não derruba nada");

        invalidate_index_if_embedding_changed(&store, Some("google:text-embedding-004:2"), "ollama:nomic:768").unwrap();
        let hash = store.h_get_all(&key(MemoryTier::Short, "a")).unwrap();
        assert!(!hash.contains_key("embedding"), "troca real de assinatura derruba o vetor velho");
    }

    /// Trocar de provedor/modelo não apaga os vetores: ficam guardados sob a assinatura antiga e voltam
    /// intactos se o usuário volta ao modelo anterior (testar outro modelo ou uma queda temporária do
    /// provedor não custa a memória semântica inteira).
    #[test]
    fn switching_the_embedding_model_stashes_vectors_and_switching_back_restores_them() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Long, "a", "x", "conversation", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();
        let original = store.h_get_all(&key(MemoryTier::Long, "a")).unwrap()["embedding"].clone();
        invalidate_index_if_embedding_changed(&store, None, "google:text-embedding-004:2").unwrap();

        invalidate_index_if_embedding_changed(&store, None, "ollama:nomic:768").unwrap();
        let hash = store.h_get_all(&key(MemoryTier::Long, "a")).unwrap();
        assert!(!hash.contains_key("embedding"), "o vetor do modelo antigo não pode ser comparado com o novo");
        assert_eq!(hash["embedding@google:text-embedding-004:2"], original, "vetor antigo guardado, não apagado");

        invalidate_index_if_embedding_changed(&store, None, "google:text-embedding-004:2").unwrap();
        let hash = store.h_get_all(&key(MemoryTier::Long, "a")).unwrap();
        assert_eq!(hash["embedding"], original, "voltar ao modelo anterior restaura o vetor");
        assert!(!hash.contains_key("embedding@google:text-embedding-004:2"));
    }

    #[test]
    fn invalidate_stale_embedding_only_when_content_changed_without_a_new_vector() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Short, "a", "x", "conversation", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();

        invalidate_stale_embedding(&store, MemoryTier::Short, "a", Some("x"), "x", None).unwrap();
        assert!(store.h_get_all(&key(MemoryTier::Short, "a")).unwrap().contains_key("embedding"), "conteúdo igual mantém o vetor");

        invalidate_stale_embedding(&store, MemoryTier::Short, "a", Some("x"), "novo conteudo", None).unwrap();
        assert!(!store.h_get_all(&key(MemoryTier::Short, "a")).unwrap().contains_key("embedding"), "conteúdo mudou sem vetor novo: vetor velho apagado");
    }

    #[test]
    fn a_content_change_also_drops_vectors_stashed_under_other_models() {
        let store = MemoryStore::open_in_memory().unwrap();
        write_entry(&store, MemoryTier::Short, "a", "x", "conversation", 1, "{}", 3600, None).unwrap();
        store.h_set(&key(MemoryTier::Short, "a"), "embedding@google:m:2", "[1.0,0.0]").unwrap();

        invalidate_stale_embedding(&store, MemoryTier::Short, "a", Some("x"), "texto novo", Some(&[0.5, 0.5])).unwrap();

        assert!(!store.h_get_all(&key(MemoryTier::Short, "a")).unwrap().contains_key("embedding@google:m:2"));
    }

    /// Cada troca de modelo guardava o vetor antigo sob `embedding@<assinatura>` para sempre: quem
    /// testa vários modelos acumulava um vetor por modelo em CADA entrada, sem teto. Ficam as 2
    /// assinaturas mais recentes; o resto sai.
    #[test]
    fn at_most_two_stashed_embedding_signatures_are_kept_per_entry() {
        let store = MemoryStore::open_in_memory().unwrap();
        let entry = key(MemoryTier::Long, "a");
        write_entry(&store, MemoryTier::Long, "a", "x", "conversation", 1, "{}", 3600, Some(&[1.0, 0.0])).unwrap();
        invalidate_index_if_embedding_changed(&store, None, "m1:2").unwrap();

        for (n, signature) in ["m2:2", "m3:2", "m4:2"].into_iter().enumerate() {
            // o re-embedding em segundo plano regrava o vetor da assinatura em uso antes da próxima troca
            store.h_set(&entry, "embedding", &format!("[{n}.0,1.0]")).unwrap();
            invalidate_index_if_embedding_changed(&store, None, signature).unwrap();
        }

        let mut stashed: Vec<String> = store.h_get_all(&entry).unwrap().into_keys().filter(|f| f.starts_with("embedding@")).collect();
        stashed.sort();
        assert_eq!(stashed, ["embedding@m2:2", "embedding@m3:2"], "só as duas últimas ficam; m1 saiu");
    }
}
