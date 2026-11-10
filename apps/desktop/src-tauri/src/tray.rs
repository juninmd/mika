//! Bandeja. Os rótulos são traduzidos AQUI desde 2026-09-22, lendo os mesmos dicionários de
//! `packages/sdk/src/i18n/locales` que o TS lia — não há segunda fonte da verdade, há a mesma
//! fonte lida de outro lado.

use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::state::ShellState;
use crate::windows;

pub const TRAY_ID: &str = "mika-tray";

const ID_TOGGLE: &str = "toggle-visible";
const ID_SETTINGS: &str = "open-settings";
const ID_ALWAYS_ON_TOP: &str = "always-on-top";
const ID_PAUSE: &str = "pause-senses";
const ID_QUIT: &str = "quit";

/// Rótulos da bandeja no idioma do usuário.
///
/// Era o sidecar quem traduzia e mandava de volta por `setTrayMenu` (`sidecar/tray-sync.ts`),
/// porque `t()` e o `ConfigKey.AppLocale` só existiam no lado Node. Hoje as duas coisas existem em
/// Rust — `modules/settings/i18n` lê os MESMOS dicionários de `packages/sdk/src/i18n/locales`, e as
/// chaves `shell.tray.*` já estão nos 7 locales. Sem este porte, apagar o sidecar deixaria a
/// bandeja no fallback em português para todo mundo, funcionando e errada, sem um único erro.
fn current_locale(app: &AppHandle) -> String {
    crate::state::config_store(app)
        .ok()
        .map(|store| crate::config::locale::app_locale(&store))
        .unwrap_or_else(|| crate::config::locale::DEFAULT_LOCALE.to_string())
}

/// Tooltip com o aviso de pausa quando mic/tela estão pausados ("Mika — mic e tela pausados"): a
/// bandeja é o único lugar que mostra, com a janela escondida, que a Mika está surda e cega.
fn tooltip_with_pause(tooltip: &str, locale: &str) -> String {
    if crate::modules::privacy::senses::paused() {
        format!("{tooltip} — {}", crate::modules::settings::t("shell.tray.statusPaused", locale))
    } else {
        tooltip.to_string()
    }
}

fn labels(app: &AppHandle) -> (String, String, String, String, String, String) {
    let locale = current_locale(app);
    let t = |key: &str| crate::modules::settings::t(key, &locale);
    // `show` e não `hide`: o Rust não sabe se a janela está visível na hora de montar o menu, e o
    // rótulo estável é o mesmo que a bandeja do Electron mostrava com a janela escondida.
    (
        t("shell.tray.show"),
        t("shell.tray.settings"),
        t("shell.tray.alwaysOnTop"),
        t("shell.tray.pauseSenses"),
        t("shell.tray.quit"),
        tooltip_with_pause("Mika", &locale),
    )
}

/// Reaplica os rótulos no idioma atual — chamado quando `app.locale` muda.
pub fn refresh_labels(app: &AppHandle) {
    let (toggle, settings, always_on_top, pause, quit, tooltip) = labels(app);
    if let Err(error) = rebuild(app, &toggle, &settings, &always_on_top, &pause, &quit, &tooltip) {
        log::warn!("bandeja não recebeu os rótulos traduzidos: {error}");
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let (toggle, settings, always_on_top, pause, quit, tooltip) = labels(app);
    let menu = build_menu(app, &toggle, &settings, &always_on_top, &pause, &quit)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().ok_or_else(|| {
            tauri::Error::AssetNotFound("ícone padrão da janela ausente".into())
        })?)
        .tooltip(&tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            ID_TOGGLE => toggle_visible(app),
            ID_SETTINGS => {
                if let Ok(window) = windows::create_settings(app) {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            ID_ALWAYS_ON_TOP => {
                let _ = crate::shell_actions::toggle_always_on_top(app);
            }
            ID_PAUSE => toggle_senses(app),
            ID_QUIT => {
                mark_quitting(app);
                app.exit(0)
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Clique esquerdo mostra/esconde o avatar; o menu fica no direito.
            if let TrayIconEvent::DoubleClick { .. } = event {
                toggle_visible(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Reconstrói o menu com os rótulos do locale ativo.
pub fn rebuild(
    app: &AppHandle,
    toggle: &str,
    settings: &str,
    always_on_top: &str,
    pause: &str,
    quit: &str,
    tooltip: &str,
) -> tauri::Result<()> {
    let menu = build_menu(app, toggle, settings, always_on_top, pause, quit)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))?;
        tray.set_tooltip(Some(tooltip))?;
    }
    Ok(())
}

/// Só o tooltip — o estado (pensando/falando/erro) muda muitas vezes por turno e não justifica
/// remontar o menu. Quem chama é `tray_status`.
pub fn set_tooltip(app: &AppHandle, tooltip: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Err(error) = tray.set_tooltip(Some(tooltip_with_pause(tooltip, &current_locale(app)))) {
            log::warn!("bandeja não aceitou o tooltip de estado: {error}");
        }
    }
}

fn build_menu(
    app: &AppHandle,
    toggle: &str,
    settings: &str,
    always_on_top: &str,
    pause: &str,
    quit: &str,
) -> tauri::Result<Menu<tauri::Wry>> {
    let checked = crate::state::always_on_top(app);

    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, ID_TOGGLE, toggle, true, None::<&str>)?,
            &MenuItem::with_id(app, ID_SETTINGS, settings, true, None::<&str>)?,
            &CheckMenuItem::with_id(
                app,
                ID_ALWAYS_ON_TOP,
                always_on_top,
                true,
                checked,
                None::<&str>,
            )?,
            &CheckMenuItem::with_id(app, ID_PAUSE, pause, true, crate::modules::privacy::senses::paused(), None::<&str>)?,
            &MenuItem::with_id(app, ID_QUIT, quit, true, None::<&str>)?,
        ],
    )
}

/// "Pausar mic e tela": alterna o interruptor ([`crate::modules::privacy::senses`]), remonta o menu
/// (marca de check) e o tooltip, e avisa dentro do app — o toast é o que diz, na hora, que mudou.
fn toggle_senses(app: &AppHandle) {
    let paused = crate::modules::privacy::senses::toggle_and_announce();
    refresh_labels(app);
    let key = if paused { "shell.tray.statusPaused" } else { "shell.tray.statusResumed" };
    let message = crate::modules::settings::t(key, &current_locale(app));
    crate::events::bus().publish(crate::events::EventKey::Notification, serde_json::json!({ "type": "info", "source": "Mika", "message": message }));
}

fn toggle_visible(app: &AppHandle) {
    let Some(window) = app.get_webview_window(windows::MAIN) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
        crate::shell_actions::start_cursor_polling(app);
    }
}

/// Mantém o `quitting` coerente quando o encerramento parte da bandeja.
pub fn mark_quitting(app: &AppHandle) {
    app.state::<ShellState>()
        .quitting
        .store(true, std::sync::atomic::Ordering::Relaxed);
}
