//! `emitScreenObservation` — port de `handlers/take-screenshot.ts`. Extraído de `capture_tool.rs`
//! nesta rodada só pelo limite de 300 linhas do AGENTS.md (a função em si é pequena; o arquivo já
//! estava no teto) — mesmo critério de `screen_capture.rs` ter sido extraído de `capture_tool.rs`
//! numa rodada anterior (ver o cabeçalho daquele arquivo).
//!
//! **Gate medido no fonte real, não deduzido:** o único gate de `emitScreenObservation` é re-checar
//! `isVisionToolEnabled()` DEPOIS da análise terminar (a visão pode ter sido desligada durante a
//! chamada ao LLM, que não é instantânea) — NÃO `vision.visionAssistAutoComment`. Esse campo gateia
//! um evento DIFERENTE: o laço periódico de [`super::watcher`], via `capture/observation-events.ts`
//! (arquivo TS homônimo mas distinto de `handlers/take-screenshot.ts`). Os dois arquivos do lado TS
//! têm nomes parecidos e gates diferentes — confundir os dois faria `screenshot`/`watch_now` (ação
//! sob demanda) herdar um gate que só existe para o observador automático.
//!
//! **`watch_now` não publica este evento no TS** (`handlers/watch-now.ts` não importa `Events` nem
//! `requireEventBus` — medido lendo o arquivo inteiro). Só `action=screenshot` chama isto.

use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use super::screen_capture::is_vision_enabled;
use crate::config::store::ConfigStore;
use crate::events::EventKey;
use crate::modules::system_signals::active_window::ActiveWindowSnapshot;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Prompt padrão de `take-screenshot.ts`, com a janela e o executável ativos.
pub(super) fn screenshot_prompt(window: Option<&ActiveWindowSnapshot>) -> String {
    let title = window.and_then(|w| w.title.as_deref()).unwrap_or("unknown");
    let process = window.and_then(|w| w.owner.as_deref()).unwrap_or("unknown");
    format!("Describe in detail what you see on all captured monitors. Active window: {title}. Active executable: {process}.")
}

/// `foregroundApp` no formato que `brain::awareness::screen_provider` lê; título/dono já vêm redigidos.
pub(super) fn foreground_json(window: Option<&ActiveWindowSnapshot>) -> Value {
    json!({ "title": window.and_then(|w| w.title.clone()), "processName": window.and_then(|w| w.owner.clone()) })
}

fn observation_payload(analysis: &str, image_count: usize, window: Option<&ActiveWindowSnapshot>) -> Value {
    json!({
        "analysis": analysis,
        "imageCount": image_count,
        "foregroundApp": foreground_json(window),
        "isLikelyGame": window.is_some_and(ActiveWindowSnapshot::is_likely_game),
        "at": now_ms(),
    })
}

/// Sem `imagePaths`/`entities` — sem captura em disco nem extração estruturada portadas.
pub(super) fn emit_screen_observation(store: &ConfigStore, analysis: &str, image_count: usize, window: Option<&ActiveWindowSnapshot>) {
    if !is_vision_enabled(store) {
        return;
    }
    crate::events::bus().publish(EventKey::ScreenObservation, observation_payload(analysis, image_count, window));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-vision-observation-events-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store_with(enabled: bool) -> (ConfigStore, TempDir) {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::boolean;

        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace { name: "vision", fields: vec![boolean("enabled", false)] }]);
        let store = ConfigStore::open(&dir.0, schema).unwrap();
        store.set("vision.enabled", json!(enabled)).unwrap();
        (store, dir)
    }

    /// Vazio: `is_vision_enabled` recusa antes de tocar `crate::events::bus()` — sem assinante
    /// nenhum, só prova que não panica publicando no vazio (mesmo comportamento documentado em
    /// `events/bus.rs::publish`).
    #[test]
    fn emit_screen_observation_is_a_no_op_when_vision_is_disabled() {
        let (store, _dir) = store_with(false);
        emit_screen_observation(&store, "não deveria publicar", 1, None);
    }

    /// O screenshot mandava ao LLM e ao `screen_provider` uma tela sem contexto: qual janela, qual
    /// executável, se é jogo. O TS punha os três no prompt padrão e no evento.
    #[test]
    fn foreground_window_reaches_the_prompt_and_the_observation_payload() {
        let window = ActiveWindowSnapshot { title: Some("Elden Ring".into()), owner: Some("eldenring".into()) };
        let prompt = screenshot_prompt(Some(&window));
        assert!(prompt.contains("Active window: Elden Ring") && prompt.contains("Active executable: eldenring"), "{prompt}");
        assert!(screenshot_prompt(None).contains("Active window: unknown"));

        let payload = observation_payload("tela", 1, Some(&window));
        assert_eq!(payload["foregroundApp"], json!({ "title": "Elden Ring", "processName": "eldenring" }));
        assert_eq!(payload["isLikelyGame"], json!(true));
    }

    /// Positivo contra o barramento GLOBAL do processo (`emit_screen_observation` não aceita um
    /// `EventBus` injetado — publica direto em `crate::events::bus()`, mesma decisão de
    /// `hud_bridge`/`chat::spawn_assistant_message_bridge`). Segue o padrão 2 do aviso em
    /// `events/bus.rs`: filtra por `EventKey::ScreenObservation` e ignora qualquer evento de outro
    /// teste concorrente, em vez de assumir "a primeira mensagem é a minha".
    #[tokio::test]
    async fn emit_screen_observation_publishes_the_reduced_payload_when_vision_is_still_enabled() {
        let (store, _dir) = store_with(true);
        let mut sub = crate::events::bus().subscribe();

        emit_screen_observation(&store, "tela parada", 2, None);

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match tokio::time::timeout_at(deadline, sub.recv()).await {
                Ok(Some(envelope)) if envelope.key == EventKey::ScreenObservation => {
                    assert_eq!(envelope.payload["analysis"], json!("tela parada"));
                    assert_eq!(envelope.payload["imageCount"], json!(2));
                    assert_eq!(envelope.payload["isLikelyGame"], json!(false));
                    assert_eq!(envelope.payload["foregroundApp"], json!({ "title": null, "processName": null }));
                    return;
                }
                Ok(Some(_)) => continue, // evento de outro teste concorrente no mesmo processo.
                _ => panic!("ScreenObservation não publicado dentro do prazo"),
            }
        }
    }
}
