//! Ciclo de vida POR SERVIDOR — o que a UI de MCP precisa e o `connect_and_register`/
//! `disconnect_all` de `mod.rs` (tudo ou nada) não dá: parar um servidor, subir um servidor, e
//! lembrar por que um servidor falhou para `mika:mcp-servers-list` mostrar.
//!
//! Corrida com a rodada inteira: [`start_server`] usa o MESMO `GENERATION` de `mod.rs`. Se um
//! `connect_and_register`/`disconnect_all` avançar o contador durante o `await` de conexão, o
//! resultado desta rodada é descartado — a rodada inteira relê o `mcp.json` (já gravado) e decide
//! sozinha. Um servidor com o mesmo nome já vivo também descarta o recém-conectado: nunca dois
//! processos para o mesmo nome, nunca tool registrada duas vezes.

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::protocol::WindowType;

use super::client::{self, ConnectedServer};
use super::config::ServerSpec;
use super::{connections, register, GENERATION};

/// Evento que a UI escuta para reler a lista: servidor salvo/removido, rodada de conexão
/// terminou, MCP desligado. Espelhado em `DESKTOP_EVENT_CHANNELS.McpServersChanged`.
pub const SERVERS_CHANGED_EVENT: &str = "mika:event:mcp:servers-changed";

/// Último erro de conexão por servidor. Zerado quando o servidor para, é removido ou conecta.
static FAILURES: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();

pub(super) fn failures() -> &'static Mutex<BTreeMap<String, String>> {
    FAILURES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(super) fn replace_failures(latest: Vec<(String, String)>) {
    *failures().lock().unwrap_or_else(|e| e.into_inner()) = latest.into_iter().collect();
}

pub(super) fn timeout_from(store: &crate::config::store::ConfigStore) -> Duration {
    Duration::from_secs(store.get("mcp.timeoutSeconds").and_then(|value| value.as_u64()).unwrap_or(20))
}

/// Tira o servidor `name` de `connections()`, desregistra as tools e fecha o processo.
pub async fn stop_server(name: &str) {
    let stopped: Vec<ConnectedServer> = {
        let mut live = connections().lock().unwrap_or_else(|e| e.into_inner());
        let (stopped, kept) = std::mem::take(&mut *live).into_iter().partition(|server| server.name == name);
        *live = kept;
        stopped
    };
    failures().lock().unwrap_or_else(|e| e.into_inner()).remove(name);
    register::unregister_tools(&stopped);
    client::close_all(stopped).await;
}

/// Sobe `name` se MCP estiver ligado, fora do modo offline e o servidor não estiver desligado no
/// `mcp.json`. Qualquer outro caso deixa o servidor parado (status `stopped`), sem erro.
pub async fn start_server(app: &AppHandle, name: &str, spec: &ServerSpec) -> Result<(), String> {
    let store = crate::state::config_store(app)?;
    let mcp_enabled = store.get("mcp.enabled").and_then(|value| value.as_bool()).unwrap_or(false);
    if !mcp_enabled || spec.disabled || crate::modules::privacy::is_offline_mode_enabled(&store) {
        return Ok(());
    }
    let timeout = timeout_from(&store);
    let my_generation = GENERATION.load(Ordering::SeqCst);
    let epochs = super::edit_epochs(std::iter::once(name));

    let connected = match client::connect_one(name, spec, timeout).await {
        Ok(connected) => connected,
        Err(error) => {
            log::error!("MCP server failed (server={name}): {error}");
            if super::edited_since(name, &epochs) {
                return Ok(());
            }
            failures().lock().unwrap_or_else(|e| e.into_inner()).insert(name.to_string(), error);
            return Ok(());
        }
    };

    let rejected = {
        let mut live = connections().lock().unwrap_or_else(|e| e.into_inner());
        let edited = super::edited_since(name, &epochs);
        let superseded =
            edited || GENERATION.load(Ordering::SeqCst) != my_generation || live.iter().any(|server| server.name == name);
        if superseded {
            Some(connected)
        } else {
            log::info!("MCP server connected (server={name}, tools={})", connected.tools.len());
            register::register_tools(std::slice::from_ref(&connected), timeout);
            failures().lock().unwrap_or_else(|e| e.into_inner()).remove(name);
            live.push(connected);
            None
        }
    };
    if let Some(connected) = rejected {
        log::info!("mcp: rodada inteira de conexão correu junto — descartando a conexão avulsa (server={name})");
        client::close_all(vec![connected]).await;
    }
    Ok(())
}

/// `connected` | `error` | `stopped` + as tools vivas — a parte de runtime de cada linha da lista.
pub fn live_status(name: &str) -> Value {
    let live = connections().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(server) = live.iter().find(|server| server.name == name) {
        let tools: Vec<Value> = server
            .tools
            .iter()
            .map(|t| json!({ "name": t.upstream_name, "key": t.key, "description": t.tool.description }))
            .collect();
        return json!({ "status": "connected", "tools": tools });
    }
    match failures().lock().unwrap_or_else(|e| e.into_inner()).get(name) {
        Some(error) => json!({ "status": "error", "error": error, "tools": [] }),
        None => json!({ "status": "stopped", "tools": [] }),
    }
}

/// Mesma regra de `settings::broadcast_config_changed`: toda janela menos `plugin-view`.
pub fn notify_changed<R: tauri::Runtime>(app: &AppHandle<R>) {
    for label in app.webview_windows().keys().cloned().collect::<Vec<_>>() {
        if WindowType::from_label(&label) == WindowType::PluginView {
            continue;
        }
        let _ = app.emit_to(label.as_str(), SERVERS_CHANGED_EVENT, json!({}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NEGATIVO — servidor nunca conectado e sem falha registrada é `stopped`, não `connected`.
    #[test]
    fn unknown_server_is_stopped() {
        assert_eq!(live_status("mcp-lifecycle-never-seen")["status"], json!("stopped"));
    }

    #[test]
    fn a_recorded_failure_is_reported_until_the_server_is_stopped() {
        let name = "mcp-lifecycle-failing";
        failures().lock().unwrap().insert(name.into(), "command 'x' not on PATH".into());

        let status = live_status(name);
        assert_eq!(status["status"], json!("error"));
        assert_eq!(status["error"], json!("command 'x' not on PATH"));

        tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(stop_server(name));
        assert_eq!(live_status(name)["status"], json!("stopped"));
    }

    /// Contrato cross-language: o renderer escuta este canal pela constante do SDK.
    #[test]
    fn servers_changed_event_is_declared_in_the_typescript_contract() {
        let contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(contract.contains(&format!("McpServersChanged: '{SERVERS_CHANGED_EVENT}'")));
    }
}
