//! Cliente multimodal mínimo para `vision_capture`/`vision_read` — porte funcional de
//! `handlers/analyze-image.ts::analyzeImages` (`generateText` com imagem), NÃO de
//! `modules/brain/turn.rs`.
//!
//! **Por que não reusar `brain::turn::run_turn_ollama`/`run_turn_ollama_raw`:** medido nesta rodada
//! — nenhuma fachada exportada de lá aceita imagem. `run_turn`/`run_turn_ollama_raw` só recebem
//! `user_content: &str` (texto puro vira `Message::user(...)`, sem slot para `UserContent::Image`).
//! `run_turn_ollama` aceita `history: Vec<Message>`, mas compõe o system prompt de PERSONA
//! (`compose_system_prompt`) e injeta o catálogo INTEIRO de tools do registro global — rodar OCR ou
//! "descreva esta imagem" through ali seria um turno agente completo (podendo, inclusive, chamar
//! `vision_capture` de novo) para uma pergunta que o TS resolve com `generateText` sem tools. Nenhum
//! ajuste caberia sem editar `brain/turn.rs`, fora do escopo desta rodada.
//!
//! **O que este arquivo faz então:** reusa a MESMA dependência (`rig-core`, já no grafo — zero crate
//! nova) e o MESMO chokepoint de privacidade (`privacy::assert_network_allowed`) que `brain::turn`
//! usa, com a config Ollama (model/base_url/api_key) resolvida em miniatura — precedente já aceito
//! no repo para duplicar leitura pequena de `ConfigStore` através de um limite de módulo que não
//! pode mudar nesta rodada (`brain::turn::resolve_ollama_config` cita o mesmo motivo sobre
//! `chat.rs::prompts_root`/`health.rs`). Sem streaming, sem tools — uma chamada, uma resposta, o
//! mesmo formato de `generateText`.
//!
//! ## Um provedor por opção do schema
//!
//! [`resolve_effective_provider`] segue `vision.provider` (`config/schemas/vision.rs`), ou
//! `llm.provider` quando ele é `"default"`/vazio:
//! - **`ollama`** (default quando `vision.provider` é `"ollama"`, OU `"default"`/vazio com
//!   `llm.provider` != `"openaiCompatible"`) — [`analyze_ollama`], inalterado byte a byte.
//! - **`openaiCompatible`/`unsloth`/`google`/`openrouter`** — [`analyze_remote`], que reusa
//!   `brain::llm_provider::build_remote_model` (MESMOS clientes, guarda SSRF e modo offline que
//!   `brain::turn` usa para chat) em vez de montar um segundo stack HTTP. `vision.model` sobrescreve
//!   o modelo do provedor quando não vazio, mesma precedência de [`resolve_model_name`].

use crate::config::schemas::llm_defaults;
use rig_core::completion::{CompletionRequest, Message};
use rig_core::message::{AssistantContent, ImageMediaType, MimeType, UserContent};

use crate::config::store::ConfigStore;
use crate::modules::brain::llm_provider;
use crate::modules::brain::provider_params::{remote_params, SamplingSettings};
use crate::modules::llm::model_profile::{self, ModelProfile};
use crate::modules::privacy;

const OLLAMA_DEFAULT_BASE_URL: &str = llm_defaults::OLLAMA_BASE_URL;
const OLLAMA_DEFAULT_MODEL: &str = llm_defaults::OLLAMA_MODEL;

/// Mesmo default e mesmo motivo de `brain::turn::OLLAMA_MAX_NUM_CTX`: sem `num_ctx` explícito o
/// Ollama assume 4096 — já causou 400 silencioso e provedor desativado noutra frente deste repo
/// (ver `project_ollama_num_ctx_4096` na memória do projeto). O pedido real é `llm.contextWindow`,
/// cortado na janela do modelo ([`ollama_num_ctx`]).
const OLLAMA_MAX_NUM_CTX: u64 = 32_768;

/// Contexto pedido ao Ollama: o escolhido na aba LLM, nunca acima da janela de treino do modelo.
fn ollama_num_ctx(requested: Option<u64>, model_window: Option<u64>) -> u64 {
    let requested = requested.filter(|&n| n > 0).unwrap_or(OLLAMA_MAX_NUM_CTX);
    model_window.map_or(requested, |window| requested.min(window))
}

