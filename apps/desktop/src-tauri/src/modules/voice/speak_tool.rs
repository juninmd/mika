//! Tool `speak` — o chamador de produção que faltava para `voice::tts::speaker::speak` (ver o
//! cabeçalho de `tts/mod.rs`, item 2 de "o que ainda falta": "nenhum canal IPC nem tool invoca
//! `speaker::speak`"). Porte estreito de
//! `apps/desktop/src/main/services/voice/handlers/speak.ts::runSpeak` — só a ação `action=speak` da
//! tool `voice` TS (`tools/voice.ts`); as outras quatro ações (`listen_start`/`listen_stop`/
//! `transcribe`/`mute`) dependem de STT/mute, fora do escopo desta rodada. Nome do campo de entrada
//! (`text`) e formato de saída (`{ success, data | error }`) espelham `runSpeak` de propósito: quem
//! consumir esta tool do lado do LLM não vê diferença de contrato.
//!
//! **Isto funciona hoje mesmo sem canal IPC próprio.** `brain::turn::run_turn` (acionado por
//! `mika:chat-send`, já nativo) chama `tools::registry().execute(name, args)` para toda tool que o
//! modelo pede — mesmo registro que este arquivo popula.
//!
//! **Barramento de processo, não isolado.** [`EventBusObserver`]/[`EventBusSink`] recebem
//! `crate::events::bus()` — o mesmo barramento que `VadPipeline`
//! (`voice/vad_pipeline.rs::apply_pending_speech_state`) assina para o barge-in. Um barramento
//! isolado aqui faria a Mika falar sem o barge-in nunca saber.
//!
//! **`TtsCache` é estado de processo.** Um `OnceLock<Mutex<TtsCache>>`, mesmo padrão de
//! `modules/chat/turns.rs::registry()` — reconstruir o cache a cada fala não seria cache nenhum.
//!
//! **`cache_dir` é `<app_data_dir>/supertonic`** — o MESMO caminho que já usa
//! `modules/desktop.rs::prepare_tts` para baixar a voz Supertonic pelo wizard
//! (`data_dir.join("supertonic")`). Inventar outro diretório aqui faria a voz já baixada pelo
//! wizard nunca ser encontrada por `speak()`.
//!
//! **`voice.ttsEnabled` é checado antes de falar** — mesma ordem de `runSpeak.ts`: o toggle de
//! settings precisa controlar a fala de verdade (`AGENTS.md` §3.2, "zero botão morto"), não só
//! desenhar um estado que ninguém lê.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::events::EventBus;
use crate::modules::tools::{ToolDefinition, ToolHandler};
use crate::modules::voice::tts::bus_bridge::{EventBusObserver, EventBusSink};
use crate::modules::voice::tts::cache::TtsCache;
use crate::modules::voice::tts::state::TtsState;
use crate::modules::voice::tts::speaker;

const DESCRIPTION: &str = "Say an extra line aloud (local TTS) outside the normal reply. Never use it to answer: your reply is already spoken, so this would repeat you.";

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "text": {
                "type": "string",
                "description": "Exact words to say aloud."
            }
        },
        "required": ["text"]
    })
}

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "speak".into(),
        description: Some(DESCRIPTION.into()),
        input_schema: schema(),
        plugin_id: "voice".into(),
        requires_window: false,
    }
}

/// Cache de áudio de TTS — estado de processo, não por chamada (ver o cabeçalho do arquivo).
///
/// `pub(crate)`: `voice::voice_tool` (ação `speak` da tool `voice`, TS `tools/voice.ts`) reusa o
/// MESMO cache — dois `OnceLock` independentes fariam a tool `voice` nunca aproveitar (nem
/// invalidar) o que a tool `speak` já sintetizou, e vice-versa.
static TTS_CACHE: OnceLock<Mutex<TtsCache>> = OnceLock::new();

pub(crate) fn cache() -> &'static Mutex<TtsCache> {
    TTS_CACHE.get_or_init(|| Mutex::new(TtsCache::new()))
}

/// Corpo de `SpeakToolHandler::call`, extraído para ser reusado por `voice::voice_tool` (ação
/// `speak`) sem duplicar a checagem de `voice.ttsEnabled` nem a chamada a `speaker::speak` — as
/// duas tools falam com a MESMA voz, pelo MESMO cache, atrás do MESMO toggle.
pub(crate) async fn run_speak(store: &ConfigStore, cache_dir: &PathBuf, bus: &'static EventBus, text: &str) -> Value {
    // Mesma ordem de `runSpeak.ts`: o toggle `voice.ttsEnabled` tem de calar a fala de verdade,
    // não só existir na UI.
    let tts_enabled = store.get("voice.ttsEnabled").and_then(|v| v.as_bool()).unwrap_or(true);
    if !tts_enabled {
        return json!({
            "success": false,
            "error": "voice.ttsEnabled is false. Enable TTS in Voice settings before speaking.",
            "errorCode": "not_configured",
        });
    }

    // O silêncio pedido pela ação `mute` vale para TODA fala, venha da tool, da ação `voice.speak`
    // ou de um `SpeechRequest` do barramento — este é o funil por onde as três passam. Mesma
    // resposta do TS (`tts-speaker.ts`: `{ success: false, error: 'Speech muted' }`).
    if super::tts::speech_mute::is_muted(crate::modules::voice::voice_tool::now_millis()) {
        // Tier humano: sem isto, "a Mika devia ter falado e ficou calada" não tinha explicação
        // nenhuma no log — o pedido só voltava `{ success: false }` pro chamador da tool.
        crate::human_log!(log::Level::Info, "voice: fiquei calada porque o mudo de fala ainda está valendo");
        return json!({ "success": false, "error": "Speech muted", "errorCode": "muted" });
    }

    let state = TtsState::from_config(store);
    let observer = EventBusObserver::new(bus);
    let sink = EventBusSink::new(bus);

    let result = speaker::speak(text, &state, cache(), cache_dir, store, &observer, &sink).await;

    if !result.success {
        return json!({
            "success": false,
            "error": result.error.unwrap_or_else(|| "Unknown error".into()),
        });
    }

    json!({
        "success": true,
        "data": {
            "spoken": true,
            "length": text.trim().chars().count(),
            "provider": state.provider,
            "chunks": result.chunks_spoken,
        }
    })
}

