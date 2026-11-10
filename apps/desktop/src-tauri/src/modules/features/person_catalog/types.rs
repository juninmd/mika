//! Modelo de dados do catálogo — porte campo a campo de
//! `apps/desktop/src/features/person-catalog/services/person-catalog.types.ts` e
//! `apps/desktop/src/shared/person-types.ts`.
//!
//! `#[serde(rename_all = "camelCase")]` em cada struct: o arquivo em disco (`catalog.json`) é
//! literalmente o `PersonCatalogIndex` que o TypeScript já escreve, e os nomes de campo do JSON
//! são camelCase lá (`primaryName`, `birthDate`, `mentionCount`...). Divergir aqui faria o usuário
//! "perder" o catálogo inteiro na primeira leitura pós-migração — os campos existiriam no arquivo,
//! só não bateriam com o que o `serde` espera.

use serde::{Deserialize, Serialize};

/// `PERSON_TYPE_VALUES` (`shared/person-types.ts`) — 20 variantes. `film` é um valor LEGADO que o
/// TS normaliza para `Movie` (`LEGACY_PERSON_TYPES`) antes mesmo de chegar ao schema; replicado em
/// `PersonType::parse` (`normalize.rs` chama isto), não aqui — um `Deserialize` derivado não sabe
/// mapear um valor legado para outro variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonType {
    RealPerson,
    FictionalCharacter,
    Organization,
    Pet,
    Politician,
    Journalist,
    Influencer,
    Athlete,
    Singer,
    Streamer,
    Movie,
    Series,
    Historical,
    Author,
    Comedian,
    Book,
    Vtuber,
    Game,
    Anime,
    Other,
}

impl PersonType {
    /// Porte de `normalizePersonType` — aceita o alias legado `film` e qualquer variante válida;
    /// devolve `None` para o resto (o TS também devolve `undefined`, nunca uma variante inventada).
    pub fn parse(raw: &str) -> Option<Self> {
        let canonical = if raw == "film" { "movie" } else { raw };
        serde_json::from_value(serde_json::Value::String(canonical.to_string())).ok()
    }
}