/// Parâmetros extras da análise remota. Visão nunca raciocina (custo e latência por quadro, e um
/// Gemma 4 atrás de `--mmproj` gastava ~300 tokens de "thinking" por chamada), qualquer que seja o
/// `llm.enableThinking` do chat; `topP` e o dialeto de cada provedor vêm de
/// `brain::provider_params`. `id_slot`: análise de visão é `PromptFamily::Background`.
fn remote_vision_params(provider: &str, model: &str, top_p: Option<f64>, profile: &ModelProfile) -> Option<serde_json::Value> {
    let settings = SamplingSettings { top_p, enable_thinking: false, thinking_max_tokens: 0 };
    let mut params = remote_params(provider, model, &settings, profile);
    if let Some(slot) = crate::modules::brain::turn::kv_cache_id_slot(provider, crate::modules::brain::turn::PromptFamily::Background) {
        params.insert("id_slot".to_string(), serde_json::json!(slot));
    }
    (!params.is_empty()).then_some(serde_json::Value::Object(params))
}

/// Default de `vision.maxOutputTokens` — o teto de `analyzeImages`/`watchNow` antes de virar controle.
const DEFAULT_MAX_OUTPUT_TOKENS: u64 = 512;

/// `vision.maxOutputTokens`, cortado na saída máxima do modelo quando o provedor a informa.
fn max_output_tokens(store: &ConfigStore, profile: &ModelProfile) -> u64 {
    let requested = store.get("vision.maxOutputTokens").and_then(|v| v.as_u64()).filter(|&n| n > 0).unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS);
    profile.max_output_tokens.map_or(requested, |ceiling| requested.min(ceiling))
}

#[derive(Debug)]
pub(super) enum VisionLlmError {
    /// Rede barrada pelo chokepoint de privacidade (modo offline + endpoint remoto).
    Privacy(String),
    /// Erro do provedor Ollama: conexão recusada, modelo ausente, resposta malformada.
    Provider(String),
    /// Resposta sem texto nenhum — nunca reportado como sucesso vazio, mesmo princípio de
    /// `analyzeImages` (TS) e de `brain::turn::TurnError::EmptyOutput`.
    EmptyOutput,
    /// No capture survived decoding for the size cap; never sent raw as a fallback.
    InvalidImage,
}

impl std::fmt::Display for VisionLlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Privacy(msg) => write!(f, "{msg}"),
            Self::Provider(msg) => write!(f, "erro do provedor de visão: {msg}"),
            Self::EmptyOutput => write!(f, "o modelo de visão devolveu uma análise vazia"),
            Self::InvalidImage => write!(f, "a captura não pôde ser preparada para o modelo de visão"),
        }
    }
}

fn get_str(store: &ConfigStore, path: &str) -> Option<String> {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty())
}

/// `vision.model` é a sobrescrita específica de visão (mesma leitura de `resolveVisionModel` no
/// TS: `store.get('vision.model')`); sem ela cai no modelo Ollama do chat, na MESMA ordem de
/// `brain::turn::resolve_ollama_config` (`llm.ollamaModel`, depois `llm.model`) —
/// nunca um modelo hardcoded diferente do que o usuário já escolheu para o resto da Mika.
/// `pub(super)`: `watcher::run_tick` precisa da MESMA chave de modelo para consultar
/// `provider_health` (03-resilience.md/06-vision.md — "quando o provedor de visão está travado, o
/// watcher para de capturar") — sem isto, a checagem de saúde usaria uma resolução DIFERENTE da
/// chamada real, e o gate nunca bateria com a chave que [`analyze`] de fato usa.
pub(super) fn resolve_model_name(store: &ConfigStore) -> String {
    get_str(store, "vision.model")
        .or_else(|| get_str(store, "llm.ollamaModel"))
        .or_else(|| get_str(store, "llm.model"))
        .unwrap_or_else(|| OLLAMA_DEFAULT_MODEL.to_string())
}

/// Texto + uso de tokens de uma chamada de análise — porte reduzido de `CompletionResponse`
/// (`rig-core`): só os dois campos que [`analyze`]/[`analyze_with_usage`] precisam expor.
/// `pub(super)`: `watcher::run_tick` lê `usage` para `token_usage::record` (surface `"vision"`).
pub(super) struct AnalyzeOutcome {
    pub text: String,
    pub usage: rig_core::completion::Usage,
}

const REMOTE_PROVIDERS: [&str; 4] = ["openaiCompatible", "unsloth", "google", "openrouter"];

