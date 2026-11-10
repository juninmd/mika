//! Consumidor de `EventKey::VisionClipboardImage` — porte funcional de
//! `vision-proactive.ts::analyzeClipboardImage`: pega a imagem deduplicada que
//! [`super::clipboard_watch`] já publicou, manda para o mesmo cliente Ollama de
//! [`super::llm_client`] (a MESMA dependência/chokepoint de privacidade que `vision_capture`/
//! `vision_read` usam), e publica `EventKey::BrainProactiveThought` com o resultado.
//!
//! **Gate já aplicado a montante.** O TS re-checa `shouldWatchClipboard(ctx)` dentro do próprio
//! handler; aqui não — os TRÊS gates (`vision.enabled`/`vision.readClipboardImages`/
//! `privacy.clipboardLogging`) já rodaram em [`super::clipboard_watch::run_tick`] ANTES de publicar
//! o evento (o chokepoint desta fronteira, ver o cabeçalho daquele arquivo). Um segundo gate aqui
//! duplicaria a checagem sem fechar lacuna nenhuma — o mesmo princípio de "chokepoint no adaptador,
//! nunca no chamador" já documentado no resto do crate.
//!
//! **Privacidade nos logs.** Erro de análise vira `warn` com a MENSAGEM do erro apenas — nunca o
//! base64 da imagem nem o texto do prompt (que já não carrega dado do usuário: é instrução fixa).
use std::sync::Arc;

use serde_json::{json, Value};

use super::llm_client;
use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};

/// Mesmo texto de `analyzeClipboardImage` (`vision-proactive.ts`) — instrução fixa, nunca
/// interpolada com dado do usuário.
const CLIPBOARD_ANALYSIS_PROMPT: &str = "The user copied this image to the clipboard. Identify and briefly describe what is in it (for example code, a meme, a UI, text, a photo). If it is code, explain what it does. Be short and direct, 2-3 sentences at most.";

/// `imageBase64` do payload de `EventKey::VisionClipboardImage` — `None` cobre payload malformado
/// (nunca deveria acontecer vindo do próprio `clipboard_watch`, mas o consumidor não confia cegamente
/// no shape de um evento de barramento não tipado).
fn clipboard_image_base64(payload: &Value) -> Option<&str> {
    payload.get("imageBase64").and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Porte puro do corpo de `ctx.events.emit(Events.BRAIN_PROACTIVE_THOUGHT, {...})` em
/// `analyzeClipboardImage` — mesmos três campos, mesma prioridade (4).
fn thought_payload(analysis: &str) -> Value {
    json!({
        "source": "clipboard-image",
        "context": format!("The user copied a new image to the clipboard. Image analysis:\n{analysis}"),
        "priority": 4,
    })
}

async fn analyze_and_publish(store: &ConfigStore, payload: &Value) {
    let Some(image_base64) = clipboard_image_base64(payload) else {
        log::debug!("vision clipboard analysis: evento sem imageBase64, ignorado");
        return;
    };

    match llm_client::analyze(store, "vision:clipboard-image", CLIPBOARD_ANALYSIS_PROMPT, &[image_base64.to_string()]).await {
        Ok(analysis) => bus().publish(EventKey::BrainProactiveThought, thought_payload(&analysis)),
        // Mesmo tratamento de `ctx.logger.warn` no TS: falha de análise não propaga, e a mensagem
        // aqui nunca carrega o base64 (o `Display` de `VisionLlmError` não o inclui).
        Err(error) => log::warn!("análise de imagem do clipboard falhou: {error}"),
    }
}

/// Chamado uma vez no boot, ao lado de [`super::spawn_clipboard_watch`] (mesmo `ConfigStore` do
/// processo) — ver o relatório desta rodada para a linha exata de
/// `modules/mod.rs::register_config_bound_tools`.
pub fn spawn_clipboard_image_analysis(store: Arc<ConfigStore>) -> tauri::async_runtime::JoinHandle<()> {
    // Assina ANTES do spawn — broadcast não entrega retroativamente (mesmo aviso do resto do crate).
    let mut sub = bus().subscribe();
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if let EventKey::VisionClipboardImage = envelope.key {
                analyze_and_publish(&store, &envelope.payload).await;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- clipboard_image_base64: payload defensivo ----------------------------------------------

    #[test]
    fn clipboard_image_base64_reads_the_field() {
        let payload = json!({ "imageBase64": "ZmFrZQ==", "at": 1_000 });
        assert_eq!(clipboard_image_base64(&payload), Some("ZmFrZQ=="));
    }

    #[test]
    fn clipboard_image_base64_is_none_for_a_missing_or_empty_field() {
        assert_eq!(clipboard_image_base64(&json!({})), None);
        assert_eq!(clipboard_image_base64(&json!({ "imageBase64": "" })), None);
    }

    // -- thought_payload: mesmo shape de `analyzeClipboardImage` --------------------------------

    #[test]
    fn thought_payload_matches_the_ts_shape() {
        let payload = thought_payload("um meme de gato programando Rust");
        assert_eq!(payload["source"], json!("clipboard-image"));
        assert_eq!(payload["priority"], json!(4));
        let context = payload["context"].as_str().unwrap();
        assert!(context.starts_with("The user copied a new image to the clipboard."));
        assert!(context.contains("um meme de gato programando Rust"));
    }
}
