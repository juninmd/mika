//! Tela cheia da janela do avatar. A preferência mora em `app.fullscreen` (config), e a janela a
//! segue: o interruptor de Configurações, o F11 e a paleta gravam a MESMA chave — `toggle` — e o
//! observador abaixo aplica `set_fullscreen`. Uma rota só, então o boot (`restore`) e o próximo
//! F11 nunca discordam do que está na tela.
//!
//! Tauri guarda o tamanho e a posição de antes da tela cheia e os devolve ao sair; não há nada a
//! salvar aqui. F11 NÃO é atalho global (`shortcuts.rs`): um F11 global engoliria a tela cheia do
//! jogo ou do navegador do usuário. Ele vale só com a janela do avatar em foco (`app-shortcuts.ts`).

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};
use tauri::{AppHandle, Manager, WebviewWindow};

pub const CONFIG_PATH: &str = "app.fullscreen";

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.WindowFullscreen` (`packages/sdk/src/contracts/desktop-ipc-channels.ts`).
pub const EVENT: &str = "window:fullscreen";

/// A janela já esteve em tela cheia desde o último `Resized`? Sem isto, um redimensionamento
/// qualquer logo depois de ligar a preferência (antes de o SO concluir a transição) pareceria
/// "o SO tirou a janela da tela cheia" e desligaria a preferência sozinho.
static SEEN_FULLSCREEN: AtomicBool = AtomicBool::new(false);

/// O que o observador de config pede à janela: só `bool` na chave certa.
fn requested(path: &str, value: &Value) -> Option<bool> {
    if path == CONFIG_PATH { value.as_bool() } else { None }
}

/// O SO tirou a janela da tela cheia (gesto de fora do app) enquanto a preferência ainda diz ligada.
fn left_externally(seen_fullscreen: bool, actual: bool, saved: bool) -> bool {
    seen_fullscreen && !actual && saved
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(crate::windows::MAIN).ok_or_else(|| "janela principal não existe".to_string())
}

pub fn is_fullscreen(app: &AppHandle) -> Result<bool, String> {
    main_window(app)?.is_fullscreen().map_err(|e| e.to_string())
}

fn saved(app: &AppHandle) -> bool {
    crate::state::config_store(app)
        .ok()
        .and_then(|store| store.get(CONFIG_PATH))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn apply(app: &AppHandle, on: bool) {
    match main_window(app).and_then(|window| window.set_fullscreen(on).map_err(|e| e.to_string())) {
        Ok(()) => crate::shell_actions::notify(app, EVENT, json!({ "fullscreen": on })),
        Err(error) => log::warn!("tela cheia ({on}) não aplicada: {error}"),
    }
}

/// Alterna a preferência a partir do estado REAL da janela e devolve o novo valor. Quem aplica na
/// janela é o observador de [`spawn`].
pub fn toggle(app: &AppHandle) -> Result<bool, String> {
    let next = !is_fullscreen(app)?;
    crate::state::config_store(app)?.set(CONFIG_PATH, json!(next))?;
    Ok(next)
}

/// Boot: a janela só aparece em `reveal_main`, e é ali que a preferência salva é reaplicada.
pub fn restore(app: &AppHandle) {
    if saved(app) {
        apply(app, true);
    }
}

/// Chamado a cada `Resized` da janela do avatar: sincroniza a preferência quando o SO tirou a
/// janela da tela cheia por fora do app. A direção contrária (entrar por gesto do SO) não existe
/// em janela sem moldura, e tratá-la competiria com um `toggle` ainda em voo.
pub fn on_resized(app: &AppHandle) {
    let Ok(actual) = is_fullscreen(app) else { return };
    let seen = SEEN_FULLSCREEN.swap(actual, Ordering::Relaxed);
    if !left_externally(seen, actual, saved(app)) {
        return;
    }
    if let Ok(store) = crate::state::config_store(app) {
        if let Err(error) = store.set(CONFIG_PATH, json!(false)) {
            log::warn!("tela cheia: não sincronizou a saída feita pelo SO ({error})");
        }
    }
}

pub fn spawn(app: &AppHandle) {
    let store = match crate::state::config_store(app) {
        Ok(store) => store,
        Err(error) => {
            log::error!("tela cheia: ConfigStore indisponível ({error}); o interruptor não vai ter efeito");
            return;
        }
    };
    let app = app.clone();
    store.watch(Box::new(move |path, value| {
        let Some(on) = requested(path, value) else { return };
        // Fora da pilha de `set()`: mexer na janela dentro do callback do store prenderia quem gravou.
        let app = app.clone();
        tauri::async_runtime::spawn(async move { apply(&app, on) });
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_boolean_on_the_fullscreen_key_reaches_the_window() {
        assert_eq!(requested("app.fullscreen", &json!(true)), Some(true));
        assert_eq!(requested("app.fullscreen", &json!(false)), Some(false));
        assert_eq!(requested("app.fullscreen", &json!("true")), None);
        assert_eq!(requested("app.fullscreen", &Value::Null), None);
        assert_eq!(requested("app.locale", &json!(true)), None);
    }

    #[test]
    fn an_os_exit_is_only_recognised_after_the_window_was_really_fullscreen() {
        // Saiu por fora: já esteve em tela cheia, agora não está, e a preferência ainda diz ligada.
        assert!(left_externally(true, false, true));
        // Preferência acabou de ser ligada e o SO ainda não concluiu a transição: NÃO é saída.
        assert!(!left_externally(false, false, true));
        // Continua em tela cheia, ou o usuário já desligou: nada a sincronizar.
        assert!(!left_externally(true, true, true));
        assert!(!left_externally(true, false, false));
    }
}