fn as_remote(provider: Option<&str>) -> Option<&'static str> {
    provider.and_then(|p| REMOTE_PROVIDERS.into_iter().find(|remote| *remote == p))
}

/// `vision.provider` efetivo — ver o cabeçalho. `get_str` já filtra string vazia, então `"default"` e
/// ausência caem no mesmo braço; lixo de config antiga degrada para `ollama`.
fn resolve_effective_provider(store: &ConfigStore) -> &'static str {
    match get_str(store, "vision.provider").as_deref() {
        Some("default") | None => as_remote(get_str(store, "llm.provider").as_deref()).unwrap_or("ollama"),
        other => as_remote(other).unwrap_or("ollama"),
    }
}

/// `vision.model` sobrescreve o modelo do provedor remoto (já resolvido em `fallback`, vindo de
/// [`llm_provider::resolve_remote_config`]) quando não vazio — mesma precedência de
/// [`resolve_model_name`] para Ollama.
fn resolve_openai_compatible_model(store: &ConfigStore, fallback: String) -> String {
    get_str(store, "vision.model").unwrap_or(fallback)
}

/// `(provider, model)` that [`analyze`] will call — the watcher keys provider health and token cost on it.
pub(super) fn resolve_health_key(store: &ConfigStore) -> (&'static str, String) {
    match resolve_effective_provider(store) {
        "ollama" => ("ollama", resolve_model_name(store)),
        remote => (remote, resolve_openai_compatible_model(store, llm_provider::resolve_remote_config(store, remote).model)),
    }
}

/// Monta a mensagem multimodal — formato `rig-core` é agnóstico de provedor (cada `Client`
/// converte `Message`/`UserContent` para o payload do seu próprio protocolo), então ollama e
/// openaiCompatible compartilham esta construção sem duplicar nada.
fn build_user_message(prompt_text: &str, images_base64: &[String]) -> Message {
    let mut content = vec![UserContent::text(format!("{prompt_text}\n\n{}", super::responses::SCREEN_DATA_RULE))];
    for image in images_base64 {
        let (data, media_type) = split_data_url(image);
        content.push(UserContent::image_base64(data.to_string(), Some(media_type), None));
    }
    Message::User { content }
}

/// Aceita base64 cru ou data URL — a compressão do watcher devolve `data:image/jpeg;base64,...`, e
/// embrulhar isso de novo dava `data:...;base64,data:...`: o servidor recusava (`Invalid uri-encoded
/// base64 value`) e o watcher de tela nunca analisava nada, em qualquer provedor.
fn split_data_url(image: &str) -> (&str, ImageMediaType) {
    let Some((mime, data)) = image.strip_prefix("data:").and_then(|rest| rest.split_once(";base64,")) else {
        return (image, ImageMediaType::JPEG);
    };
    (data, ImageMediaType::from_mime_type(mime).unwrap_or(ImageMediaType::JPEG))
}

