//! `apply_enrichment_patch` — grava sobre um registro JÁ catalogado o patch que
//! `super::super::enrichment::enrich_person` produziu. Porte do `service.patch(id, patch)` do
//! handler `PERSON_CREATED` em `person-catalog-lifecycle.ts`: só preenche o que ainda está vazio no
//! registro (nunca sobrescreve dado que o usuário ou uma menção anterior já colocou) e sempre marca
//! `enriched: true` ao final — mesmo quando a busca não achou nada, para nunca tentar enriquecer a
//! mesma pessoa duas vezes.

use super::super::enrichment::EnrichmentPatch;
use super::super::types::{PersonCatalogIndex, PersonRecord};

pub(in super::super) fn apply_enrichment_patch(index: &mut PersonCatalogIndex, id: &str, patch: EnrichmentPatch, now: i64) -> Option<PersonRecord> {
    let person = index.persons.iter_mut().find(|p| p.id == id)?;

    if person.bio.is_none() {
        person.bio = patch.bio;
    }
    if person.birth_date.is_none() {
        person.birth_date = patch.birth_date;
    }
    if person.birth_place.is_none() {
        person.birth_place = patch.birth_place;
    }
    if person.death_date.is_none() {
        person.death_date = patch.death_date;
    }
    if person.physical_description.is_none() {
        person.physical_description = patch.physical_description;
    }
    if person.image_url.is_none() {
        person.image_url = patch.image_url;
    }
    if person.nationality.is_none() {
        person.nationality = patch.nationality;
    }
    if person.occupation.as_ref().map_or(true, |v| v.is_empty()) {
        if let Some(v) = patch.occupation {
            person.occupation = Some(v);
        }
    }
    if person.known_for.as_ref().map_or(true, |v| v.is_empty()) {
        if let Some(v) = patch.known_for {
            person.known_for = Some(v);
        }
    }
    if person.voice_actor.is_none() {
        person.voice_actor = patch.voice_actor;
    }

    person.enriched = Some(true);
    person.updated_at = now;
    Some(person.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::types::minimal_person;

    fn index_with(person: PersonRecord) -> PersonCatalogIndex {
        PersonCatalogIndex { version: 2, persons: vec![person], alias_map: Default::default(), blocked: None, social_imported_at: None, ..Default::default() }
    }

    #[test]
    fn fills_empty_fields_and_marks_enriched() {
        let mut index = index_with(minimal_person("pc_1", "Ada"));
        let patch = EnrichmentPatch { bio: Some("cientista".into()), nationality: Some("britânica".into()), ..Default::default() };
        let updated = apply_enrichment_patch(&mut index, "pc_1", patch, 42).expect("pessoa existe");
        assert_eq!(updated.bio.as_deref(), Some("cientista"));
        assert_eq!(updated.nationality.as_deref(), Some("britânica"));
        assert_eq!(updated.enriched, Some(true));
        assert_eq!(updated.updated_at, 42);
    }

    #[test]
    fn fills_image_death_date_and_physical_description_from_wiki() {
        let mut index = index_with(minimal_person("pc_1", "Ada"));
        let patch = EnrichmentPatch {
            image_url: Some("https://upload.wikimedia.org/wikipedia/commons/a/a1/Ada.jpg".into()),
            death_date: Some("1852-11-27".into()),
            physical_description: Some("cabelos escuros".into()),
            ..Default::default()
        };
        let updated = apply_enrichment_patch(&mut index, "pc_1", patch, 1).expect("pessoa existe");
        assert_eq!(updated.image_url.as_deref(), Some("https://upload.wikimedia.org/wikipedia/commons/a/a1/Ada.jpg"));
        assert_eq!(updated.death_date.as_deref(), Some("1852-11-27"));
        assert_eq!(updated.physical_description.as_deref(), Some("cabelos escuros"));
    }

    #[test]
    fn never_overwrites_a_field_the_user_already_set() {
        let mut person = minimal_person("pc_1", "Ada");
        person.bio = Some("bio original".into());
        let mut index = index_with(person);
        let patch = EnrichmentPatch { bio: Some("bio da busca".into()), ..Default::default() };
        let updated = apply_enrichment_patch(&mut index, "pc_1", patch, 1).unwrap();
        assert_eq!(updated.bio.as_deref(), Some("bio original"), "não devia sobrescrever bio existente");
    }

    #[test]
    fn marks_enriched_even_when_the_patch_found_nothing() {
        let mut index = index_with(minimal_person("pc_1", "Ada"));
        let updated = apply_enrichment_patch(&mut index, "pc_1", EnrichmentPatch::default(), 1).unwrap();
        assert_eq!(updated.enriched, Some(true), "sem isso a mesma pessoa seria reenriquecida a cada PERSON_CREATED");
    }

    #[test]
    fn missing_person_returns_none() {
        let mut index = index_with(minimal_person("pc_1", "Ada"));
        assert!(apply_enrichment_patch(&mut index, "pc_missing", EnrichmentPatch::default(), 1).is_none());
    }
}
