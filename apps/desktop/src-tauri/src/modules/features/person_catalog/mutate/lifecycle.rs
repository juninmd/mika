//! `add_mention` e `delete` — porte de `person-catalog-mutations.ts::addMentionToPerson` e
//! `person-catalog-deletion.ts::deletePerson`.

use super::super::merge_fields::{dead_names, remove_orphan_relationships};
use super::super::normalize::catalog_key;
use super::super::types::{NewMention, PersonCatalogIndex, PersonRecord};
use super::shared::{build_mention, rebuild_alias_map, IdGen};

/// Porte de `addMentionToPerson` — acha por id, senão por alias; capado em `max_mentions`.
pub(in super::super) fn add_mention_to_person(
    index: &mut PersonCatalogIndex,
    identifier: &str,
    mention: NewMention,
    now: i64,
    max_mentions: usize,
    gen_id: IdGen,
) -> Result<PersonRecord, String> {
    let position = index.persons.iter().position(|p| p.id == identifier).or_else(|| {
        let key = catalog_key(identifier);
        let owner = index.alias_map.get(&key)?.clone();
        index.persons.iter().position(|p| p.id == owner)
    });
    let Some(position) = position else {
        return Err(format!("Person not found: {identifier}"));
    };

    let new_mention = build_mention(&mention, gen_id());
    let person = &mut index.persons[position];
    person.mentions.push(new_mention);
    let overflow = person.mentions.len().saturating_sub(max_mentions);
    if overflow > 0 {
        person.mentions.drain(0..overflow);
    }
    person.mention_count = person.mentions.len();
    person.updated_at = now;
    person.last_seen_at = Some(now);
    Ok(person.clone())
}

/// Porte de `deletePerson` — MENOS a limpeza de foto cacheada (`person-image-cache.ts` não portado)
/// e o `events.emit` (ver o cabeçalho de `mutate/mod.rs`). A varredura de relações órfãs
/// (`removeOrphanRelationships`) é feita, porque é só dado do próprio catálogo — não depende de
/// nenhum módulo fora daqui.
pub(in super::super) fn delete_person(index: &mut PersonCatalogIndex, id: &str) -> Result<(), String> {
    let before = index.persons.len();
    let target = index.persons.iter().find(|p| p.id == id).cloned();
    index.persons.retain(|p| p.id != id);
    if index.persons.len() == before {
        return Err(format!("Person not found: {id}"));
    }

    if let Some(target) = &target {
        let mut names = vec![target.primary_name.clone()];
        names.extend(target.aliases.clone());
        let dead = dead_names(&names);
        if !dead.is_empty() {
            remove_orphan_relationships(&mut index.persons, &dead);
        }
    }
    rebuild_alias_map(index);
    Ok(())
}