/// Extrai texto + uso de uma `CompletionResponse` — MESMA regra "nunca sucesso vazio" para os dois
/// provedores (ver [`VisionLlmError::EmptyOutput`]).
fn outcome_from_response(response: rig_core::completion::CompletionResponse) -> Result<AnalyzeOutcome, VisionLlmError> {
    let usage = response.usage;
    let text: String = response
        .choice
        .into_iter()
        .filter_map(|item| match item {
            AssistantContent::Text(text) => Some(text.text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    if text.trim().is_empty() {
        return Err(VisionLlmError::EmptyOutput);
    }
    Ok(AnalyzeOutcome { text, usage })
}

/// Visão nunca raciocina: sem `think: false`, um modelo com a capacidade (Gemma 4, Qwen 3.5) gasta o
/// teto de `maxOutputTokens` inteiro no campo `thinking` e devolve `content` vazio (`EmptyOutput`).
fn ollama_vision_params(num_ctx: u64) -> serde_json::Value {
    serde_json::json!({ "num_ctx": num_ctx, "think": false, "keep_alive": crate::modules::brain::turn::OLLAMA_KEEP_ALIVE })
}

/// Chama o Ollama local com texto + N imagens JPEG (base64) — o equivalente do `generateText` que
/// `analyzeImages`/`ocrScreen`/`watchNow` (TS) usam. `caller` identifica o chokepoint de privacidade
/// na mensagem de erro, mesmo padrão de `brain::turn::run_turn_ollama`'s `"brain:turn:ollama"`.
async fn analyze_ollama(store: &ConfigStore, caller: &str, prompt_text: &str, images_base64: &[String]) -> Result<AnalyzeOutcome, VisionLlmError> {
    let base_url = get_str(store, "apiKeys.ollamaBaseUrl").unwrap_or_else(|| OLLAMA_DEFAULT_BASE_URL.to_string());
    let api_key = get_str(store, "apiKeys.ollamaApiKey").unwrap_or_default();
    let model_name = resolve_model_name(store);

    // O CHOKEPOINT: nenhuma imagem sai para a rede antes desta checagem — modo offline com
    // endpoint remoto tem de barrar aqui, não depois de já ter mandado o base64.
    privacy::assert_network_allowed(store, &base_url, caller).map_err(VisionLlmError::Privacy)?;

    let model = llm_provider::build_ollama_model(&base_url, &api_key, &model_name);
    let profile = model_profile::resolve(store, "ollama", &model_name).await;
    let num_ctx = ollama_num_ctx(store.get("llm.contextWindow").and_then(|v| v.as_u64()), profile.context_window);
    let message = build_user_message(prompt_text, images_base64);

    let request = CompletionRequest::new(message)
        .max_tokens(max_output_tokens(store, &profile))
        // `keep_alive` do turno de chat: sem ele, cada quadro analisado rebaixava o modelo (o mesmo,
        // no caso comum) para os 5 min default do Ollama, e o próximo turno pagava a recarga.
        .additional_params(ollama_vision_params(num_ctx));
    let response = model.call(request).await.map_err(|err| VisionLlmError::Provider(err.to_string()))?;

    outcome_from_response(response)
}

/// Provedor remoto via `brain::llm_provider::build_remote_model` (SSRF + modo offline já aplicados lá).
/// Parâmetros extras em [`remote_vision_params`], num objeto só: duas chamadas separadas de
/// `additional_params` substituiriam uma pela outra, não mesclam.
async fn analyze_remote(store: &ConfigStore, provider: &str, prompt_text: &str, images_base64: &[String]) -> Result<AnalyzeOutcome, VisionLlmError> {
    let mut remote_config = llm_provider::resolve_remote_config(store, provider);
    remote_config.model = resolve_openai_compatible_model(store, remote_config.model);
    let model = llm_provider::build_remote_model(store, provider, &remote_config).map_err(VisionLlmError::Provider)?;
    let message = build_user_message(prompt_text, images_base64);

    let profile = model_profile::resolve(store, provider, &remote_config.model).await;
    let top_p = store.get("llm.topP").and_then(|v| v.as_f64());
    let request = CompletionRequest::new(message)
        .max_tokens(max_output_tokens(store, &profile))
        .additional_params(remote_vision_params(provider, &remote_config.model, top_p, &profile));
    let response = model.call(request).await.map_err(|err| VisionLlmError::Provider(err.to_string()))?;

    outcome_from_response(response)
}

/// Despacha para o provedor efetivo de visão — ver o cabeçalho do
/// arquivo. `caller` só se aplica ao caminho Ollama: `analyze_remote` não passa por
/// `privacy::assert_network_allowed` à parte (`build_remote_model` já aplica SSRF + modo offline ao
/// endpoint digitado pelo usuário, mesmo raciocínio do cabeçalho de `brain::llm_provider`).
/// Entra na fila de fundo do LLM: o watcher de tela não pode ocupar o slot do turno de chat.
async fn analyze_raw(store: &ConfigStore, caller: &str, prompt_text: &str, images_base64: &[String]) -> Result<AnalyzeOutcome, VisionLlmError> {
    crate::modules::brain::turn::as_background_llm_call(|| async {
        match resolve_effective_provider(store) {
            "ollama" => analyze_ollama(store, caller, prompt_text, images_base64).await,
            remote => analyze_remote(store, remote, prompt_text, images_base64).await,
        }
    })
    .await
    .unwrap_or_else(|| Err(VisionLlmError::Provider("análise de tela passou do teto da fila de fundo e foi cancelada".into())))
}

/// Fachada usada pelos handlers sob demanda (`vision_capture`/`vision_read`/clipboard) — só o
/// texto, descarta o uso de tokens. Nenhum desses caminhos tem `AppHandle` disponível para gravar em
/// `token_usage.rs` (ver `mod.rs` §3), então gravar ali exigiria o mesmo obstáculo que o watcher
/// (que TEM `AppHandle`) não tem — ver [`analyze_with_usage`].
pub(super) async fn analyze(store: &ConfigStore, caller: &str, prompt_text: &str, images_base64: &[String]) -> Result<String, VisionLlmError> {
    let images = prepare_images(store, images_base64).await?;
    analyze_raw(store, caller, prompt_text, &images).await.map(|outcome| outcome.text)
}

/// Applies `vision.llmQuality`/`maxImageDimension`/size target to on-demand captures, the same
/// recompression the watcher runs before `analyze_with_usage` — a raw 1920x1080 frame overflowed
/// the local server's batch and cost the most tokens of any call.
pub(super) async fn prepare_images(store: &ConfigStore, images_base64: &[String]) -> Result<Vec<String>, VisionLlmError> {
    let cfg = super::compression::read_compression_config(store);
    let raw = images_base64.to_vec();
    // Seconds of JPEG work per frame: on a runtime worker it starved the bus subscribers, which
    // dropped the turn's ThinkingStop (memory never recorded) and speech chunks.
    let images = tokio::task::spawn_blocking(move || super::compression::compress_images_for_llm(&raw, &cfg))
        .await
        .unwrap_or_else(|error| {
            log::error!("visão: a compressão da captura entrou em pânico: {error}");
            Vec::new()
        });
    if images.is_empty() && !images_base64.is_empty() {
        return Err(VisionLlmError::InvalidImage);
    }
    Ok(images)
}

/// Screen frames -> the LLM's JPEGs in one encode each (`compression::encode_frames_for_llm`), off
/// the runtime like [`prepare_images`]. `capture_quality` caps how high the adaptive quality climbs.
pub(super) async fn prepare_frames(store: &ConfigStore, frames: Vec<image::DynamicImage>, capture_quality: u8) -> Result<Vec<String>, VisionLlmError> {
    let cfg = super::compression::read_compression_config(store);
    let images = tokio::task::spawn_blocking(move || super::compression::encode_frames_for_llm(frames, &cfg, capture_quality))
        .await
        .unwrap_or_else(|error| {
            log::error!("visão: a compressão da captura entrou em pânico: {error}");
            Vec::new()
        });
    if images.is_empty() {
        return Err(VisionLlmError::InvalidImage);
    }
    Ok(images)
}

/// WP9 item 4 — grava o mesmo `token_usage::record(surface="vision")` que `watcher::run_tick` já
/// grava, para as chamadas SOB DEMANDA que chamam o LLM de visão (`screenshot`/`watch_now` de
/// `capture_tool`, `describe`/`ocr` de `read_tool`, `people` de `people_tool`) — antes desta rodada
/// só o laço periódico aparecia no painel de custo, nunca uma pergunta direta ("o que tem na minha
/// tela?"). Compartilhado aqui (não duplicado nos 3 arquivos) porque é o único lugar que já conhece
/// [`resolve_health_key`]/[`AnalyzeOutcome`]. `app: None` (todo teste destes 3 handlers, que não têm
/// `AppHandle` — ver `mod.rs::register`) degrada em silêncio, nunca falha a resposta ao usuário.
pub(super) fn record_usage(store: &ConfigStore, app: Option<&tauri::AppHandle>, outcome: &AnalyzeOutcome) {
    let Some(app) = app else { return };
    if !super::watcher::usage_reported(&outcome.usage) {
        return;
    }
    let (provider, model) = resolve_health_key(store);
    let cost = crate::modules::token_usage::cost_for(app, provider, &model, &outcome.usage);
    crate::modules::token_usage::record(app, "vision", provider, &model, &outcome.usage, cost);
}

/// Mesma chamada, com o uso de tokens exposto — usado por `watcher::run_tick`, que TEM `AppHandle`
/// (injetado em `spawn_screen_watcher`) para gravar em `token_usage::record` (surface `"vision"`).
pub(super) async fn analyze_with_usage(
    store: &ConfigStore,
    caller: &str,
    prompt_text: &str,
    images_base64: &[String],
) -> Result<AnalyzeOutcome, VisionLlmError> {
    analyze_raw(store, caller, prompt_text, images_base64).await
}

#[cfg(test)]
#[path = "llm_client_tests.rs"]
mod tests;
