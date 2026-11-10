//! Refuses to click/type/press keys against Mika's own windows — the LLM asked to "click the
//! settings toggle" is a self-modification vector the approval dialog alone doesn't stop (the
//! human approves "let Mika click something", not each pixel).
//!
//! **Detection, not a window list.** Every `WebviewWindow` Mika opens (main, settings, wizard,
//! plugin-view, …) is a native window of THIS same OS process — Tauri/WebView2 windows are not
//! separate child processes. So "is the foreground window one of Mika's" reduces to "does the
//! foreground window belong to this process", which needs no `AppHandle`/window-label list to stay
//! correct as windows are added or renamed.
//!
//! **Windows-only probe, like `system_signals::active_window`.** That module documents the same
//! cut: the Electron original only armed its watcher on `win32`, and this crate's `windows` API
//! access is the same one already declared for it (`Cargo.toml`, `Win32_UI_WindowsAndMessaging`).
//! On Linux/macOS the probe can't run yet — the approval dialog remains the only guard there,
//! same as it is today for the whole computer-use surface on every OS. `foreground_guard.rs`
//! duplicates ~10 lines of that module's `GetForegroundWindow` call instead of importing it,
//! deliberately: this feature is worked on in its own worktree (see AGENTS.md — `computer_use` is
//! its own feature module) and reaching into `system_signals` would couple two parallel work
//! packages over a handful of lines.

/// Pure decision: refuse only when we could positively identify the foreground window as this
/// process. `None` (probe unavailable/failed) is NOT refused — see the module header: a platform
/// gap in the secondary check must not silently disable the whole tool everywhere the primary gate
/// (human approval) already runs.
pub fn is_own_window(foreground_pid: Option<u32>, current_pid: u32) -> bool {
    foreground_pid == Some(current_pid)
}

#[cfg(target_os = "windows")]
pub fn foreground_window_pid() -> Option<u32> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        None
    } else {
        Some(pid)
    }
}

/// No probe outside Windows yet — same cut as `system_signals::active_window::probe_foreground`.
#[cfg(not(target_os = "windows"))]
pub fn foreground_window_pid() -> Option<u32> {
    None
}

/// `None` when we cannot verify (probe unavailable/failed) — the caller decides whether that means
/// "proceed" (approval already ran) or "refuse", never this function.
pub fn foreground_is_own_window() -> Option<bool> {
    foreground_window_pid().map(|pid| is_own_window(Some(pid), std::process::id()))
}

/// Precise counterpart to [`foreground_window_pid`] for the one action that has coordinates
/// (`click{x,y}`): resolves the window actually AT that point, not whatever currently has OS
/// focus. Mika is a background companion by design — NOT being foreground is the common case — so
/// the foreground-only probe trivially passes while clicking a visible-but-unfocused Mika window
/// (settings on a second monitor, an always-on-top overlay). `click` uses this instead of
/// [`foreground_is_own_window`]; `type_text`/`key` have no coordinates and keep the foreground
/// probe, which is correct for them (keyboard input always lands on whatever has focus).
#[cfg(target_os = "windows")]
pub fn window_at_point_pid(x: i32, y: i32) -> Option<u32> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, WindowFromPoint};

    let hwnd = unsafe { WindowFromPoint(POINT { x, y }) };
    if hwnd.0.is_null() {
        return None;
    }
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid != 0).then_some(pid)
}

/// No probe outside Windows yet — same cut as [`foreground_window_pid`].
#[cfg(not(target_os = "windows"))]
pub fn window_at_point_pid(_x: i32, _y: i32) -> Option<u32> {
    None
}

/// `None` when we cannot verify — same "capability gap degrades to allow, not refuse" rule as
/// [`foreground_is_own_window`].
pub fn point_is_own_window(x: i32, y: i32) -> Option<bool> {
    window_at_point_pid(x, y).map(|pid| is_own_window(Some(pid), std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_when_the_foreground_pid_matches_this_process() {
        assert!(is_own_window(Some(42), 42));
    }

    #[test]
    fn allows_a_different_process() {
        assert!(!is_own_window(Some(42), 99));
    }

    /// Unable to verify is NOT the same as "it is our window" — a probe failure must never look
    /// like a positive match to the caller.
    #[test]
    fn an_unavailable_probe_is_not_treated_as_a_match() {
        assert!(!is_own_window(None, 42));
    }
}
