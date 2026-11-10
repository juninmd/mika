//! Tools report failure in two shapes: `Err(..)` from the registry and `Ok({"success": false})`
//! from the handler itself — which includes the approval gate's denial. Counting only `Err` as a
//! failure told the renderer a denied `clear_history` had succeeded.

use serde_json::Value;

pub fn succeeded(outcome: &Result<Value, String>) -> bool {
    match outcome {
        Ok(value) => value.get("success").and_then(Value::as_bool) != Some(false),
        Err(_) => false,
    }
}

/// The user said no (approval dialog, timeout, no handler): the tool never ran, so it is no evidence
/// the tool is broken. Counting it as a failure blocked a perfectly healthy tool for 5 minutes after
/// three refusals.
pub fn is_user_denial(outcome: &Result<Value, String>) -> bool {
    matches!(outcome, Ok(value) if value.get("error").and_then(Value::as_str) == Some("user_denied"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_denied_tool_is_not_a_success() {
        assert!(!succeeded(&Ok(json!({ "success": false, "error": "user_denied" }))));
    }

    #[test]
    fn errors_fail_and_plain_values_succeed() {
        assert!(!succeeded(&Err("boom".into())));
        assert!(succeeded(&Ok(json!({ "success": true }))));
        assert!(succeeded(&Ok(json!("12:00"))));
        assert!(succeeded(&Ok(json!({ "data": 1 }))));
    }

    #[test]
    fn only_the_approval_refusal_is_a_user_denial() {
        assert!(is_user_denial(&Ok(json!({ "success": false, "error": "user_denied" }))));
        assert!(!is_user_denial(&Ok(json!({ "success": false, "error": "boom" }))));
        assert!(!is_user_denial(&Err("user_denied".into())));
    }
}
