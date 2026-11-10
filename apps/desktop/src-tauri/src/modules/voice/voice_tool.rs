//! Tool `voice` (LLM) — port PARCIAL de `apps/desktop/src/main/services/voice/tools/voice.ts`.
//! Não confundir com a tool `speak` ([`super::speak_tool`]): aquela é um atalho de produção só
//! para `action=speak`, esta é a tool INTEIRA que o catálogo TS declara (`voice: voiceTool`,
//! `voice.feature.ts`), com as 5 ações (`speak`/`listen_start`/`listen_stop`/`transcribe`/`mute`).
//!
//! ## Tabela TS × Rust (medida nesta rodada)
//!
//! | Ação | Depende de | Rust hoje |
//! |---|---|---|
//! | `speak` | `speaker::speak` (Supertonic) | **portado** — reusa [`super::speak_tool::run_speak`], MESMO cache/config/barramento da tool `speak` |
//! | `transcribe` | `inference::whisper::transcribe` | **portado, com uma divergência de FORMATO** — reusa [`super::transcribe::transcribe_pcm`], que só aceita PCM16 cru (`audio/pcm*`). O schema TS anuncia `audio/mpeg`/`mp3`/`wav`/`ogg`/`webm`/`mp4` (decodificados no lado Node antes do STT); o backend Rust de hoje não decodifica contêiner nenhum, só PCM cru — qualquer `mimeType` fora de `audio/pcm*` recebe o erro NOMEADO de `non_pcm_response`, nunca um resultado inventado. |
//! | `mute` | `EventKey::MuteSpeech` + quem o lê | **portado, e agora com efeito** — publica o mesmo payload de `runMute`, e desde 2026-09-22 `tts::bus_bridge` assina a chave e alimenta `tts::speech_mute`, que `speak_tool::run_speak` consulta antes de sintetizar. Até então o evento morria no barramento: mandar calar devolvia sucesso e a Mika continuava falando |
//! | `listen_start` / `listen_stop` | `sttPort()` — o FLAG `STTService.listening`/`startedAt` | **portado** — ver [`super::listening_state`] (o estado) e [`super::listen_actions`] (o porte de `runStartListening`/`runStopListening`): o TS nunca inicia um laço de captura aqui, só alterna o flag e publica `voice:listening-started`/`-stopped`; a captura real é 100% do renderer e já independe deste flag |
//!
//! `plugin_id` é `"mika.voice"` — o `manifest.id` de `apps/desktop/src/main/services/voice/index.ts`,
//! não o `"voice"` que a tool `speak` usa (peça isolada, registrada antes desta tool existir).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::events::{bus, EventBus, EventKey};
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::listen_actions;
use super::listening_state::ListeningState;
use super::speak_tool;
use super::transcribe::{self, TranscribeRequest};

const DESCRIPTION: &str = "Voice and microphone controls. action=mute: go quiet for `minutes` (0 = talk again) when the user asks for silence or focus. action=listen_start / listen_stop: continuous microphone listening on/off. action=transcribe: `audioBase64` PCM16 mono 16 kHz only. action=speak: say `text` outside your reply — rarely needed, your reply is already spoken.";

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "voice".into(),
        description: Some(DESCRIPTION.into()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["speak", "listen_start", "listen_stop", "transcribe", "mute"] },
                "text": { "type": "string", "minLength": 1, "maxLength": 2000 },
                "audioBase64": { "type": "string" },
                "mimeType": {
                    "type": "string",
                    "enum": ["audio/mpeg", "audio/mp3", "audio/wav", "audio/ogg", "audio/webm", "audio/mp4"],
                },
                "language": { "type": "string", "description": "action=transcribe: language hint, e.g. \"pt\"." },
                "minutes": { "type": "integer", "minimum": 0, "maximum": 240 },
            },
            "required": ["action"],
        }),
        plugin_id: "mika.voice".into(),
        requires_window: false,
    }
}

/// `pub(super)` porque `listen_actions.rs` reusa o mesmo relógio para `at`/`startedAt`.
pub(super) fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Espelha `runMute`: `minutes <= 0` desliga o silêncio; `minutes > 0` publica o prazo em
/// `untilMs`. Publica SEMPRE — `MuteSpeech` tolera perda (ver `events/keys.rs`), mas nunca deixa
/// de tentar.
fn run_mute(bus: &'static EventBus, minutes: i64) -> Value {
    let minutes = minutes.clamp(0, 240);
    if minutes > 0 {
        let until_ms = now_millis() + minutes * 60_000;
        bus.publish(EventKey::MuteSpeech, json!({ "muted": true, "untilMs": until_ms }));
        json!({ "success": true, "data": { "muted": true, "minutes": minutes, "untilMs": until_ms } })
    } else {
        bus.publish(EventKey::MuteSpeech, json!({ "muted": false }));
        json!({ "success": true, "data": { "muted": false, "minutes": minutes, "untilMs": null } })
    }
}

