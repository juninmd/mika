//! `mika:plugin-test` — porte parcial de `ipc/settings.ts` (rota `pluginTest`) +
//! `ipc/plugin-connection-test.ts::executePluginTest`. O botão "Validar" de cada aba de Settings.
//!
//! **Este arquivo NÃO reivindica o canal.** `modules/plugins/mod.rs` e `channel_gate.rs` já mediram
//! o porquê: `bridge.rs` roteia por CANAL INTEIRO (`state.native.owns(&channel)`), nunca por
//! `pluginId`/método — reivindicar hoje cortaria o sidecar fora da rota inteira e quebraria os 8
//! plugins tardios (`character`, `mcp`, `user`, `privacy`, `scheduler`, `person-catalog`, `mood`,
//! `gamer-companion`) que hoje respondem via `plugin.definition.testConnection` real. Só `desktop`
//! não declara — o denominador do ramo genérico é 8, não 9.
//!
//! O que este arquivo entrega, sem reivindicar nada: o registro `pluginId → TestConnection` (mesmo
//! formato de `modules/plugins/rpc.rs::RpcRegistry` — `OnceLock`, `register`; o numerador contra os
//! 8 é o número de entradas) e os TRÊS ramos que `executePluginTest` resolve embutidos, ANTES do ramo
//! genérico: `mika.voice` (voz), `llm`, `mika.embeddings`. Cada feature portada registra o próprio
//! teste aqui; quando o registro alcançar 8 entradas, o ramo genérico fecha e outra rodada
//! decide se reivindica o canal.
//!
//! ## Fidelidade medida, não assumida
//!
//! - **Fallback de `pluginId`** (`ipc/settings.ts`, rota `pluginTest`): `payload.pluginId ||
//!   payload.tabId`, e sem nenhum dos dois a rota devolve `{ ok: false, message: "Nenhum plugin
//!   especificado para teste." }` ANTES de chamar `executePluginTest` — reproduzido em [`dispatch`].
//! - **`mika.voice`**: `testVoiceConnection` roteia por `testType==='stt'` (setado quando
//!   `tabId==='stt'`) ou por `voice.ttsProvider`. Só dois caminhos são reais em Rust sem rede: STT
//!   `electron`/`local-whisper` (mensagem fixa, sem I/O) e `browser` (mensagem fixa), mais TTS
//!   `supertonic` (`voice/tts/dispatch.rs::generate_with_provider`, gera áudio de verdade) e o
//!   pseudo-provedor `system` (TS devolve `ok:true` sem gerar áudio — reproduzido aqui do MESMO jeito,
//!   não é mock, é o comportamento real documentado em `voice-test.ts`). **STT `groq`/`openai`
//!   deixou de degradar em 2026-09-22**: `voice/stt_cloud.rs::validate_credentials` faz o mesmo GET
//!   em `/v1/models` do TS, e é ela que conhece endpoint e caminho da chave — esta tela não. Os
//!   provedores de TTS que ainda estiverem em `voice/tts/dispatch.rs::CUT_PROVIDERS` continuam
//!   degradando nomeado.
//! - **`llm`**: `testLlmConnection` (TS) resolve QUALQUER provider configurado, e **o Rust também
//!   resolve desde 2026-09-22** — `brain::turn::run_turn_llm_raw` cobre `ollama`, `google`,
//!   `openrouter`, `openaiCompatible` e `unsloth` (os dois últimos só em loopback, pela mesma guarda de SSRF
//!   fail-closed do catálogo). Chama a fachada de propósito, em vez de escolher o cliente aqui: a
//!   tela que TESTA a conexão tem de resolver o provedor pelo mesmo caminho da CONVERSA, senão ela
//!   dá verde num provedor que o chat não usa. Erro do provedor passa por [`mask_secret`] antes de
//!   sair, e a query de qualquer URL é cortada junto — a chave do Google vai por query string.
//! - **`mika.embeddings`**: os 5 provedores têm cliente real desde 2026-09-24
//!   (`memory::episodic::test_embedding`, reexportado pra fora do submódulo privado
//!   `memory::episodic::embedding` só para este chamador) — mesma tabela `resolve_embedding_target`
//!   que a produção (`generate_embedding`) usa, para a tela nunca dar verde num provider que a
//!   memória não usaria.
//!
//! Guarda de superfície: `assertOwnerSurface(context, 'pluginTest')` no TS libera só
//! `main`/`settings`/`wizard` — reuso de `super::handlers::assert_owner_surface`, já testado ali,
//! em vez de duplicar a regra.

