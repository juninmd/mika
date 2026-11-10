//! Port de `packages/sdk/src/features/localize-settings.ts::localizeFeatureSettings` — resolve as
//! chaves i18n de uma `Declaration` (`tabLabel`/`tabDescription`/`tabGroup`/`heroNote`, e por
//! grupo `title`/`description`, e por campo `label`/`description`/`helperText`/`patternError`/
//! `options[].label`) contra o locale atual, usando `locales::t`.
//!
//! Opera sobre `serde_json::Value` solto (não um `FeatureSettingsDeclaration` tipado — o registro
//! não duplica esse contrato, ver `feature_registry::types::Declaration`), então cada passo
//! confere a forma antes de mexer: um campo ausente ou de tipo errado é deixado como está, nunca
//! pânico — mesma postura defensiva de `feature_registry::merge`.

use serde_json::Value;

use super::locales::t;

fn localize_in_place(obj: &mut serde_json::Map<String, Value>, key: &str, locale: &str) {
    if let Some(Value::String(raw)) = obj.get(key) {
        let resolved = t(raw, locale);
        obj.insert(key.to_string(), Value::String(resolved));
    }
}

fn localize_group(group: &mut Value, locale: &str) {
    let Some(obj) = group.as_object_mut() else {
        return;
    };
    localize_in_place(obj, "title", locale);
    localize_in_place(obj, "description", locale);

    let Some(fields) = obj.get_mut("fields").and_then(Value::as_array_mut) else {
        return;
    };
    for field in fields {
        localize_field(field, locale);
    }
}

fn localize_field(field: &mut Value, locale: &str) {
    let Some(obj) = field.as_object_mut() else {
        return;
    };
    localize_in_place(obj, "label", locale);
    localize_in_place(obj, "description", locale);
    localize_in_place(obj, "helperText", locale);
    localize_in_place(obj, "patternError", locale);

    let Some(options) = obj.get_mut("options").and_then(Value::as_array_mut) else {
        return;
    };
    for option in options {
        if let Some(option_obj) = option.as_object_mut() {
            localize_in_place(option_obj, "label", locale);
        }
    }
}

/// Resolve as chaves i18n de uma lista de `Declaration` contra `locale`. Devolve uma cópia — a
/// declaração registrada no `FeatureRegistry` continua crua (ver o cabeçalho de
/// `feature_registry/mod.rs`: i18n é responsabilidade de quem lê, não do registro).
pub fn localize_feature_settings(declarations: &[Value], locale: &str) -> Vec<Value> {
    declarations
        .iter()
        .map(|declaration| {
            let mut localized = declaration.clone();
            let Some(obj) = localized.as_object_mut() else {
                return localized;
            };

            localize_in_place(obj, "tabLabel", locale);
            localize_in_place(obj, "tabDescription", locale);
            localize_in_place(obj, "tabGroup", locale);
            localize_in_place(obj, "heroNote", locale);

            if let Some(groups) = obj.get_mut("groups").and_then(Value::as_array_mut) {
                for group in groups {
                    localize_group(group, locale);
                }
            }

            localized
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_tab_group_and_field_level_keys() {
        let declarations = vec![json!({
            "tabId": "vision",
            "tabLabel": "settings.vision.tab.label",
            "tabDescription": "settings.vision.tab.description",
            "groups": [{
                "title": "settings.vision.tab.label",
                "fields": [{
                    "configPath": "vision.enabled",
                    "label": "settings.vision.field.enabled",
                    "description": "settings.vision.field.enabled",
                }],
            }],
        })];

        let localized = localize_feature_settings(&declarations, "pt-BR");

        assert_eq!(localized[0]["tabLabel"], json!("Visão"));
        assert_eq!(
            localized[0]["groups"][0]["fields"][0]["label"],
            json!("Ativar Visão")
        );
    }

    /// Rótulo literal (não chave i18n) passa reto — `t()` devolve a própria entrada quando não
    /// acha em dicionário nenhum.
    #[test]
    fn literal_option_labels_pass_through_unchanged() {
        let declarations = vec![json!({
            "tabId": "llm",
            "tabLabel": "settings.llm.tab.label",
            "groups": [{
                "title": "settings.llm.group.connection",
                "fields": [{
                    "configPath": "llm.provider",
                    "label": "settings.llm.field.provider.label",
                    "options": [{ "label": "Ollama (local)", "value": "ollama" }],
                }],
            }],
        })];

        let localized = localize_feature_settings(&declarations, "pt-BR");
        assert_eq!(
            localized[0]["groups"][0]["fields"][0]["options"][0]["label"],
            json!("Ollama (local)")
        );
    }

    /// Uma `Declaration` fora do formato esperado (sem `groups`, por exemplo) não gera pânico —
    /// só não localiza o que não existe.
    #[test]
    fn a_malformed_declaration_is_left_as_is_instead_of_panicking() {
        let declarations = vec![json!({ "tabId": "sem-groups" })];
        let localized = localize_feature_settings(&declarations, "pt-BR");
        assert_eq!(localized[0]["tabId"], json!("sem-groups"));
    }
}
