//! Declaração de settings (`FeatureSettingsDeclaration`, `packages/sdk/src/contracts/config-ui`)
//! construída a partir do schema de config JÁ PORTADO (`config/schemas/<domínio>.rs`) — evita
//! duplicar a lista de campos numa segunda estrutura Rust que divergiria da validação real.
//! Consumida por `user`/`character` para se registrarem em `crate::feature_registry::shared()`
//! (canal `mika:settings-declarations`, ligado por outra frente).
//!
//! ## Duas saídas: um cartão só, ou os grupos do TS
//!
//! [`declare_namespace`] transforma um `Namespace` inteiro em UM `SettingsGroup`, na ordem em que o
//! schema declara os campos. Serve para aba sem subdivisão — `mood`, cujo `mood.schema.ts` também
//! declara um grupo só.
//!
//! [`declare_fields`] devolve os MESMOS campos sem agrupar, e é o que `character`/`user` usam para
//! recompor os grupos do TS (`character.schema.ts` tem 3, `user.schema.ts` tem 7), cada um com seu
//! `component`/`badge`/`columns`. Essa distinção existe porque um cartão único nessas duas abas era
//! regressão de verdade, não simplificação: `AGENTS.md` §3.6 as nomeia "a referência canônica visual
//! e de UX para todas as interfaces", com agrupamento em `GlassSection`. Achatá-las é perder
//! justamente o que o resto da UI deve copiar.
//!
//! O que NÃO se duplica é o `SchemaRegistry`: ele continua a única fonte de quais campos existem, de
//! que tipo e com que limites. O mapa de agrupamento que vive em cada `feature.rs` responde outra
//! pergunta — em qual grupo cada campo aparece — e não revalida nada.

use serde_json::{json, Map, Value};

use crate::config::registry::SchemaRegistry;
use crate::config::schema::{FieldKind, FieldSchema};

/// Infere o `SettingsFieldType` (`packages/sdk/src/contracts/config-ui/index.ts`). `ui.control`
/// vence quando já é um dos valores válidos do contrato (ex.: `"textarea"`, `"select"`); senão cai
/// para o tipo inferido do `FieldKind`, mesma regra de fallback que `inferFieldMeta` (TS) aplica a
/// um campo sem `m({...})`.
fn field_type(field: &FieldSchema) -> &'static str {
    const VALID: &[&str] =
        &["text", "number", "range", "boolean", "select", "password", "textarea", "url", "object"];
    if let Some(control) = field.ui.as_ref().and_then(|ui| ui.control) {
        if VALID.contains(&control) {
            return control;
        }
    }
    match field.kind {
        FieldKind::Bool => "boolean",
        FieldKind::Int { .. } | FieldKind::Float { .. } => "number",
        FieldKind::Str { .. } => "text",
        FieldKind::Enum { .. } => "select",
        FieldKind::List | FieldKind::Map => "object",
    }
}

/// Os limites que o `FieldKind` já conhece — `min`/`max`/`step`/`minLength`/`maxLength`, todos
/// declarados em `packages/sdk/src/contracts/config-ui`.
///
/// **Por que isto não é cosmético.** Sem eles um `range` chega ao renderer sem faixa: o
/// `GlassSlider` não tem de onde tirar o intervalo, e um `number` aceita qualquer coisa na tela
/// para só ser recusado depois, pelo `ConfigStore`. O usuário vê um controle que parece funcionar e
/// um erro que aparece tarde — o schema SEMPRE soube o limite (`FieldKind::Int { min, max }`), ele
/// só não estava atravessando para a UI.
///
/// `step` só para `Float`: em `Int` o passo é 1 por definição do tipo, e mandar `step: 1` explícito
/// não acrescenta nada que o `type: "number"` já não diga.
fn insert_bounds(out: &mut Map<String, Value>, field: &FieldSchema) {
    match field.kind {
        FieldKind::Int { min, max } => {
            if let Some(min) = min {
                out.insert("min".into(), json!(min));
            }
            if let Some(max) = max {
                out.insert("max".into(), json!(max));
            }
        }
        FieldKind::Float { min, max } => {
            if let Some(min) = min {
                out.insert("min".into(), json!(min));
            }
            if let Some(max) = max {
                out.insert("max".into(), json!(max));
            }
        }
        FieldKind::Str { min_len, max_len } => {
            if let Some(min_len) = min_len {
                out.insert("minLength".into(), json!(min_len));
            }
            if let Some(max_len) = max_len {
                out.insert("maxLength".into(), json!(max_len));
            }
        }
        FieldKind::Bool | FieldKind::Enum { .. } | FieldKind::List | FieldKind::Map => {}
    }
}

