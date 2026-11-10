//! `upsert` — porte de `person-catalog-mutations.ts::upsertPerson` +
//! `person-upsert.ts::{mergeUpsertIntoExisting,buildPersonFromInput}`. O caminho mais denso do
//! módulo: bloqueio > qualidade de nome > resolução de candidato > funde OU cria.

use super::super::merge_fields::{merge_bio, merge_relationships_replace_by_name, merge_string_array, resolve_relationship_type};
use super::super::normalize::{catalog_key, is_likely_person_name};
use super::super::query::{get_person, resolve_candidates};
use super::super::types::{Confidence, PersonAttribute, PersonCatalogIndex, PersonRecord, UpsertInput, UpsertOutcome};
use super::shared::{build_mention, is_blocked, mention_source_str, rebuild_alias_map, IdGen};

fn merge_social_links(
    existing: Option<&serde_json::Map<String, serde_json::Value>>,
    incoming: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    if existing.is_none() && incoming.is_none() {
        return None;
    }
    let mut merged = existing.cloned().unwrap_or_default();
    if let Some(incoming) = incoming {
        for (k, v) in incoming {
            merged.insert(k.clone(), v.clone());
        }
    }
    Some(merged)
}

/// Porte de `mergeUpsertIntoExisting` — funde um upsert num registro JÁ catalogado.
fn merge_upsert_into_existing(existing: &PersonRecord, input: &UpsertInput, now: i64, max_mentions: usize, gen_id: IdGen) -> PersonRecord {
    let mut aliases: Vec<String> = existing.aliases.iter().filter(|a| a.as_str() != input.name.as_str()).cloned().collect();
    for alias in input.aliases.iter().flatten() {
        if alias != &input.name && !aliases.contains(alias) {
            aliases.push(alias.clone());
        }
    }
    // Promoção de nome ("meu irmão" -> "João"): o nome antigo continua resolvendo como alias.
    if catalog_key(&input.name) != catalog_key(&existing.primary_name) && !aliases.contains(&existing.primary_name) {
        aliases.push(existing.primary_name.clone());
    }

    let new_affinity = (existing.affinity + input.affinity.unwrap_or(0.0)).clamp(-100.0, 100.0);
    let new_trust = (existing.trust_level + input.trust_level.unwrap_or(0.0)).clamp(0.0, 100.0);
    let new_confidence = existing.confidence.map(|c| (c + if input.mention.is_some() { 0.15 } else { 0.0 }).min(1.0));

    let relationships = merge_relationships_replace_by_name(existing.relationships.as_deref().unwrap_or(&[]), input.relationships.as_deref().unwrap_or(&[]));

    let mut updated = PersonRecord {
        primary_name: input.name.clone(),
        aliases,
        kind: input.kind.unwrap_or(existing.kind),
        bio: if let Some(bio) = &input.bio { Some(merge_bio(existing.bio.as_deref(), bio)) } else { existing.bio.clone() },
        gender: input.gender.clone().or_else(|| existing.gender.clone()),
        birth_date: input.birth_date.clone().or_else(|| existing.birth_date.clone()),
        birth_place: input.birth_place.clone().or_else(|| existing.birth_place.clone()),
        death_date: input.death_date.clone().or_else(|| existing.death_date.clone()),
        nationality: input.nationality.clone().or_else(|| existing.nationality.clone()),
        languages: Some(merge_string_array(existing.languages.as_deref(), input.languages.as_deref())),
        occupation: Some(merge_string_array(existing.occupation.as_deref(), input.occupation.as_deref())),
        known_for: Some(merge_string_array(existing.known_for.as_deref(), input.known_for.as_deref())),
        active_years: input.active_years.clone().or_else(|| existing.active_years.clone()),
        awards: Some(merge_string_array(existing.awards.as_deref(), input.awards.as_deref())),
        social_links: merge_social_links(existing.social_links.as_ref(), input.social_links.as_ref()),
        relationships: Some(relationships),
        relationship_type: resolve_relationship_type(new_affinity, input.relationship_type.or(existing.relationship_type)).into(),
        relation_to_user: input.relation_to_user.clone().or_else(|| existing.relation_to_user.clone()),
        affinity: new_affinity,
        trust_level: new_trust,
        origin: input.origin.clone().or_else(|| existing.origin.clone()),
        role: input.role.clone().or_else(|| existing.role.clone()),
        physical_description: input.physical_description.clone().or_else(|| existing.physical_description.clone()),
        abilities: Some(merge_string_array(existing.abilities.as_deref(), input.abilities.as_deref())),
        affiliation: input.affiliation.clone().or_else(|| existing.affiliation.clone()),
        species: input.species.clone().or_else(|| existing.species.clone()),
        first_appearance: input.first_appearance.clone().or_else(|| existing.first_appearance.clone()),
        voice_actor: input.voice_actor.clone().or_else(|| existing.voice_actor.clone()),
        user_rating: input.user_rating.clone().or_else(|| existing.user_rating.clone()),
        user_notes: if let Some(notes) = &input.user_notes {
            Some(merge_bio(existing.user_notes.as_deref(), notes))
        } else {
            existing.user_notes.clone()
        },
        tags: merge_string_array(Some(&existing.tags), input.tags.as_deref()),
        notes: input.notes.clone().or_else(|| existing.notes.clone()),
        image_url: input.image_url.clone().or_else(|| existing.image_url.clone()),
        confidence: new_confidence,
        updated_at: now,
        last_seen_at: if input.mention.is_some() { Some(now) } else { existing.last_seen_at },
        ..existing.clone()
    };

    if let Some(input_attrs) = &input.attributes {
        if !input_attrs.is_empty() {
            let mut attributes = updated.attributes.clone();
            for attr in input_attrs {
                let new_attr = PersonAttribute {
                    key: attr.key.clone(),
                    value: attr.value.clone(),
                    confidence: attr.confidence.unwrap_or(Confidence::Medium),
                    source: Some(input.mention.as_ref().map(|m| mention_source_str(m.source)).unwrap_or("manual").to_string()),
                    updated_at: now,
                };
                match attributes.iter().position(|a| a.key.to_lowercase() == attr.key.to_lowercase()) {
                    Some(idx) => attributes[idx] = new_attr,
                    None => attributes.push(new_attr),
                }
            }
            updated.attributes = attributes;
        }
    }

    if let Some(mention) = &input.mention {
        let new_mention = build_mention(mention, gen_id());
        updated.mentions.push(new_mention);
        let overflow = updated.mentions.len().saturating_sub(max_mentions);
        if overflow > 0 {
            updated.mentions.drain(0..overflow);
        }
        updated.mention_count = updated.mentions.len();
    }

    updated
}

