//! `merge` — porte de
//! `apps/desktop/src/features/person-catalog/services/{person-merge.ts::mergeRecords,
//! person-catalog-merge-ops.ts::mergePersons}`. Só usado por `person_catalog_merge` (RPC de
//! `mika:plugin-rpc`), não pelas 11 ações da tool.
//!
//! **Não portado.** `deleteReplacedImage` (`person-catalog-merge-ops.ts`) apaga do cache de fotos a
//! imagem da pessoa descartada — o lado Rust não tem cache de fotos em disco
//! (`image.rs` busca e devolve; quem guarda é o renderer, em memória), então não há o que apagar. O mesmo vale para
//! `events.emit(PERSON_DELETED/PERSON_UPDATED)` — sem assinante Rust hoje (mesmo gap já documentado
//! em `mutate/mod.rs`).

use super::super::merge_fields::{dead_names, merge_bio, merge_relationships_replace_by_name, merge_string_array, repoint_orphan_relationships};
use super::super::types::{PersonCatalogIndex, PersonRecord};
use super::shared::rebuild_alias_map;

// `Math.min`/`Math.max` com `undefined` dá `NaN` no TS; aqui a ausência de um lado simplesmente
// devolve o outro. Porte de `minDefined`/`maxDefined`.
fn min_defined(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
}

fn max_defined(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
}

fn merge_social_links(
    target: Option<&serde_json::Map<String, serde_json::Value>>,
    source: Option<&serde_json::Map<String, serde_json::Value>>,
) -> serde_json::Map<String, serde_json::Value> {
    // `{...source, ...target}` no TS: alvo vence em conflito, o resultado é sempre um objeto
    // (mesmo vazio), nunca `undefined`.
    let mut merged = source.cloned().unwrap_or_default();
    for (k, v) in target.into_iter().flatten() {
        merged.insert(k.clone(), v.clone());
    }
    merged
}

fn merge_bio_lines(existing: Option<&str>, incoming: Option<&str>) -> Option<String> {
    let mut bio = existing.map(str::to_string);
    for line in incoming.unwrap_or("").split('\n') {
        let clean = line.trim_start_matches('-').trim();
        if !clean.is_empty() {
            bio = Some(merge_bio(bio.as_deref(), clean));
        }
    }
    bio
}

