//! Timer que drena `log_buffer::drain_pending()` e encaminha o lote às janelas do dono, no canal
//! `system:log:batch` (`DESKTOP_RENDERER_EVENT_CHANNELS.SystemLogBatch`) que
//! `mika-runtime-system.ts::onLog` já espera (fan-out de `LogItem[]` para um callback por item).
//! Mesmo padrão de `hud_bridge::spawn_hud_bridge`: `tauri::async_runtime::spawn` (nunca
//! `tokio::spawn` fora do reator), alvo resolvido por `renderer_event_targets`.
//!
//! **Por que um lote, não um `emit` por linha:** um turno de LLM verboso gera dezenas de `log::`
//! por segundo; emitir cada um por IPC seria o mesmo desperdício de token/CPU que
//! `FLUSH_INTERVAL_MS` já resolve do lado do renderer (`logs-panel-filters.ts`) — aqui o lote nasce
//! do lado Rust, então o renderer nunca recebe mais que 1 mensagem IPC a cada [`BATCH_INTERVAL`].
//! **Nunca emite lote vazio:** silêncio sem log novo não vira ruído de IPC.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use super::log_buffer;
use crate::protocol::renderer_event_targets;

/// 1s (era 250ms): 240 acordadas/min com o painel de Logs fechado quase sempre, para um lote que o
/// painel já acumula e desenha no próprio ritmo.
const BATCH_INTERVAL: Duration = Duration::from_millis(1_000);

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.SystemLogBatch` (`packages/sdk/src/contracts/
/// desktop-ipc-channels.ts`) — o teste no fim deste arquivo lê esse fonte.
const CHANNEL: &str = "system:log:batch";

pub fn spawn(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(BATCH_INTERVAL);
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = ticker.tick() => {}
            }
            let batch = log_buffer::drain_pending();
            if batch.is_empty() {
                continue;
            }
            for label in renderer_event_targets(app.webview_windows().keys().cloned().collect()) {
                let _ = app.emit_to(label.as_str(), CHANNEL, &batch);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Literal errado = painel de Logs nunca recebe lote, sem erro em lugar nenhum — a classe de
    /// defeito que `hud_bridge.rs::every_channel_literal_exists_in_the_typescript_contract` já
    /// pega para os canais da HUD.
    #[test]
    fn channel_literal_exists_in_the_typescript_contract() {
        let contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(contract.contains(&format!("'{CHANNEL}'")), "canal '{CHANNEL}' não aparece em desktop-ipc-channels.ts");
    }
}
