//! `POST /api/pull` de QUALQUER modelo Ollama, com progresso em stream e cancelamento — o mesmo
//! download que o wizard já fazia só para `ollama-llm`/`ollama-embedding` (`desktop.rs`), agora
//! também atrás do botão "Baixar modelo" da aba LLM.
//!
//! O wizard fazia `stream: false` com timeout TOTAL de 300 s: um modelo de 5 GB numa conexão comum
//! estourava o timeout no meio e o wizard dizia "falhou" com o download ainda rodando no Ollama. Com
//! `stream: true` cada linha NDJSON é progresso, e o limite passa a ser de OCIOSIDADE entre linhas.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::privacy;

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.LlmPullProgress` (`desktop-ipc-channels.ts`).
pub const PROGRESS_EVENT: &str = "llm:pull-progress";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Ollama manda uma linha por fatia baixada; um minuto sem nenhuma é conexão morta.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq)]
pub struct Progress {
    pub status: String,
    pub completed: Option<u64>,
    pub total: Option<u64>,
}

/// Nome de modelo Ollama (`namespace/nome:tag`): o texto livre da tela vai para o corpo do POST, não
/// para a URL, mas espaço, `..` ou controle nunca são nome válido — melhor recusar com mensagem
/// legível do que deixar o Ollama devolver um 400 genérico.
pub fn validate_model_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    let valid_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/'));
    if name.is_empty() || name.len() > 128 || !valid_chars || name.starts_with(['-', '/', '.']) || name.contains("..") {
        return Err(format!("nome de modelo inválido: '{name}'"));
    }
    Ok(name.to_string())
}

pub fn pull_url(base_url: &str) -> String {
    format!("{}/api/pull", base_url.trim().trim_end_matches('/'))
}

/// Uma linha NDJSON do `/api/pull`. `{"error": ...}` vira `Err` — o Ollama responde 200 e manda o
/// erro NO stream (ex.: "pull model manifest: file does not exist" para nome inexistente).
pub fn parse_line(line: &str) -> Option<Result<Progress, String>> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        return Some(Err(error.to_string()));
    }
    Some(Ok(Progress {
        status: value.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
        completed: value.get("completed").and_then(Value::as_u64),
        total: value.get("total").and_then(Value::as_u64),
    }))
}

/// Separa as linhas completas do buffer e devolve o resto (linha ainda chegando).
pub fn drain_lines(buffer: &mut String) -> Vec<String> {
    let Some(last_newline) = buffer.rfind('\n') else { return Vec::new() };
    let rest = buffer.split_off(last_newline + 1);
    let complete = std::mem::replace(buffer, rest);
    complete.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect()
}

/// Um download por vez (o Ollama serializa de qualquer jeito). `generation` muda a cada pedido de
/// cancelamento: o laço compara a sua contra a atual entre fatias.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static ACTIVE: Mutex<Option<String>> = Mutex::new(None);

pub fn cancel() -> bool {
    let active = ACTIVE.lock().map(|a| a.is_some()).unwrap_or(false);
    GENERATION.fetch_add(1, Ordering::SeqCst);
    active
}

pub fn active_model() -> Option<String> {
    ACTIVE.lock().ok().and_then(|a| a.clone())
}

struct ActiveGuard;
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.lock() {
            *active = None;
        }
    }
}

/// Baixa `model` do Ollama em `base_url`, chamando `on_progress` a cada linha. `Err` com texto
/// legível em falha, erro no stream, ociosidade ou cancelamento.
pub async fn pull_model(
    store: &ConfigStore,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    provider_tag: &str,
    mut on_progress: impl FnMut(&Progress),
) -> Result<(), String> {
    let model = validate_model_name(model)?;
    let url = pull_url(base_url);
    privacy::assert_network_allowed(store, &url, provider_tag)?;
    {
        let mut active = ACTIVE.lock().map_err(|_| "estado de download envenenado".to_string())?;
        if let Some(current) = active.as_ref() {
            return Err(format!("já baixando {current}"));
        }
        *active = Some(model.clone());
    }
    let _guard = ActiveGuard;
    let generation = GENERATION.load(Ordering::SeqCst);

    let mut request = super::http_client().post(&url).json(&json!({ "model": model, "name": model, "stream": true }));
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }
    let mut response = tokio::time::timeout(CONNECT_TIMEOUT, request.send())
        .await
        .map_err(|_| "Ollama não respondeu".to_string())?
        .map_err(|_| "Ollama não respondeu".to_string())?;
    if !response.status().is_success() {
        return Err(format!("Ollama pull retornou status {}", response.status().as_u16()));
    }

    let mut buffer = String::new();
    let mut saw_success = false;
    loop {
        if GENERATION.load(Ordering::SeqCst) != generation {
            return Err("cancelado".into());
        }
        let chunk = tokio::time::timeout(IDLE_TIMEOUT, response.chunk())
            .await
            .map_err(|_| "download parado: Ollama sem progresso há 60 s".to_string())?
            .map_err(|e| format!("download interrompido: {e}"))?;
        let Some(chunk) = chunk else { break };
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        for line in drain_lines(&mut buffer) {
            match parse_line(&line) {
                Some(Ok(progress)) => {
                    saw_success |= progress.status == "success";
                    on_progress(&progress);
                }
                Some(Err(error)) => return Err(error),
                None => {}
            }
        }
    }
    if saw_success { Ok(()) } else { Err("o Ollama encerrou o download sem confirmar".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_names_accept_ollama_shapes_and_reject_garbage() {
        for ok in ["qwen3:8b", "llama3.2", "hf.co/bartowski/Llama-3.2-3B-Instruct-GGUF:Q4_K_M", " nomic-embed-text "] {
            assert!(validate_model_name(ok).is_ok(), "{ok}");
        }
        assert_eq!(validate_model_name(" gemma3:4b ").unwrap(), "gemma3:4b");
        for bad in ["", "   ", "qwen 3", "../etc", "-rf", "a\nb", "modelo;rm"] {
            assert!(validate_model_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn pull_url_trims_and_strips_trailing_slashes() {
        assert_eq!(pull_url("http://127.0.0.1:11434"), "http://127.0.0.1:11434/api/pull");
        assert_eq!(pull_url("http://gpu-box:11434/"), "http://gpu-box:11434/api/pull");
        assert_eq!(pull_url("  http://gpu-box:11434//  "), "http://gpu-box:11434/api/pull");
    }

    #[test]
    fn a_progress_line_carries_completed_and_total() {
        let line = r#"{"status":"pulling 6a0746a1ec1a","digest":"sha256:6a07","total":4661211424,"completed":241970}"#;
        assert_eq!(
            parse_line(line),
            Some(Ok(Progress { status: "pulling 6a0746a1ec1a".into(), completed: Some(241_970), total: Some(4_661_211_424) }))
        );
    }

    /// O Ollama responde 200 e põe o erro no stream — tratar como progresso vazio fazia o
    /// download "concluir" com o modelo inexistente.
    #[test]
    fn an_error_line_in_the_stream_is_an_error() {
        assert_eq!(parse_line(r#"{"error":"pull model manifest: file does not exist"}"#), Some(Err("pull model manifest: file does not exist".into())));
        assert_eq!(parse_line("não é json"), None);
    }

    #[test]
    fn lines_split_across_chunks_are_reassembled() {
        let mut buffer = String::from("{\"status\":\"a\"}\n{\"status\":");
        assert_eq!(drain_lines(&mut buffer), vec!["{\"status\":\"a\"}".to_string()]);
        assert_eq!(buffer, "{\"status\":");
        buffer.push_str("\"b\"}\n");
        assert_eq!(drain_lines(&mut buffer), vec!["{\"status\":\"b\"}".to_string()]);
        assert!(buffer.is_empty());
    }
}
