//! `find`/`click_element` — Windows-only element lookup via UI Automation (crate `uiautomation`,
//! default features `input`+`control`, which pulls in `pattern` — see `Cargo.toml`). Every other OS
//! gets an explicit `"unsupported on this platform"` result, never a silent no-op: `desktop`'s
//! `window` action already sets the precedent of naming a platform gap instead of hiding it.
//!
//! **Two pure decisions live here, unit-tested without touching COM:** [`role_to_control_type`]
//! (string -> `ControlType`, so an unknown role is a named `invalid_role` error instead of an
//! empty/wrong search) and [`refuse_if_password`] (the `IsPassword` gate the brief requires before
//! `type_text` is allowed to reach the focused control). The live COM calls around them
//! (`find`/`click_element`/`focused_element_is_password`) are thin dispatch, same precedent as
//! `features::desktop::window::list_open_windows` — not unit-tested line by line, proven by
//! dispatching for real (`cargo test` on Windows) and by the pure functions they compose.
//!
//! **Own-window refusal happens here too, before any search runs**, using the UIA element's own
//! `get_process_id()` against `std::process::id()` — precise (the exact window about to be
//! touched). `click{x,y}` gets the same precision via `foreground_guard::point_is_own_window`
//! (resolves the window AT the point, not whatever has OS focus); `type_text`/`key` have no
//! coordinates to resolve, so they keep the foreground-only probe, which is correct for keyboard
//! input (it always lands on whatever has focus).

use serde_json::{json, Value};
// `uiautomation` é dependência só de Windows (Cargo.toml): fora dele o import não resolve.
#[cfg(target_os = "windows")]
use uiautomation::types::ControlType;

#[cfg(not(target_os = "windows"))]
pub const UNSUPPORTED: &str = "unsupported on this platform";

/// `"button"` -> `ControlType::Button`, case-insensitive, covering the roles a `computer.find`
/// result names back to the model. `None` for anything not listed — the caller turns that into a
/// named `invalid_role` error instead of silently matching everything.
#[cfg(target_os = "windows")]
pub fn role_to_control_type(role: &str) -> Option<ControlType> {
    Some(match role.to_ascii_lowercase().as_str() {
        "button" => ControlType::Button,
        "checkbox" => ControlType::CheckBox,
        "combobox" => ControlType::ComboBox,
        "edit" | "textbox" | "input" => ControlType::Edit,
        "text" | "label" => ControlType::Text,
        "hyperlink" | "link" => ControlType::Hyperlink,
        "image" => ControlType::Image,
        "listitem" => ControlType::ListItem,
        "list" => ControlType::List,
        "menu" => ControlType::Menu,
        "menubar" => ControlType::MenuBar,
        "menuitem" => ControlType::MenuItem,
        "radiobutton" | "radio" => ControlType::RadioButton,
        "slider" => ControlType::Slider,
        "spinner" => ControlType::Spinner,
        "tab" => ControlType::Tab,
        "tabitem" => ControlType::TabItem,
        "tree" => ControlType::Tree,
        "treeitem" => ControlType::TreeItem,
        "window" => ControlType::Window,
        "pane" => ControlType::Pane,
        "group" => ControlType::Group,
        "table" => ControlType::Table,
        "toolbar" => ControlType::ToolBar,
        _ => return None,
    })
}

/// Inverse of [`role_to_control_type`], for `find` results. The OS's localized name ("botão") went
/// back to the model, which passed it to `click_element` and got `invalid_role` (smoke 2026-09-27).
#[cfg(target_os = "windows")]
pub fn control_type_to_role(control_type: ControlType) -> Option<&'static str> {
    Some(match control_type {
        ControlType::Button => "button",
        ControlType::CheckBox => "checkbox",
        ControlType::ComboBox => "combobox",
        ControlType::Edit => "edit",
        ControlType::Text => "text",
        ControlType::Hyperlink => "hyperlink",
        ControlType::Image => "image",
        ControlType::ListItem => "listitem",
        ControlType::List => "list",
        ControlType::Menu => "menu",
        ControlType::MenuBar => "menubar",
        ControlType::MenuItem => "menuitem",
        ControlType::RadioButton => "radiobutton",
        ControlType::Slider => "slider",
        ControlType::Spinner => "spinner",
        ControlType::Tab => "tab",
        ControlType::TabItem => "tabitem",
        ControlType::Tree => "tree",
        ControlType::TreeItem => "treeitem",
        ControlType::Window => "window",
        ControlType::Pane => "pane",
        ControlType::Group => "group",
        ControlType::Table => "table",
        ControlType::ToolBar => "toolbar",
        _ => return None,
    })
}

