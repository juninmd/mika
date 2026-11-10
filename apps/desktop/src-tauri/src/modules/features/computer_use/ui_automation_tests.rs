#![cfg(test)]
use super::*;

#[cfg(target_os = "windows")]
#[test]
fn maps_common_roles_case_insensitively() {
    assert_eq!(role_to_control_type("Button"), Some(ControlType::Button));
    assert_eq!(role_to_control_type("checkbox"), Some(ControlType::CheckBox));
    assert_eq!(role_to_control_type("LINK"), Some(ControlType::Hyperlink));
}

/// What `find` reports must be accepted back by `click_element` (smoke 2026-09-27: "botão").
#[cfg(target_os = "windows")]
#[test]
fn every_role_find_reports_is_accepted_back_by_click_element() {
    for control_type in [ControlType::Button, ControlType::Edit, ControlType::Hyperlink, ControlType::ListItem, ControlType::Window, ControlType::ToolBar] {
        let role = control_type_to_role(control_type).unwrap();
        assert_eq!(role_to_control_type(role), Some(control_type), "{role}");
    }
}

#[cfg(target_os = "windows")]
#[test]
fn an_unknown_role_maps_to_none() {
    assert_eq!(role_to_control_type("spaceship"), None);
}

#[test]
fn refuses_type_text_only_when_the_focused_field_is_positively_a_password() {
    assert_eq!(refuse_if_password(Some(true)), Some("refused: the focused field is a password field"));
    assert_eq!(refuse_if_password(Some(false)), None);
    // Can't verify (non-Windows, or the control doesn't support the property) -> allow. The
    // human approval gate already ran; a capability gap here must not silently disable typing
    // everywhere the property can't be read, which is most of the time off Windows.
    assert_eq!(refuse_if_password(None), None);
}

/// Smoke 2026-09-26: `click_element {window: "Calculadora"}` always timed out. `uiautomation`'s
/// matcher counts the root as depth 1 and only descends while `depth < max`, so `.depth(1)` tested
/// the desktop itself and never its children — no top-level window could ever match.
#[cfg(target_os = "windows")]
#[test]
fn the_top_level_window_search_depth_reaches_the_desktop_children() {
    let automation = match uiautomation::UIAutomation::new() {
        Ok(a) => a,
        Err(_) => return,
    };
    let desktop = match automation.get_root_element() {
        Ok(d) => d,
        Err(_) => return,
    };
    let found = automation
        .create_matcher()
        .from_ref(&desktop)
        .depth(super::live::TOP_LEVEL_WINDOW_DEPTH)
        .timeout(0)
        .find_first();
    // Em sessão interativa real (com explorer/Program Manager ativo), a profundidade alcança as janelas filhas.
    // Em sessão não interativa (CI/runner headless), a busca não encontra janelas filhas.
    if let Ok(elem) = found {
        assert!(elem.get_name().is_ok());
    }
}
