//! Mika's visible pointer: every pointer action of the `computer` tool first moves a small
//! click-through window (`windows::create_agent_cursor`) to where it is about to act, so the user
//! sees what she is doing before it happens. It hides itself after [`IDLE`] without actions.
//!
//! The window sits [`OFFSET`] px right/below the target, never ON it: `foreground_guard`'s
//! `WindowFromPoint` probe would otherwise find Mika's own overlay at the click point and refuse.
//!
//! Without an `AppHandle` (unit tests, or before boot armed it) every call is a no-op.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tauri::{AppHandle, Manager, PhysicalPosition};

const IDLE: Duration = Duration::from_millis(2500);
const OFFSET: i32 = 2;
/// Restarts the CSS ripple of `agent-cursor.html`; a fixed string, never built from input.
const PULSE: &str =
    "document.body.classList.remove('click');void document.body.offsetWidth;document.body.classList.add('click')";

static APP: OnceLock<AppHandle> = OnceLock::new();
/// Bumped on every show; a hide timer only hides when no newer action happened meanwhile.
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CursorAction {
    Move,
    Click,
}

pub(crate) fn arm(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

pub(super) fn position_for(x: i32, y: i32) -> (i32, i32) {
    (x.saturating_add(OFFSET), y.saturating_add(OFFSET))
}

fn window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(existing) = app.get_webview_window(crate::windows::AGENT_CURSOR) {
        return Some(existing);
    }
    crate::windows::create_agent_cursor(app)
        .map_err(|error| log::warn!("computer_use: cursor da Mika não abriu: {error}"))
        .ok()
}

pub fn show(x: i32, y: i32, action: CursorAction) {
    let Some(window) = APP.get().and_then(window) else {
        return;
    };
    let (left, top) = position_for(x, y);
    let _ = window.set_position(PhysicalPosition::new(left, top));
    let _ = window.show();
    if action == CursorAction::Click {
        let _ = window.eval(PULSE);
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(IDLE);
        if GENERATION.load(Ordering::SeqCst) == generation {
            let _ = window.hide();
        }
    });
}

/// Emergency stop: the pointer disappears at once instead of waiting for [`IDLE`].
pub fn hide() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Some(window) = APP.get().and_then(|app| app.get_webview_window(crate::windows::AGENT_CURSOR)) {
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The overlay never covers the target pixel, or the own-window probe would refuse the click.
    #[test]
    fn the_overlay_sits_beside_the_target_never_on_it() {
        let (left, top) = position_for(100, 200);
        assert!(left > 100 && top > 200);
        assert_eq!(position_for(i32::MAX, 0).0, i32::MAX, "no overflow at the screen edge");
    }

    #[test]
    fn without_an_app_handle_show_and_hide_are_no_ops() {
        show(1, 2, CursorAction::Click);
        hide();
    }
}
