//! `run_listen_start`/`run_listen_stop` — porte de
//! `apps/desktop/src/main/services/voice/handlers/{start,stop}-listening.ts`, extraído de
//! `voice_tool.rs` para manter o dono do dispatch abaixo do teto de 300 linhas (mesma regra que
//! `mod.rs` já cita para `vad_audio.rs`). Ver o cabeçalho de `listening_state.rs` para o porquê de
//! nenhuma das duas ações iniciar um laço de captura de áudio.

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::events::{EventBus, EventKey};

use super::listening_state::ListeningState;
use super::voice_tool::now_millis;

/// Mesmos caminhos/defaults de `start-listening.ts`/`stop-listening.ts` (`?? true`, `??
/// 'continuous'`, `?? 0.5`) — `ConfigStore::open` já mescla o schema sobre o disco
/// (`config/schemas/voice.rs`: `sttEnabled=true`, `inputMode="continuous"`,
/// `voiceActivityThreshold=0.5`), então o fallback aqui só cobre um `get` que retorne `None`.
fn stt_enabled(store: &ConfigStore) -> bool {
    store.get("voice.sttEnabled").and_then(|v| v.as_bool()).unwrap_or(true)
}

fn input_mode(store: &ConfigStore) -> String {
    store
        .get("voice.inputMode")
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "continuous".to_string())
}

fn voice_activity_threshold(store: &ConfigStore) -> f64 {
    store.get("voice.voiceActivityThreshold").and_then(|v| v.as_f64()).unwrap_or(0.5)
}

/// Porte de `runStartListening`: mesma checagem `voice.sttEnabled`, mesmo `errorCode`/mensagem em
/// caso de recusa, e o MESMO payload de evento (`at`/`restarted`/`inputMode`/`threshold`) —
/// `restarted` é `!started`, não um segundo campo independente, igual ao TS (`!result.started`).
pub fn run_listen_start(store: &ConfigStore, bus: &EventBus, state: &ListeningState) -> Value {
    if !stt_enabled(store) {
        return json!({
            "success": false,
            "error": "voice.sttEnabled is false. Enable STT in Voice settings before listening.",
            "errorCode": "not_configured",
        });
    }

    let mode = input_mode(store);
    let threshold = voice_activity_threshold(store);
    let outcome = state.start();

    bus.publish(
        EventKey::ListeningStarted,
        json!({
            "at": outcome.started_at_ms,
            "restarted": !outcome.started,
            "inputMode": mode,
            "threshold": threshold,
        }),
    );

    json!({
        "success": true,
        "data": {
            "listening": state.is_listening(),
            "inputMode": mode,
            "threshold": threshold,
            "started": outcome.started,
            "startedAt": outcome.started_at_ms,
        },
    })
}

