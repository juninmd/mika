//! Emergency stop: a global hotkey that turns PC control OFF at once — `computer-use.enabled`
//! goes to `false` in the config (so both `computer` and `pc_control` refuse every later call, a
//! pending approval included) and Mika's cursor disappears. Turning it back on is a deliberate act
//! in the settings tab, never automatic.

use std::sync::Arc;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::config::store::ConfigStore;

/// Ctrl+Alt+End: unused by Windows/macOS/Linux desktops and by common games, and hard to hit by
/// accident. Mirrored in the settings panel text (`settings.computerUse.panel.stopText`).
pub(crate) const ACCELERATOR: &str = "CommandOrControl+Alt+End";

pub(super) fn stop(store: &ConfigStore) -> Result<(), String> {
    super::cursor_overlay::hide();
    store.set("computer-use.enabled", serde_json::Value::Bool(false))
}

pub(crate) fn register(app: &AppHandle, store: Arc<ConfigStore>) {
    let result = app.global_shortcut().on_shortcut(ACCELERATOR, move |_app, _shortcut, event| {
        if event.state() != ShortcutState::Pressed {
            return;
        }
        match stop(&store) {
            Ok(()) => crate::human_log!(log::Level::Warn, "computer_use: parada de emergência — controle do PC desligado"),
            Err(error) => log::error!("computer_use: parada de emergência não gravou o config: {error}"),
        }
    });
    if let Err(error) = result {
        crate::human_log!(
            log::Level::Warn,
            "computer_use: a parada de emergência ({ACCELERATOR}) não vai funcionar porque o sistema recusou o atalho ({error})"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_turns_the_master_switch_off() {
        let dir = std::env::temp_dir().join(format!("mika-emergency-stop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        store.set("computer-use.enabled", serde_json::json!(true)).unwrap();

        stop(&store).unwrap();

        assert_eq!(store.get("computer-use.enabled"), Some(serde_json::json!(false)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
