//! Helpers comuns a [`super::upsert`] e [`super::lifecycle`]: geração de id, montagem de menção,
//! o gate de bloqueio (`isPersonBlocked`) e a reconstrução do `aliasMap`.

use std::collections::HashMap;

use super::super::normalize::catalog_key;
use super::super::types::{MentionSource, NewMention, PersonCatalogIndex, PersonMention};

/// Gerador de id — `fn() -> String` sem captura (não closure): mesmo `ctx.genId()` do TS serve
/// tanto para o id da pessoa quanto para o id da menção, sempre `pc_<8 hex>` (ver
/// `super::super::store::generate_id`).
pub(super) type IdGen = fn() -> String;

pub(super) fn build_mention(input: &NewMention, id: String) -> PersonMention {
    PersonMention {
        id,
        source: input.source,
        text: input.text.clone(),
        timestamp: input.timestamp,
        url: input.url.clone(),
        app_name: input.app_name.clone(),
        window_title: input.window_title.clone(),
        screenshot_path: None,
        context: input.context.clone(),
    }
}

pub(super) fn mention_source_str(source: MentionSource) -> &'static str {
    match source {
        MentionSource::Chat => "chat",
        MentionSource::Voice => "voice",
        MentionSource::Vision => "vision",
        MentionSource::Manual => "manual",
        MentionSource::Import => "import",
    }
}

/// Porte de `isPersonBlocked` — só o gate de `upsert` usa (ver `mutate/upsert.rs`).
pub(super) fn is_blocked(index: &PersonCatalogIndex, name_or_alias: &str) -> bool {
    let key = catalog_key(name_or_alias);
    !key.is_empty() && index.blocked.as_ref().is_some_and(|list| list.contains(&key))
}

/// Porte de `rebuildAliasMap` — colisão de alias (não no `primaryName`) é apagada da PRÓPRIA pessoa,
/// senão a colisão reapareceria em todo upsert seguinte. Sem `logger.warn` aqui: nenhuma função pura
/// deste diretório recebe logger (ver o cabeçalho de `mutate/mod.rs`) e os dois valores logados no
/// TS são só IDs, não dado pessoal — a integridade do `aliasMap` é o que importa, não a
/// observabilidade.
pub(in super::super) fn rebuild_alias_map(index: &mut PersonCatalogIndex) {
    let mut alias_map: HashMap<String, String> = HashMap::new();
    for person in index.persons.iter_mut() {
        let names: Vec<String> = std::iter::once(person.primary_name.clone()).chain(person.aliases.iter().cloned()).collect();
        let mut to_remove: Vec<String> = Vec::new();
        for name in &names {
            let key = catalog_key(name);
            if key.is_empty() {
                continue;
            }
            if let Some(owner) = alias_map.get(&key) {
                if owner != &person.id {
                    if name != &person.primary_name {
                        to_remove.push(name.clone());
                    }
                    continue;
                }
            }
            alias_map.insert(key, person.id.clone());
        }
        if !to_remove.is_empty() {
            person.aliases.retain(|a| !to_remove.contains(a));
        }
    }
    index.alias_map = alias_map;
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::types::PersonCatalogIndex as Index;

    fn empty_index() -> Index {
        Index { version: 2, persons: vec![], alias_map: Default::default(), blocked: None, social_imported_at: None, ..Default::default() }
    }

    #[test]
    fn is_blocked_matches_by_normalized_catalog_key() {
        let mut idx = empty_index();
        idx.blocked = Some(vec!["ana silva".to_string()]);
        assert!(is_blocked(&idx, "Ana Silva"));
        assert!(is_blocked(&idx, "  ANA SILVA  "));
        assert!(!is_blocked(&idx, "Outra Pessoa"));
    }

    #[test]
    fn rebuild_alias_map_strips_a_colliding_alias_from_the_person_that_is_not_the_owner() {
        let mut idx = empty_index();
        let mut owner = super::super::super::types::minimal_person("owner", "First");
        owner.aliases = vec!["Shared".to_string()];
        let mut other = super::super::super::types::minimal_person("other", "Second");
        other.aliases = vec!["Shared".to_string()];
        idx.persons = vec![owner, other];

        rebuild_alias_map(&mut idx);

        assert_eq!(idx.alias_map.get("shared"), Some(&"owner".to_string()));
        assert!(idx.persons[1].aliases.is_empty(), "alias colidente devia sair só de quem não é dono");
    }
}