struct SpeakToolHandler {
    store: Arc<ConfigStore>,
    cache_dir: PathBuf,
    bus: &'static EventBus,
}

#[async_trait]
impl ToolHandler for SpeakToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let text = args.get("text").and_then(Value::as_str).unwrap_or("");
        Ok(run_speak(&self.store, &self.cache_dir, self.bus, text).await)
    }
}

/// Boot: com a fala ligada e o Supertonic escolhido, carrega o modelo em segundo plano para o
/// primeiro áudio não esperar por ele. Mesmo `cache_dir` de [`register`].
pub fn prewarm_local_tts(app: &AppHandle) {
    let (Ok(store), Ok(data_dir)) = (crate::state::config_store(app), crate::state::app_data_dir(app)) else {
        return;
    };
    let tts_enabled = store.get("voice.ttsEnabled").and_then(|v| v.as_bool()).unwrap_or(true);
    if tts_enabled && TtsState::from_config(&store).provider == "supertonic" {
        tauri::async_runtime::spawn(super::tts::providers::supertonic::prewarm(data_dir.join("supertonic")));
    }
}

/// Registra a tool `speak` no `ToolRegistry` nativo. Chamada por
/// `modules::register_config_bound_tools(app)` — segunda passada de boot, com `AppHandle` — pelo
/// mesmo motivo de `vision::register`/`scheduler::tool::register`: o handler precisa do
/// `ConfigStore` do processo (`TtsState::from_config`, `voice.ttsEnabled`) e do `AppHandle` para
/// resolver `cache_dir` a partir de `app_data_dir`.
pub fn register(store: Arc<ConfigStore>, app: &AppHandle) -> Result<(), String> {
    let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?;
    let cache_dir = data_dir.join("supertonic");

    crate::modules::tools::registry()
        .register(tool_definition(), Arc::new(SpeakToolHandler { store, cache_dir, bus: crate::events::bus() }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-speak-tool-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Schema real do processo (não um recorte à mão): `voice.ttsEnabled` precisa existir de
    /// verdade para o teste de toggle abaixo gravar um valor válido.
    fn store() -> (Arc<ConfigStore>, TempDir) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        (Arc::new(store), dir)
    }

    fn handler_with(store: Arc<ConfigStore>, dir: &TempDir) -> SpeakToolHandler {
        SpeakToolHandler { store, cache_dir: dir.0.clone(), bus: crate::events::bus() }
    }

    /// O caminho central pedido pela rodada: texto vazio devolve o erro NOMEADO de
    /// `runSpeak.ts`/`speaker::speak`, nunca panic — sem tocar ONNX nenhum.
    #[tokio::test]
    async fn empty_text_returns_the_named_typescript_error_instead_of_panicking() {
        let (store, dir) = store();
        let handler = handler_with(store, &dir);

        let result = handler.call(json!({ "text": "" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["error"], json!("Text cannot be empty"));
    }

    /// Mesmo caminho quando `text` nem chega no payload — `unwrap_or("")` não pode panicar.
    #[tokio::test]
    async fn missing_text_field_is_the_same_named_error_not_a_panic() {
        let (store, dir) = store();
        let handler = handler_with(store, &dir);

        let result = handler.call(json!({})).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["error"], json!("Text cannot be empty"));
    }

    /// `voice.ttsEnabled = false` cala a fala de verdade, antes de qualquer chamada a
    /// `speaker::speak` — o toggle de settings não pode ser um botão morto.
    #[tokio::test]
    async fn tts_disabled_short_circuits_before_reaching_the_speaker() {
        let (store, dir) = store();
        store.set("voice.ttsEnabled", json!(false)).unwrap();
        let handler = handler_with(store, &dir);

        let result = handler.call(json!({ "text": "oi, tudo bem?" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("not_configured"));
    }

    /// A prova de registro pedida pela rodada: a tool aparece no catálogo com o `plugin_id` certo.
    #[tokio::test]
    async fn the_speak_tool_registers_with_the_expected_name_and_plugin_id() {
        let (store, dir) = store();
        crate::modules::tools::registry().register(tool_definition(), Arc::new(handler_with(store, &dir)));

        let registered = crate::modules::tools::registry().get("speak").expect("tool 'speak' devia estar registrada");
        assert_eq!(registered.name, "speak");
        assert_eq!(registered.plugin_id, "voice");
    }
}
