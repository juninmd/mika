//! OS probe of the foreground window, one per platform — the raw half of `active_window.rs` (which
//! owns the loop, the dedup decision and the shared snapshot).
//!
//! - **Windows**: `GetForegroundWindow` + `GetWindowTextW` + `GetWindowThreadProcessId` (unchanged).
//! - **Linux, X11**: EWMH `_NET_ACTIVE_WINDOW` on the root window, its `_NET_WM_NAME` (else
//!   `WM_NAME`) and `_NET_WM_PID`, over `x11rb` — the protocol every X11 window manager speaks, no
//!   `xdotool` child process. One connection, reopened only after an error.
//! - **Linux, Wayland**: unsupported, on purpose. Wayland has no cross-client "which window is
//!   focused" query, and XWayland's `_NET_ACTIVE_WINDOW` only knows X clients: it would keep
//!   reporting the last X window while a native Wayland one has focus — a stale snapshot that the
//!   screen-capture gate (`vision::capture_gate`) would then trust. No snapshot keeps that gate on
//!   its documented fail-open path, exactly as before.
//! - **macOS**: `NSWorkspace.frontmostApplication` — the app, not the window (window titles need
//!   the Accessibility permission): its localized name stands in for the title.
//!
//! The process name comes from `sysinfo` for that one PID only.

use super::foreground::RawForeground;

/// Why a platform has no probe, or `None` when it has one. Pure over the session variables so the
/// Wayland rule is testable on any machine.
pub(super) fn unsupported_reason(os: &str, session_type: Option<&str>, wayland_display: Option<&str>) -> Option<&'static str> {
    match os {
        "windows" | "macos" => None,
        "linux" if session_type == Some("wayland") || (session_type.is_none() && wayland_display.is_some()) => {
            Some("Wayland has no focused-window query for other clients (XWayland only sees X clients)")
        }
        "linux" => None,
        _ => Some("no foreground-window probe for this OS"),
    }
}

/// [`unsupported_reason`] for the running process.
pub(super) fn unsupported_here() -> Option<&'static str> {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    unsupported_reason(std::env::consts::OS, var("XDG_SESSION_TYPE").as_deref(), var("WAYLAND_DISPLAY").as_deref())
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn process_name_for_pid(pid: u32) -> String {
    // Só este PID, só o nome: `System::new_all()` aqui enumerava TODOS os processos (com CPU, disco,
    // memória de cada um), mais discos/rede/sensores, a cada 4s — o custo mais alto da Mika ociosa
    // ao lado de um jogo.
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(sysinfo::ProcessesToUpdate::Some(&[pid]), false, sysinfo::ProcessRefreshKind::nothing());
    system.process(pid).map(|p| p.name().to_string_lossy().into_owned()).unwrap_or_default()
}

#[cfg(target_os = "windows")]
pub(super) fn probe_foreground() -> Option<RawForeground> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }

    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    let title = if len > 0 { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() };

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return Some(RawForeground { title, process_name: String::new(), pid });
    }

    Some(RawForeground { title, process_name: process_name_for_pid(pid), pid })
}

#[cfg(target_os = "macos")]
pub(super) fn probe_foreground() -> Option<RawForeground> {
    let app = objc2_app_kit::NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let name = app.localizedName().map(|name| name.to_string()).unwrap_or_default();
    let pid = u32::try_from(app.processIdentifier()).unwrap_or(0);
    let process_name = if pid == 0 { String::new() } else { process_name_for_pid(pid) };
    let process_name = if process_name.is_empty() { name.clone() } else { process_name };
    Some(RawForeground { title: name, process_name, pid })
}

#[cfg(target_os = "linux")]
pub(super) fn probe_foreground() -> Option<RawForeground> {
    use std::sync::Mutex;
    static X11: Mutex<Option<x11::Session>> = Mutex::new(None);

    let mut session = X11.lock().ok()?;
    if session.is_none() {
        *session = x11::Session::open().inspect_err(|error| log::debug!("janela ativa (X11): sem conexão ({error})")).ok();
    }
    let probed = session.as_ref()?.active_window();
    match probed {
        Ok(Some((title, pid))) => {
            let process_name = if pid == 0 { String::new() } else { process_name_for_pid(pid) };
            Some(RawForeground { title, process_name, pid })
        }
        Ok(None) => None,
        Err(error) => {
            log::debug!("janela ativa (X11): sonda falhou, reconecta no próximo tick ({error})");
            *session = None;
            None
        }
    }
}

#[cfg(target_os = "linux")]
mod x11 {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, Window};
    use x11rb::rust_connection::RustConnection;

    pub(super) struct Session {
        conn: RustConnection,
        root: Window,
        active: Atom,
        net_name: Atom,
        utf8: Atom,
        pid: Atom,
    }

    type Probe<T> = Result<T, Box<dyn std::error::Error>>;

    impl Session {
        pub(super) fn open() -> Probe<Self> {
            let (conn, screen) = x11rb::connect(None)?;
            let root = conn.setup().roots.get(screen).ok_or("screen fora do setup")?.root;
            let atom = |name: &str| -> Probe<Atom> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
            let (active, net_name, utf8, pid) = (atom("_NET_ACTIVE_WINDOW")?, atom("_NET_WM_NAME")?, atom("UTF8_STRING")?, atom("_NET_WM_PID")?);
            Ok(Self { conn, root, active, net_name, utf8, pid })
        }

        fn property(&self, window: Window, name: impl Into<Atom>, kind: impl Into<Atom>, len: u32) -> Probe<Vec<u8>> {
            Ok(self.conn.get_property(false, window, name, kind, 0, len)?.reply()?.value)
        }

        fn card32(&self, window: Window, name: Atom, kind: AtomEnum) -> Probe<Option<u32>> {
            Ok(self.conn.get_property(false, window, name, kind, 0, 1)?.reply()?.value32().and_then(|mut values| values.next()))
        }

        /// `(title, pid)` of the focused window; `None` when the WM reports none (desktop focused).
        pub(super) fn active_window(&self) -> Probe<Option<(String, u32)>> {
            let Some(window) = self.card32(self.root, self.active, AtomEnum::WINDOW)?.filter(|w| *w != 0) else { return Ok(None) };
            let mut title = self.property(window, self.net_name, self.utf8, 1024)?;
            if title.is_empty() {
                title = self.property(window, AtomEnum::WM_NAME, AtomEnum::STRING, 1024)?;
            }
            let pid = self.card32(window, self.pid, AtomEnum::CARDINAL)?.unwrap_or(0);
            Ok(Some((String::from_utf8_lossy(&title).into_owned(), pid)))
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub(super) fn probe_foreground() -> Option<RawForeground> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x11_windows_and_macos_have_a_probe() {
        assert_eq!(unsupported_reason("windows", None, None), None);
        assert_eq!(unsupported_reason("macos", None, None), None);
        assert_eq!(unsupported_reason("linux", Some("x11"), None), None);
        assert_eq!(unsupported_reason("linux", None, None), None, "a bare X session (no XDG_SESSION_TYPE)");
    }

    /// XWayland would answer with the last X client while a Wayland window has focus: a stale
    /// snapshot the capture gate would trust. Wayland is declared unsupported instead.
    #[test]
    fn wayland_is_unsupported_even_with_xwayland_running() {
        assert!(unsupported_reason("linux", Some("wayland"), Some("wayland-0")).is_some());
        assert!(unsupported_reason("linux", None, Some("wayland-0")).is_some());
        assert!(unsupported_reason("freebsd", None, None).is_some());
    }
}