/// Porte de `buildPersonFromInput` — registro novo, ninguém casou com `name`/`aliases`.
fn build_person_from_input(input: UpsertInput, now: i64, id: String, gen_id: IdGen, ambiguous_with: Option<Vec<String>>) -> PersonRecord {
    let has_mention = input.mention.is_some();
    let mentions = input.mention.as_ref().map(|m| vec![build_mention(m, gen_id())]).unwrap_or_default();
    let attribute_source = input.mention.as_ref().map(|m| mention_source_str(m.source)).unwrap_or("manual").to_string();
    let attributes = input
        .attributes
        .unwrap_or_default()
        .into_iter()
        .map(|a| PersonAttribute {
            key: a.key,
            value: a.value,
            confidence: a.confidence.unwrap_or(Confidence::Medium),
            source: Some(attribute_source.clone()),
            updated_at: now,
        })
        .collect();

    PersonRecord {
        id,
        primary_name: input.name.clone(),
        aliases: input.aliases.unwrap_or_default().into_iter().filter(|a| a != &input.name).collect(),
        kind: input.kind.unwrap_or_default(),
        bio: input.bio,
        gender: input.gender,
        birth_date: input.birth_date,
        birth_place: input.birth_place,
        death_date: input.death_date,
        nationality: input.nationality,
        languages: Some(input.languages.unwrap_or_default()),
        occupation: Some(input.occupation.unwrap_or_default()),
        known_for: Some(input.known_for.unwrap_or_default()),
        active_years: input.active_years,
        awards: Some(input.awards.unwrap_or_default()),
        social_links: input.social_links.or_else(|| Some(serde_json::Map::new())),
        relationships: Some(input.relationships.unwrap_or_default()),
        relationship_type: resolve_relationship_type(input.affinity.unwrap_or(0.0), input.relationship_type).into(),
        relation_to_user: input.relation_to_user,
        affinity: input.affinity.unwrap_or(0.0),
        trust_level: input.trust_level.unwrap_or(0.0),
        origin: input.origin,
        role: input.role,
        physical_description: input.physical_description,
        abilities: Some(input.abilities.unwrap_or_default()),
        affiliation: input.affiliation,
        species: input.species,
        first_appearance: input.first_appearance,
        voice_actor: input.voice_actor,
        mentions,
        mention_count: if has_mention { 1 } else { 0 },
        first_seen_at: has_mention.then_some(now),
        last_seen_at: has_mention.then_some(now),
        user_rating: input.user_rating,
        user_notes: input.user_notes,
        attributes,
        tags: input.tags.unwrap_or_default(),
        image_url: input.image_url,
        notes: input.notes,
        // Espec 05:31 (TS) — nascido de conversa (menção) começa com confiança baixa; entrada manual é 1.
        enriched: Some(false),
        confidence: Some(if has_mention { 0.3 } else { 1.0 }),
        ambiguous_with,
        facts: vec![],
        unnamed: false,
        created_at: now,
        updated_at: now,
    }
}

