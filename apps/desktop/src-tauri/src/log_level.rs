//! `system.logLevel` (Logs panel) applied to the process logger, at boot and on every change —
//! port of `setMinLogLevel` + `store.subscribe('system.logLevel')` from `bootstrap/tools.ts`.
//! The log plugin accepts every level; `log::set_max_level` is the effective filter.
use log::LevelFilter;
use serde_json::Value;
use tauri::AppHandle;

const KEY: &str = "system.logLevel";

fn level_of(value: Option<&Value>) -> LevelFilter {
    match value.and_then(Value::as_str) {
        Some("debug") => LevelFilter::Debug,
        Some("warn") => LevelFilter::Warn,
        Some("error") => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}

pub fn spawn(app: &AppHandle) {
    let store = match crate::state::config_store(app) {
        Ok(store) => store,
        Err(error) => {
            log::error!("log level: ConfigStore indisponível ({error}); nível fica em info");
            return;
        }
    };
    log::set_max_level(level_of(store.get(KEY).as_ref()));
    store.watch(Box::new(|path, value| {
        if path == KEY {
            log::set_max_level(level_of(Some(value)));
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_panel_option_maps_to_its_level_and_anything_else_is_info() {
        assert_eq!(level_of(Some(&json!("debug"))), LevelFilter::Debug);
        assert_eq!(level_of(Some(&json!("warn"))), LevelFilter::Warn);
        assert_eq!(level_of(Some(&json!("error"))), LevelFilter::Error);
        assert_eq!(level_of(Some(&json!("info"))), LevelFilter::Info);
        assert_eq!(level_of(None), LevelFilter::Info);
    }
}
