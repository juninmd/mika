//! Tool `needs` — o LLM lê o corpo (`get`) e registra quando ela bebe, come, descansa ou se
//! distrai (`satisfy`: "bebe uma água, Mika" → `need=drink`). A aba Humor usa a MESMA tool para as
//! barras vivas e os botões — um caminho só para tela e LLM.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::runtime::{body, current_needs, enabled, intensity, play_gesture, satisfy_gesture};
use super::Need;

fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "needs".into(),
        description: Some(
            "Mika's simulated body (sleepiness, thirst, hunger, boredom, 0..1). The current state is \
             ALREADY in the system prompt when it matters — do not call action='get' for that. Call \
             action='satisfy' when Mika actually drinks, eats, rests or has fun in the conversation \
             (e.g. the user tells her to drink some water): need='drink'|'eat'|'sleep'|'play'."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["get", "satisfy"] },
                "need": { "type": "string", "enum": ["drink", "eat", "sleep", "play"] },
            },
            "required": ["action"],
        }),
        plugin_id: "mika.mood".into(),
        requires_window: false,
    }
}

struct NeedsToolHandler {
    store: Arc<ConfigStore>,
}

impl NeedsToolHandler {
    fn snapshot(&self) -> Value {
        let clock = body().map(|b| b.snapshot());
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        json!({
            "success": true,
            "data": {
                "enabled": enabled(&self.store),
                "intensity": intensity(&self.store),
                "needs": current_needs(&self.store),
                "awakeHours": clock.map(|c| (c.awake_hours(now) * 10.0).round() / 10.0),
            },
        })
    }
}

#[async_trait]
impl ToolHandler for NeedsToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        match args.get("action").and_then(Value::as_str).unwrap_or_default() {
            "get" => Ok(self.snapshot()),
            "satisfy" => {
                let raw = args.get("need").and_then(Value::as_str).unwrap_or_default();
                let need = Need::parse(raw).ok_or_else(|| format!("needs: necessidade desconhecida \"{raw}\""))?;
                let body = body().ok_or("needs: corpo ainda não montado")?;
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
                body.update(|clock| clock.satisfy(need, now));
                if let Some(gesture) = satisfy_gesture(need).filter(|_| enabled(&self.store)) {
                    play_gesture(gesture).await;
                }
                Ok(self.snapshot())
            }
            other => Err(format!("needs: ação desconhecida \"{other}\"")),
        }
    }
}

/// Chamado por `mood::persistence::register` (segunda passada, com `ConfigStore`): monta o corpo,
/// sobe o batimento e registra a tool.
pub(crate) fn register(store: Arc<ConfigStore>) {
    super::runtime::spawn(Arc::clone(&store), &store.data_dir().to_path_buf());
    crate::modules::tools::registry().register(definition(), Arc::new(NeedsToolHandler { store }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;

    fn handler(needs_enabled: bool) -> (NeedsToolHandler, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mika-needs-tool-{needs_enabled}-{}", std::process::id()));
        let schema = SchemaRegistry::new(vec![Namespace { name: "mood", fields: vec![boolean("enabled", true), boolean("needsEnabled", needs_enabled)] }]);
        (NeedsToolHandler { store: Arc::new(ConfigStore::open(&dir, schema).unwrap()) }, dir)
    }

    #[tokio::test]
    async fn satisfy_rejects_an_unknown_need_by_name() {
        let (handler, dir) = handler(true);
        let err = handler.call(json!({ "action": "satisfy", "need": "fly" })).await.unwrap_err();
        assert!(err.contains("desconhecida"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Toggle desligado: o corpo some do `get` (a tela mostra "desligado", o prompt não carrega nada).
    #[tokio::test]
    async fn get_reports_no_needs_when_the_toggle_is_off() {
        let (handler, dir) = handler(false);
        assert_eq!(definition().name, "needs");
        let result = handler.call(json!({ "action": "get" })).await.unwrap();
        assert_eq!(result["data"]["enabled"], json!(false));
        assert_eq!(result["data"]["needs"], Value::Null);
        let _ = std::fs::remove_dir_all(dir);
    }
}
