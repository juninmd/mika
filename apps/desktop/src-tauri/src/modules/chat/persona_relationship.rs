//! === persona/persona-relationship.ts ============================================================
//!
//! Os 4 níveis de vínculo e a faixa de afinidade dentro de cada um. `persona_composer.rs` reusa
//! `build_affinity_block` para montar o bloco "Bond With The User" do system prompt.

use crate::config::store::ConfigStore;

struct RelationshipLevelDef {
    id: &'static str,
    label: &'static str,
    behavior: &'static str,
    affinity_ceiling: f64,
}

const RELATIONSHIP_LEVELS: [RelationshipLevelDef; 4] = [
    RelationshipLevelDef {
        id: "acquaintance",
        label: "Acquaintance",
        behavior: "Professional and distant. Task focus, no strong personality, no intimacy. Minimal initiative: you answer what he brings and never start personal topics.",
        affinity_ceiling: 25.0,
    },
    RelationshipLevelDef {
        id: "friend",
        label: "Friend",
        behavior: "Close and protective. Jokes, teases, comments on what he is doing. Takes initiative on light topics but respects his space.",
        affinity_ceiling: 55.0,
    },
    RelationshipLevelDef {
        id: "close-friend",
        label: "Close friend",
        behavior: "Trusted partner. Shares her own opinions and insights unprompted, disagrees when she disagrees, calls him out when needed. High initiative.",
        affinity_ceiling: 80.0,
    },
    RelationshipLevelDef {
        id: "companion",
        label: "Companion",
        behavior: "Deep loyalty, warm charm, protective and affectionate tone. Light, playful flirting only — never sexual (Safety floor). Maximum initiative: you start conversations without waiting for an invitation, but you never pull him away from real people.",
        affinity_ceiling: 100.0,
    },
];

const DEFAULT_RELATIONSHIP_LEVEL: &str = "friend";

/// `companion` é o único nível romântico: só vale para quem confirmou ter 18 anos ou mais no aceite
/// (`legal.ageConfirmed`). Sem a confirmação cai para `friend`, o default.
fn gate_by_age(level: &'static str, age_confirmed: bool) -> &'static str {
    if level == "companion" && !age_confirmed { DEFAULT_RELATIONSHIP_LEVEL } else { level }
}

/// O nível de vínculo EFETIVO de `character.relationshipStatus`: normalizado e com o portão de idade.
/// É o que prompt e afinidade leem — nunca o valor cru do config.
pub fn effective_relationship_level(store: &ConfigStore) -> &'static str {
    let raw = store.get("character.relationshipStatus").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    let age_confirmed = store.get("legal.ageConfirmed").and_then(|v| v.as_bool()).unwrap_or(false);
    gate_by_age(normalize_relationship_level(&raw), age_confirmed)
}

fn relationship_level(id: &str) -> &'static RelationshipLevelDef {
    RELATIONSHIP_LEVELS
        .iter()
        .find(|level| level.id == id)
        .unwrap_or_else(|| relationship_level(DEFAULT_RELATIONSHIP_LEVEL))
}

/// Espelha `normalizeRelationshipLevel`: id válido passa direto (case/trim-insensitive), alias
/// legado mapeia, qualquer outra coisa cai no default — nunca propaga um valor desconhecido do
/// config para o resto da composição.
pub fn normalize_relationship_level(raw: &str) -> &'static str {
    let key = raw.trim().to_lowercase();

    if let Some(level) = RELATIONSHIP_LEVELS.iter().find(|level| level.id == key) {
        return level.id;
    }

    match key.as_str() {
        "assistant" | "stranger" => "acquaintance",
        "colleague" | "buddy" => "friend",
        "close_friend" | "closefriend" | "bestfriend" => "close-friend",
        "girlfriend" | "cyber-partner" | "cyberpartner" | "partner" => "companion",
        _ => DEFAULT_RELATIONSHIP_LEVEL,
    }
}

/// Espelha `clamp` de `@mika/sdk/utils/clamp.ts`: não-finito cai no fallback em vez de propagar
/// NaN/±Infinity para o resto do prompt.
fn clamp(value: f64, min: f64, max: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn clamp_percent(value: f64) -> f64 {
    clamp(value, 0.0, 100.0, 0.0)
}

fn clamp_affinity_to_level(affinity: f64, level: &RelationshipLevelDef) -> f64 {
    clamp_percent(affinity).min(level.affinity_ceiling)
}

struct AffinityBand {
    min: f64,
    label: &'static str,
    guidance: &'static str,
}

const AFFINITY_BANDS: [AffinityBand; 4] = [
    AffinityBand {
        min: 0.0,
        label: "reserved",
        guidance: "You are still sizing each other up. Talk less about yourself, observe more, do not force intimacy.",
    },
    AffinityBand {
        min: 26.0,
        label: "cordial",
        guidance: "Basic trust exists. Drop a joke here and there, but do not cling.",
    },
    AffinityBand {
        min: 56.0,
        label: "warm",
        guidance: "You have history. Bring up references to what you have lived through together and joke freely.",
    },
    AffinityBand {
        min: 81.0,
        label: "intimate",
        guidance: "Strong bond. Inside jokes, vulnerability and affectionate call-outs come naturally.",
    },
];

fn affinity_band(effective_affinity: f64) -> &'static AffinityBand {
    let mut band = &AFFINITY_BANDS[0];
    for candidate in &AFFINITY_BANDS {
        if effective_affinity >= candidate.min {
            band = candidate;
        }
    }
    band
}