/// Pure gate for `type_text`. `Some(true)` (positively confirmed password field) refuses;
/// `Some(false)` or `None` (not a password field, or the platform/control couldn't tell us)
/// allows — see the module header on why "can't verify" degrades to "allow", not "refuse", for a
/// capability gap rather than a demonstrated risk.
pub fn refuse_if_password(is_password: Option<bool>) -> Option<&'static str> {
    if is_password == Some(true) {
        Some("refused: the focused field is a password field")
    } else {
        None
    }
}

fn error_result(message: impl std::fmt::Display, error_code: &str) -> Value {
    json!({ "success": false, "error": message.to_string(), "errorCode": error_code })
}

#[cfg(target_os = "windows")]
fn own_window_result() -> Value {
    error_result("refused: the target window belongs to Mika itself", "own_window")
}

#[cfg(target_os = "windows")]
mod live {
    use std::time::Duration;

    use uiautomation::{UIAutomation, UIElement};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    use super::super::foreground_guard;
    use super::super::input_backend::{InputBackend, MouseButton};
    use super::*;

    const SEARCH_TIMEOUT_MS: u64 = 1500;
    /// The matcher counts the root (the desktop) as depth 1: its children, the top-level windows, sit at 2.
    pub(super) const TOP_LEVEL_WINDOW_DEPTH: u32 = 2;
    const MAX_FIND_RESULTS: usize = 40;

    fn automation() -> Result<UIAutomation, String> {
        UIAutomation::new().map_err(|error| error.to_string())
    }

