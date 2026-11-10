//! O modelo de schema de configuração — o que em TypeScript era Zod.
//!
//! Sem sidecar Node não existe Zod em runtime, e o config store é o alicerce de 62 arquivos do
//! backend. Mas o schema nunca foi só validação: ele também É a tela de Settings. Cada campo
//! carrega os metadados (`m({...})`) que o renderer consome por `mika:settings-declarations` para
//! desenhar o controle, com chaves i18n em vez de texto. Um port que guardasse só tipo e default
//! entregaria um app que valida certo e não tem mais painel de configuração.
//!
//! Por isso o modelo aqui é declarativo e serializável, não um `serde` com `#[validate]`: ele
//! precisa responder três perguntas com a mesma fonte — qual o default, o valor é válido, e como
//! isto se desenha na tela.

use serde::Serialize;
use serde_json::{json, Value};

/// O tipo do campo, com as restrições que o Zod correspondente declarava.
///
/// `Int` e `Float` são separados porque `z.number().int()` e `z.number()` recusam entradas
/// diferentes: gravar 1.5 em `episodicListLimit` (que é `.int()`) tem de falhar, e um único
/// `Number` deixaria passar.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FieldKind {
    Bool,
    Int {
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        min: Option<f64>,
        max: Option<f64>,
    },
    Str {
        min_len: Option<usize>,
        max_len: Option<usize>,
    },
    /// `z.enum([...])` — valor fora da lista é recusado.
    Enum {
        variants: &'static [&'static str],
    },
    /// Lista livre (`z.array(...)`). A validação por item fica a cargo de quem consome; o store
    /// garante apenas que é um array, que é o que o Zod fazia na maioria destes campos.
    List,
    /// Objeto livre (`z.record(...)`), usado por mapas de configuração dinâmicos.
    Map,
}

/// Como o campo se desenha. Espelha `m({...})` do lado TypeScript — os valores são CHAVES de
/// tradução (`settings.memory.field.enabled.label`), nunca texto final.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldUi {
    pub label: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'static str>,
    /// `select`, `slider`, `password`… Ausente deixa o renderer escolher pelo tipo.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub control: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helper_text: Option<&'static str>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub options: &'static [UiOption],
}

#[derive(Debug, Clone, Serialize)]
pub struct UiOption {
    pub label: &'static str,
    pub value: &'static str,
}

impl FieldUi {
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            description: None,
            control: None,
            helper_text: None,
            options: &[],
        }
    }

    pub const fn described(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    pub const fn control(mut self, control: &'static str) -> Self {
        self.control = Some(control);
        self
    }

    pub const fn helper(mut self, helper_text: &'static str) -> Self {
        self.helper_text = Some(helper_text);
        self
    }

    pub const fn options(mut self, options: &'static [UiOption]) -> Self {
        self.options = options;
        self
    }
}

/// Um campo de configuração: o nome relativo ao namespace, o tipo, o default e a UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSchema {
    pub key: &'static str,
    #[serde(flatten)]
    pub kind: FieldKind,
    pub default: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<FieldUi>,
}

impl FieldSchema {
    /// Valida um valor contra este campo. A mensagem diz o caminho e o motivo porque ela sobe
    /// inteira para o renderer — `settings-set` recusado sem motivo legível é um controle que
    /// simplesmente não obedece, e o usuário não tem como saber por quê.
    pub fn validate(&self, path: &str, value: &Value) -> Result<(), String> {
        match (&self.kind, value) {
            (FieldKind::Bool, Value::Bool(_)) => Ok(()),

            (FieldKind::Int { min, max }, Value::Number(n)) => {
                let Some(int) = n.as_i64() else {
                    return Err(format!("{path}: esperava inteiro, veio {n}"));
                };
                if min.is_some_and(|m| int < m) || max.is_some_and(|m| int > m) {
                    return Err(format!("{path}: {int} fora da faixa {min:?}..{max:?}"));
                }
                Ok(())
            }

            (FieldKind::Float { min, max }, Value::Number(n)) => {
                let Some(float) = n.as_f64() else {
                    return Err(format!("{path}: esperava número, veio {n}"));
                };
                if min.is_some_and(|m| float < m) || max.is_some_and(|m| float > m) {
                    return Err(format!("{path}: {float} fora da faixa {min:?}..{max:?}"));
                }
                Ok(())
            }

            (FieldKind::Str { min_len, max_len }, Value::String(s)) => {
                // Conta CARACTERES, não bytes: um limite de 8 que recusasse "configuração" por
                // ela ocupar 13 bytes seria um campo que rejeita o próprio idioma do app.
                let len = s.chars().count();
                if min_len.is_some_and(|m| len < m) || max_len.is_some_and(|m| len > m) {
                    return Err(format!("{path}: {len} caracteres, fora de {min_len:?}..{max_len:?}"));
                }
                Ok(())
            }

            (FieldKind::Enum { variants }, Value::String(s)) => {
                if variants.contains(&s.as_str()) {
                    Ok(())
                } else {
                    Err(format!("{path}: '{s}' não é um de {variants:?}"))
                }
            }

            (FieldKind::List, Value::Array(_)) => Ok(()),
            (FieldKind::Map, Value::Object(_)) => Ok(()),

            (kind, other) => Err(format!(
                "{path}: tipo errado — esperava {}, veio {}",
                kind_name(kind),
                type_name(other)
            )),
        }
    }
}

fn kind_name(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Bool => "booleano",
        FieldKind::Int { .. } => "inteiro",
        FieldKind::Float { .. } => "número",
        FieldKind::Str { .. } => "texto",
        FieldKind::Enum { .. } => "uma das opções",
        FieldKind::List => "lista",
        FieldKind::Map => "objeto",
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "nulo",
        Value::Bool(_) => "booleano",
        Value::Number(_) => "número",
        Value::String(_) => "texto",
        Value::Array(_) => "lista",
        Value::Object(_) => "objeto",
    }
}