impl Default for PersonType {
    fn default() -> Self {
        Self::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    Stranger,
    Acquaintance,
    Friend,
    CloseFriend,
    BestFriend,
    Rival,
    Enemy,
    Vvip,
    Creator,
    /// specs/08-gamer-companion.md ("Pessoas no jogo") — parceiro de jogo recorrente. Travado como
    /// `Creator`/`Vvip` em `merge_fields::resolve_relationship_type` (não regride por afinidade);
    /// atribuído por `gamer::game_partners::promote_recurring_partner` quando a mesma pessoa é vista
    /// (via visão) um número suficiente de vezes ENQUANTO uma sessão de jogo está aberta.
    PlaysWith,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MentionSource {
    Chat,
    Voice,
    Vision,
    Manual,
    Import,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonMention {
    pub id: String,
    pub source: MentionSource,
    pub text: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonAttribute {
    pub key: String,
    pub value: String,
    pub confidence: Confidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRelationship {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affinity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_interaction_at: Option<i64>,
}

/// `PersonRecord` — ~40 campos, a maioria opcional. Um `struct` só (não dividido) porque é
/// exatamente ISSO que o `serde` serializa/desserializa contra `catalog.json`; espalhar os campos
/// por vários tipos exigiria achatar de novo na borda, o oposto de "mesmo arquivo que o TS já
/// escreve".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRecord {
    pub id: String,
    pub primary_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(rename = "type")]
    pub kind: PersonType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_place: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub death_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nationality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub occupation: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub known_for: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_years: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub awards: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub social_links: Option<serde_json::Map<String, serde_json::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationships: Option<Vec<PersonRelationship>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_type: Option<RelationshipType>,
    /// Parentesco/vínculo desta pessoa com o USUÁRIO em texto livre (ex. "irmão", "chefe") — distinto
    /// de `relationship_type` (a escada de afinidade stranger/friend/...) e de `role` (ocupação ou
    /// papel narrativo, não relação com o usuário). Item 1/3 do pacote "people catalog + relationship
    /// graph": preenchido pela extração automática (`people_memory::apply_ops`, ref `relation`)
    /// e lido de volta por `brain::persona::format_people_block` para o recall.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_to_user: Option<String>,
    #[serde(default)]
    pub affinity: f64,
    #[serde(default)]
    pub trust_level: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub abilities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affiliation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub species: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_appearance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice_actor: Option<String>,

    #[serde(default)]
    pub mentions: Vec<PersonMention>,
    #[serde(default)]
    pub mention_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_seen_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_rating: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_notes: Option<String>,

    #[serde(default)]
    pub attributes: Vec<PersonAttribute>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enriched: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambiguous_with: Option<Vec<String>>,
    /// Long-term facts about THIS person, in the user's own words ("gosta de sorvete") — written by
    /// `people_memory::apply_ops` (post-turn extraction) and edited from the catalog UI. Read back by
    /// `brain::persona::people_block` as the person's card.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<PersonFact>,
    /// Placeholder known only by relation ("minha esposa" before her name is said): `primary_name`
    /// holds the relation label as the user said it, and the first real name replaces it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unnamed: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One remembered fact about a person. `source` is the (redacted) user utterance it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonFact {
    pub id: String,
    pub text: String,
    pub confidence: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub updated_at: i64,
}

/// "Qual irmão?" — a relation-only reference that matched ≥2 people. Facts wait here until the
/// user names which one (or the entry expires); `asked` stops Mika from asking twice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingClarification {
    pub id: String,
    /// Canonical relation key (`people_memory::relations`), e.g. `brother`.
    pub relation: String,
    /// The relation as the user said it ("irmão").
    pub label: String,
    pub candidate_ids: Vec<String>,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub asked: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonCatalogIndex {
    pub version: u32,
    #[serde(default)]
    pub persons: Vec<PersonRecord>,
    #[serde(default)]
    pub alias_map: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub social_imported_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_clarifications: Vec<PendingClarification>,
}

/// Menção nova, sem `id` — porte de `Omit<PersonMention, 'id'>`, o tipo que `UpsertInput.mention`
/// tem no TS. O id nasce dentro do upsert/`add_mention` (mesmo gerador que dá o id da pessoa), nunca
/// antes.
#[derive(Debug, Clone)]
pub struct NewMention {
    pub source: MentionSource,
    pub text: String,
    pub timestamp: i64,
    pub url: Option<String>,
    pub app_name: Option<String>,
    pub window_title: Option<String>,
    pub context: Option<String>,
}

/// `UpsertInput` — igual ao TS, com `mention` sem `id` (gerado no upsert). Todos os campos
/// opcionais além de `name`.
#[derive(Debug, Clone, Default)]
pub struct UpsertInput {
    pub id: Option<String>,
    pub name: String,
    pub aliases: Option<Vec<String>>,
    pub kind: Option<PersonType>,
    pub bio: Option<String>,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub birth_place: Option<String>,
    pub death_date: Option<String>,
    pub nationality: Option<String>,
    pub languages: Option<Vec<String>>,
    pub occupation: Option<Vec<String>>,
    pub known_for: Option<Vec<String>>,
    pub active_years: Option<String>,
    pub awards: Option<Vec<String>>,
    pub social_links: Option<serde_json::Map<String, serde_json::Value>>,
    pub relationships: Option<Vec<PersonRelationship>>,
    pub relationship_type: Option<RelationshipType>,
    pub relation_to_user: Option<String>,
    pub affinity: Option<f64>,
    pub trust_level: Option<f64>,
    pub origin: Option<String>,
    pub role: Option<String>,
    pub physical_description: Option<String>,
    pub abilities: Option<Vec<String>>,
    pub affiliation: Option<String>,
    pub species: Option<String>,
    pub first_appearance: Option<String>,
    pub voice_actor: Option<String>,
    pub user_rating: Option<String>,
    pub user_notes: Option<String>,
    pub attributes: Option<Vec<UpsertAttribute>>,
    pub mention: Option<NewMention>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertAttribute {
    pub key: String,
    pub value: String,
    pub confidence: Option<Confidence>,
}

pub struct UpsertOutcome {
    pub created: bool,
    pub person: Option<PersonRecord>,
    pub blocked: bool,
    pub ambiguous_with: Option<Vec<String>>,
}

/// Construtor de fixture mínima — evita repetir o struct literal de ~40 campos em CADA teste de
/// [`super::query`]/[`super::mutate`]/[`super::merge_fields`]/[`super::store`]. `pub(super)`: só o
/// resto de `person_catalog` (irmãos deste módulo) precisa dele, nunca produção.
#[cfg(test)]
pub(super) fn minimal_person(id: &str, primary_name: &str) -> PersonRecord {
    PersonRecord {
        id: id.to_string(),
        primary_name: primary_name.to_string(),
        aliases: vec![],
        kind: PersonType::Other,
        bio: None,
        gender: None,
        birth_date: None,
        birth_place: None,
        death_date: None,
        nationality: None,
        languages: None,
        occupation: None,
        known_for: None,
        active_years: None,
        awards: None,
        social_links: None,
        relationships: None,
        relationship_type: None,
        relation_to_user: None,
        affinity: 0.0,
        trust_level: 0.0,
        origin: None,
        role: None,
        physical_description: None,
        abilities: None,
        affiliation: None,
        species: None,
        first_appearance: None,
        voice_actor: None,
        mentions: vec![],
        mention_count: 0,
        first_seen_at: None,
        last_seen_at: None,
        user_rating: None,
        user_notes: None,
        attributes: vec![],
        tags: vec![],
        image_url: None,
        notes: None,
        enriched: None,
        confidence: None,
        ambiguous_with: None,
        facts: vec![],
        unnamed: false,
        created_at: 0,
        updated_at: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_type_parses_the_legacy_film_alias_as_movie() {
        assert_eq!(PersonType::parse("film"), Some(PersonType::Movie));
        assert_eq!(PersonType::parse("movie"), Some(PersonType::Movie));
    }

    #[test]
    fn person_type_rejects_an_unknown_value_instead_of_inventing_one() {
        assert_eq!(PersonType::parse("wizard"), None);
    }

    #[test]
    fn person_record_round_trips_through_camel_case_json_like_the_typescript_file() {
        let record = PersonRecord {
            id: "pc_1".into(),
            primary_name: "Ada".into(),
            aliases: vec!["A.".into()],
            kind: PersonType::RealPerson,
            bio: None,
            gender: None,
            birth_date: None,
            birth_place: None,
            death_date: None,
            nationality: None,
            languages: None,
            occupation: None,
            known_for: None,
            active_years: None,
            awards: None,
            social_links: None,
            relationships: None,
            relationship_type: None,
            relation_to_user: None,
            affinity: 0.0,
            trust_level: 0.0,
            origin: None,
            role: None,
            physical_description: None,
            abilities: None,
            affiliation: None,
            species: None,
            first_appearance: None,
            voice_actor: None,
            mentions: vec![],
            mention_count: 0,
            first_seen_at: None,
            last_seen_at: None,
            user_rating: None,
            user_notes: None,
            attributes: vec![],
            tags: vec![],
            image_url: None,
            notes: None,
            enriched: None,
            confidence: None,
            ambiguous_with: None,
            facts: vec![],
            unnamed: false,
            created_at: 1,
            updated_at: 1,
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["primaryName"], serde_json::json!("Ada"));
        assert_eq!(json["type"], serde_json::json!("real_person"));
        assert!(json.get("bio").is_none(), "campo ausente devia sumir, não virar null");

        let back: PersonRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back.primary_name, "Ada");
    }
}