/// Porte de `runStopListening`: sem checagem de `sttEnabled` (o TS também não checa no stop —
/// parar é sempre permitido), idempotente, mesmo payload de evento.
pub fn run_listen_stop(store: &ConfigStore, bus: &EventBus, state: &ListeningState) -> Value {
    let mode = input_mode(store);
    let outcome = state.stop();

    bus.publish(
        EventKey::ListeningStopped,
        json!({
            "at": now_millis(),
            "inputMode": mode,
            "stopped": outcome.stopped,
            "durationMs": outcome.duration_ms,
        }),
    );

    json!({
        "success": true,
        "data": {
            "listening": state.is_listening(),
            "inputMode": mode,
            "stopped": outcome.stopped,
            "durationMs": outcome.duration_ms,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-listen-actions-test-{}-{unique}", std::process::id()));
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

    #[test]
    fn listen_start_with_stt_disabled_is_a_named_not_configured_error() {
        let (store, _dir) = store();
        store.set("voice.sttEnabled", json!(false)).unwrap();
        let bus = EventBus::new();
        let state = ListeningState::new();

        let result = run_listen_start(&store, &bus, &state);
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("not_configured"));
        assert!(!state.is_listening(), "recusado não pode deixar o flag ligado");
    }

    #[test]
    fn listen_start_with_stt_enabled_reports_listening_true_with_config_echoed() {
        let (store, _dir) = store();
        let bus = EventBus::new();
        let state = ListeningState::new();

        let result = run_listen_start(&store, &bus, &state);
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["listening"], json!(true));
        assert_eq!(result["data"]["inputMode"], json!("continuous"));
        assert_eq!(result["data"]["threshold"], json!(0.5));
        assert_eq!(result["data"]["started"], json!(true));
    }

    /// Idempotência: um segundo `listen_start` enquanto já ouvindo é `started:false` — mesmo
    /// contrato de `STTService.start()`, nunca um segundo "começo".
    #[test]
    fn a_second_listen_start_while_already_listening_reports_restarted_not_a_new_start() {
        let (store, _dir) = store();
        let bus = EventBus::new();
        let state = ListeningState::new();

        let first = run_listen_start(&store, &bus, &state);
        let second = run_listen_start(&store, &bus, &state);

        assert_eq!(first["data"]["started"], json!(true));
        assert_eq!(second["data"]["started"], json!(false));
        assert_eq!(second["data"]["startedAt"], first["data"]["startedAt"]);
        assert_eq!(second["data"]["listening"], json!(true));
    }

    /// `listen_stop` sem nunca ter ouvido é NO-OP idempotente — nunca erro, mesmo contrato de
    /// `STTService.stop()`.
    #[test]
    fn listen_stop_without_having_listened_is_a_no_op_not_an_error() {
        let (store, _dir) = store();
        let bus = EventBus::new();
        let state = ListeningState::new();

        let result = run_listen_stop(&store, &bus, &state);
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["stopped"], json!(false));
        assert_eq!(result["data"]["durationMs"], json!(0));
        assert_eq!(result["data"]["listening"], json!(false));
    }

    #[test]
    fn listen_start_then_listen_stop_reports_stopped_true_and_clears_listening() {
        let (store, _dir) = store();
        let bus = EventBus::new();
        let state = ListeningState::new();

        run_listen_start(&store, &bus, &state);
        let result = run_listen_stop(&store, &bus, &state);

        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["stopped"], json!(true));
        assert_eq!(result["data"]["listening"], json!(false));
    }

    /// A prova de concorrência pedida ao nível das ações: stop -> start não deixa "escuta
    /// dupla", nem em `ListeningState` (já provado em `listening_state.rs`) nem no PAR
    /// evento/resposta que estas funções produzem. Barramento ISOLADO (`EventBus::new()`), não
    /// `crate::events::bus()`: o singleton de processo receberia envelopes de QUALQUER outro
    /// teste rodando em paralelo, tornando uma contagem exata flake — mesmo cuidado do cabeçalho
    /// de `bus_bridge.rs`.
    #[tokio::test]
    async fn stop_then_start_publishes_exactly_one_started_and_one_stopped_envelope() {
        let (store, _dir) = store();
        let bus = EventBus::new();
        let state = ListeningState::new();
        // Assina ANTES das chamadas: `broadcast::Sender` não entrega retroativamente.
        let mut sub = bus.subscribe();

        run_listen_start(&store, &bus, &state);
        run_listen_stop(&store, &bus, &state);
        let restarted = run_listen_start(&store, &bus, &state);
        assert_eq!(restarted["data"]["started"], json!(true), "após stop, o novo start é um início real");

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        let mut started_count = 0;
        let mut stopped_count = 0;
        while started_count + stopped_count < 3 {
            let Ok(Some(envelope)) = tokio::time::timeout_at(deadline, sub.recv()).await else {
                break;
            };
            match envelope.key {
                EventKey::ListeningStarted => started_count += 1,
                EventKey::ListeningStopped => stopped_count += 1,
                _ => {}
            }
        }
        // Barramento isolado: agora a contagem EXATA é prova válida, não flake — nada mais publica nele.
        assert_eq!(started_count, 2, "esperava exatamente 2 ListeningStarted (start inicial + restart)");
        assert_eq!(stopped_count, 1, "esperava exatamente 1 ListeningStopped entre os dois starts");
    }
}
