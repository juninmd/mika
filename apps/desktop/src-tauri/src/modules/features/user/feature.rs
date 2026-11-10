//! Mapa de AGRUPAMENTO da aba `user` para `crate::feature_registry::shared()` — port de
//! `user.schema.ts::USER_SETTINGS` (`defineFeatureSchema`), na parte que
//! `super::super::settings_declaration::declare_namespace` não cobre.
//!
//! Mesma razão de existir de `character/feature.rs` (ler o cabeçalho de lá primeiro): `user` é,
//! pelo `AGENTS.md` §3.6, referência de "Padrão Ouro de UI/UX" junto com `character` — achatar os
//! 7 grupos de `user.schema.ts` num cartão só (o que `declare_namespace` faz) perde exatamente a
//! subdivisão em `GlassSection` que o resto do app deve copiar. `SchemaRegistry` continua dono de
//! QUAIS campos existem/tipo/limites ([`declare_fields`]); este arquivo só decide o grupo.
//!
//! Campo sem grupo não some da tela — cai num grupo `"outros"` ao final; ver o cabeçalho de
//! `character/feature.rs` para o raciocínio completo (idêntico aqui) e
//! `field_count_and_every_field_is_grouped` para a prova.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::config::registry::SchemaRegistry;

use super::super::settings_declaration::declare_fields;

const NAMESPACE: &str = "user";

struct GroupSpec {
    title: &'static str,
    badge: &'static str,
    columns: Option<u64>,
    /// Chaves (sem prefixo `user.`), na MESMA ordem de `user.schema.ts`.
    keys: &'static [&'static str],
}

/// `user.schema.ts::groups` — os 7 grupos, cada um lendo direto de um `*GroupSchema` de
/// `user-identity-config.schema.ts` / `user-interests-config.schema.ts` /
/// `user-relations-config.schema.ts`. Todos compartilham `component: 'UserProfilePanel'`
/// (aplicado em [`declaration`], não por grupo, já que os 7 são idênticos).
const GROUPS: &[GroupSpec] = &[
    GroupSpec {
        title: "settings.user.group.identity",
        badge: "Você",
        columns: Some(2),
        keys: &["name", "nickname", "birthDate", "age", "gender"],
    },
    GroupSpec {
        title: "settings.user.group.location",
        badge: "Região",
        columns: Some(2),
        keys: &["country", "state", "city", "timezone"],
    },
    GroupSpec {
        title: "settings.user.group.professional",
        badge: "Trabalho",
        columns: Some(2),
        keys: &["profession", "company", "role", "skills"],
    },
    GroupSpec {
        title: "settings.user.group.interests",
        badge: "Gostos",
        columns: None,
        keys: &["hobbies", "favoriteGames", "favoriteAnime", "favoriteMusic", "favoriteFood"],
    },
    GroupSpec {
        title: "settings.user.group.importantPeople",
        badge: "Relações",
        columns: None,
        keys: &["partnerName", "partnerRelationship", "partnerLikes", "partnerNotes"],
    },
    GroupSpec {
        title: "settings.user.group.personality",
        badge: "Vibes",
        columns: None,
        keys: &["personalityTraits", "communicationStyle", "communicationNotes"],
    },
    GroupSpec {
        title: "settings.user.group.bio",
        badge: "Bio",
        columns: None,
        keys: &["bio", "notes", "learnedByLLM"],
    },
];

const PANEL_COMPONENT: &str = "UserProfilePanel";

fn key_of(field: &Value) -> String {
    field["configPath"].as_str().and_then(|path| path.rsplit('.').next()).unwrap_or_default().to_string()
}

fn build_group(spec: &GroupSpec, by_key: &mut HashMap<String, Value>) -> Value {
    let fields: Vec<Value> = spec.keys.iter().filter_map(|key| by_key.remove(*key)).collect();
    let mut group = json!({
        "title": spec.title,
        "component": PANEL_COMPONENT,
        "groupPrefix": NAMESPACE,
        "badge": spec.badge,
        "fields": fields,
    });
    if let Some(columns) = spec.columns {
        group["columns"] = json!(columns);
    }
    group
}