struct VoiceToolHandler {
    store: Arc<ConfigStore>,
    cache_dir: PathBuf,
    data_dir: PathBuf,
    bus: &'static EventBus,
    /// `&'static`: em produção é `listening_state::global()`, o MESMO estado que
    /// `interruption_transcript.rs` agora lê para pular a transcrição do barge-in quando o STT
    /// contínuo já está ouvindo (ver o cabeçalho de `listening_state.rs`). Testes usam
    /// `Box::leak(Box::new(ListeningState::new()))` para um estado ISOLADO por teste — mesmo
    /// padrão de `EventBus` em `tts/bus_bridge.rs`.
    listening: &'static ListeningState,
}

#[async_trait]
impl ToolHandler for VoiceToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        match action {
            "speak" => {
                let text = args.get("text").and_then(Value::as_str).unwrap_or("").trim();
                if text.is_empty() {
                    return Ok(json!({ "success": false, "error": "Text cannot be empty", "errorCode": "invalid_args" }));
                }
                Ok(speak_tool::run_speak(&self.store, &self.cache_dir, self.bus, text).await)
            }
            "transcribe" => {
                let audio_base64 = args.get("audioBase64").and_then(Value::as_str).filter(|s| !s.is_empty());
                let Some(audio_base64) = audio_base64 else {
                    return Ok(json!({
                        "success": false,
                        "error": "audioBase64 is required for action=transcribe.",
                        "errorCode": "invalid_args",
                    }));
                };
                let req = TranscribeRequest {
                    audio_base64: audio_base64.to_string(),
                    mime_type: args.get("mimeType").and_then(Value::as_str).map(str::to_string),
                    language: args.get("language").and_then(Value::as_str).map(str::to_string),
                };
                Ok(transcribe::transcribe_pcm(&self.store, self.data_dir.clone(), req).await)
            }
            "mute" => {
                let minutes = args.get("minutes").and_then(Value::as_i64).unwrap_or(15);
                Ok(run_mute(self.bus, minutes))
            }
            "listen_start" => Ok(listen_actions::run_listen_start(&self.store, self.bus, self.listening)),
            "listen_stop" => Ok(listen_actions::run_listen_stop(&self.store, self.bus, self.listening)),
            other => Err(format!("voice: unknown action \"{other}\"")),
        }
    }
}