/// specs/08-gamer-companion.md ("Pessoas no jogo") — sobrescreve `relationship_type` direto, sem
/// passar pela escada de afinidade de [`super::super::merge_fields::resolve_relationship_type`].
/// Único chamador de produção: `gamer::game_partners::promote_recurring_partner`, quando a mesma
/// pessoa é vista via visão um número suficiente de vezes com uma sessão de jogo aberta.
pub(in super::super) fn set_relationship_type(
    index: &mut PersonCatalogIndex,
    id: &str,
    relationship_type: super::super::types::RelationshipType,
    now: i64,
) -> Result<PersonRecord, String> {
    let person = index.persons.iter_mut().find(|p| p.id == id).ok_or_else(|| format!("Person not found: {id}"))?;
    person.relationship_type = Some(relationship_type);
    person.updated_at = now;
    Ok(person.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::types::{minimal_person, MentionSource, PersonCatalogIndex as Index, PersonRelationship, UpsertInput};
    use super::super::upsert::upsert_person;

    fn test_id() -> String {
        "pc_test1234".to_string()
    }

    fn empty_index() -> Index {
        Index { version: 2, persons: vec![], alias_map: Default::default(), blocked: None, social_imported_at: None, ..Default::default() }
    }

    fn mention(text: &str) -> NewMention {
        NewMention { source: MentionSource::Voice, text: text.into(), timestamp: 5, url: None, app_name: None, window_title: None, context: None }
    }

    #[test]
    fn add_mention_finds_the_person_by_alias_when_id_does_not_match() {
        let mut idx = empty_index();
        let input = UpsertInput { name: "Ada".into(), aliases: Some(vec!["Lovelace".into()]), ..Default::default() };
        let person = upsert_person(&mut idx, input, 1000, 100, test_id).person.unwrap();

        let updated = add_mention_to_person(&mut idx, "Lovelace", mention("oi"), 2000, 100, test_id).unwrap();
        assert_eq!(updated.id, person.id);
        assert_eq!(updated.mentions.len(), 1);
        assert_eq!(updated.last_seen_at, Some(2000));
    }

    #[test]
    fn add_mention_caps_at_max_mentions_dropping_the_oldest() {
        let mut idx = empty_index();
        let input = UpsertInput { name: "Ada".into(), ..Default::default() };
        let person = upsert_person(&mut idx, input, 1000, 100, test_id).person.unwrap();

        for i in 0..3 {
            add_mention_to_person(&mut idx, &person.id, mention(&format!("m{i}")), 2000 + i, 2, test_id).unwrap();
        }
        let updated = idx.persons.iter().find(|p| p.id == person.id).unwrap();
        assert_eq!(updated.mentions.len(), 2);
        assert_eq!(updated.mentions[0].text, "m1");
        assert_eq!(updated.mentions[1].text, "m2");
    }

    #[test]
    fn add_mention_to_an_unknown_identifier_is_a_named_error() {
        let mut idx = empty_index();
        let err = add_mention_to_person(&mut idx, "ghost", mention("x"), 0, 100, test_id).unwrap_err();
        assert!(err.contains("ghost"));
    }

    #[test]
    fn delete_removes_the_person_and_the_alias_map_entry() {
        let mut idx = empty_index();
        let input = UpsertInput { name: "Ada".into(), ..Default::default() };
        let person = upsert_person(&mut idx, input, 1000, 100, test_id).person.unwrap();

        delete_person(&mut idx, &person.id).unwrap();
        assert!(idx.persons.is_empty());
        assert!(idx.alias_map.get("ada").is_none());
    }

    #[test]
    fn delete_strips_the_dead_name_from_other_people_relationships() {
        let mut idx = empty_index();
        let ghost = upsert_person(&mut idx, UpsertInput { name: "Ghost".into(), ..Default::default() }, 1000, 100, test_id).person.unwrap();
        let mut survivor = minimal_person("survivor", "Survivor");
        survivor.relationships = Some(vec![PersonRelationship {
            name: "Ghost".into(),
            kind: "friend".into(),
            affinity: None,
            trust: None,
            description: None,
            last_interaction_at: None,
        }]);
        idx.persons.push(survivor);

        delete_person(&mut idx, &ghost.id).unwrap();

        let survivor = idx.persons.iter().find(|p| p.id == "survivor").unwrap();
        assert!(survivor.relationships.as_ref().unwrap().is_empty(), "relação órfã devia ter sido removida");
    }

    #[test]
    fn delete_an_unknown_id_is_a_named_error() {
        let mut idx = empty_index();
        assert!(delete_person(&mut idx, "does-not-exist").is_err());
    }

    #[test]
    fn set_relationship_type_overwrites_the_field_and_bumps_updated_at() {
        use super::super::super::types::RelationshipType;
        let mut idx = empty_index();
        let person = upsert_person(&mut idx, UpsertInput { name: "Duo".into(), ..Default::default() }, 1000, 100, test_id).person.unwrap();

        let updated = set_relationship_type(&mut idx, &person.id, RelationshipType::PlaysWith, 2000).unwrap();
        assert_eq!(updated.relationship_type, Some(RelationshipType::PlaysWith));
        assert_eq!(updated.updated_at, 2000);
    }

    #[test]
    fn set_relationship_type_on_an_unknown_id_is_a_named_error() {
        use super::super::super::types::RelationshipType;
        let mut idx = empty_index();
        let err = set_relationship_type(&mut idx, "ghost", RelationshipType::PlaysWith, 0).unwrap_err();
        assert!(err.contains("ghost"));
    }
}
