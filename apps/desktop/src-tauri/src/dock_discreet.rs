//! Modo discreto do dock de borda (`app.dockDiscreet`): com um app em tela cheia na frente — jogo,
//! apresentação —, o dock some de vez e a janela inteira deixa o clique passar (`EdgeDock.tsx`).
//! Só o atalho global de conversa (`shortcuts.rs`, `DockTalk`) ou o chat já aberto o trazem de volta.
//!
//! A sonda (`system_signals::active_window::foreground_fullscreen`) só roda com o dock na tela: fora
//! dele não há o que esconder, e o laço fica num `is_dock()` barato a cada passada. A mudança vai ao
//! renderer pelo evento [`EVENT`], só na borda (ligou/desligou), nunca a cada passada.

use std::time::Duration;

use serde_json::json;
use tauri::AppHandle;

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.WindowDiscreet` (`packages/sdk/src/contracts/desktop-ipc-channels.ts`).
pub const EVENT: &str = "window:discreet";
const CONFIG_PATH: &str = "app.dockDiscreet";
const POLL: Duration = Duration::from_secs(2);

fn enabled(app: &AppHandle) -> bool {
    crate::state::config_store(app).ok().and_then(|store| store.get(CONFIG_PATH)).and_then(|v| v.as_bool()).unwrap_or(true)
}

/// Discreto só com o dock na tela, a preferência ligada e a sonda dizendo "tela cheia" com certeza:
/// sem sonda (Wayland) o dock fica como está.
fn discreet(watching: bool, fullscreen: Option<bool>) -> bool {
    watching && fullscreen == Some(true)
}

pub fn spawn(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last = false;
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = tokio::time::sleep(POLL) => {}
            }
            let watching = crate::mini_mode::is_dock(&app) && enabled(&app);
            let fullscreen = if watching {
                tokio::task::spawn_blocking(crate::modules::system_signals::active_window::foreground_fullscreen).await.ok().flatten()
            } else {
                None
            };
            let now = discreet(watching, fullscreen);
            if now != last {
                last = now;
                crate::shell_actions::notify(&app, EVENT, json!({ "discreet": now }));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_sure_fullscreen_with_the_dock_on_screen_hides_it() {
        assert!(discreet(true, Some(true)));
        assert!(!discreet(true, Some(false)));
        assert!(!discreet(true, None), "sem sonda (Wayland) o dock fica como está");
        assert!(!discreet(false, Some(true)), "fora do dock não há o que esconder");
    }

    /// O canal tem de existir no contrato que a ponte escuta, ou o evento vai para ninguém.
    #[test]
    fn the_discreet_channel_is_in_the_renderer_event_contract() {
        let contract = include_str!("../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(contract.contains(&format!("WindowDiscreet: '{EVENT}'")));
    }
}
