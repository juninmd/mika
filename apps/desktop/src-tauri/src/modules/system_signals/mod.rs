//! Sinais de sistema publicados direto no `EventBus`, sem canal IPC próprio — poller de energia
//! ([`power`]) e, desde 2026-09-22, watcher de janela ativa ([`active_window`], porte de
//! `active-window-watcher.ts`). Ver o cabeçalho de `crate::power` para a lacuna original do primeiro
//! (poller apagado junto com o sidecar Node) e o de `active_window` para a dependência nova
//! (`windows`) que fechou a lacuna que `gamer::game_detection.rs` documentava.
//!
//! Arquivo por sinal, não um só: cada um já cresce sozinho (`power` tem dois eventos e dois
//! detectores; `active_window` tem a sonda de SO, a decisão de dedup e o estado pull-based que
//! `slang_context` lê) — inchar um arquivo comum por sinal repetiria o corte que `active_window.rs`/
//! `foreground.rs` já precisou fazer entre si pelo limite de 300 linhas.

pub mod active_window;
mod foreground;
mod memory;
mod power;
mod probe;
mod redact;
pub(crate) use redact::redact_pii;

use std::sync::Arc;

use crate::config::store::ConfigStore;
use crate::events::EventBus;

/// Sobe todos os publicadores deste módulo. Chamado de
/// `modules::register_config_bound_tools` — mesmo ponto que `hud_bridge::spawn_hud_bridge` e
/// `brain::awareness::spawn_proactive_engine`, por estar ao lado das outras pontes de barramento.
/// `store` entrou nesta rodada só para `active_window` ler o gate `brain.enabled` (mesmo corte do
/// TS: `power` não precisa de config nenhuma).
pub fn spawn(bus: &'static EventBus, store: Arc<ConfigStore>) {
    power::spawn_power_signal_listener(bus);
    memory::spawn_memory_pressure_watcher(bus, Arc::clone(&store));
    active_window::spawn_active_window_watcher(bus, store);
}
