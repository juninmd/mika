//! Modo mini da janela do avatar: só a cabeça, num quadrado pequeno ancorado no canto inferior
//! esquerdo da área útil do monitor. Mesma rota da tela cheia (`fullscreen.rs`): a preferência mora
//! em `app.miniMode`, o interruptor de Configurações, o menu de contexto e a paleta chamam o MESMO
//! canal — [`set`] —, e o observador abaixo aplica na janela. O renderer segue o evento
//! `window:mini-mode` para enquadrar a cabeça e esconder HUD e dock.
//!
//! Mini e tela cheia são mutuamente exclusivos, e quem entra desfaz o outro ANTES de gravar a
//! própria chave ([`set`] aqui, `fullscreen::toggle` lá): o Tauri guarda a geometria "de antes da
//! tela cheia" no instante do `set_fullscreen`, e ela tem de ser a normal, nunca o quadrado mini.
//!
//! Wayland não deixa o cliente posicionar a janela: lá o quadrado encolhe onde o compositor puser.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const CONFIG_PATH: &str = "app.miniMode";

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.WindowMiniMode` (`packages/sdk/src/contracts/desktop-ipc-channels.ts`).
pub const EVENT: &str = "window:mini-mode";

/// Lado do quadrado, em px lógicos: cabe a cabeça e o menu de contexto reduzido (232 px de largura).
const SIDE: f64 = 240.0;
/// Folga até as bordas da área útil, em px lógicos.
const MARGIN: f64 = 16.0;

type Bounds = (PhysicalPosition<i32>, PhysicalSize<u32>);

/// A janela ESTÁ em mini (estado real, não a preferência).
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Geometria a devolver ao sair. `None` com [`ACTIVE`] ligado = a leitura não era confiável
/// (janela ainda saindo da tela cheia), e a saída volta ao tamanho padrão, centralizado.
static BEFORE: Mutex<Option<Bounds>> = Mutex::new(None);

/// O que o observador de config pede à janela: só `bool` na chave certa.
fn requested(path: &str, value: &Value) -> Option<bool> {
    if path == CONFIG_PATH { value.as_bool() } else { None }
}

/// Canto inferior esquerdo da área útil (a barra de tarefas fica de fora), em px físicos.
fn corner(area: (i32, i32, u32, u32), side: u32, margin: i32) -> (i32, i32) {
    let (x, y, _, height) = area;
    (x + margin, y + height as i32 - side as i32 - margin)
}

/// A janela cobre o monitor inteiro: é a geometria da tela cheia (ainda em transição no macOS e no
/// GTK), não a que o usuário tinha — guardá-la devolveria uma janela sem moldura do tamanho da tela.
fn covers(window: PhysicalSize<u32>, monitor: PhysicalSize<u32>) -> bool {
    window.width >= monitor.width && window.height >= monitor.height
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(crate::windows::MAIN).ok_or_else(|| "janela principal não existe".to_string())
}

pub fn is_mini() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

fn saved(app: &AppHandle) -> bool {
    crate::state::config_store(app)
        .ok()
        .and_then(|store| store.get(CONFIG_PATH))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn enter(window: &WebviewWindow) -> tauri::Result<()> {
    if ACTIVE.swap(true, Ordering::Relaxed) {
        return Ok(());
    }
    let monitor = window.current_monitor()?;
    let size = window.inner_size()?;
    let trusted = !monitor.as_ref().is_some_and(|m| covers(size, *m.size()));
    *BEFORE.lock().unwrap_or_else(|e| e.into_inner()) = trusted.then_some((window.outer_position()?, size));

    // O piso da janela normal (300x400) é maior que o quadrado: baixa o piso antes de encolher.
    window.set_min_size(Some(LogicalSize::new(SIDE, SIDE)))?;
    window.set_size(LogicalSize::new(SIDE, SIDE))?;
    let Some(monitor) = monitor else { return Ok(()) };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (x, y) = corner(
        (area.position.x, area.position.y, area.size.width, area.size.height),
        (SIDE * scale).round() as u32,
        (MARGIN * scale).round() as i32,
    );
    window.set_position(PhysicalPosition::new(x, y))
}

fn leave_window(window: &WebviewWindow) -> tauri::Result<()> {
    if !ACTIVE.swap(false, Ordering::Relaxed) {
        return Ok(());
    }
    let (min_width, min_height) = crate::windows::AVATAR_MIN_SIZE;
    window.set_min_size(Some(LogicalSize::new(min_width, min_height)))?;
    match BEFORE.lock().unwrap_or_else(|e| e.into_inner()).take() {
        Some((position, size)) => {
            window.set_size(size)?;
            window.set_position(position)
        }
        None => {
            let (width, height) = crate::windows::AVATAR_SIZE;
            window.set_size(LogicalSize::new(width, height))?;
            window.center()
        }
    }
}

fn apply(app: &AppHandle, on: bool) {
    let moved = main_window(app)
        .and_then(|window| if on { enter(&window) } else { leave_window(&window) }.map_err(|e| e.to_string()));
    match moved {
        Ok(()) => crate::shell_actions::notify(app, EVENT, json!({ "mini": on })),
        Err(error) => log::warn!("modo mini ({on}) não aplicado: {error}"),
    }
}

/// Grava a preferência; quem aplica na janela é o observador de [`spawn`]. Idempotente de
/// propósito (`set`, não `toggle`): o renderer pede "sai do mini" quando algo precisa da tela
/// inteira (aprovação de ferramenta, chat), e dois pedidos seguidos não podem religar o modo.
pub fn set(app: &AppHandle, on: bool) -> Result<(), String> {
    if on {
        crate::fullscreen::leave(app)?;
    }
    crate::state::config_store(app)?.set(CONFIG_PATH, json!(on))
}

/// Sai do mini AGORA, sem esperar o observador — chamado por `fullscreen::toggle` antes de a tela
/// cheia gravar a chave dela (ver o cabeçalho do módulo).
pub fn leave(app: &AppHandle) -> Result<(), String> {
    if !is_mini() {
        return Ok(());
    }
    apply(app, false);
    crate::state::config_store(app)?.set(CONFIG_PATH, json!(false))
}

/// Boot: reaplicado quando o avatar é revelado. Com as duas preferências ligadas (só por edição
/// manual da config), a tela cheia vence.
pub fn restore(app: &AppHandle) {
    if saved(app) && !crate::fullscreen::saved(app) {
        apply(app, true);
    }
}

pub fn spawn(app: &AppHandle) {
    let store = match crate::state::config_store(app) {
        Ok(store) => store,
        Err(error) => {
            log::error!("modo mini: ConfigStore indisponível ({error}); o interruptor não vai ter efeito");
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
    fn only_a_boolean_on_the_mini_mode_key_reaches_the_window() {
        assert_eq!(requested("app.miniMode", &json!(true)), Some(true));
        assert_eq!(requested("app.miniMode", &json!(false)), Some(false));
        assert_eq!(requested("app.miniMode", &json!("true")), None);
        assert_eq!(requested("app.fullscreen", &json!(true)), None);
    }

    #[test]
    fn the_square_sits_in_the_bottom_left_of_the_work_area() {
        // 1920x1080 com barra de tarefas de 40 px embaixo: a área útil termina em y = 1040.
        assert_eq!(corner((0, 0, 1920, 1040), 240, 16), (16, 784));
        // Monitor secundário à esquerda do principal (origem negativa) e barra no topo (y = 30).
        assert_eq!(corner((-2560, 30, 2560, 1410), 480, 32), (-2528, 928));
    }

    #[test]
    fn a_monitor_sized_window_is_not_a_geometry_worth_restoring() {
        let monitor = PhysicalSize::new(1920, 1080);
        assert!(covers(PhysicalSize::new(1920, 1080), monitor));
        assert!(!covers(PhysicalSize::new(400, 640), monitor));
        // Maximizada na altura não é tela cheia: a largura ainda é a do usuário.
        assert!(!covers(PhysicalSize::new(400, 1080), monitor));
    }
}