/// `USER_SETTINGS` (`user.schema.ts`), sem `pluginId`/`isCore` — o registro injeta os dois. Devolve
/// `None` se o namespace `user` não existir no `schema` — mesma regra de `declare_namespace`.
pub fn declaration(schema: &SchemaRegistry) -> Option<Value> {
    let mut by_key: HashMap<String, Value> =
        declare_fields(schema, NAMESPACE)?.into_iter().map(|field| (key_of(&field), field)).collect();

    let mut groups: Vec<Value> = GROUPS.iter().map(|spec| build_group(spec, &mut by_key)).collect();

    // Campo do schema que nenhum `GroupSpec` reivindicou — ver o cabeçalho deste arquivo.
    if !by_key.is_empty() {
        let mut leftover: Vec<(String, Value)> = by_key.into_iter().collect();
        leftover.sort_by(|a, b| a.0.cmp(&b.0));
        groups.push(json!({
            "title": "settings.user.group.other",
            "groupPrefix": NAMESPACE,
            "fields": leftover.into_iter().map(|(_, field)| field).collect::<Vec<_>>(),
        }));
    }

    Some(json!({
        "tabId": "user",
        "tabLabel": "settings.user.tab.label",
        "tabDescription": "settings.user.tab.description",
        "tabIcon": "User",
        "tabGroup": "settings.user.tab.group",
        "heroNote": "settings.user.tab.heroNote",
        "accentColor": "emerald",
        // `inferTabComponent`: os 7 grupos compartilham `component: 'UserProfilePanel'` — TODOS
        // iguais, então o lado TS promove o valor único para a aba.
        "component": PANEL_COMPONENT,
        // Nenhum dos 7 grupos declara campo `enabled` — sem `enabledConfigPath`, de propósito
        // (mesmo caso de `privacy/feature.rs`, sem candidato para `inferEnabledConfigPath`).
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
    fn declaration_has_the_seven_ts_groups_in_order() {
        let d = declaration(&schema()).unwrap();
        assert_eq!(d["tabId"], json!("user"));
        assert_eq!(d["accentColor"], json!("emerald"));
        assert_eq!(d["component"], json!("UserProfilePanel"));
        assert!(d.get("enabledConfigPath").is_none(), "nenhum grupo tem campo 'enabled' — sem enabledConfigPath");

        let groups = d["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 7, "nenhum grupo 'outros' — todo campo tem grupo no mapa");

        let expected_titles = [
            "settings.user.group.identity",
            "settings.user.group.location",
            "settings.user.group.professional",
            "settings.user.group.interests",
            "settings.user.group.importantPeople",
            "settings.user.group.personality",
            "settings.user.group.bio",
        ];
        for (group, title) in groups.iter().zip(expected_titles) {
            assert_eq!(group["title"], json!(title));
            assert_eq!(group["component"], json!("UserProfilePanel"));
        }

        // Só os 3 primeiros grupos têm `columns: 2` — os outros 4 não declaram `columns` no TS.
        for group in &groups[..3] {
            assert_eq!(group["columns"], json!(2));
        }
        for group in &groups[3..] {
            assert!(group.get("columns").is_none());
        }

        assert_eq!(groups[0]["fields"].as_array().unwrap().len(), 5);
        assert_eq!(groups[0]["fields"][0]["configPath"], json!("user.name"));
    }

    /// NEGATIVO — mesma classe de defeito de `character/feature.rs`: campo sem grupo não some.
    #[test]
    fn field_count_and_every_field_is_grouped() {
        let all_fields = declare_fields(&schema(), NAMESPACE).unwrap();
        let d = declaration(&schema()).unwrap();
        let groups = d["groups"].as_array().unwrap();

        assert_eq!(groups.len(), 7, "um campo apareceu fora do mapa e criou o grupo 'outros'");

        let grouped_total: usize = groups.iter().map(|g| g["fields"].as_array().unwrap().len()).sum();
        assert_eq!(grouped_total, all_fields.len(), "contagem de campos agrupados divergiu do schema");
        assert_eq!(all_fields.len(), 28);
    }

    // `user-identity-config.schema.ts`, `user-interests-config.schema.ts` e
    // `user-relations-config.schema.ts` foram removidos (backend Node deletado na migração para
    // Tauri, sem twin vivo). O invariante que essa suíte pinava — nenhuma chave de `GROUPS` fica
    // sem casar um campo real — já é coberto behavioralmente por
    // `field_count_and_every_field_is_grouped` acima: uma chave errada deixaria um campo de fora
    // do agrupamento e faria `groups.len()` != 7 ou o total divergir de 28.
}