use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

use async_trait::async_trait;
use base64::Engine as _;
use serde_json::{json, Value};

use super::test_redaction::mask_secret;
use crate::config::store::ConfigStore;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortHandle};
use crate::modules::voice::tts::abort_registry;
use crate::modules::voice::tts::dispatch as tts_dispatch;
use crate::modules::voice::tts::state::TtsState;
use crate::protocol::InvokeContext;

const DEFAULT_WHISPER_MODEL: &str = "onnx-community/whisper-base";
const LLM_DEFAULT_PROMPT: &str = "Responda apenas com MIKA_OK para validar a conexao LLM.";
const TTS_TEST_PHRASE: &str = "Oi! Esta é a minha voz, gerada aqui na sua máquina.";

/// Contraparte de `PluginDefinition.testConnection` — cada feature tardia registra a própria
/// implementação aqui, com as dependências (config, etc.) já capturadas no construtor, mesmo padrão
/// de `ContentHandler` (`brain/tools/content.rs`) e de `RpcHandler` (`plugins/rpc.rs`).
#[async_trait]
pub trait TestConnection: Send + Sync {
    async fn call(&self, prompt: Option<&str>) -> Value;
}

#[derive(Default)]
pub struct TestRegistry {
    handlers: RwLock<std::collections::HashMap<String, Arc<dyn TestConnection>>>,
}

impl TestRegistry {
    pub fn register(&self, plugin_id: &str, handler: Arc<dyn TestConnection>) {
        let mut handlers = self.handlers.write().unwrap_or_else(|e| e.into_inner());
        if handlers.insert(plugin_id.to_string(), handler).is_some() {
            log::warn!("plugin test handler replaced: {plugin_id}");
        }
    }

    fn get(&self, plugin_id: &str) -> Option<Arc<dyn TestConnection>> {
        self.handlers.read().unwrap_or_else(|e| e.into_inner()).get(plugin_id).cloned()
    }
}

static REGISTRY: OnceLock<TestRegistry> = OnceLock::new();

pub fn registry() -> &'static TestRegistry {
    REGISTRY.get_or_init(TestRegistry::default)
}

fn named_ok(message: String) -> Value {
    json!({ "ok": true, "message": message })
}

fn named_fail(message: String) -> Value {
    json!({ "ok": false, "message": message })
}

/// Espelha `path in combinedConfig ? combinedConfig[path] : store.get(path)` — transient config
/// (edição não salva na tela) vence, config persistida é o fallback.
fn cfg_str(transient: &Value, store: &ConfigStore, path: &str) -> Option<String> {
    if let Some(value) = transient.get(path) {
        return value.as_str().map(str::to_string);
    }
    store.get(path).and_then(|v| v.as_str().map(str::to_string))
}

/// Par endereço↔credencial: a chave guardada só acompanha o endereço guardado. Endereço vindo da
/// tela leva, no máximo, a chave que veio junto com ele — nunca a do store.
fn endpoint_and_key(transient: &Value, store: &ConfigStore, url_path: &str, key_path: &str) -> (Option<String>, String) {
    if transient.get(url_path).is_some() {
        let key = transient.get(key_path).and_then(Value::as_str).unwrap_or_default();
        return (cfg_str(transient, store, url_path), key.to_string());
    }
    (cfg_str(transient, store, url_path), cfg_str(&Value::Null, store, key_path).unwrap_or_default())
}

// === mika.voice ==================================================================================

