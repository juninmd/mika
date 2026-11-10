//! The one seam that actually moves the mouse or types — a `trait` so [`super::tool`]'s dispatch
//! logic (argument validation, action routing, the password/own-window refusals) is testable
//! without ever touching the real cursor. Tests inject a recording fake; production wires
//! [`EnigoBackend`], the only caller of the `enigo` crate in this module.
//!
//! One `enigo::Enigo` per call, not a stored instance: `Enigo`'s `Keyboard`/`Mouse` methods take
//! `&mut self`, and [`crate::modules::tools::ToolHandler::call`] takes `&self` (the registry holds
//! handlers behind `Arc<dyn ToolHandler>`, shared across concurrent calls — see
//! `modules/tools/registry/mod.rs`). Opening a fresh connection per call avoids smuggling a
//! platform handle across `.await` points and avoids a lock nobody else in this codebase needs for
//! an occasional, human-approved action.

use enigo::{Coordinate, Direction, Enigo, Settings};

use super::key_combo::KeyCombo;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    /// `"left"`/`"right"`/`"middle"`, defaulting to `Left` — the same default `click {x,y}` takes
    /// when the model omits `button` (a plain click is what "clica em X" means without qualifiers).
    pub fn parse(value: Option<&str>) -> MouseButton {
        match value.map(str::to_ascii_lowercase).as_deref() {
            Some("right") => MouseButton::Right,
            Some("middle") => MouseButton::Middle,
            _ => MouseButton::Left,
        }
    }

    fn to_enigo(self) -> enigo::Button {
        match self {
            MouseButton::Left => enigo::Button::Left,
            MouseButton::Right => enigo::Button::Right,
            MouseButton::Middle => enigo::Button::Middle,
        }
    }
}

/// Injected into [`super::tool::ComputerToolHandler`]; production gets [`EnigoBackend`], tests get
/// a recording fake that never calls into the OS.
pub trait InputBackend: Send + Sync {
    fn move_to(&self, x: i32, y: i32) -> Result<(), String>;
    fn click(&self, button: MouseButton) -> Result<(), String>;
    fn type_text(&self, text: &str) -> Result<(), String>;
    fn send_key_combo(&self, combo: &KeyCombo) -> Result<(), String>;
}

pub struct EnigoBackend;

fn open() -> Result<Enigo, String> {
    Enigo::new(&Settings::default()).map_err(|error| format!("could not open input backend: {error}"))
}

impl InputBackend for EnigoBackend {
    fn move_to(&self, x: i32, y: i32) -> Result<(), String> {
        use enigo::Mouse;
        open()?.move_mouse(x, y, Coordinate::Abs).map_err(|error| error.to_string())
    }

    fn click(&self, button: MouseButton) -> Result<(), String> {
        use enigo::Mouse;
        open()?.button(button.to_enigo(), Direction::Click).map_err(|error| error.to_string())
    }

    fn type_text(&self, text: &str) -> Result<(), String> {
        use enigo::Keyboard;
        open()?.text(text).map_err(|error| error.to_string())
    }

    /// Press every modifier, click the main key, release modifiers in reverse order — releasing
    /// out of order (e.g. Shift before Ctrl in `ctrl+shift+t`) can change what the held key means
    /// on some layouts, so the release order is deliberately the mirror of the press order.
    fn send_key_combo(&self, combo: &KeyCombo) -> Result<(), String> {
        use enigo::Keyboard;
        let mut enigo = open()?;
        for modifier in &combo.modifiers {
            enigo.key(*modifier, Direction::Press).map_err(|error| error.to_string())?;
        }
        let result = enigo.key(combo.main, Direction::Click).map_err(|error| error.to_string());
        for modifier in combo.modifiers.iter().rev() {
            let _ = enigo.key(*modifier, Direction::Release);
        }
        result
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    pub enum Recorded {
        MoveTo(i32, i32),
        Click(MouseButton),
        TypeText(String),
        KeyCombo(KeyCombo),
    }

    /// Records every call instead of touching the OS — what makes `tool.rs`'s dispatch tests safe
    /// to run in CI without a display server, and without ever moving the real cursor.
    #[derive(Default)]
    pub struct FakeInputBackend {
        pub calls: Mutex<Vec<Recorded>>,
        pub fail_next: Mutex<Option<String>>,
        /// When non-zero, `move_to` blocks the calling thread for this long — simulates a slow
        /// synchronous OS call so a test can prove `computer` calls run off the async worker
        /// (`tokio::task::spawn_blocking`) instead of stalling it. Zero (the default) never sleeps,
        /// so every other test stays fast.
        pub block_millis: Mutex<u64>,
    }

    impl FakeInputBackend {
        pub fn calls(&self) -> Vec<Recorded> {
            self.calls.lock().unwrap().clone()
        }

        fn maybe_fail(&self) -> Result<(), String> {
            match self.fail_next.lock().unwrap().take() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }

        fn maybe_block(&self) {
            let millis = *self.block_millis.lock().unwrap();
            if millis > 0 {
                std::thread::sleep(std::time::Duration::from_millis(millis));
            }
        }
    }

    impl InputBackend for FakeInputBackend {
        fn move_to(&self, x: i32, y: i32) -> Result<(), String> {
            self.maybe_fail()?;
            self.maybe_block();
            self.calls.lock().unwrap().push(Recorded::MoveTo(x, y));
            Ok(())
        }

        fn click(&self, button: MouseButton) -> Result<(), String> {
            self.maybe_fail()?;
            self.calls.lock().unwrap().push(Recorded::Click(button));
            Ok(())
        }

        fn type_text(&self, text: &str) -> Result<(), String> {
            self.maybe_fail()?;
            self.calls.lock().unwrap().push(Recorded::TypeText(text.to_string()));
            Ok(())
        }

        fn send_key_combo(&self, combo: &KeyCombo) -> Result<(), String> {
            self.maybe_fail()?;
            self.calls.lock().unwrap().push(Recorded::KeyCombo(combo.clone()));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{FakeInputBackend, Recorded};
    use super::*;

    #[test]
    fn parses_the_three_known_buttons_and_defaults_to_left() {
        assert_eq!(MouseButton::parse(Some("left")), MouseButton::Left);
        assert_eq!(MouseButton::parse(Some("RIGHT")), MouseButton::Right);
        assert_eq!(MouseButton::parse(Some("middle")), MouseButton::Middle);
        assert_eq!(MouseButton::parse(None), MouseButton::Left);
        assert_eq!(MouseButton::parse(Some("nonsense")), MouseButton::Left);
    }

    #[test]
    fn fake_backend_records_calls_instead_of_touching_the_os() {
        let backend = FakeInputBackend::default();
        backend.move_to(10, 20).unwrap();
        backend.click(MouseButton::Right).unwrap();
        backend.type_text("hi").unwrap();

        assert_eq!(
            backend.calls(),
            vec![
                Recorded::MoveTo(10, 20),
                Recorded::Click(MouseButton::Right),
                Recorded::TypeText("hi".into()),
            ]
        );
    }

    #[test]
    fn fake_backend_can_be_told_to_fail_the_next_call_once() {
        let backend = FakeInputBackend::default();
        *backend.fail_next.lock().unwrap() = Some("simulated failure".into());

        assert!(backend.click(MouseButton::Left).is_err());
        assert!(backend.click(MouseButton::Left).is_ok(), "the failure should only apply once");
        assert_eq!(backend.calls().len(), 1);
    }
}