    fn foreground_root(automation: &UIAutomation) -> Result<UIElement, String> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return Err("no foreground window".into());
        }
        automation.element_from_handle(hwnd.into()).map_err(|error| error.to_string())
    }

    fn window_root(automation: &UIAutomation, title: &str) -> Result<UIElement, String> {
        let desktop = automation.get_root_element().map_err(|error| error.to_string())?;
        automation
            .create_matcher()
            .from_ref(&desktop)
            .depth(TOP_LEVEL_WINDOW_DEPTH)
            .control_type(ControlType::Window)
            .contains_name(title)
            .timeout(SEARCH_TIMEOUT_MS)
            .find_first()
            .map_err(|error| format!("window '{title}' not found: {error}"))
    }

    fn resolve_root(automation: &UIAutomation, window: Option<&str>) -> Result<UIElement, String> {
        match window.map(str::trim).filter(|w| !w.is_empty()) {
            Some(title) => window_root(automation, title),
            None => foreground_root(automation),
        }
    }

    /// `None` when the root belongs to another process (safe to proceed); `Some(refusal)` when it
    /// is this one.
    fn own_window_guard(root: &UIElement) -> Option<Value> {
        let pid = root.get_process_id().ok()?;
        foreground_guard::is_own_window(Some(pid), std::process::id()).then(own_window_result)
    }

    fn element_summary(element: &UIElement) -> Value {
        json!({
            "name": element.get_name().unwrap_or_default(),
            "role": element.get_control_type().ok().and_then(control_type_to_role).unwrap_or_default(),
            "automationId": element.get_automation_id().unwrap_or_default(),
        })
    }

    pub fn find(name: Option<&str>, role: Option<&str>, window: Option<&str>) -> Value {
        let automation = match automation() {
            Ok(automation) => automation,
            Err(error) => return error_result(error, "internal"),
        };
        let root = match resolve_root(&automation, window) {
            Ok(root) => root,
            Err(error) => return error_result(error, "not_found"),
        };
        if let Some(refusal) = own_window_guard(&root) {
            return refusal;
        }

        let control_type = match role {
            Some(role) => match super::role_to_control_type(role) {
                Some(control_type) => Some(control_type),
                None => return error_result(format!("unknown role '{role}'"), "invalid_role"),
            },
            None => None,
        };

        let mut matcher = automation.create_matcher().from_ref(&root).timeout(SEARCH_TIMEOUT_MS);
        if let Some(name) = name {
            matcher = matcher.contains_name(name);
        }
        if let Some(control_type) = control_type {
            matcher = matcher.control_type(control_type);
        }

        let elements = matcher.find_all().unwrap_or_default();
        let truncated = elements.len() > MAX_FIND_RESULTS;
        let items: Vec<Value> = elements.iter().take(MAX_FIND_RESULTS).map(element_summary).collect();
        json!({ "success": true, "count": items.len(), "truncated": truncated, "elements": items })
    }

    pub fn click_element(name: &str, role: Option<&str>, window: Option<&str>, backend: &dyn InputBackend) -> Value {
        let automation = match automation() {
            Ok(automation) => automation,
            Err(error) => return error_result(error, "internal"),
        };
        let root = match resolve_root(&automation, window) {
            Ok(root) => root,
            Err(error) => return error_result(error, "not_found"),
        };
        if let Some(refusal) = own_window_guard(&root) {
            return refusal;
        }

        let mut matcher = automation.create_matcher().from_ref(&root).contains_name(name).timeout(SEARCH_TIMEOUT_MS);
        if let Some(role) = role {
            match super::role_to_control_type(role) {
                Some(control_type) => matcher = matcher.control_type(control_type),
                None => return error_result(format!("unknown role '{role}'"), "invalid_role"),
            }
        }

        let element = match matcher.find_first() {
            Ok(element) => element,
            Err(_) => {
                return json!({
                    "success": false, "found": false,
                    "error": format!("no element named '{name}' found"), "errorCode": "not_found",
                })
            }
        };

        if let Err(error) = element.set_focus() {
            return json!({
                "success": false, "found": true,
                "error": format!("could not focus the element before clicking: {error}"),
                "errorCode": "focus_failed",
            });
        }
        std::thread::sleep(Duration::from_millis(150));

        let invoked = element
            .get_pattern::<uiautomation::patterns::UIInvokePattern>()
            .and_then(|pattern| pattern.invoke())
            .is_ok();

        let clicked_at = if invoked {
            None
        } else {
            match element.get_click_point() {
                Ok(point) => {
                    let (x, y) = (point.get_x(), point.get_y());
                    match backend.move_to(x, y).and_then(|()| backend.click(MouseButton::Left)) {
                        Ok(()) => Some((x, y)),
                        Err(error) => {
                            return json!({
                                "success": false, "found": true,
                                "error": format!("found the element but the click failed: {error}"),
                                "errorCode": "input_failed",
                            })
                        }
                    }
                }
                Err(error) => {
                    return json!({
                        "success": false, "found": true,
                        "error": error.to_string(), "errorCode": "no_click_point",
                    })
                }
            }
        };

        json!({
            "success": true,
            "found": true,
            "invoked": invoked,
            "clickedAt": clicked_at.map(|(x, y)| json!({ "x": x, "y": y })),
            "name": element.get_name().unwrap_or_default(),
        })
    }

    pub fn focused_element_is_password() -> Option<bool> {
        automation().ok()?.get_focused_element().ok()?.is_password().ok()
    }
}

#[cfg(not(target_os = "windows"))]
mod live {
    use super::super::input_backend::InputBackend;

    pub fn find(_name: Option<&str>, _role: Option<&str>, _window: Option<&str>) -> super::Value {
        super::error_result(super::UNSUPPORTED, "unsupported")
    }

    pub fn click_element(_name: &str, _role: Option<&str>, _window: Option<&str>, _backend: &dyn InputBackend) -> super::Value {
        super::error_result(super::UNSUPPORTED, "unsupported")
    }

    pub fn focused_element_is_password() -> Option<bool> {
        None
    }
}

pub use live::{click_element, find, focused_element_is_password};

// Suíte em arquivo separado só pela regra de 300 linhas do AGENTS.md — mesmo padrão de
// `tool.rs`/`tool_tests.rs`. Continua sendo o mesmo módulo `ui_automation::tests`.
#[cfg(test)]
#[path = "ui_automation_tests.rs"]
mod tests;