/// Construtores curtos. O port dos 38 schemas é mecânico e longo; sem isto cada campo viraria seis
/// linhas de struct literal e a revisão de paridade ficaria impossível de ler.
pub fn boolean(key: &'static str, default: bool) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::Bool,
        default: Value::Bool(default),
        ui: None,
    }
}

#[cfg(test)] // só o teste de `features/user/profile_config.rs` precisa de um inteiro sem faixa
pub fn int(key: &'static str, default: i64) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::Int { min: None, max: None },
        default: json!(default),
        ui: None,
    }
}

pub fn int_range(key: &'static str, default: i64, min: i64, max: i64) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::Int {
            min: Some(min),
            max: Some(max),
        },
        default: json!(default),
        ui: None,
    }
}

pub fn float(key: &'static str, default: f64) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::Float { min: None, max: None },
        default: json!(default),
        ui: None,
    }
}

pub fn float_range(key: &'static str, default: f64, min: f64, max: f64) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::Float {
            min: Some(min),
            max: Some(max),
        },
        default: json!(default),
        ui: None,
    }
}

pub fn text(key: &'static str, default: &str) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::Str {
            min_len: None,
            max_len: None,
        },
        default: Value::String(default.to_string()),
        ui: None,
    }
}

pub fn choice(key: &'static str, default: &str, variants: &'static [&'static str]) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::Enum { variants },
        default: Value::String(default.to_string()),
        ui: None,
    }
}

pub fn list(key: &'static str) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::List,
        default: Value::Array(vec![]),
        ui: None,
    }
}

pub fn map(key: &'static str) -> FieldSchema {
    FieldSchema {
        key,
        kind: FieldKind::Map,
        default: Value::Object(serde_json::Map::new()),
        ui: None,
    }
}

impl FieldSchema {
    pub fn with_ui(mut self, ui: FieldUi) -> Self {
        self.ui = Some(ui);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_field_accepts_only_bool() {
        let field = boolean("enabled", true);
        assert!(field.validate("memory.enabled", &json!(false)).is_ok());
        assert!(field.validate("memory.enabled", &json!("sim")).is_err());
        assert!(field.validate("memory.enabled", &json!(1)).is_err());
    }

    /// `z.number().int()` recusa fracionário. Um único tipo numérico deixaria 1.5 entrar num
    /// limite de lista e o consumidor quebraria longe daqui.
    #[test]
    fn int_field_rejects_fractional() {
        let field = int_range("episodicListLimit", 500, 1, 5000);
        assert!(field.validate("memory.episodicListLimit", &json!(500)).is_ok());
        assert!(field.validate("memory.episodicListLimit", &json!(1.5)).is_err());
    }

    #[test]
    fn int_field_enforces_range() {
        let field = int_range("episodicListLimit", 500, 1, 5000);
        assert!(field.validate("memory.episodicListLimit", &json!(0)).is_err());
        assert!(field.validate("memory.episodicListLimit", &json!(5001)).is_err());
        assert!(field.validate("memory.episodicListLimit", &json!(1)).is_ok());
        assert!(field.validate("memory.episodicListLimit", &json!(5000)).is_ok());
    }

    #[test]
    fn enum_field_rejects_unknown_variant() {
        let field = choice("backend", "embedded", &["embedded", "redis"]);
        assert!(field.validate("memory.backend", &json!("redis")).is_ok());
        assert!(field.validate("memory.backend", &json!("postgres")).is_err());
    }

    /// Limite de texto em CARACTERES. Em bytes, um campo de 8 recusaria "configuração" — o app é
    /// em português e o próprio idioma passaria a ser entrada inválida.
    #[test]
    fn text_length_counts_characters_not_bytes() {
        let field = FieldSchema {
            key: "nick",
            kind: FieldKind::Str {
                min_len: None,
                max_len: Some(12),
            },
            default: Value::String(String::new()),
            ui: None,
        };
        assert!(field.validate("user.nick", &json!("configuração")).is_ok());
        assert!(field.validate("user.nick", &json!("configurações!")).is_err());
    }

    /// A mensagem de erro é o que o renderer mostra quando recusa um `settings-set`. Sem o caminho
    /// e o motivo, o controle apenas não obedece e o usuário não tem como saber por quê.
    #[test]
    fn error_names_the_path_and_the_reason() {
        let field = choice("backend", "embedded", &["embedded", "redis"]);
        let error = field.validate("memory.backend", &json!("postgres")).unwrap_err();
        assert!(error.contains("memory.backend"), "sem o caminho: {error}");
        assert!(error.contains("postgres"), "sem o valor recusado: {error}");
        assert!(error.contains("embedded"), "sem as opções válidas: {error}");
    }

    /// A UI viaja para o renderer com as CHAVES de i18n, não com texto — é assim que o painel
    /// continua traduzido depois do port.
    #[test]
    fn ui_serializes_i18n_keys() {
        let field = choice("backend", "embedded", &["embedded", "redis"]).with_ui(
            FieldUi::new("settings.memory.field.backend.label")
                .control("select")
                .options(&[UiOption {
                    label: "settings.memory.field.backend.option.embedded",
                    value: "embedded",
                }]),
        );

        let json = serde_json::to_value(&field).unwrap();
        assert_eq!(json["ui"]["label"], "settings.memory.field.backend.label");
        assert_eq!(json["ui"]["type"], "select");
        assert_eq!(json["ui"]["options"][0]["value"], "embedded");
        assert_eq!(json["default"], "embedded");
    }
}
