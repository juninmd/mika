//! Mapa de AGRUPAMENTO da aba `character` para `crate::feature_registry::shared()` — port de
//! `character.schema.ts::CHARACTER_SETTINGS` (`defineFeatureSchema`), na parte que
//! `super::super::settings_declaration::declare_namespace` não cobre.
//!
//! **Por que isto existe ao lado de `declare_namespace`, não no lugar dele.** `character` e `user`
//! são, pelo `AGENTS.md` §3.6 ("Padrão Ouro de UI/UX"), a referência visual do resto do app —
//! "agrupamentos em `GlassSection` com ícone temático e descrição contextual". `declare_namespace`
//! achata o namespace inteiro num cartão só (ver o cabeçalho dele): correto para uma aba sem
//! subdivisão, errado para as duas telas que o resto da UI deve copiar. `character.schema.ts`
//! declara 3 grupos (`identity` — que funde `characterIdentityFields` + `characterRelationshipFields`
//! — `appearance`, `behavior`), cada um com seu próprio `component`/`badge`/`columns`.
//!
//! **O `SchemaRegistry` continua sendo a ÚNICA fonte da verdade de QUAIS campos existem, de que
//! tipo e com que limites** — [`declare_fields`] devolve exatamente o que `declare_namespace`
//! devolveria, campo a campo. Este arquivo só decide EM QUAL GRUPO cada campo (já pronto) entra.
//! Duas fontes da verdade para "quais campos existem" é o que `settings_declaration.rs` recusa,
//! com razão; um mapa de apresentação ao lado de um schema de validação é uma pergunta diferente.
//!
//! **Campo sem grupo não pode sumir da tela.** Se `character.schema.ts` ganhar um campo e este
//! mapa não for atualizado, [`declaration`] não descarta — o campo cai num grupo `"outros"`
//! ao final, visível ainda que malformatado. `field_count_and_every_field_is_grouped` prova que,
//! contra o schema de hoje, esse grupo fica vazio; se um campo aparecer nele, o teste falha e
//! avisa qual chave falta no mapa, em vez de o campo sumir em silêncio.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::config::registry::SchemaRegistry;
use crate::config::store::ConfigStore;

use super::super::settings_declaration::declare_fields;

const NAMESPACE: &str = "character";

/// Toggle mestre da feature (`character.enabled`), lido do `ConfigStore` REAL a cada chamada —
/// nunca cacheado, mesma técnica de `person_catalog::tool::catalog_enabled`. Gateia a tool
/// `avatar` (`super::tool::AvatarToolHandler`) — a única superfície de "character tools" que este
/// módulo controla; a UI já descrevia esse efeito (`settings.character.field.enabled.description`)
/// sem nada no backend o ler (varredura de controles de UI de 2026-09-27).
pub fn master_enabled(store: &ConfigStore) -> bool {
    store.get("character.enabled").and_then(|v| v.as_bool()) != Some(false)
}

/// Resposta padrão de tool quando `character.enabled=false` — mesma forma de
/// `person_catalog::tool::catalog_disabled`.
pub fn master_disabled_response() -> Value {
    json!({
        "success": false,
        "error": "The character feature is disabled (character.enabled=false); no action was taken.",
        "errorCode": "denied",
    })
}

struct GroupSpec {
    title: &'static str,
    component: &'static str,
    badge: &'static str,
    columns: Option<u64>,
    /// Chaves (sem prefixo `character.`), na MESMA ordem de `character.schema.ts`.
    keys: &'static [&'static str],
}

/// `character.schema.ts::groups`. `identity` funde `characterIdentityFields` (11) +
/// `characterRelationshipFields` (7) igual ao `z.object({ ...a, ...b })` do original; `behavior`
/// funde `characterToneFields` (7) + `characterHabitsFields` (12) na mesma ordem.
const GROUPS: &[GroupSpec] = &[
    GroupSpec {
        title: "settings.character.group.identity",
        component: "CharacterIdentityPanel",
        badge: "Persona",
        columns: Some(2),
        keys: &[
            "enabled",
            "name",
            "personality",
            "personalityProfile",
            "visualDescription",
            "greetingStyle",
            "responseStyle",
            "modelName",
            "modelPath",
            "backgroundMode",
            "backgroundSolidColor",
            // Avatar wind (hair/skirt spring bones), rendered by `SkinWindSection` on the Skin tab.
            "windEnabled",
            "windStrength",
            // Avatar render sharpness, rendered by `SkinRenderSection` on the Skin tab.
            "renderSharpness",
            "relationshipStatus",
            "affinityScore",
            "hobbies",
            "dislikes",
            "nicknamesForUser",
            "secretGoals",
            "customInstructions",
        ],
    },
    GroupSpec {
        title: "settings.character.group.appearance",
        component: "ThemePickerPanel",
        badge: "Tema",
        columns: None,
        keys: &["theme"],
    },
    GroupSpec {
        title: "settings.character.group.behavior",
        component: "BehaviorPanel",
        badge: "Personalidade",
        columns: Some(2),
        keys: &[
            "moodBaseline",
            "energyLevel",
            "formalityLevel",
            "humorStyle",
            "sarcasmLevel",
            "chaosFactor",
            "tsundereIntensity",
            "slangUsage",
            "emojiUsage",
            "jokeFrequency",
            "proactiveness",
            "empathyLevel",
            "swearTolerance",
            "responseLength",
            "memoryRecall",
            "slangEnabled",
            "slangDictionary",
            "quietHoursStart",
            "quietHoursEnd",
        ],
    },
];

