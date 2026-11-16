//! The pointer actions beyond a single click — `move`, `double_click`, `scroll`, `drag` — split
//! from `tool.rs` by the 300-line rule. Same refusals as `click`: a point that lands on a Mika
//! window is refused before anything moves, and every action shows the agent cursor
//! ([`super::cursor_overlay`]) where it acts, so the user sees what Mika is pointing at.

use serde_json::{json, Value};

use super::cursor_overlay::{self, CursorAction};
use super::foreground_guard;
use super::input_backend::{InputBackend, MouseButton};

/// More than this many wheel notches in one call is a runaway, not a request.
const MAX_SCROLL_NOTCHES: i32 = 50;

fn invalid_args(message: &str) -> Value {
    json!({ "success": false, "error": message, "errorCode": "invalid_args" })
}

fn own_window() -> Value {
    json!({ "success": false, "error": "refused: that point is on a Mika window", "errorCode": "own_window" })
}

fn input_failed(error: String) -> Value {
    json!({ "success": false, "error": error, "errorCode": "input_failed" })
}

/// See `tool.rs::coerce_i32`: JSON numbers may arrive as `100.0`.
pub(super) fn coerce_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(Value::as_f64).filter(|n| n.is_finite()).map(|n| n.round() as i32)
}

fn point(args: &Value, x: &str, y: &str) -> Option<(i32, i32)> {
    Some((coerce_i32(args.get(x))?, coerce_i32(args.get(y))?))
}

fn targets_own_window((x, y): (i32, i32)) -> bool {
    foreground_guard::point_is_own_window(x, y) == Some(true)
}

pub(super) fn move_to(args: &Value, backend: &dyn InputBackend) -> Value {
    let Some((x, y)) = point(args, "x", "y") else {
        return invalid_args("move requires numeric 'x' and 'y'");
    };
    cursor_overlay::show(x, y, CursorAction::Move);
    match backend.move_to(x, y) {
        Ok(()) => json!({ "success": true, "action": "move", "x": x, "y": y }),
        Err(error) => input_failed(error),
    }
}

pub(super) fn double_click(args: &Value, backend: &dyn InputBackend) -> Value {
    let Some((x, y)) = point(args, "x", "y") else {
        return invalid_args("double_click requires numeric 'x' and 'y'");
    };
    cursor_overlay::show(x, y, CursorAction::Click);
    if targets_own_window((x, y)) {
        return own_window();
    }
    let result = backend
        .move_to(x, y)
        .and_then(|()| backend.click(MouseButton::Left))
        .and_then(|()| backend.click(MouseButton::Left));
    match result {
        Ok(()) => json!({ "success": true, "action": "double_click", "x": x, "y": y }),
        Err(error) => input_failed(error),
    }
}

/// `amount` is in wheel notches, positive = down (or right with `direction: "right"`).
fn scroll_delta(args: &Value) -> Result<(i32, i32), &'static str> {
    let amount = coerce_i32(args.get("amount")).unwrap_or(3);
    if amount == 0 || amount.abs() > MAX_SCROLL_NOTCHES {
        return Err("scroll 'amount' must be between 1 and 50 notches (negative scrolls the other way)");
    }
    match args.get("direction").and_then(Value::as_str).unwrap_or("down") {
        "down" => Ok((0, amount)),
        "up" => Ok((0, -amount)),
        "right" => Ok((amount, 0)),
        "left" => Ok((-amount, 0)),
        _ => Err("scroll 'direction' must be up, down, left or right"),
    }
}

pub(super) fn scroll(args: &Value, backend: &dyn InputBackend) -> Value {
    let (dx, dy) = match scroll_delta(args) {
        Ok(delta) => delta,
        Err(message) => return invalid_args(message),
    };
    let at = point(args, "x", "y");
    if let Some((x, y)) = at {
        cursor_overlay::show(x, y, CursorAction::Move);
    }
    if at.is_some_and(targets_own_window) {
        return own_window();
    }
    if let Some((x, y)) = at {
        if let Err(error) = backend.move_to(x, y) {
            return input_failed(error);
        }
    }
    match backend.scroll(dx, dy) {
        Ok(()) => json!({ "success": true, "action": "scroll", "dx": dx, "dy": dy }),
        Err(error) => input_failed(error),
    }
}

/// Press at the start, move, release at the end. The button is released even when the move
/// fails, so a failed drag never leaves the user's mouse stuck down.
pub(super) fn drag(args: &Value, backend: &dyn InputBackend) -> Value {
    let (Some(from), Some(to)) = (point(args, "x", "y"), point(args, "to_x", "to_y")) else {
        return invalid_args("drag requires numeric 'x', 'y', 'to_x' and 'to_y'");
    };
    cursor_overlay::show(from.0, from.1, CursorAction::Click);
    if targets_own_window(from) || targets_own_window(to) {
        return own_window();
    }
    if let Err(error) = backend.move_to(from.0, from.1).and_then(|()| backend.button(MouseButton::Left, true)) {
        return input_failed(error);
    }
    cursor_overlay::show(to.0, to.1, CursorAction::Move);
    let moved = backend.move_to(to.0, to.1);
    let released = backend.button(MouseButton::Left, false);
    match moved.and(released) {
        Ok(()) => json!({ "success": true, "action": "drag", "from": [from.0, from.1], "to": [to.0, to.1] }),
        Err(error) => input_failed(error),
    }
}

