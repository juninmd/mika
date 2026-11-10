//! Validação de argumento de tool contra o JSON Schema declarado — chamada por
//! `ToolRegistry::execute` (`registry.rs`) ANTES de despachar para o handler.
//!
//! Subconjunto de JSON Schema (`type`, `required`, `properties` aninhadas, `enum`, `minimum`/`maximum`,
//! `minLength`/`maxLength`, `additionalProperties` booleano ou schema) — o
//! que os schemas gerados por `toJsonSchema(zodSchema)` de fato produzem para as tools do produto.
//! Não é um validador de JSON Schema completo (sem `$ref`, `anyOf`, formatos): schema ausente ou
//! sem forma de objeto é permissivo, igual ao fallback de `parseToolInput` quando não há
//! `safeParse` no lado TS.

use serde_json::{json, Value};

/// Repairs the argument shapes small models get wrong before [`validate_args`] judges them: a
/// missing/`null`/`""` payload becomes `{}`, an object sent as a JSON string is parsed, and a scalar
/// sent as a string (`"3"`, `"true"`) is converted to the type the schema declares. Anything it
/// cannot convert is left untouched so the validator rejects it with a message the model can act on.
pub(super) fn coerce_args(schema: &Value, args: Value) -> Value {
    if !declares_object(schema) {
        return args;
    }
    let args = match args {
        Value::Null => json!({}),
        Value::String(text) if text.trim().is_empty() => json!({}),
        Value::String(text) => serde_json::from_str::<Value>(&text).ok().filter(Value::is_object).unwrap_or(Value::String(text)),
        other => other,
    };
    let (Value::Object(mut fields), Some(properties)) = (args.clone(), schema.get("properties").and_then(Value::as_object)) else { return args };
    for (field, value) in fields.iter_mut() {
        let expected = properties.get(field).and_then(|s| s.get("type")).and_then(Value::as_str);
        if let Some(fixed) = expected.and_then(|expected| coerce_value(expected, value)) {
            *value = fixed;
        }
    }
    Value::Object(fields)
}

fn coerce_value(expected: &str, value: &Value) -> Option<Value> {
    if matches_json_type(expected, value) {
        return None;
    }
    match (expected, value) {
        ("string", Value::Number(_) | Value::Bool(_)) => Some(json!(value.to_string())),
        ("integer", Value::String(text)) => text.trim().parse::<i64>().ok().map(|n| json!(n)),
        ("number", Value::String(text)) => text.trim().parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| json!(n)),
        ("boolean", Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
            "true" => Some(json!(true)),
            "false" => Some(json!(false)),
            _ => None,
        },
        ("array" | "object", Value::String(text)) => serde_json::from_str::<Value>(text).ok().filter(|parsed| matches_json_type(expected, parsed)),
        _ => None,
    }
}

fn declares_object(schema: &Value) -> bool {
    schema.as_object().is_some_and(|obj| {
        obj.contains_key("properties") || obj.contains_key("required") || obj.get("type").and_then(Value::as_str) == Some("object")
    })
}

pub(super) fn validate_args(schema: &Value, args: &Value) -> Result<(), String> {
    let Some(schema_obj) = schema.as_object().filter(|_| declares_object(schema)) else { return Ok(()) };

    let Some(args_obj) = args.as_object() else {
        return Err(format!("expected an object, got {}", json_type_name(args)));
    };

    if let Some(required) = schema_obj.get("required").and_then(Value::as_array) {
        for field in required.iter().filter_map(Value::as_str) {
            if !args_obj.contains_key(field) {
                return Err(format!("missing required field \"{field}\""));
            }
        }
    }

    let properties = schema_obj.get("properties").and_then(Value::as_object);
    for (field, value) in args_obj {
        match properties.and_then(|p| p.get(field)) {
            Some(field_schema) => check_value(field, field_schema, value)?,
            None => check_extra_field(schema_obj, properties, field, value)?,
        }
    }
    Ok(())
}

/// A field the schema does not list: `additionalProperties: false` rejects it (the message names the
/// real fields, so a misspelled one is fixable); a schema there types it; absent allows anything.
fn check_extra_field(schema: &serde_json::Map<String, Value>, properties: Option<&serde_json::Map<String, Value>>, field: &str, value: &Value) -> Result<(), String> {
    match schema.get("additionalProperties") {
        Some(Value::Bool(false)) => {
            let known: Vec<&str> = properties.map(|p| p.keys().map(String::as_str).collect()).unwrap_or_default();
            Err(format!("unknown field \"{field}\" (allowed: {})", known.join(", ")))
        }
        Some(extra_schema @ Value::Object(_)) => check_value(field, extra_schema, value),
        _ => Ok(()),
    }
}

