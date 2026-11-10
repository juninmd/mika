//! Reação a `EventKey::HardwareAlert` (`system_signals/memory.rs`): a Mika avisa, com a própria
//! voz, que a memória do PC está acabando. Arquivo próprio porque `event_reactions.rs` já passou do
//! limite de 300 linhas. O toast sai do publicador; esta fala é o complemento e respeita a cadência
//! do motor como qualquer outra reação.

use serde_json::Value;

use super::engine::ProactiveEngine;
use super::prompts;
use super::types::ProactiveCandidate;

/// Puro: "llama-server (17.0 GB), rust-analyzer (7.5 GB)" — ou vazio sem lista.
fn consumers_line(payload: &Value) -> String {
    let names: Vec<String> = payload["top"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| Some(format!("{} ({})", item["name"].as_str()?, item["memory"].as_str()?)))
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        format!(" Programs using the most right now: {}.", names.join(", "))
    }
}

pub async fn on_hardware_alert(engine: &ProactiveEngine, payload: &Value, now_ms: u64) {
    let critical = payload["level"] == "critical";
    let (Some(free), Some(total)) = (payload["free"].as_str(), payload["total"].as_str()) else { return };
    let level = if critical { "at its limit" } else { "running low" };
    let prompt = match prompts::hardware_alert(level, free, total, &consumers_line(payload)) {
        Ok(p) => p,
        Err(error) => {
            log::error!("reação proativa 'memória acabando' sem prompt (hardware-alert): {error}");
            return;
        }
    };
    let candidate = ProactiveCandidate {
        key: "hardware:memory".to_string(),
        priority: if critical { 10 } else { 8 },
        label: "memória do PC acabando".to_string(),
        prompt,
    };
    engine.request(candidate, now_ms).await;
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::json;

    use super::*;
    use crate::modules::brain::awareness::engine::ProactiveSender;

    struct RecordingSender(Arc<Mutex<Vec<String>>>);

    #[async_trait]
    impl ProactiveSender for RecordingSender {
        async fn send(&self, prompt: &str) -> String {
            self.0.lock().unwrap().push(prompt.to_string());
            "falei".to_string()
        }
    }

    fn engine(prompts: Arc<Mutex<Vec<String>>>) -> ProactiveEngine {
        ProactiveEngine::new(
            vec![],
            Box::new(RecordingSender(prompts)),
            false,
            super::super::constants::PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
        )
    }

    #[tokio::test]
    async fn a_critical_alert_speaks_the_numbers_and_the_biggest_consumer() {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let payload = json!({
            "level": "critical", "free": "2.0 GB", "total": "55.7 GB",
            "top": [{ "name": "llama-server", "memory": "17.0 GB" }],
        });
        on_hardware_alert(&engine(prompts.clone()), &payload, 10_000_000).await;
        let prompts = prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        for expected in ["at its limit", "2.0 GB", "55.7 GB", "llama-server (17.0 GB)"] {
            assert!(prompts[0].contains(expected), "falta {expected:?} em: {}", prompts[0]);
        }
        assert!(!prompts[0].contains("{{"), "placeholder sem substituir: {}", prompts[0]);
    }

    #[tokio::test]
    async fn an_alert_without_numbers_stays_silent_instead_of_inventing_them() {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        on_hardware_alert(&engine(prompts.clone()), &json!({ "level": "warning" }), 10_000_000).await;
        assert!(prompts.lock().unwrap().is_empty());
    }
}
