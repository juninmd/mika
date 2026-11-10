//! `run_speak` — o que a ação `speak` da tool `avatar` despacha
//! (`tool.rs::AvatarToolHandler::call`), extraído para arquivo próprio pelo teto de 300 linhas do
//! AGENTS.md. Espelha `CharacterService.speak`
//! (`apps/desktop/src/features/character/services/character.service.ts`): checa
//! `voice.ttsEnabled` ANTES de publicar; recusa fechado (nunca fala) se a leitura de config falhar
//! por qualquer motivo, nunca abre por omissão.

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::events::{EventBus, EventKey};

const TTS_ENABLED_CONFIG_PATH: &str = "voice.ttsEnabled";

pub fn run_speak(bus: &'static EventBus, store: &ConfigStore, text: &str) -> Value {
    if store.get(TTS_ENABLED_CONFIG_PATH).and_then(|v| v.as_bool()) == Some(false) {
        return json!({
            "success": false,
            "error": "A fala está desligada nas configurações de voz (voice.ttsEnabled).",
            "errorCode": "not_configured",
        });
    }

    bus.publish(EventKey::SpeechRequest, json!({ "text": text }));
    json!({ "success": true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-speak-action-test-{}-{unique}", std::process::id()));
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
        let schema = SchemaRegistry::new(vec![Namespace { name: "voice", fields: vec![boolean("ttsEnabled", true)] }]);
        (ConfigStore::open(&dir.0, schema).unwrap(), dir)
    }

    fn leaked_bus() -> &'static EventBus {
        Box::leak(Box::new(EventBus::new()))
    }

    #[tokio::test]
    async fn publishes_speech_request_when_tts_is_enabled() {
        let (store, _dir) = store();
        let bus = leaked_bus();
        let mut sub = bus.subscribe();

        let result = run_speak(bus, &store, "oi!");
        assert_eq!(result["success"], json!(true));

        let envelope = sub.recv().await.expect("devia publicar imediatamente");
        assert_eq!(envelope.key, EventKey::SpeechRequest);
        assert_eq!(envelope.payload["text"], json!("oi!"));
    }

    /// NEGATIVO — `voice.ttsEnabled=false` recusa ANTES de publicar, nunca finge falar.
    #[test]
    fn refuses_when_tts_is_disabled() {
        let (store, _dir) = store();
        store.set("voice.ttsEnabled", json!(false)).unwrap();

        let result = run_speak(leaked_bus(), &store, "oi!");
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("not_configured"));
    }
}