/// Virou `async` em 2026-09-22, quando o STT em nuvem passou a transcrever de verdade
/// (`voice/stt_cloud.rs`). Antes disso este ramo devolvia "ainda não foi portado" para groq/openai
/// — e teria continuado devolvendo, com a transcrição já funcionando: **o botão de testar mentiria
/// sobre a feature que o app acabara de ganhar.** Quem pega esse tipo de defeito não é o gate, é a
/// frente que porta a peça avisando que o teste dela ficou para trás.
async fn test_stt_provider(store: &ConfigStore, combined: &Value) -> Value {
    let provider = cfg_str(combined, store, "asrProvider")
        .or_else(|| cfg_str(combined, store, "voice.asrProvider"))
        .unwrap_or_else(|| "nemotron".to_string());

    match provider.as_str() {
        "nemotron" => named_ok("Nemotron local roda dentro do app. Na primeira vez, baixe o modelo (~790 MB) no card de reconhecimento de voz.".to_string()),
        "electron" | "local-whisper" => {
            let model = cfg_str(combined, store, "whisperModel")
                .or_else(|| cfg_str(combined, store, "voice.whisperModel"))
                .unwrap_or_else(|| DEFAULT_WHISPER_MODEL.to_string());
            named_ok(format!("Whisper local ({model}) roda dentro do app. O modelo é baixado na primeira transcrição."))
        }
        "browser" => named_ok("Reconhecimento de fala via navegador (Web Speech API) ativo.".to_string()),
        // `validate_credentials` devolve `None` para id que não seja de nuvem, e é ela quem sabe
        // endpoint e caminho de chave — esta tela nunca aprende a URL do provedor.
        other => match crate::modules::voice::stt_cloud::validate_credentials(store, other).await {
            Some(result) => result,
            None => named_fail(format!("Provedor STT \"{other}\" não reconhecido.")),
        },
    }
}

async fn test_tts_connection(store: &ConfigStore, cache_dir: &Path, combined: &Value) -> Value {
    let provider = cfg_str(combined, store, "voice.ttsProvider")
        .or_else(|| cfg_str(combined, store, "ttsProvider"))
        .unwrap_or_else(|| "supertonic".to_string());

    // Igual ao TS: `system` é um provedor real (fala silenciada de propósito), não um corte.
    if provider == "system" {
        return named_ok("Modo Sistema ativado: sintetizador de voz silenciado por configuração (não gera áudio).".to_string());
    }

    let mut state = TtsState::from_config(store);
    state.provider = provider.clone();
    if let Some(voice) = cfg_str(combined, store, "voice.supertonicVoice").or_else(|| cfg_str(combined, store, "supertonicVoice")) {
        state.supertonic_voice = voice;
    }

    let guard = abort_registry::global().register_active();
    let cancel = guard.handle();
    let result = tts_dispatch::generate_with_provider(&state, TTS_TEST_PHRASE, cache_dir, store, &cancel).await;
    drop(guard);

    match result {
        // `provider != "system"` está garantido pelo `return` antecipado acima — nenhum outro
        // provedor devolve `Ok(None)` (ver o cabeçalho de `dispatch.rs`), mas o tipo exige o ramo.
        Ok(None) => named_fail("provedor não gerou áudio".to_string()),
        Ok(Some(audio)) => json!({
            "ok": true,
            "message": "Voz local (Supertonic) gerada com sucesso — tudo rodando na sua máquina!",
            "audio": base64::engine::general_purpose::STANDARD.encode(&audio.audio),
        }),
        Err(err) if provider == "supertonic" => named_fail(format!("A síntese local (Supertonic) falhou: {err}")),
        // Provedores cortados/desconhecidos: `generate_with_provider` já nomeia a causa.
        Err(err) => named_fail(err),
    }
}

async fn test_voice_connection(store: &ConfigStore, cache_dir: &Path, tab_id: Option<&str>, transient: &Value) -> Value {
    let mut combined = transient.clone();
    if tab_id == Some("stt") {
        match &mut combined {
            Value::Object(map) => {
                map.insert("testType".to_string(), json!("stt"));
            }
            _ => combined = json!({ "testType": "stt" }),
        }
    }

    if combined.get("testType").and_then(Value::as_str) == Some("stt") {
        test_stt_provider(store, &combined).await
    } else {
        test_tts_connection(store, cache_dir, &combined).await
    }
}

