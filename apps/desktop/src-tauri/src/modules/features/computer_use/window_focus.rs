//! `pc_control focus_window`: brings another app's window to the front. Windows only (the
//! primary target); elsewhere a named `unsupported` result, same cut as `ui_automation`.

use serde_json::{json, Value};

#[cfg(target_os = "windows")]
mod win {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let found = unsafe { &mut *(lparam.0 as *mut Vec<(HWND, String, u32)>) };
        if unsafe { IsWindowVisible(hwnd) }.as_bool() {
            let mut buffer = [0u16; 512];
            let length = unsafe { GetWindowTextW(hwnd, &mut buffer) }.max(0) as usize;
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            found.push((hwnd, String::from_utf16_lossy(&buffer[..length]), pid));
        }
        BOOL(1)
    }

    pub fn visible_windows() -> Vec<(HWND, String, u32)> {
        let mut found: Vec<(HWND, String, u32)> = Vec::new();
        let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut found as *mut _ as isize)) };
        found
    }

    /// Windows only lets a process take the foreground right after user input; a synthetic Alt
    /// tap counts as input, so the second attempt succeeds where the first is refused.
    pub fn bring_to_front(hwnd: HWND) -> bool {
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            if !SetForegroundWindow(hwnd).as_bool() {
                use enigo::{Direction, Enigo, Key, Keyboard, Settings};
                if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
                    let _ = enigo.key(Key::Alt, Direction::Click);
                }
                let _ = SetForegroundWindow(hwnd);
            }
            GetForegroundWindow() == hwnd
        }
    }
}

#[cfg(target_os = "windows")]
pub fn focus(query: &str) -> Value {
    let windows = win::visible_windows();
    let titles: Vec<(String, u32)> = windows.iter().map(|(_, title, pid)| (title.clone(), *pid)).collect();
    let Some(index) = super::pc_control::pick_window(&titles, query, std::process::id()) else {
        return json!({ "success": false, "error": format!("no open window matches \"{query}\""), "errorCode": "not_found" });
    };
    let (hwnd, title, _) = &windows[index];
    if win::bring_to_front(*hwnd) {
        json!({ "success": true, "action": "focus_window", "title": title })
    } else {
        json!({ "success": false, "error": "Windows refused to switch the foreground window", "errorCode": "focus_refused", "title": title })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn focus(_query: &str) -> Value {
    json!({ "success": false, "error": "focus_window is unsupported on this platform (Windows only)", "errorCode": "unsupported" })
}
