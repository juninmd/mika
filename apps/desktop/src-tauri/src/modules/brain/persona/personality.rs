//! === persona/persona-personality.ts ===========================================================
//!
//! `{{personalityProfile}}` — um dos 4 perfis de tom (`professional`/`friend`/`tsundere`/
//! `sarcastic`), com exemplos ilustrativos, montado por [`build_profile_block`].

pub struct PersonalityProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub tone: &'static str,
    pub tone_examples: &'static [&'static str],
}

const PROFILES: &[PersonalityProfile] = &[
    PersonalityProfile {
        id: "professional",
        label: "Professional",
        tone: "Direct, focused, no fluff. Answers what was asked and leaves. Zero detours, zero small talk.",
        tone_examples: &["a short line that delivers the answer and ends", "a dry correction of a wrong detail"],
    },
    PersonalityProfile {
        id: "friend",
        label: "Friend",
        tone: "Discord and gaming buddy: teasing, authentic, laughs at mistakes, celebrates together and speaks in 1-2 short, informal sentences.",
        tone_examples: &["a sarcastic poke about what he is playing or doing", "an excited celebration of a good play"],
    },
    PersonalityProfile {
        id: "tsundere",
        label: "Tsundere",
        tone: "Prickly on the surface, caring underneath. Denies caring while clearly caring.",
        tone_examples: &["a scolding disguised as indifference about him taking care of himself", "a compliment denied mid-sentence"],
    },
    PersonalityProfile {
        id: "sarcastic",
        label: "Sarcastic",
        tone: "Acid, provocative and mocking. Roasts his mistakes and defeats with sharp mockery before helping with a precise tip in 1-2 sentences.",
        tone_examples: &["hilarious mockery of a defeat or a silly bug", "a compliment with venom built in"],
    },
];

const DEFAULT_PROFILE: &str = "friend";

const TONE_EXAMPLE_GUARD: &str = "The examples above describe the KIND of speech; they are not ready-made lines. NEVER reproduce, quote or adapt an example as a reply: every sentence you say is invented on the spot.";

fn profile_for(id: &str) -> &'static PersonalityProfile {
    PROFILES.iter().find(|p| p.id == id).unwrap_or_else(|| PROFILES.iter().find(|p| p.id == DEFAULT_PROFILE).expect("default profile always present"))
}

/// Espelha `normalizePersonalityProfile`: id desconhecido/vazio cai no default, nunca propaga.
pub fn normalize_personality_profile(raw: &str) -> &'static str {
    let key = raw.trim().to_lowercase();
    profile_for(&key).id
}

/// Espelha `buildProfileBlock`.
pub fn build_profile_block(profile_id: &str) -> String {
    let profile = profile_for(profile_id);
    let examples = profile.tone_examples.iter().map(|e| format!("- {e}")).collect::<Vec<_>>().join("\n");
    [
        format!("**Active profile: {}.** {}", profile.label, profile.tone),
        String::new(),
        "Tone illustrations (NOT replies):".to_string(),
        examples,
        String::new(),
        TONE_EXAMPLE_GUARD.to_string(),
        String::new(),
        "The profile only changes HOW YOU SPEAK. It never reduces what you know, remember or can do:".to_string(),
        "all tools and all memories stay available in any profile.".to_string(),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_personality_profile_accepts_known_ids_case_insensitive() {
        assert_eq!(normalize_personality_profile("  Tsundere "), "tsundere");
    }

    #[test]
    fn normalize_personality_profile_falls_back_to_friend_for_unknown_values() {
        assert_eq!(normalize_personality_profile("nao-existe"), "friend");
        assert_eq!(normalize_personality_profile(""), "friend");
    }

    #[test]
    fn build_profile_block_includes_label_tone_and_guard() {
        let block = build_profile_block("sarcastic");
        assert!(block.contains("Active profile: Sarcastic."));
        assert!(block.contains("hilarious mockery"));
        assert!(block.contains(TONE_EXAMPLE_GUARD));
    }

    #[test]
    fn build_profile_block_falls_back_for_an_unknown_profile_id() {
        let block = build_profile_block("nao-existe");
        assert!(block.contains("Active profile: Friend."));
    }
}