// === llm ==========================================================================================

/// Deixou de ser Ollama-only em 2026-09-22, no mesmo dia em que `chat/send.rs` passou a resolver os
/// 4 provedores. Antes disto o ramo recusava qualquer provedor != `ollama` — e teria continuado
/// recusando com a conversa já rodando em Gemini. É o mesmo defeito que o botão de STT teve nesta
/// rodada: **o teste de uma feature envelhece separado dela.**
///
/// Chama `run_turn_llm_raw`, que resolve o provedor pelo MESMO caminho da conversa. Se esta tela
/// resolvesse por conta própria, o botão poderia dar verde num provedor que o chat não usa.
async fn test_llm_connection(store: &ConfigStore, prompt: Option<&str>, transient: &Value) -> Value {
    let provider = cfg_str(transient, store, "llm.provider").or_else(|| cfg_str(transient, store, "provider")).unwrap_or_else(|| "ollama".to_string());

    // Espelha `prompt || 'Responda...'` do TS: só string vazia/ausente é falsy, sem trim — um
    // prompt custom em branco ainda é "fornecido pelo usuário", não o default.
    let has_custom_prompt = prompt.map(|p| !p.is_empty()).unwrap_or(false);
    let user_content = if has_custom_prompt { prompt.unwrap_or(LLM_DEFAULT_PROMPT) } else { LLM_DEFAULT_PROMPT };
    let (_handle, signal) = TurnAbortHandle::new();

    // Primeiro plano: o botão do usuário não entra na fila atrás de visão/reflexão/aprendizado.
    match turn::run_as_foreground(turn::run_turn_llm_raw(store, None, user_content, signal, &NullObserver)).await {
        Ok(result) => {
            let normalized = result.text.trim().to_string();
            if !has_custom_prompt && !normalized.to_uppercase().contains("MIKA_OK") {
                return named_fail(mask_secret(format!("LLM respondeu fora do contrato ({provider}). Resposta: {normalized}"), store));
            }
            named_ok(normalized)
        }
        Err(err) => named_fail(mask_secret(format!("Falha ao validar LLM: {err}"), store)),
    }
}

// === mika.embeddings ==============================================================================