fn field_json(namespace: &str, field: &FieldSchema) -> Value {
    let mut out = Map::new();
    out.insert("configPath".into(), json!(format!("{namespace}.{}", field.key)));
    out.insert("type".into(), json!(field_type(field)));
    insert_bounds(&mut out, field);

    match &field.ui {
        Some(ui) => {
            out.insert("label".into(), json!(ui.label));
            if let Some(description) = ui.description {
                out.insert("description".into(), json!(description));
            }
            if let Some(helper_text) = ui.helper_text {
                out.insert("helperText".into(), json!(helper_text));
            }
            if !ui.options.is_empty() {
                let options: Vec<Value> =
                    ui.options.iter().map(|option| json!({ "label": option.label, "value": option.value })).collect();
                out.insert("options".into(), json!(options));
            }
        }
        // Campo sem `m({...})` no original (ex.: `character.slangEnabled`) — mesmo fallback do TS
        // `toLabel(key)`: a própria chave, sem quebrar a serialização por um `label` ausente.
        None => {
            out.insert("label".into(), json!(field.key));
        }
    }

    Value::Object(out)
}

/// Os campos de um namespace inteiro, já com `configPath`/`type`/... inferidos do schema
/// (`field_json`), SEM agrupar num cartão. Ponto de extensão para quem precisa recompor os MESMOS
/// campos em VÁRIOS grupos — `features::character::feature`/`features::user::feature` mapeiam cada
/// chave ao grupo do `*.schema.ts` correspondente por cima disto, sem reabrir o `SchemaRegistry`
/// por conta própria. Duas fontes da verdade para "quais campos existem, de que tipo, com que
/// limites" é o que o cabeçalho deste arquivo recusa; um mapa de AGRUPAMENTO ao lado é uma
/// pergunta diferente ("em qual grupo cada campo aparece"), não uma segunda validação.
///
/// Devolve `None` se o namespace não existir no `schema` — mesma regra de [`declare_namespace`].
pub fn declare_fields(schema: &SchemaRegistry, namespace: &str) -> Option<Vec<Value>> {
    let declaration = schema.declarations().into_iter().find(|entry| entry.namespace == namespace)?;
    Some(declaration.fields.iter().map(|field| field_json(namespace, field)).collect())
}

