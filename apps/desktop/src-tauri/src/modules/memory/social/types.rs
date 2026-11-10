//! Modelo de dados do grafo social — porte literal de
//! `apps/desktop/src/main/services/memory/social/types.ts`. Nomes de campo em `camelCase` (via
//! `serde(rename_all)`) porque este JSON é gravado no MESMO `memory.db` que o build TypeScript já
//! escreve (ver o cabeçalho de `social/mod.rs`) — divergir de um campo faria o outro lado não
//! reconhecer o registro.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonType {
    User,
    Friend,
    Family,
    Colleague,
    Character,
    Competitor,
    #[default]
    Other,
}

/// = `PersonRecord`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRecord {
    pub id: String,
    pub primary_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(rename = "type")]
    pub kind: PersonType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub universe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub affinity: f64,
    pub last_mentioned_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Links this social-graph person to its `person_catalog` record (`pc_*` id) — the single
    /// canonical identity the two independent stores share. `None` until `person_catalog::lifecycle`
    /// links them (a person known only through `add_fact`/`add_relationship`, never mentioned in a
    /// chat turn the catalog extractor ran on, has no catalog counterpart yet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipDirection {
    UserToPerson,
    PersonToPerson,
}

/// = `RelationshipEdge`. `person_id` is always the edge's "from" person. For a `PersonToPerson`
/// edge, `to_person_id` is the second person (the edge used to reuse `person_id` for both ends —
/// see the `person_catalog` work-package report). `UserToPerson` leaves `to_person_id` `None`: the
/// other end is the user, implicit (there is no social-graph node id for "user").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipEdge {
    pub id: String,
    pub person_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_person_id: Option<String>,
    pub label: String,
    pub direction: RelationshipDirection,
    pub confidence: f64,
    pub updated_at: i64,
    /// Set when a later correction supersedes this edge (see `relationship_graph::supersede_pair`).
    /// A superseded edge is kept — never deleted — so its history survives; only edges with
    /// `valid_to: None` count as the CURRENT truth (recall block, BFS `path`/`relationships`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<i64>,
}

/// = `LearnedFact`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnedFact {
    pub id: String,
    pub subject: String,
    pub fact: String,
    pub confidence: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    pub updated_at: i64,
}

/// = `SocialEvent`. Só o suficiente para `person_graph::forget_person` varrer eventos que citam a
/// pessoa apagada — nenhuma ação do escopo desta rodada GRAVA um evento novo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialEvent {
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub people: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub created_at: i64,
}

/// = `PersonalityProfile` (`user-profile-store.ts`). Campos escalares em `String` livre, não enum
/// Rust: já chegam validados pelo `input_schema` JSON da tool (`features::user::tool`), e um enum
/// aqui duplicaria essa validação sem ganhar nada — mesma escolha que `LearnedFact`/`PersonRecord`
/// já fazem para campos de texto livre.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalityProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub communication_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub humor_tolerance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_level: Option<String>,
    #[serde(default)]
    pub top_interests: Vec<String>,
}

/// = `UserProfile` (`user-profile-store.ts`). `skip_serializing_if` nos campos opcionais espelha o
/// `JSON.stringify` do TS: um campo `undefined` some da chave, nunca vira `null` — importa para
/// `forgetUserProfile` (ver `user_profile.rs`), que apaga um campo tornando-o ausente, não nulo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pronouns: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personality: Option<PersonalityProfile>,
    pub updated_at: i64,
}

/// As 5 tabelas de `social-tables.ts::SOCIAL_TABLES`, nomeadas aqui pela mesma razão que lá: um
/// `Vec<&str>` centralizado, não uma string mágica espalhada por `person_graph`/`fact_store`.
pub const SOCIAL_TABLE_PEOPLE: &str = "social_people";
pub const SOCIAL_TABLE_RELATIONSHIPS: &str = "social_relationships";
pub const SOCIAL_TABLE_FACTS: &str = "social_facts";
pub const SOCIAL_TABLE_EVENTS: &str = "social_events";
pub const SOCIAL_TABLE_USER_PROFILE: &str = "social_user_profile";