#[cfg(test)]
mod tests {
    use super::super::input_backend::fake::{FakeInputBackend, Recorded};
    use super::*;

    #[test]
    fn double_click_moves_once_then_clicks_left_twice() {
        let backend = FakeInputBackend::default();
        let result = double_click(&json!({ "x": 10, "y": 20.4 }), &backend);
        assert_eq!(result["success"], json!(true), "{result}");
        assert_eq!(
            backend.calls(),
            vec![Recorded::MoveTo(10, 20), Recorded::Click(MouseButton::Left), Recorded::Click(MouseButton::Left)]
        );
    }

    #[test]
    fn scroll_maps_direction_to_the_wheel_axis_and_sign() {
        let backend = FakeInputBackend::default();
        scroll(&json!({ "direction": "up", "amount": 5 }), &backend);
        scroll(&json!({ "direction": "right" }), &backend);
        assert_eq!(backend.calls(), vec![Recorded::Scroll(0, -5), Recorded::Scroll(3, 0)]);
    }

    #[test]
    fn scroll_at_a_point_moves_there_first() {
        let backend = FakeInputBackend::default();
        scroll(&json!({ "x": 5, "y": 6, "amount": 2 }), &backend);
        assert_eq!(backend.calls(), vec![Recorded::MoveTo(5, 6), Recorded::Scroll(0, 2)]);
    }

    /// A model asking for 10_000 notches would spin a page for minutes; nothing reaches the OS.
    #[test]
    fn scroll_refuses_runaway_or_zero_amounts_and_unknown_directions() {
        let backend = FakeInputBackend::default();
        for args in [json!({ "amount": 0 }), json!({ "amount": 10_000 }), json!({ "direction": "sideways" })] {
            assert_eq!(scroll(&args, &backend)["errorCode"], json!("invalid_args"), "{args}");
        }
        assert!(backend.calls().is_empty());
    }

    #[test]
    fn drag_presses_moves_and_releases_in_order() {
        let backend = FakeInputBackend::default();
        let result = drag(&json!({ "x": 1, "y": 2, "to_x": 30, "to_y": 40 }), &backend);
        assert_eq!(result["success"], json!(true), "{result}");
        assert_eq!(
            backend.calls(),
            vec![
                Recorded::MoveTo(1, 2),
                Recorded::Button(MouseButton::Left, true),
                Recorded::MoveTo(30, 40),
                Recorded::Button(MouseButton::Left, false),
            ]
        );
    }

    /// The user's real mouse button must never stay held because the second move failed.
    #[test]
    fn drag_releases_the_button_even_when_the_move_fails() {
        struct FailSecondMove(FakeInputBackend, std::sync::atomic::AtomicU32);
        impl InputBackend for FailSecondMove {
            fn move_to(&self, x: i32, y: i32) -> Result<(), String> {
                if self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                    return Err("simulated".into());
                }
                self.0.move_to(x, y)
            }
            fn click(&self, b: MouseButton) -> Result<(), String> { self.0.click(b) }
            fn button(&self, b: MouseButton, down: bool) -> Result<(), String> { self.0.button(b, down) }
            fn scroll(&self, dx: i32, dy: i32) -> Result<(), String> { self.0.scroll(dx, dy) }
            fn type_text(&self, t: &str) -> Result<(), String> { self.0.type_text(t) }
            fn send_key_combo(&self, c: &super::super::key_combo::KeyCombo) -> Result<(), String> { self.0.send_key_combo(c) }
        }
        let backend = FailSecondMove(FakeInputBackend::default(), Default::default());
        let result = drag(&json!({ "x": 1, "y": 2, "to_x": 3, "to_y": 4 }), &backend);
        assert_eq!(result["errorCode"], json!("input_failed"));
        assert_eq!(backend.0.calls().last(), Some(&Recorded::Button(MouseButton::Left, false)));
    }

    #[test]
    fn pointer_actions_without_coordinates_are_invalid_and_touch_nothing() {
        let backend = FakeInputBackend::default();
        assert_eq!(move_to(&json!({ "x": 1 }), &backend)["errorCode"], json!("invalid_args"));
        assert_eq!(double_click(&json!({}), &backend)["errorCode"], json!("invalid_args"));
        assert_eq!(drag(&json!({ "x": 1, "y": 2 }), &backend)["errorCode"], json!("invalid_args"));
        assert!(backend.calls().is_empty());
    }
}