/// Monta a `FeatureSettingsDeclaration` de um namespace inteiro como um cartão único. Devolve
/// `None` se o namespace não existir no `schema` (nada a declarar — chamador decide se isso é erro).
pub fn declare_namespace(
    schema: &SchemaRegistry,
    namespace: &'static str,
    tab_id: &'static str,
    tab_label: &'static str,
    tab_icon: &'static str,
    group_title: &'static str,
) -> Option<Value> {
    let fields = declare_fields(schema, namespace)?;

    Some(json!({
        "tabId": tab_id,
        "tabLabel": tab_label,
        "tabIcon": tab_icon,
        "groups": [{ "title": group_title, "fields": fields }],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::Namespace;
    use crate::config::schema::{boolean, choice, float_range, int_range, text, FieldUi, UiOption};

    fn schema() -> SchemaRegistry {
        SchemaRegistry::new(vec![Namespace {
            name: "demo",
            fields: vec![
                boolean("enabled", true).with_ui(FieldUi::new("settings.demo.field.enabled.label")),
                text("name", "").with_ui(FieldUi::new("settings.demo.field.name.label").control("textarea")),
                choice("mode", "a", &["a", "b"]).with_ui(
                    FieldUi::new("settings.demo.field.mode.label")
                        .options(&[UiOption { label: "A", value: "a" }, UiOption { label: "B", value: "b" }]),
                ),
                text("noUi", ""),
                int_range("count", 5, 1, 10).with_ui(FieldUi::new("settings.demo.field.count.label")),
                float_range("rate", 0.5, 0.0, 1.0).with_ui(FieldUi::new("settings.demo.field.rate.label")),
            ],
        }])
    }

    /// O schema SEMPRE soube o intervalo; o que faltava era ele atravessar para a UI. Sem isto um
    /// `range` chega ao renderer sem faixa e um `number` aceita na tela o que o `ConfigStore` vai
    /// recusar depois — controle que parece funcionar e erro que aparece tarde.
    #[test]
    fn numeric_fields_carry_the_bounds_the_schema_already_validates() {
        let declaration = declare_namespace(&schema(), "demo", "demo", "Demo", "Settings", "Demo").unwrap();
        let fields = declaration["groups"][0]["fields"].as_array().unwrap();
        let by_path = |suffix: &str| {
            fields.iter().find(|field| field["configPath"] == json!(format!("demo.{suffix}"))).unwrap()
        };

        assert_eq!(by_path("count")["min"], json!(1));
        assert_eq!(by_path("count")["max"], json!(10));
        assert_eq!(by_path("rate")["min"], json!(0.0));
        assert_eq!(by_path("rate")["max"], json!(1.0));

        // Booleano não tem faixa, e emitir `min`/`max` nele faria o renderer desenhar um controle
        // que não existe — o Zod do lado TS descarta campo não declarado, então o erro seria mudo.
        assert!(by_path("enabled").get("min").is_none());
        assert!(by_path("enabled").get("max").is_none());
    }

    #[test]
    fn declares_every_field_with_config_path_and_inferred_type() {
        let declaration = declare_namespace(&schema(), "demo", "demo", "Demo", "Settings", "Demo").unwrap();

        assert_eq!(declaration["tabId"], json!("demo"));
        let fields = declaration["groups"][0]["fields"].as_array().unwrap();
        // Todos os campos do namespace, nenhum filtrado: um campo que o schema declara e que não
        // chega aqui some da tela sem erro nenhum.
        assert_eq!(fields.len(), 6);

        assert_eq!(fields[0]["configPath"], json!("demo.enabled"));
        assert_eq!(fields[0]["type"], json!("boolean"));

        // `control("textarea")` vence sobre o tipo inferido do `FieldKind::Str`.
        assert_eq!(fields[1]["type"], json!("textarea"));

        assert_eq!(fields[2]["type"], json!("select"));
        assert_eq!(fields[2]["options"][1]["value"], json!("b"));

        // Sem `m({...})`: rotulado pela própria chave, nunca omitido.
        assert_eq!(fields[3]["label"], json!("noUi"));
    }

    #[test]
    fn unknown_namespace_is_none_not_a_panic() {
        assert!(declare_namespace(&schema(), "does-not-exist", "x", "X", "X", "X").is_none());
    }

    /// [`declare_fields`] é o mesmo cálculo de campo que [`declare_namespace`] usa por baixo —
    /// quem recompõe vários grupos por cima (`features::character::feature`) parte exatamente
    /// destes `Value`, sem reabrir o `SchemaRegistry`.
    #[test]
    fn declare_fields_matches_declare_namespace_fields() {
        let via_namespace = declare_namespace(&schema(), "demo", "demo", "Demo", "Settings", "Demo").unwrap();
        let via_fields = declare_fields(&schema(), "demo").unwrap();
        assert_eq!(via_namespace["groups"][0]["fields"], json!(via_fields));
    }

    #[test]
    fn declare_fields_is_none_for_an_unknown_namespace() {
        assert!(declare_fields(&schema(), "does-not-exist").is_none());
    }
}