fn key_of(field: &Value) -> String {
    field["configPath"].as_str().and_then(|path| path.rsplit('.').next()).unwrap_or_default().to_string()
}

fn build_group(spec: &GroupSpec, by_key: &mut HashMap<String, Value>) -> Value {
    let fields: Vec<Value> = spec.keys.iter().filter_map(|key| by_key.remove(*key)).collect();
    let mut group = json!({
        "title": spec.title,
        "component": spec.component,
        "groupPrefix": NAMESPACE,
        "badge": spec.badge,
        "fields": fields,
    });
    if let Some(columns) = spec.columns {
        group["columns"] = json!(columns);
    }
    group
}

/// `CHARACTER_SETTINGS` (`character.schema.ts`), sem `pluginId`/`isCore` — o registro injeta os
/// dois. Devolve `None` se o namespace `character` não existir no `schema` — mesma regra de
/// `declare_namespace`.
pub fn declaration(schema: &SchemaRegistry) -> Option<Value> {
    let mut by_key: HashMap<String, Value> =
        declare_fields(schema, NAMESPACE)?.into_iter().map(|field| (key_of(&field), field)).collect();

    let mut groups: Vec<Value> = GROUPS.iter().map(|spec| build_group(spec, &mut by_key)).collect();

    // Campo do schema que nenhum `GroupSpec` reivindicou — ver o cabeçalho deste arquivo.
    if !by_key.is_empty() {
        let mut leftover: Vec<(String, Value)> = by_key.into_iter().collect();
        leftover.sort_by(|a, b| a.0.cmp(&b.0));
        groups.push(json!({
            "title": "settings.character.group.other",
            "groupPrefix": NAMESPACE,
            "fields": leftover.into_iter().map(|(_, field)| field).collect::<Vec<_>>(),
        }));
    }

    Some(json!({
        "tabId": "character",
        "tabLabel": "settings.character.tab.label",
        "tabDescription": "settings.character.tab.description",
        "tabIcon": "User",
        "tabGroup": "settings.character.group.identity",
        "heroNote": "settings.character.tab.heroNote",
        "accentColor": "violet",
        // `inferTabComponent`: os 3 grupos têm `component` distinto (`CharacterIdentityPanel` /
        // `ThemePickerPanel` / `BehaviorPanel`) — o lado TS devolve `undefined`, então nenhum
        // `component` de aba é emitido aqui (mesma regra de "sem `enabledConfigPath`" documentada
        // em `privacy/feature.rs`: ausência intencional, não omissão por engano).
        "enabledConfigPath": "character.enabled",
        "groups": groups,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> SchemaRegistry {
        crate::config::schemas::registry()
    }

    #[test]
    fn declaration_has_the_three_ts_groups_in_order() {
        let d = declaration(&schema()).unwrap();
        assert_eq!(d["tabId"], json!("character"));
        assert_eq!(d["accentColor"], json!("violet"));
        assert_eq!(d["enabledConfigPath"], json!("character.enabled"));
        assert!(d.get("component").is_none(), "3 componentes distintos — sem component de aba, igual ao TS");

        let groups = d["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 3, "identity, appearance, behavior — nenhum grupo 'outros'");

        assert_eq!(groups[0]["title"], json!("settings.character.group.identity"));
        assert_eq!(groups[0]["component"], json!("CharacterIdentityPanel"));
        assert_eq!(groups[0]["columns"], json!(2));
        assert_eq!(groups[0]["fields"].as_array().unwrap().len(), 21);
        // `theme` some do primeiro grupo, mesmo campo do segundo grupo TEM que estar lá.
        assert_eq!(groups[0]["fields"][0]["configPath"], json!("character.enabled"));

        assert_eq!(groups[1]["title"], json!("settings.character.group.appearance"));
        assert_eq!(groups[1]["fields"].as_array().unwrap().len(), 1);
        assert_eq!(groups[1]["fields"][0]["configPath"], json!("character.theme"));
        assert!(groups[1].get("columns").is_none());

        assert_eq!(groups[2]["title"], json!("settings.character.group.behavior"));
        assert_eq!(groups[2]["fields"].as_array().unwrap().len(), 19);
    }

    /// NEGATIVO — a classe de defeito que este mapa existe para não repetir: um campo do schema
    /// que nenhum `GroupSpec` reivindica não pode desaparecer da declaração.
    #[test]
    fn field_count_and_every_field_is_grouped() {
        let all_fields = declare_fields(&schema(), NAMESPACE).unwrap();
        let d = declaration(&schema()).unwrap();
        let groups = d["groups"].as_array().unwrap();

        assert_eq!(groups.len(), 3, "um campo apareceu fora do mapa e criou o grupo 'outros'");

        let grouped_total: usize = groups.iter().map(|g| g["fields"].as_array().unwrap().len()).sum();
        assert_eq!(grouped_total, all_fields.len(), "contagem de campos agrupados divergiu do schema");
    }

    #[test]
    fn master_enabled_defaults_to_true_when_config_is_absent() {
        let dir = std::env::temp_dir().join(format!("mika-character-feature-master-enabled-default-{}", std::process::id()));
        let store = ConfigStore::open(&dir, SchemaRegistry::new(vec![])).unwrap();
        assert!(master_enabled(&store));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn master_enabled_is_false_only_when_explicitly_disabled() {
        let dir = std::env::temp_dir().join(format!("mika-character-feature-master-enabled-explicit-{}", std::process::id()));
        let store = ConfigStore::open(&dir, schema()).unwrap();
        assert!(master_enabled(&store));
        store.set("character.enabled", json!(false)).unwrap();
        assert!(!master_enabled(&store));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