/// Chamado por `modules::register_config_bound_tools(app)`. Precisa do `AppHandle` para resolver
/// `cache_dir` (voz sintetizada, MESMO diretório da tool `speak`) e `data_dir` (Whisper).
pub fn register(store: Arc<ConfigStore>, app: &AppHandle) -> Result<(), String> {
    let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("could not resolve app_data_dir: {e}"))?;
    let cache_dir = data_dir.join("supertonic");

    crate::modules::tools::registry().register(
        tool_definition(),
        Arc::new(VoiceToolHandler { store, cache_dir, data_dir, bus: bus(), listening: super::listening_state::global() }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler(store: Arc<ConfigStore>, dir: &std::path::Path) -> VoiceToolHandler {
        VoiceToolHandler {
            store,
            cache_dir: dir.join("supertonic"),
            data_dir: dir.to_path_buf(),
            bus: bus(),
            // Isolado por chamada de teste — `listening_state::global()` compartilharia estado
            // entre testes rodando em paralelo (ver o cabeçalho de `listening_state.rs`).
            listening: Box::leak(Box::new(ListeningState::new())),
        }
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-voice-tool-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (Arc<ConfigStore>, TempDir) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        (Arc::new(store), dir)
    }

    #[tokio::test]
    async fn speak_with_empty_text_is_a_named_invalid_args_error_not_a_panic() {
        let (store, dir) = store();
        let result = handler(store, &dir.0).call(json!({ "action": "speak", "text": "  " })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    #[tokio::test]
    async fn transcribe_without_audio_base64_is_a_named_invalid_args_error() {
        let (store, dir) = store();
        let result = handler(store, &dir.0).call(json!({ "action": "transcribe" })).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    /// Divergência documentada no cabeçalho do arquivo: mimeType fora de `audio/pcm*` é recusado
    /// NOMEADO, nunca transcrito com um resultado inventado.
    #[tokio::test]
    async fn transcribe_with_a_non_pcm_mime_type_names_the_gap_instead_of_faking_a_result() {
        let (store, dir) = store();
        let result = handler(store, &dir.0)
            .call(json!({ "action": "transcribe", "audioBase64": "AAA=", "mimeType": "audio/mpeg" }))
            .await
            .unwrap();
        assert_eq!(result["success"], json!(false));
        assert!(result["error"].as_str().unwrap().contains("audio/mpeg"));
    }

    #[tokio::test]
    async fn mute_with_positive_minutes_publishes_muted_true_with_a_deadline() {
        let (store, dir) = store();
        let mut sub = bus().subscribe();

        let result = handler(store, &dir.0).call(json!({ "action": "mute", "minutes": 30 })).await.unwrap();
        assert_eq!(result["data"]["muted"], json!(true));
        assert_eq!(result["data"]["minutes"], json!(30));

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        let envelope = loop {
            let envelope = tokio::time::timeout_at(deadline, sub.recv()).await.expect("MuteSpeech não publicado").unwrap();
            if envelope.key == EventKey::MuteSpeech {
                break envelope;
            }
        };
        assert_eq!(envelope.payload["muted"], json!(true));
    }

    #[tokio::test]
    async fn mute_with_zero_minutes_publishes_muted_false() {
        let (store, dir) = store();
        let result = handler(store, &dir.0).call(json!({ "action": "mute", "minutes": 0 })).await.unwrap();
        assert_eq!(result["data"]["muted"], json!(false));
    }

    #[tokio::test]
    async fn mute_without_minutes_defaults_to_fifteen() {
        let (store, dir) = store();
        let result = handler(store, &dir.0).call(json!({ "action": "mute" })).await.unwrap();
        assert_eq!(result["data"]["minutes"], json!(15));
    }

    /// Prova de FIAÇÃO ponta a ponta: o dispatch da tool (`action` -> `listen_actions::run_*`)
    /// chega no MESMO `ListeningState` nas duas chamadas (persistido no campo do handler, não um
    /// novo por invocação) e no config real (`voice.sttEnabled=false` recusa `listen_start`). A
    /// cobertura de contrato/idempotência/concorrência de cada ação já mora em
    /// `listen_actions.rs`, que testa `run_listen_start`/`run_listen_stop` diretamente — duplicar
    /// aqui só inflaria este arquivo sem provar nada a mais.
    #[tokio::test]
    async fn listen_start_and_listen_stop_dispatch_for_real_through_the_tool() {
        let (store, dir) = store();
        let h = handler(store, &dir.0);

        let started = h.call(json!({ "action": "listen_start" })).await.unwrap();
        assert_eq!(started["success"], json!(true));
        assert_eq!(started["data"]["listening"], json!(true));

        let stopped = h.call(json!({ "action": "listen_stop" })).await.unwrap();
        assert_eq!(stopped["success"], json!(true));
        assert_eq!(stopped["data"]["stopped"], json!(true));
        assert_eq!(stopped["data"]["listening"], json!(false));
    }

    #[tokio::test]
    async fn listen_start_with_stt_disabled_is_a_named_not_configured_error_through_the_tool() {
        let (store, dir) = store();
        store.set("voice.sttEnabled", json!(false)).unwrap();
        let result = handler(store, &dir.0).call(json!({ "action": "listen_start" })).await.unwrap();

        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("not_configured"));
    }

    /// `register` precisa de `AppHandle` de verdade (resolve `app_data_dir`), que não dá para
    /// forjar num teste de unidade sem subir o Tauri — mesma limitação documentada em
    /// `speak_tool.rs`/`transcribe.rs`. A prova de registro em si já é coberta por
    /// `the_speak_tool_registers_with_the_expected_name_and_plugin_id` (`speak_tool.rs`) no MESMO
    /// padrão; aqui só provamos que a definição da tool está correta sem precisar de `AppHandle`.
    #[test]
    fn tool_definition_has_the_expected_name_and_plugin_id() {
        let definition = tool_definition();
        assert_eq!(definition.name, "voice");
        assert_eq!(definition.plugin_id, "mika.voice");
    }
}
