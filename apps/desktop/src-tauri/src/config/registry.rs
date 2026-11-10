//! Composição dos schemas por namespace — o que em TypeScript era `SchemaRegistry.build()`.
//!
//! Cada módulo declara seu namespace (`memory`, `telemetry`, `voice`…) com os campos dele. O
//! registro junta tudo e responde as três perguntas que o store precisa: qual o config padrão,
//! este `set` é válido, e o que o renderer recebe em `mika:settings-declarations`.

use serde::Serialize;
use serde_json::{Map, Value};

use super::schema::{FieldKind, FieldSchema};

pub struct Namespace {
    pub name: &'static str,
    pub fields: Vec<FieldSchema>,
}

/// O que viaja para o renderer desenhar a tela de Settings.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Declaration<'a> {
    pub namespace: &'a str,
    pub fields: &'a [FieldSchema],
}

pub struct SchemaRegistry {
    namespaces: Vec<Namespace>,
}

impl SchemaRegistry {
    /// Namespace declarado duas vezes derruba o boot, pela mesma razão do registro de canais: o
    /// segundo sobrescreveria o primeiro e um módulo inteiro perderia sua configuração em silêncio.
    pub fn new(namespaces: Vec<Namespace>) -> Self {
        for (index, namespace) in namespaces.iter().enumerate() {
            if namespaces[..index].iter().any(|other| other.name == namespace.name) {
                panic!("namespace de config '{}' declarado duas vezes", namespace.name);
            }
        }
        Self { namespaces }
    }

    /// O config completo com todos os defaults. É o ponto de partida de uma instalação nova e a
    /// base sobre a qual o arquivo em disco é mesclado.
    pub fn defaults(&self) -> Value {
        let mut root = Map::new();

        for namespace in &self.namespaces {
            let mut section = Map::new();
            for field in &namespace.fields {
                section.insert(field.key.to_string(), field.default.clone());
            }
            root.insert(namespace.name.to_string(), Value::Object(section));
        }

        Value::Object(root)
    }

    /// Resolve `namespace.campo`, e também `namespace.campo.subchave` quando `campo` é um mapa.
    ///
    /// Mapa existe justamente para chaves descobertas em runtime — `llm.apiKeys.openai`,
    /// `llm.apiKeys.google`. Casar só dois segmentos recusaria toda credencial de provedor como
    /// "caminho não declarado", e o único sintoma seria a chave não salvar.
    fn field(&self, path: &str) -> Option<(&FieldSchema, bool)> {
        let (namespace, rest) = path.split_once('.')?;
        let fields = &self
            .namespaces
            .iter()
            .find(|candidate| candidate.name == namespace)?
            .fields;

        if let Some(exact) = fields.iter().find(|field| field.key == rest) {
            return Some((exact, false));
        }

        let (head, _) = rest.split_once('.')?;
        let parent = fields.iter().find(|field| field.key == head)?;

        matches!(parent.kind, FieldKind::Map).then_some((parent, true))
    }

    /// Valida um `set`. Caminho desconhecido é RECUSADO, não ignorado.
    ///
    /// O lado TypeScript já mordeu esta classe: o Zod do router apagava campo não declarado sem
    /// erro, e o valor sumia entre o handler e o disco sem ninguém ver. Aceitar caminho
    /// desconhecido aqui reproduziria exatamente isso — a UI diria "salvo" e nada teria sido.
    pub fn validate(&self, path: &str, value: &Value) -> Result<(), String> {
        let Some((field, inside_map)) = self.field(path) else {
            return Err(format!("{path}: caminho não declarado em nenhum schema"));
        };

        // Uma entrada DENTRO de um mapa não é validada contra o tipo do mapa — `llm.apiKeys` é
        // `Map`, mas `llm.apiKeys.openai` é o texto da credencial. Cobrar `Map` ali recusaria
        // qualquer chave; o Zod original também só tipava o valor do record.
        if inside_map {
            return Ok(());
        }

        field.validate(path, value)
    }

    pub fn declarations(&self) -> Vec<Declaration<'_>> {
        self.namespaces
            .iter()
            .map(|namespace| Declaration {
                namespace: namespace.name,
                fields: &namespace.fields,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::schema::{boolean, choice, int_range, text};
    use super::*;
    use serde_json::json;

    fn registry() -> SchemaRegistry {
        SchemaRegistry::new(vec![
            Namespace {
                name: "memory",
                fields: vec![
                    boolean("enabled", true),
                    choice("backend", "embedded", &["embedded", "redis"]),
                    int_range("episodicListLimit", 500, 1, 5000),
                ],
            },
            Namespace {
                name: "telemetry",
                fields: vec![boolean("enabled", false), text("sessionId", "")],
            },
        ])
    }

    #[test]
    fn defaults_carry_every_declared_field() {
        let defaults = registry().defaults();

        assert_eq!(defaults["memory"]["enabled"], json!(true));
        assert_eq!(defaults["memory"]["backend"], json!("embedded"));
        assert_eq!(defaults["memory"]["episodicListLimit"], json!(500));
        assert_eq!(defaults["telemetry"]["enabled"], json!(false));
    }

    #[test]
    fn validate_routes_to_the_right_field() {
        let registry = registry();

        assert!(registry.validate("memory.backend", &json!("redis")).is_ok());
        assert!(registry.validate("memory.backend", &json!("postgres")).is_err());
        assert!(registry.validate("telemetry.enabled", &json!(true)).is_ok());
    }

    /// Caminho não declarado tem de FALHAR. O Zod do router de IPC apagava campo não declarado em
    /// silêncio, e o valor sumia entre o handler e o disco — aceitar aqui repetiria o mesmo
    /// defeito: a UI diria "salvo" sem nada ter sido gravado.
    #[test]
    fn unknown_path_is_refused_not_ignored() {
        let registry = registry();

        let error = registry.validate("memory.naoExiste", &json!(1)).unwrap_err();
        assert!(error.contains("memory.naoExiste"), "erro sem o caminho: {error}");

        assert!(registry.validate("inexistente.campo", &json!(1)).is_err());
        assert!(registry.validate("semPonto", &json!(1)).is_err());
    }

    #[test]
    #[should_panic(expected = "declarado duas vezes")]
    fn duplicate_namespace_is_a_boot_failure() {
        SchemaRegistry::new(vec![
            Namespace {
                name: "memory",
                fields: vec![boolean("enabled", true)],
            },
            Namespace {
                name: "memory",
                fields: vec![boolean("outro", true)],
            },
        ]);
    }

    /// As declarações são o que desenha a tela de Settings. Se elas vierem vazias o app valida
    /// certo e não tem mais painel de configuração — falha silenciosa e cara de perceber.
    #[test]
    fn declarations_expose_every_namespace_and_field() {
        let registry = registry();
        let declarations = registry.declarations();

        assert_eq!(declarations.len(), 2);

        let json = serde_json::to_value(&declarations).unwrap();
        assert_eq!(json[0]["namespace"], "memory");
        assert_eq!(json[0]["fields"][0]["key"], "enabled");
        assert_eq!(json[0]["fields"][1]["variants"][1], "redis");
    }
}
