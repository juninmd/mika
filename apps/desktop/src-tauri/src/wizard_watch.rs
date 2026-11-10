//! `setup.wizardCompleted` voltando a `false` no meio da sessão reabre o onboarding — porte do
//! `store.subscribe(SetupWizardCompleted)` de `main/index.ts`. Sem isto, "refazer o setup" nas
//! configurações gravava a chave e nada acontecia.

use serde_json::Value;
use tauri::AppHandle;

const WIZARD_KEY: &str = "setup.wizardCompleted";

/// Só `false` literal reabre, como no TS (`value === false`): apagar a chave não é pedido de setup.
fn wizard_reset_requested(path: &str, value: &Value) -> bool {
    path == WIZARD_KEY && value == &Value::Bool(false)
}

pub fn spawn(app: &AppHandle) {
    let store = match crate::state::config_store(app) {
        Ok(store) => store,
        Err(error) => {
            log::error!("wizard: ConfigStore indisponível ({error}); reabrir o onboarding não vai funcionar");
            return;
        }
    };
    let app = app.clone();
    store.watch(Box::new(move |path, value| {
        if !wizard_reset_requested(path, value) {
            return;
        }
        // Fora da pilha de `set()`: criar janela dentro do callback do store prenderia quem gravou.
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = crate::shell_actions::handle(&app, "openWizard", Value::Null) {
                log::error!("wizard: reabrir o onboarding falhou ({error})");
            }
        });
    }));
}

#[cfg(test)]
mod tests {
    use super::wizard_reset_requested;
    use serde_json::json;

    #[test]
    fn only_an_explicit_false_on_the_wizard_key_reopens_it() {
        assert!(wizard_reset_requested("setup.wizardCompleted", &json!(false)));
        assert!(!wizard_reset_requested("setup.wizardCompleted", &json!(true)));
        assert!(!wizard_reset_requested("setup.wizardCompleted", &json!(null)));
        assert!(!wizard_reset_requested("setup.other", &json!(false)));
    }
}
