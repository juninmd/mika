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
//! Estilo (`app.miniModeStyle`): `head` é o quadrado da cabeça; `dock-right`/`dock-left` é o dock
//! de borda — uma coluna alta presa à lateral da área útil, centrada na vertical, com o ícone da Mika,
//! atalhos e o chat compacto (`EdgeDock.tsx`). A área transparente da coluna deixa o clique passar
//! (`useClickThrough`). Trocar o estilo com o mini ligado só reposiciona: a geometria guardada em
//! [`BEFORE`] continua a de antes do mini.
//!
//! Wayland não deixa o cliente posicionar a janela: lá o quadrado encolhe onde o compositor puser.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const CONFIG_PATH: &str = "app.miniMode";
pub const STYLE_PATH: &str = "app.miniModeStyle";

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.WindowMiniMode` (`packages/sdk/src/contracts/desktop-ipc-channels.ts`).
pub const EVENT: &str = "window:mini-mode";

/// Lado do quadrado, em px lógicos: cabe a cabeça e o menu de contexto reduzido (232 px de largura).
const SIDE: f64 = 240.0;
/// Folga até as bordas da área útil, em px lógicos.
const MARGIN: f64 = 16.0;
/// Dock de borda, em px lógicos: coluna de ícones (72) + chat compacto (340) + respiro.
const DOCK_WIDTH: f64 = 432.0;
const DOCK_HEIGHT: f64 = 600.0;
/// Folga do dock até a borda lateral: colado, como o dock do sistema.
const DOCK_MARGIN: f64 = 8.0;

/// Onde a janela mini mora. Valor desconhecido na config cai no quadrado da cabeça.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Style {
    Head,
    DockRight,
    DockLeft,
}

impl Style {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("dock-right") => Self::DockRight,
            Some("dock-left") => Self::DockLeft,
            _ => Self::Head,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::DockRight => "dock-right",
            Self::DockLeft => "dock-left",
        }
    }
}

type Bounds = (PhysicalPosition<i32>, PhysicalSize<u32>);

/// A janela ESTÁ em mini (estado real, não a preferência).
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Geometria a devolver ao sair. `None` com [`ACTIVE`] ligado = a leitura não era confiável
/// (janela ainda saindo da tela cheia), e a saída volta ao tamanho padrão, centralizado.
static BEFORE: Mutex<Option<Bounds>> = Mutex::new(None);

/// O observador de config só reage a `bool` em `app.miniMode` ou a texto em `app.miniModeStyle`.
fn requested(path: &str, value: &Value) -> bool {
    (path == CONFIG_PATH && value.is_boolean()) || (path == STYLE_PATH && value.is_string())
}

/// Canto inferior esquerdo da área útil (a barra de tarefas fica de fora), em px físicos.
fn corner(area: (i32, i32, u32, u32), side: u32, margin: i32) -> (i32, i32) {
    let (x, y, _, height) = area;
    (x + margin, y + height as i32 - side as i32 - margin)
}

/// Dock de borda: altura limitada à área útil, centrado na vertical, colado na lateral. Retorna
/// `(x, y, largura, altura)` em px físicos.
fn edge(area: (i32, i32, u32, u32), size: (u32, u32), margin: i32, right: bool) -> (i32, i32, u32, u32) {
    let (x, y, width, height) = area;
    let h = size.1.min(height.saturating_sub(2 * margin.max(0) as u32));
    let w = size.0.min(width);
    let left = if right { x + width as i32 - w as i32 - margin } else { x + margin };
    (left, y + (height as i32 - h as i32) / 2, w, h)
}

/// Retângulo da janela mini no estilo pedido, em px físicos.
fn rect(style: Style, area: (i32, i32, u32, u32), scale: f64) -> (i32, i32, u32, u32) {
    let px = |v: f64| (v * scale).round();
    match style {
        Style::Head => {
            let side = px(SIDE) as u32;
            let (x, y) = corner(area, side, px(MARGIN) as i32);
            (x, y, side, side)
        }
        dock => edge(
            area,
            (px(DOCK_WIDTH) as u32, px(DOCK_HEIGHT) as u32),
            px(DOCK_MARGIN) as i32,
            dock == Style::DockRight,
        ),
    }
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

/// A janela está em mini no estilo dock de borda — o que o modo discreto (`dock_discreet.rs`) esconde.
pub fn is_dock(app: &AppHandle) -> bool {
    is_mini() && style(app) != Style::Head
}

fn saved(app: &AppHandle) -> bool {
    crate::state::config_store(app)
        .ok()
        .and_then(|store| store.get(CONFIG_PATH))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn style(app: &AppHandle) -> Style {
    let value = crate::state::config_store(app).ok().and_then(|store| store.get(STYLE_PATH));
    Style::parse(value.as_ref().and_then(Value::as_str))
}

/// O que a janela deve ser AGORA, lido da config e não do valor que disparou o observador: dois
/// pedidos seguidos convergem para o último, em qualquer ordem de chegada. A tela cheia vence.
fn wanted(mini: bool, fullscreen: bool) -> bool {
    mini && !fullscreen
}

fn enter(window: &WebviewWindow, style: Style) -> tauri::Result<()> {
    if is_mini() {
        return place(window, style);
    }
    // Leituras que podem falhar ANTES de marcar o estado: um erro aqui não deixa `ACTIVE` ligado
    // com a janela ainda no tamanho normal.
    let monitor = window.current_monitor()?;
    let size = window.inner_size()?;
    let position = window.outer_position()?;
    let trusted = !monitor.as_ref().is_some_and(|m| covers(size, *m.size()));
    *BEFORE.lock().unwrap_or_else(|e| e.into_inner()) = trusted.then_some((position, size));
    ACTIVE.store(true, Ordering::Relaxed);
    place(window, style)
}

/// Encolhe e posiciona no estilo pedido. Sem monitor conhecido, só o tamanho muda.
fn place(window: &WebviewWindow, style: Style) -> tauri::Result<()> {
    // O piso da janela normal (300x400) é maior que o quadrado: baixa o piso antes de encolher.
    window.set_min_size(Some(LogicalSize::new(SIDE, SIDE)))?;
    let Some(monitor) = window.current_monitor()? else {
        let (w, h) = if style == Style::Head { (SIDE, SIDE) } else { (DOCK_WIDTH, DOCK_HEIGHT) };
        return window.set_size(LogicalSize::new(w, h));
    };
    let area = monitor.work_area();
    let (x, y, w, h) = rect(
        style,
        (area.position.x, area.position.y, area.size.width, area.size.height),
        monitor.scale_factor(),
    );
    window.set_size(PhysicalSize::new(w, h))?;
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
    let style = style(app);
    let moved = main_window(app)
        .and_then(|window| if on { enter(&window, style) } else { leave_window(&window) }.map_err(|e| e.to_string()));
    match moved {
        Ok(()) => crate::shell_actions::notify(app, EVENT, json!({ "mini": on, "style": style.name() })),
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

fn sync(app: &AppHandle) {
    apply(app, wanted(saved(app), crate::fullscreen::saved(app)));
}

/// Boot: reaplicado quando o avatar é revelado.
pub fn restore(app: &AppHandle) {
    if saved(app) {
        sync(app);
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
        if !requested(path, value) {
            return;
        }
        // Fora da pilha de `set()` (mexer na janela dentro do callback do store prenderia quem
        // gravou) e na thread principal: os pedidos são aplicados um por vez, na ordem da fila.
        let handle = app.clone();
        if let Err(error) = app.run_on_main_thread(move || sync(&handle)) {
            log::warn!("modo mini: pedido não chegou à thread principal ({error})");
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_boolean_mini_mode_or_a_text_style_reaches_the_window() {
        assert!(requested("app.miniMode", &json!(true)));
        assert!(requested("app.miniMode", &json!(false)));
        assert!(!requested("app.miniMode", &json!("true")));
        assert!(requested("app.miniModeStyle", &json!("dock-right")));
        assert!(!requested("app.miniModeStyle", &json!(true)));
        assert!(!requested("app.fullscreen", &json!(true)));
    }

    #[test]
    fn an_unknown_style_falls_back_to_the_head_square() {
        assert_eq!(Style::parse(Some("dock-left")), Style::DockLeft);
        assert_eq!(Style::parse(Some("dock-right")).name(), "dock-right");
        assert_eq!(Style::parse(Some("dock-top")), Style::Head);
        assert_eq!(Style::parse(None), Style::Head);
    }

    #[test]
    fn the_dock_hugs_the_chosen_side_centered_in_the_work_area() {
        // 1920x1040 de área útil, escala 1: 432x600 colado à direita com 8 px de folga.
        assert_eq!(rect(Style::DockRight, (0, 0, 1920, 1040), 1.0), (1480, 220, 432, 600));
        assert_eq!(rect(Style::DockLeft, (0, 0, 1920, 1040), 1.0), (8, 220, 432, 600));
        // Escala 2 numa área baixa (1366x768 físicos → altura cortada para caber com a folga).
        assert_eq!(rect(Style::DockRight, (0, 0, 1366, 768), 2.0), (486, 16, 864, 736));
        // O quadrado da cabeça não mudou.
        assert_eq!(rect(Style::Head, (0, 0, 1920, 1040), 1.0), (16, 784, 240, 240));
    }

    #[test]
    fn fullscreen_wins_when_both_preferences_are_on() {
        assert!(wanted(true, false));
        assert!(!wanted(true, true));
        assert!(!wanted(false, false));
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