/// Porte de `mergeRecords` — alvo vence em campos de opinião (afinidade/confiança), fonte preenche
/// lacunas, listas e menções se combinam. Pura: quem chama decide se/quando gravar.
pub(in super::super) fn merge_records(target: &PersonRecord, source: &PersonRecord, now: i64, max_mentions: usize) -> PersonRecord {
    let mut attributes = target.attributes.clone();
    for attr in &source.attributes {
        if !attributes.iter().any(|a| a.key.to_lowercase() == attr.key.to_lowercase()) {
            attributes.push(attr.clone());
        }
    }

    // Alvo vence em conflito (mesma convenção do resto de `merge_records`): source entra primeiro,
    // target por cima substitui qualquer relação com o mesmo nome.
    let relationships = merge_relationships_replace_by_name(source.relationships.as_deref().unwrap_or(&[]), target.relationships.as_deref().unwrap_or(&[]));

    let mut mentions: Vec<_> = target.mentions.iter().cloned().chain(source.mentions.iter().cloned()).collect();
    mentions.sort_by_key(|m| m.timestamp);
    let overflow = mentions.len().saturating_sub(max_mentions);
    if overflow > 0 {
        mentions.drain(0..overflow);
    }
    let mention_count = mentions.len();

    // A placeholder ("Esposa", known only by relation) never wins the name over a real one.
    let primary_name = if target.unnamed && !source.unnamed { source.primary_name.clone() } else { target.primary_name.clone() };
    let target_key = super::super::normalize::catalog_key(&primary_name);
    let mut aliases: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let names = std::iter::once(&target.primary_name).chain(target.aliases.iter()).chain(std::iter::once(&source.primary_name)).chain(source.aliases.iter());
    for alias in names {
        let dropped = super::super::people_memory::relations::is_multi_relation_word(alias);
        if !dropped && super::super::normalize::catalog_key(alias) != target_key && seen.insert(alias.clone()) {
            aliases.push(alias.clone());
        }
    }

    PersonRecord {
        id: target.id.clone(),
        primary_name,
        aliases,
        kind: target.kind,
        bio: merge_bio_lines(target.bio.as_deref(), source.bio.as_deref()),
        gender: target.gender.clone().or_else(|| source.gender.clone()),
        birth_date: target.birth_date.clone().or_else(|| source.birth_date.clone()),
        birth_place: target.birth_place.clone().or_else(|| source.birth_place.clone()),
        death_date: target.death_date.clone().or_else(|| source.death_date.clone()),
        nationality: target.nationality.clone().or_else(|| source.nationality.clone()),
        languages: Some(merge_string_array(target.languages.as_deref(), source.languages.as_deref())),
        occupation: Some(merge_string_array(target.occupation.as_deref(), source.occupation.as_deref())),
        known_for: Some(merge_string_array(target.known_for.as_deref(), source.known_for.as_deref())),
        active_years: target.active_years.clone().or_else(|| source.active_years.clone()),
        awards: Some(merge_string_array(target.awards.as_deref(), source.awards.as_deref())),
        social_links: Some(merge_social_links(target.social_links.as_ref(), source.social_links.as_ref())),
        relationships: Some(relationships),
        relationship_type: target.relationship_type,
        relation_to_user: target.relation_to_user.clone().or_else(|| source.relation_to_user.clone()),
        // "Zero é 'nunca avaliado', não 'medido neutro'" — mesmo comentário do TS.
        affinity: if target.affinity != 0.0 { target.affinity } else { source.affinity },
        trust_level: if target.trust_level != 0.0 { target.trust_level } else { source.trust_level },
        origin: target.origin.clone().or_else(|| source.origin.clone()),
        role: target.role.clone().or_else(|| source.role.clone()),
        physical_description: target.physical_description.clone().or_else(|| source.physical_description.clone()),
        abilities: Some(merge_string_array(target.abilities.as_deref(), source.abilities.as_deref())),
        affiliation: target.affiliation.clone().or_else(|| source.affiliation.clone()),
        species: target.species.clone().or_else(|| source.species.clone()),
        first_appearance: target.first_appearance.clone().or_else(|| source.first_appearance.clone()),
        voice_actor: target.voice_actor.clone().or_else(|| source.voice_actor.clone()),
        mentions,
        mention_count,
        first_seen_at: min_defined(target.first_seen_at, source.first_seen_at),
        last_seen_at: max_defined(target.last_seen_at, source.last_seen_at),
        user_rating: target.user_rating.clone().or_else(|| source.user_rating.clone()),
        user_notes: merge_bio_lines(target.user_notes.as_deref(), source.user_notes.as_deref()),
        attributes,
        tags: merge_string_array(Some(&target.tags), Some(&source.tags)),
        image_url: target.image_url.clone().or_else(|| source.image_url.clone()),
        // `notes` não aparece no objeto que `mergeRecords` devolve — herda do alvo via `...target`,
        // nunca funde com a fonte (diferente de `bio`/`userNotes`, que são explicitamente fundidos).
        notes: target.notes.clone(),
        enriched: target.enriched,
        confidence: target.confidence.or(source.confidence),
        ambiguous_with: target.ambiguous_with.clone(),
        facts: super::super::people_memory::merge_facts(&target.facts, &source.facts),
        unnamed: target.unnamed && source.unnamed,
        created_at: target.created_at.min(source.created_at),
        updated_at: now,
    }
}

pub struct MergeOutcome {
    pub person: Option<PersonRecord>,
    pub merged_from: Option<String>,
    pub already_merged: bool,
}