/// Os 4 provedores têm cliente real desde 2026-09-24 (`memory/episodic/embedding.rs::test_embedding`,
/// mesmo transporte que `generate_embedding` usa em produção, escolhido pela MESMA tabela
/// `resolve_embedding_target` — a tela nunca resolve o provedor por conta própria). `endpoint_and_key`
/// resolve `base_url`/`api_key` pelo par correto de cada provider (nunca a chave salva de um provider
/// diferente do endereço pedido — "par assimétrico endereço↔credencial"). Passa pelo mesmo chokepoint
/// de privacidade do resto do processo (`assert_network_allowed`) ANTES de qualquer rede, quando o
/// provider tem endereço fixo (Google/OpenRouter) o gate é contra essa URL fixa — nunca loopback,
/// então modo offline sempre bloqueia os dois, igual à produção.
async fn test_embedding_connection(store: &ConfigStore, prompt: Option<&str>, transient: &Value) -> Value {
    let provider = cfg_str(transient, store, "llm.embeddingProvider").unwrap_or_else(|| "local".to_string());
    let model = cfg_str(transient, store, "llm.embeddingModel").unwrap_or_default();

    let (base_url, api_key, guard_url) = match provider.as_str() {
        // In-process e5: loopback guard, same as `resolve_embedding_target`, so offline mode allows it.
        "local" => (String::new(), String::new(), "http://127.0.0.1".to_string()),
        "ollama" => {
            let (base_url, api_key) = endpoint_and_key(transient, store, "apiKeys.ollamaBaseUrl", "apiKeys.ollamaApiKey");
            let base_url = base_url.filter(|s| !s.is_empty()).unwrap_or_else(|| crate::config::schemas::llm_defaults::OLLAMA_BASE_URL.to_string());
            let guard_url = base_url.clone();
            (base_url, api_key, guard_url)
        }
        "google" => {
            let api_key = cfg_str(transient, store, "apiKeys.googleAI").unwrap_or_default();
            (String::new(), api_key, crate::config::schemas::provider_endpoints::GOOGLE_AI_ORIGIN.to_string())
        }
        "openrouter" => {
            let api_key = cfg_str(transient, store, "apiKeys.openrouter").unwrap_or_default();
            (String::new(), api_key, crate::config::schemas::provider_endpoints::OPENROUTER_ORIGIN.to_string())
        }
        "openaiCompatible" => {
            let (base_url, api_key) = endpoint_and_key(transient, store, "apiKeys.openaiCompatibleBaseUrl", "apiKeys.openaiCompatibleApiKey");
            let base_url = base_url.unwrap_or_default();
            if base_url.is_empty() {
                return named_fail("OpenAI-compatible: baseUrl não configurado".to_string());
            }
            let guard_url = base_url.clone();
            (base_url, api_key, guard_url)
        }
        "unsloth" => {
            let (base_url, api_key) = endpoint_and_key(transient, store, "apiKeys.unslothBaseUrl", "apiKeys.unslothApiKey");
            let base_url = base_url.filter(|s| !s.is_empty()).unwrap_or_else(|| crate::modules::llm::UNSLOTH_DEFAULT_BASE_URL.to_string());
            let guard_url = base_url.clone();
            (base_url, api_key, guard_url)
        }
        other => return named_fail(format!("Provedor de embeddings \"{other}\" não reconhecido.")),
    };

    if let Err(err) = crate::modules::privacy::assert_network_allowed(store, &guard_url, "settings:test:embeddings") {
        return named_fail(err);
    }

    let has_custom_prompt = prompt.map(|p| !p.is_empty()).unwrap_or(false);
    let content = if has_custom_prompt { prompt.unwrap_or_default() } else { "teste de conexão de embeddings da Mika" };

    match crate::modules::memory::episodic::test_embedding(&provider, &base_url, &api_key, &model, content).await {
        Ok(vector) => named_ok(format!("Embedding gerado com {} dimensões.", vector.len())),
        Err(err) => named_fail(mask_secret(format!("Falha ao validar embeddings: {err}"), store)),
    }
}

// === despacho ======================================================================================

/// Porte de `ipc/settings.ts` (rota `pluginTest`) + `executePluginTest`. `cache_dir` já resolvido
/// pelo chamador (mesmo padrão de `speak_tool::run_speak` — `AppHandle` só existe na borda do boot,
/// nunca aqui, para o caminho continuar testável sem montar um app Tauri).
pub async fn dispatch(store: &ConfigStore, ctx: &InvokeContext, cache_dir: &Path, payload: &Value) -> Result<Value, String> {
    super::handlers::assert_owner_surface(ctx, "pluginTest")?;

    let raw_plugin_id = payload.get("pluginId").and_then(Value::as_str).filter(|s| !s.is_empty());
    let tab_id = payload.get("tabId").and_then(Value::as_str).filter(|s| !s.is_empty());
    let prompt = payload.get("prompt").and_then(Value::as_str);
    let transient = payload.get("config").cloned().unwrap_or_else(|| json!({}));

    let Some(plugin_id) = raw_plugin_id.or(tab_id) else {
        return Ok(named_fail("Nenhum plugin especificado para teste.".to_string()));
    };

    if plugin_id == "mika.voice" {
        return Ok(test_voice_connection(store, cache_dir, tab_id, &transient).await);
    }
    if plugin_id == "llm" || tab_id == Some("llm") {
        return Ok(test_llm_connection(store, prompt, &transient).await);
    }
    if plugin_id == "mika.embeddings" || tab_id == Some("embeddings") {
        return Ok(test_embedding_connection(store, prompt, &transient).await);
    }

    match registry().get(plugin_id) {
        Some(handler) => Ok(handler.call(prompt).await),
        None => Ok(named_fail(format!("Plugin \"{plugin_id}\" não tem teste de conexão portado para Rust ainda."))),
    }
}

#[cfg(test)]
#[path = "plugin_test_tests.rs"]
mod tests;
