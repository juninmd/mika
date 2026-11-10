//! Porte de `toCatalogInputSchema` (`main/ipc/utils.ts`) — o achatamento do JSON Schema para a
//! forma que a UI de settings desenha.
//!
//! **Por que não devolver o schema cru.** `ToolCatalogItem` (`registry.rs`) guarda o JSON Schema
//! completo, que é o que o LLM consome. A tela de tools lê outra coisa: `{ type, properties: {
//! nome: { type, description, enum } }, required }`, e nada mais. Mandar o cru faria a UI ler
//! `properties.x.type` de um `anyOf`/`$ref` e desenhar campo vazio sem erro nenhum — a classe de
//! defeito que este repositório já pagou com controle sumindo da tela em silêncio.
//!
//! `normalize_schema_type` cai para `"string"` quando o tipo não é legível. É o mesmo default do
//! TS, e ele é deliberado: um campo desenhado como texto é editável e visivelmente errado; um campo
//! sem tipo não desenha nada.

use serde_json::{json, Map, Value};

use super::registry::ToolCatalogItem;

fn normalize_schema_type(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        // `type: ["string", "null"]` (a forma que um campo opcional toma em vários geradores) vira
        // o primeiro tipo legível, não a lista inteira — a UI espera uma string.
        Some(Value::Array(items)) => items
            .iter()
            .find_map(Value::as_str)
            .unwrap_or("string")
            .to_string(),
        _ => "string".to_string(),
    }
}

/// Só os itens que já são string — porte de `toStringList`. Um `enum` misto (`['a', 1]`) perde o
/// `1` em vez de virar `["a", "1"]`: inventar a coerção faria a UI oferecer um valor que a tool
/// recusaria na validação.
fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Achata um JSON Schema de tool para a forma do catálogo.
pub fn to_catalog_input_schema(schema: &Value) -> Value {
    let properties = schema.get("properties").and_then(Value::as_object);
    let mut catalog_properties = Map::new();

    for (name, property) in properties.into_iter().flatten() {
        let mut field = Map::new();
        field.insert("type".into(), json!(normalize_schema_type(property.get("type"))));
        if let Some(description) = property.get("description").and_then(Value::as_str) {
            field.insert("description".into(), json!(description));
        }
        let variants = string_list(property.get("enum"));
        if !variants.is_empty() {
            field.insert("enum".into(), json!(variants));
        }
        catalog_properties.insert(name.clone(), Value::Object(field));
    }

    json!({
        "type": normalize_schema_type(schema.get("type")),
        "properties": Value::Object(catalog_properties),
        "required": string_list(schema.get("required")),
    })
}

/// A forma exata que `mika:tools-list` devolve por item — mesmos campos de `mapToolsByPlugin`
/// (`tool-access.ts`), mais `destructive`, que o Rust já carrega e o TS expõe no próprio `ToolDef`.
pub fn catalog_entry(item: &ToolCatalogItem) -> Value {
    let mut entry = Map::new();
    entry.insert("name".into(), json!(item.name));
    if let Some(description) = &item.description {
        entry.insert("description".into(), json!(description));
    }
    entry.insert("inputSchema".into(), to_catalog_input_schema(&item.input_schema));
    entry.insert("pluginId".into(), json!(item.plugin_id));
    entry.insert("requiresWindow".into(), json!(item.requires_window));
    entry.insert("destructive".into(), json!(item.destructive));
    Value::Object(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> ToolCatalogItem {
        ToolCatalogItem {
            name: "steam".into(),
            description: Some("Steam library".into()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "description": "o que fazer", "enum": ["library", "achievements"] },
                    "appId": { "type": ["number", "null"] },
                },
                "required": ["action"],
            }),
            plugin_id: "mika.gamer".into(),
            requires_window: false,
            destructive: false,
        }
    }

    /// Os nomes de campo são camelCase porque é o que o renderer lê. Um `input_schema` snake_case
    /// aqui não quebraria nada visível: a tela de tools desenharia a linha sem schema nenhum.
    #[test]
    fn the_entry_uses_the_camel_case_field_names_the_renderer_reads() {
        let entry = catalog_entry(&item());
        assert_eq!(entry["name"], json!("steam"));
        assert_eq!(entry["pluginId"], json!("mika.gamer"));
        assert_eq!(entry["requiresWindow"], json!(false));
        assert!(entry.get("inputSchema").is_some(), "inputSchema é o que a UI desenha");
        assert!(entry.get("input_schema").is_none(), "snake_case aqui deixaria a tela sem schema, sem erro");
    }

    #[test]
    fn a_union_type_collapses_to_the_first_readable_type() {
        let flattened = to_catalog_input_schema(&item().input_schema);
        assert_eq!(flattened["properties"]["appId"]["type"], json!("number"));
        assert_eq!(flattened["properties"]["action"]["enum"], json!(["library", "achievements"]));
        assert_eq!(flattened["required"], json!(["action"]));
    }

    /// Schema sem `properties` (tool sem argumento) vira objeto VAZIO, não ausência de campo: a UI
    /// itera `Object.entries(properties)` e um `undefined` ali lança em vez de desenhar nada.
    #[test]
    fn a_schema_without_properties_still_carries_an_empty_object() {
        let flattened = to_catalog_input_schema(&json!({ "type": "object" }));
        assert_eq!(flattened["properties"], json!({}));
        assert_eq!(flattened["required"], json!([]));
    }

    /// Tipo ilegível cai para `"string"` — mesmo default do TS, pelo motivo do cabeçalho.
    #[test]
    fn an_unreadable_type_falls_back_to_string_instead_of_disappearing() {
        let flattened = to_catalog_input_schema(&json!({ "properties": { "x": { "anyOf": [] } } }));
        assert_eq!(flattened["type"], json!("string"));
        assert_eq!(flattened["properties"]["x"]["type"], json!("string"));
    }

    /// `description` ausente é campo AUSENTE, não `null`: o TS usa `undefined`, e um `null` no JSON
    /// faria `typeof description === 'string'` falhar do mesmo jeito mas ocupar espaço no payload.
    #[test]
    fn a_missing_description_is_omitted_rather_than_null() {
        let flattened = to_catalog_input_schema(&json!({ "properties": { "x": { "type": "string" } } }));
        assert!(flattened["properties"]["x"].get("description").is_none());
        assert!(flattened["properties"]["x"].get("enum").is_none());
    }
}
