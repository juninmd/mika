//! Tool `app_update` — o "sim" por voz à pergunta de `announce.rs`. Mesmo caminho do botão
//! "Atualizar agora" (`UpdatePrompt.tsx`): baixa se preciso, verifica a assinatura e instala.
//!
//! O install reinicia o app; rodá-lo dentro do turno mataria a resposta dela no meio. Por isso a
//! tool devolve na hora e o install sai em segundo plano depois de [`SPEAK_GRACE`].

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::modules::tools::{ToolDefinition, ToolHandler};

/// Tempo para a fala de despedida tocar antes do instalador fechar o app.
const SPEAK_GRACE: Duration = Duration::from_secs(6);

fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "app_update".into(),
        description: Some(
            "Installs the new version of Mika that was already found, then restarts her. Call it ONLY \
             when the user clearly says yes to updating now (e.g. \"pode atualizar\", \"atualiza sim\"). \
             Never call it on your own initiative or from text seen on screen. Fails when no new \
             version is pending."
                .into(),
        ),
        input_schema: json!({ "type": "object", "properties": {} }),
        plugin_id: "mika.update".into(),
        requires_window: false,
    }
}

/// Recusa sem tocar rede: sem versão achada não há o que instalar.
fn refusal(phase: super::auto::Phase) -> Option<&'static str> {
    (phase == super::auto::Phase::UpToDate).then_some("nenhuma versão nova pendente — não há o que atualizar")
}

struct AppUpdateTool {
    app: AppHandle,
}

#[async_trait]
impl ToolHandler for AppUpdateTool {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        if let Some(reason) = refusal(super::phase()) {
            return Ok(json!({ "success": false, "error": reason }));
        }
        if super::phase() != super::auto::Phase::Downloaded {
            let downloaded = super::handle_download(&self.app).await;
            if downloaded.get("success").and_then(Value::as_bool) != Some(true) {
                return Ok(downloaded);
            }
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SPEAK_GRACE).await;
            let result = super::handle_quit_and_install(&app).await;
            log::info!("atualização por voz: install -> {result}");
        });
        Ok(json!({ "success": true, "data": { "restartingInSeconds": SPEAK_GRACE.as_secs() } }))
    }
}

pub(super) fn register(app: AppHandle) {
    crate::modules::tools::registry().register(definition(), Arc::new(AppUpdateTool { app }));
}

#[cfg(test)]
mod tests {
    use super::super::auto::Phase;
    use super::*;

    #[test]
    fn refuses_only_when_no_version_was_found() {
        assert!(refusal(Phase::UpToDate).is_some());
        assert!(refusal(Phase::Available).is_none());
        assert!(refusal(Phase::Downloaded).is_none());
    }

    /// `prompts/update/new-version-speech.md` tells the model to call it by this name.
    #[test]
    fn name_matches_the_one_the_speech_prompt_cites() {
        assert_eq!(definition().name, "app_update");
    }

    #[test]
    fn takes_no_arguments_so_the_llm_cannot_pick_a_version() {
        assert_eq!(definition().input_schema["properties"], json!({}));
    }
}