/// Porte de `upsertPerson` — o orquestrador: bloqueio > qualidade de nome > resolução de candidato
/// (id explícito, senão nome/alias) > funde OU cria.
pub(in super::super) fn upsert_person(
    index: &mut PersonCatalogIndex,
    input: UpsertInput,
    now: i64,
    max_mentions: usize,
    gen_id: IdGen,
) -> UpsertOutcome {
    let all_names: Vec<String> = std::iter::once(input.name.clone()).chain(input.aliases.clone().unwrap_or_default()).collect();

    if all_names.iter().any(|n| is_blocked(index, n)) {
        return UpsertOutcome { created: false, person: None, blocked: true, ambiguous_with: None };
    }
    if !is_likely_person_name(&input.name) {
        return UpsertOutcome { created: false, person: None, blocked: false, ambiguous_with: None };
    }

    let mut existing_id = input.id.clone().filter(|id| get_person(index, id).is_some());
    let mut ambiguous_with: Option<Vec<String>> = None;
    if existing_id.is_none() {
        let candidates = resolve_candidates(index, &all_names);
        if candidates.len() > 1 {
            ambiguous_with = Some(candidates.iter().map(|c| c.id.clone()).collect());
        } else if let Some(candidate) = candidates.first() {
            existing_id = Some(candidate.id.clone());
        }
    }

    if let Some(id) = existing_id {
        let position = index.persons.iter().position(|p| p.id == id).expect("id resolvido tem de existir no índice");
        let updated = merge_upsert_into_existing(&index.persons[position], &input, now, max_mentions, gen_id);
        index.persons[position] = updated.clone();
        rebuild_alias_map(index);
        return UpsertOutcome { created: false, person: Some(updated), blocked: false, ambiguous_with: None };
    }

    let new_id = gen_id();
    let person = build_person_from_input(input, now, new_id, gen_id, ambiguous_with.clone());
    index.persons.push(person.clone());
    rebuild_alias_map(index);
    UpsertOutcome { created: true, person: Some(person), blocked: false, ambiguous_with }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::types::{MentionSource, NewMention, PersonType};

    fn test_id() -> String {
        "pc_test1234".to_string()
    }

    fn empty_index() -> PersonCatalogIndex {
        PersonCatalogIndex { version: 2, persons: vec![], alias_map: Default::default(), blocked: None, social_imported_at: None, ..Default::default() }
    }

    fn input(name: &str) -> UpsertInput {
        UpsertInput { name: name.to_string(), ..Default::default() }
    }

    #[test]
    fn creates_a_new_record_when_nothing_matches() {
        let mut idx = empty_index();
        let outcome = upsert_person(&mut idx, input("Ada Lovelace"), 1000, 100, test_id);
        assert!(outcome.created);
        let person = outcome.person.unwrap();
        assert_eq!(person.primary_name, "Ada Lovelace");
        assert_eq!(idx.persons.len(), 1);
        assert_eq!(idx.alias_map.get("ada lovelace"), Some(&person.id));
    }

    #[test]
    fn merges_into_the_same_person_found_by_name() {
        let mut idx = empty_index();
        let first = upsert_person(&mut idx, input("Ada Lovelace"), 1000, 100, test_id).person.unwrap();
        let second = upsert_person(&mut idx, UpsertInput { affinity: Some(10.0), ..input("Ada Lovelace") }, 2000, 100, test_id);
        assert!(!second.created);
        assert_eq!(second.person.unwrap().id, first.id);
        assert_eq!(idx.persons.len(), 1, "merge não devia duplicar o registro");
    }

    #[test]
    fn adds_affinity_as_a_delta_clamped_to_the_range() {
        let mut idx = empty_index();
        upsert_person(&mut idx, UpsertInput { affinity: Some(95.0), ..input("Ada") }, 1000, 100, test_id);
        let result = upsert_person(&mut idx, UpsertInput { affinity: Some(50.0), ..input("Ada") }, 2000, 100, test_id);
        assert_eq!(result.person.unwrap().affinity, 100.0, "afinidade não foi travada no teto");
    }

    #[test]
    fn refuses_a_blocked_name_without_cataloging_it() {
        let mut idx = empty_index();
        idx.blocked = Some(vec!["ada lovelace".to_string()]);
        let outcome = upsert_person(&mut idx, input("Ada Lovelace"), 1000, 100, test_id);
        assert!(outcome.blocked);
        assert!(outcome.person.is_none());
        assert!(idx.persons.is_empty(), "pessoa bloqueada não pode ser catalogada");
    }

    #[test]
    fn refuses_a_generic_descriptor_as_a_name() {
        let mut idx = empty_index();
        let outcome = upsert_person(&mut idx, input("um garoto"), 1000, 100, test_id);
        assert!(!outcome.blocked, "não é bloqueio, é qualidade de nome");
        assert!(outcome.person.is_none());
    }

    /// Ambiguidade nasce de `name`+`aliases` do upsert ATUAL casando com o `primaryName` de DUAS
    /// pessoas já distintas — não de duas pessoas compartilhando o MESMO alias entre si, porque
    /// `rebuild_alias_map` já teria removido essa colisão da segunda no upsert que a criou.
    #[test]
    fn creates_a_new_record_and_flags_ambiguity_when_name_and_alias_match_two_different_people() {
        let mut idx = empty_index();
        upsert_person(&mut idx, input("Alice"), 1000, 100, test_id);
        upsert_person(&mut idx, input("Bob"), 2000, 100, test_id);
        let outcome = upsert_person(&mut idx, UpsertInput { aliases: Some(vec!["Bob".into()]), ..input("Alice") }, 3000, 100, test_id);
        assert!(outcome.created, "ambiguidade cria um registro novo, nunca funde num dos candidatos");
        assert_eq!(outcome.ambiguous_with.map(|v| v.len()), Some(2));
        assert_eq!(idx.persons.len(), 3);
    }

    #[test]
    fn upsert_by_explicit_id_updates_that_record_even_with_a_different_name() {
        let mut idx = empty_index();
        let created = upsert_person(&mut idx, input("Ada"), 1000, 100, test_id).person.unwrap();
        let updated = upsert_person(
            &mut idx,
            UpsertInput { id: Some(created.id.clone()), bio: Some("Matemática".into()), ..input("Ada Lovelace") },
            2000,
            100,
            test_id,
        )
        .person
        .unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.primary_name, "Ada Lovelace");
        assert!(updated.aliases.contains(&"Ada".to_string()), "nome antigo devia virar alias");
    }

    /// Correção ("Maria: colega" -> "Maria: prima") tem de SUBSTITUIR a relação para o mesmo nome,
    /// não empilhar uma segunda entrada — senão o cap de 2 em `people_block.rs` (oldest-first) pode
    /// descartar a correção e a Mika continua falando o rótulo obsoleto.
    #[test]
    fn a_relationship_correction_replaces_the_stored_entry_for_the_same_name() {
        use super::super::super::types::PersonRelationship;
        let mut idx = empty_index();
        let rel = |kind: &str| PersonRelationship { name: "Maria".into(), kind: kind.into(), affinity: None, trust: None, description: None, last_interaction_at: None };

        upsert_person(&mut idx, UpsertInput { relationships: Some(vec![rel("colega")]), ..input("João") }, 1000, 100, test_id);
        let updated = upsert_person(&mut idx, UpsertInput { relationships: Some(vec![rel("prima")]), ..input("João") }, 2000, 100, test_id)
            .person
            .unwrap();

        let relationships = updated.relationships.unwrap();
        assert_eq!(relationships.len(), 1, "correção não pode virar uma segunda entrada para o mesmo nome");
        assert_eq!(relationships[0].kind, "prima");
    }

    #[test]
    fn caps_mentions_at_max_mentions() {
        let mut idx = empty_index();
        let mut id = None;
        for i in 0..5 {
            let mut inp = input("Ada");
            inp.mention = Some(NewMention {
                source: MentionSource::Chat,
                text: format!("m{i}"),
                timestamp: i,
                url: None,
                app_name: None,
                window_title: None,
                context: None,
            });
            let outcome = upsert_person(&mut idx, inp, 1000 + i, 3, test_id);
            id = outcome.person.map(|p| p.id);
        }
        let person = idx.persons.iter().find(|p| Some(&p.id) == id.as_ref()).unwrap();
        assert_eq!(person.mentions.len(), 3, "menções deviam estar limitadas a max_mentions");
        assert_eq!(person.mention_count, 3);
        assert_eq!(person.mentions.last().unwrap().text, "m4", "devia manter as mais recentes");
    }

    /// NEGATIVO — o tipo default de um registro novo é `Other`, nunca inventado de outro upsert.
    #[test]
    fn new_person_defaults_to_other_type_when_unset() {
        let mut idx = empty_index();
        let person = upsert_person(&mut idx, input("Sem Tipo"), 1000, 100, test_id).person.unwrap();
        assert_eq!(person.kind, PersonType::Other);
    }
}