/// Porte de `mergePersons` — funde `source` em `target` e apaga `source`; devolve `already_merged`
/// (sem gravar) quando a fonte não existe ou é o próprio alvo, mesma condição de guarda do TS.
pub(in super::super) fn merge_two_persons(
    index: &mut PersonCatalogIndex,
    target_id: &str,
    source_id: &str,
    now: i64,
    max_mentions: usize,
) -> Result<MergeOutcome, String> {
    let target = index.persons.iter().find(|p| p.id == target_id).cloned().ok_or_else(|| format!("Person not found: {target_id}"))?;

    let Some(source) = index.persons.iter().find(|p| p.id == source_id).cloned() else {
        return Ok(MergeOutcome { person: Some(target), merged_from: None, already_merged: true });
    };
    if source.id == target.id {
        return Ok(MergeOutcome { person: Some(target), merged_from: None, already_merged: true });
    }

    let merged = merge_records(&target, &source, now, max_mentions);
    let dead = dead_names(&[vec![source.primary_name.clone()], source.aliases.clone()].concat());

    index.persons.retain(|p| p.id != source.id);
    for person in index.persons.iter_mut() {
        if person.id == target.id {
            *person = merged.clone();
        }
    }
    repoint_orphan_relationships(&mut index.persons, &dead, &merged.primary_name);

    for person in index.persons.iter_mut() {
        if let Some(ambiguous) = &person.ambiguous_with {
            let kept: Vec<String> = ambiguous.iter().filter(|id| **id != source.id && **id != target.id).cloned().collect();
            person.ambiguous_with = if kept.is_empty() { None } else { Some(kept) };
        }
    }

    rebuild_alias_map(index);
    Ok(MergeOutcome { person: Some(merged), merged_from: Some(source.id), already_merged: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::types::{minimal_person, UpsertInput};
    use super::super::upsert::upsert_person;

    fn test_id() -> String {
        "pc_test1234".to_string()
    }

    /// `test_id` devolve sempre o MESMO literal — ótimo para testar "funde no mesmo registro", mas
    /// os testes de merge daqui criam DOIS registros novos de propósito (alvo e fonte) e precisam de
    /// ids distintos para resolvê-los por id depois; senão a segunda `upsert_person` colidiria com a
    /// primeira e `merge_two_persons` veria `target_id == source_id`.
    fn unique_test_id() -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        format!("pc_test{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    fn empty_index() -> PersonCatalogIndex {
        PersonCatalogIndex { version: 2, persons: vec![], alias_map: Default::default(), blocked: None, social_imported_at: None, ..Default::default() }
    }

    #[test]
    fn merge_records_prefers_target_affinity_when_it_was_ever_assessed() {
        let mut target = minimal_person("t", "Target");
        target.affinity = 40.0;
        let mut source = minimal_person("s", "Source");
        source.affinity = 90.0;
        let merged = merge_records(&target, &source, 5000, 100);
        assert_eq!(merged.affinity, 40.0, "afinidade do alvo já formada não pode ser sobrescrita pela fonte");
    }

    #[test]
    fn merge_records_falls_back_to_source_affinity_when_target_was_never_assessed() {
        let target = minimal_person("t", "Target");
        let mut source = minimal_person("s", "Source");
        source.affinity = 90.0;
        let merged = merge_records(&target, &source, 5000, 100);
        assert_eq!(merged.affinity, 90.0);
    }

    #[test]
    fn merge_records_unions_tags_and_turns_source_name_into_an_alias() {
        let mut target = minimal_person("t", "Target");
        target.tags = vec!["a".into()];
        let mut source = minimal_person("s", "Source Name");
        source.tags = vec!["b".into()];
        let merged = merge_records(&target, &source, 5000, 100);
        assert_eq!(merged.tags, vec!["a".to_string(), "b".to_string()]);
        assert!(merged.aliases.contains(&"Source Name".to_string()));
    }

    #[test]
    fn merge_two_persons_deletes_the_source_and_points_the_target_at_the_merged_record() {
        let mut idx = empty_index();
        let target =
            upsert_person(&mut idx, UpsertInput { name: "Ada Lovelace".into(), ..Default::default() }, 1000, 100, unique_test_id).person.unwrap();
        let source =
            upsert_person(&mut idx, UpsertInput { name: "Ada L".into(), ..Default::default() }, 1000, 100, unique_test_id).person.unwrap();

        let outcome = merge_two_persons(&mut idx, &target.id, &source.id, 2000, 100).unwrap();
        assert!(!outcome.already_merged);
        assert_eq!(outcome.merged_from, Some(source.id.clone()));
        assert_eq!(idx.persons.len(), 1, "fonte tinha de ter sido removida");
        assert_eq!(idx.persons[0].id, target.id);
        assert!(idx.alias_map.get("ada l").is_some(), "alias da fonte tinha de resolver pro alvo depois do merge");
    }

    #[test]
    fn merge_two_persons_with_a_missing_source_reports_already_merged() {
        let mut idx = empty_index();
        let target = upsert_person(&mut idx, UpsertInput { name: "Ada".into(), ..Default::default() }, 1000, 100, test_id).person.unwrap();
        let outcome = merge_two_persons(&mut idx, &target.id, "ghost", 2000, 100).unwrap();
        assert!(outcome.already_merged);
        assert_eq!(outcome.person.unwrap().id, target.id);
        assert_eq!(idx.persons.len(), 1, "nada devia ter sido removido");
    }

    #[test]
    fn merge_two_persons_with_an_unknown_target_is_a_named_error() {
        let mut idx = empty_index();
        assert!(merge_two_persons(&mut idx, "ghost-target", "ghost-source", 2000, 100).is_err());
    }

    #[test]
    fn merge_two_persons_repoints_relationships_from_the_source_name_to_the_target() {
        let mut idx = empty_index();
        let target = upsert_person(&mut idx, UpsertInput { name: "Ada".into(), ..Default::default() }, 1000, 100, unique_test_id).person.unwrap();
        let source =
            upsert_person(&mut idx, UpsertInput { name: "Source".into(), ..Default::default() }, 1000, 100, unique_test_id).person.unwrap();
        let mut bystander = minimal_person("bystander", "Bystander");
        bystander.relationships = Some(vec![super::super::super::types::PersonRelationship {
            name: "Source".into(),
            kind: "friend".into(),
            affinity: None,
            trust: None,
            description: None,
            last_interaction_at: None,
        }]);
        idx.persons.push(bystander);

        merge_two_persons(&mut idx, &target.id, &source.id, 2000, 100).unwrap();

        let bystander = idx.persons.iter().find(|p| p.id == "bystander").unwrap();
        let names: Vec<&str> = bystander.relationships.as_ref().unwrap().iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Ada"], "relação com a fonte tinha de apontar pro alvo depois do merge");
    }
}