/// One value against its field schema: type, enum, numeric bounds, string length and — for nested
/// objects — their own properties. `path` is the dotted name shown to the model.
fn check_value(path: &str, schema: &Value, value: &Value) -> Result<(), String> {
    let Some(schema) = schema.as_object() else { return Ok(()) };
    if let Some(expected) = schema.get("type").and_then(Value::as_str) {
        if !matches_json_type(expected, value) {
            return Err(format!("field \"{path}\" expected type \"{expected}\", got {}", json_type_name(value)));
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.contains(value) {
            return Err(format!("field \"{path}\" is not one of the allowed values"));
        }
    }
    check_number_bounds(path, schema, value)?;
    check_length_bounds(path, schema, value)?;
    check_nested_object(path, schema, value)
}

fn check_number_bounds(path: &str, schema: &serde_json::Map<String, Value>, value: &Value) -> Result<(), String> {
    let Some(n) = value.as_f64() else { return Ok(()) };
    let (min, max) = (schema.get("minimum").and_then(Value::as_f64), schema.get("maximum").and_then(Value::as_f64));
    let range = match (min, max) {
        (Some(lo), Some(hi)) if n < lo || n > hi => format!("between {lo} and {hi}"),
        (Some(lo), None) if n < lo => format!("at least {lo}"),
        (None, Some(hi)) if n > hi => format!("at most {hi}"),
        _ => return Ok(()),
    };
    Err(format!("field \"{path}\" must be {range}, got {n}"))
}

fn check_length_bounds(path: &str, schema: &serde_json::Map<String, Value>, value: &Value) -> Result<(), String> {
    let Some(text) = value.as_str() else { return Ok(()) };
    let len = text.chars().count() as u64;
    if let Some(min) = schema.get("minLength").and_then(Value::as_u64).filter(|&min| len < min) {
        return Err(format!("field \"{path}\" must have at least {min} character(s), got {len}"));
    }
    match schema.get("maxLength").and_then(Value::as_u64).filter(|&max| len > max) {
        Some(max) => Err(format!("field \"{path}\" must have at most {max} character(s), got {len}")),
        None => Ok(()),
    }
}

/// Only an object that declares `properties` or a typed `additionalProperties` is walked; a bare
/// `{"type":"object"}` stays permissive like the top-level schema without a shape.
fn check_nested_object(path: &str, schema: &serde_json::Map<String, Value>, value: &Value) -> Result<(), String> {
    let (Some(fields), true) = (value.as_object(), schema.contains_key("properties") || schema.get("additionalProperties").is_some_and(Value::is_object)) else {
        return Ok(());
    };
    let properties = schema.get("properties").and_then(Value::as_object);
    for (name, inner) in fields {
        let inner_path = format!("{path}.{name}");
        match properties.and_then(|p| p.get(name)) {
            Some(inner_schema) => check_value(&inner_path, inner_schema, inner)?,
            None => check_extra_field(schema, properties, &inner_path, inner)?,
        }
    }
    Ok(())
}

fn matches_json_type(expected: &str, value: &Value) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64() || value.as_f64().is_some_and(|n| n.fract() == 0.0),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        // Tipo que este validador não conhece: melhor deixar passar do que rejeitar por engano.
        _ => true,
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema_requiring_text() -> Value {
        json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"]
        })
    }

    #[test]
    fn accepts_valid_args() {
        assert!(validate_args(&schema_requiring_text(), &json!({ "text": "oi" })).is_ok());
    }

    #[test]
    fn rejects_missing_required_field() {
        let err = validate_args(&schema_requiring_text(), &json!({})).unwrap_err();
        assert!(err.contains("missing required field"), "got: {err}");
    }

    #[test]
    fn rejects_wrong_type() {
        let err = validate_args(&schema_requiring_text(), &json!({ "text": 42 })).unwrap_err();
        assert!(err.contains("expected type"), "got: {err}");
    }

    #[test]
    fn rejects_value_outside_declared_enum() {
        let schema = json!({
            "type": "object",
            "properties": { "mode": { "type": "string", "enum": ["a", "b"] } },
            "required": ["mode"]
        });
        let err = validate_args(&schema, &json!({ "mode": "c" })).unwrap_err();
        assert!(err.contains("not one of the allowed values"), "got: {err}");
    }

    #[test]
    fn permissive_when_schema_has_no_object_shape() {
        assert!(validate_args(&json!({ "type": "string" }), &json!("hello")).is_ok());
    }

    #[test]
    fn rejects_non_object_args_when_schema_declares_an_object() {
        let err = validate_args(&schema_requiring_text(), &json!("not an object")).unwrap_err();
        assert!(err.contains("expected an object"), "got: {err}");
    }

    fn typed_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer" },
                "ratio": { "type": "number" },
                "loud": { "type": "boolean" },
                "tags": { "type": "array" },
                "name": { "type": "string" }
            }
        })
    }

    /// Small models emit no-arg calls as `null`, `""` or a JSON string instead of `{}`.
    #[test]
    fn coerce_turns_empty_and_stringified_arguments_into_an_object() {
        let schema = typed_schema();
        assert_eq!(coerce_args(&schema, json!(null)), json!({}));
        assert_eq!(coerce_args(&schema, json!("")), json!({}));
        assert_eq!(coerce_args(&schema, json!("{\"name\":\"a\"}")), json!({ "name": "a" }));
        assert_eq!(coerce_args(&schema, json!("not json")), json!("not json"));
    }

    #[test]
    fn coerce_fixes_scalar_types_the_schema_declares() {
        let args = json!({ "count": "3", "ratio": "0.5", "loud": "true", "tags": "[\"a\"]", "name": 7 });
        assert_eq!(coerce_args(&typed_schema(), args), json!({ "count": 3, "ratio": 0.5, "loud": true, "tags": ["a"], "name": "7" }));
    }

    #[test]
    fn coerce_leaves_values_it_cannot_convert_for_the_validator_to_reject() {
        let args = json!({ "count": "três", "loud": "talvez", "name": { "x": 1 }, "extra": "1" });
        assert_eq!(coerce_args(&typed_schema(), args.clone()), args);
        assert!(validate_args(&typed_schema(), &coerce_args(&typed_schema(), args)).is_err());
    }

    fn bounded_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "minutes": { "type": "integer", "minimum": 0, "maximum": 240 },
                "text": { "type": "string", "minLength": 1, "maxLength": 5 }
            },
            "additionalProperties": false
        })
    }

    #[test]
    fn rejects_numbers_outside_minimum_and_maximum() {
        for bad in [-1, 241] {
            let err = validate_args(&bounded_schema(), &json!({ "minutes": bad })).unwrap_err();
            assert!(err.contains("\"minutes\"") && err.contains("between 0 and 240"), "got: {err}");
        }
        assert!(validate_args(&bounded_schema(), &json!({ "minutes": 0 })).is_ok());
        assert!(validate_args(&bounded_schema(), &json!({ "minutes": 240 })).is_ok());
    }

    #[test]
    fn a_one_sided_bound_names_only_that_side() {
        let schema = json!({ "type": "object", "properties": { "n": { "type": "number", "minimum": 0.5 } } });
        let err = validate_args(&schema, &json!({ "n": 0.1 })).unwrap_err();
        assert!(err.contains("at least 0.5"), "got: {err}");
    }

    #[test]
    fn rejects_strings_outside_the_length_bounds_counted_in_characters() {
        assert!(validate_args(&bounded_schema(), &json!({ "text": "" })).unwrap_err().contains("at least 1 character"));
        assert!(validate_args(&bounded_schema(), &json!({ "text": "seis!!" })).unwrap_err().contains("at most 5 character"));
        assert!(validate_args(&bounded_schema(), &json!({ "text": "ãéíõü" })).is_ok(), "5 caracteres, 10 bytes");
    }

    #[test]
    fn additional_properties_false_names_the_unknown_field() {
        let err = validate_args(&bounded_schema(), &json!({ "minutos": 5 })).unwrap_err();
        assert!(err.contains("unknown field \"minutos\"") && err.contains("minutes, text"), "got: {err}");
    }

    #[test]
    fn extra_fields_pass_when_the_schema_does_not_forbid_them() {
        let schema = json!({ "type": "object", "properties": { "a": { "type": "string" } } });
        assert!(validate_args(&schema, &json!({ "a": "x", "b": 1 })).is_ok());
    }

    #[test]
    fn additional_properties_as_a_schema_types_the_extra_fields() {
        let schema = json!({ "type": "object", "properties": { "socialLinks": { "type": "object", "additionalProperties": { "type": "string" } } } });
        assert!(validate_args(&schema, &json!({ "socialLinks": { "x": "https://a" } })).is_ok());
        let err = validate_args(&schema, &json!({ "socialLinks": { "x": 3 } })).unwrap_err();
        assert!(err.contains("socialLinks.x") && err.contains("string"), "got: {err}");
    }
}