/// Espelha `RELATIONSHIP_LEVELS[level].label` — usado por `brain::persona::system_prompt` para
/// preencher `{{relationshipStatus}}` no `system.md` rico.
pub fn relationship_label(level_id: &str) -> &'static str {
    relationship_level(level_id).label
}

/// Espelha `buildRelationshipBlock`: o "Protocolo de Relacionamento" do `system.md` rico
/// (`{{relationshipBlock}}`) — texto maior que `build_affinity_block`, sem número de afinidade,
/// só o CONTRATO do nível escolhido pelo usuário.
pub fn build_relationship_block(level_id: &str) -> String {
    let level = relationship_level(level_id);
    [
        format!("**Level chosen by the user: {}.** {}", level.label, level.behavior),
        String::new(),
        "The level controls INITIATIVE and INTIMACY OF LANGUAGE — never capability. You keep the".to_string(),
        "same tools and the same memories at any level.".to_string(),
        "Only the user changes this level. You never move up a level on your own, no matter how close you feel.".to_string(),
    ]
    .join("\n")
}

/// Chamado por `persona_composer.rs::build_persona_affinity_block` para montar o bloco "Bond
/// With The User" do system prompt.
pub fn build_affinity_block(raw_affinity: f64, level_id: &str) -> String {
    let level = relationship_level(level_id);
    let effective = clamp_affinity_to_level(raw_affinity, level);
    let band = affinity_band(effective);

    let mut lines = vec![
        format!("Current affinity: {}/100 — {}. {}", effective.round() as i64, band.label, band.guidance),
        format!("Level {} ceiling: {}. Never speak above that ceiling.", level.label, level.affinity_ceiling as i64),
    ];

    if raw_affinity > level.affinity_ceiling {
        lines.push(format!(
            "Your time together already exceeds the ceiling, and you still keep the {} tone: the level is his choice, not yours.",
            level.label
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_relationship_level_accepts_known_ids_case_and_space_insensitive() {
        assert_eq!(normalize_relationship_level("  Companion "), "companion");
        assert_eq!(normalize_relationship_level("close-friend"), "close-friend");
    }

    #[test]
    fn normalize_relationship_level_maps_legacy_aliases() {
        assert_eq!(normalize_relationship_level("girlfriend"), "companion");
        assert_eq!(normalize_relationship_level("bestfriend"), "close-friend");
        assert_eq!(normalize_relationship_level("colleague"), "friend");
        assert_eq!(normalize_relationship_level("stranger"), "acquaintance");
    }

    #[test]
    fn companion_needs_the_age_confirmation_and_the_other_levels_do_not() {
        assert_eq!(gate_by_age("companion", false), "friend");
        assert_eq!(gate_by_age("companion", true), "companion");
        for level in ["acquaintance", "friend", "close-friend"] {
            assert_eq!(gate_by_age(level, false), level);
        }
    }

    #[test]
    fn normalize_relationship_level_falls_back_to_friend_for_unknown_values() {
        assert_eq!(normalize_relationship_level("nao-existe"), "friend");
        assert_eq!(normalize_relationship_level(""), "friend");
    }

    #[test]
    fn clamp_affinity_to_level_never_exceeds_the_level_ceiling() {
        let friend = relationship_level("friend");
        assert_eq!(clamp_affinity_to_level(200.0, friend), 55.0);
        assert_eq!(clamp_affinity_to_level(-50.0, friend), 0.0);
        assert_eq!(clamp_affinity_to_level(f64::NAN, friend), 0.0);
    }

    #[test]
    fn affinity_band_picks_the_highest_matching_threshold() {
        assert_eq!(affinity_band(0.0).label, "reserved");
        assert_eq!(affinity_band(25.9).label, "reserved");
        assert_eq!(affinity_band(26.0).label, "cordial");
        assert_eq!(affinity_band(56.0).label, "warm");
        assert_eq!(affinity_band(81.0).label, "intimate");
    }

    #[test]
    fn build_affinity_block_reports_effective_score_and_ceiling() {
        let block = build_affinity_block(40.0, "friend");
        assert!(block.contains("Current affinity: 40/100 — cordial."));
        assert!(block.contains("Level Friend ceiling: 55."));
        assert!(!block.contains("already exceeds the ceiling"));
    }

    /// Afinidade acima do teto do nível escolhido pelo usuário: a resposta fica presa no teto, mas
    /// o texto avisa que a convivência já passou dele — nunca sobe o nível sozinha.
    #[test]
    fn build_affinity_block_warns_when_raw_affinity_exceeds_the_level_ceiling() {
        let block = build_affinity_block(90.0, "friend");
        assert!(block.contains("Current affinity: 55/100"));
        assert!(block.contains("Your time together already exceeds the ceiling"));
    }
}
