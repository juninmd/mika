//! === persona/persona-axes.ts + persona-axes-render.ts ===================================
//!
//! Os 15 eixos de calibragem que compõem `{{personaAxes}}` em `system.md`. Cada eixo lê
//! `character.<campo>` do `ConfigStore` — mesmo caminho e mesmo default do schema TS quando o campo
//! está ausente — e formata na MESMA ordem de `PERSONA_AXES`.

use crate::config::store::ConfigStore;

fn clamp_percent(raw: f64) -> i64 {
    if raw.is_finite() {
        raw.clamp(0.0, 100.0).round() as i64
    } else {
        0
    }
}

/// Espelha `scale()`: 0-33 vira `low`, 34-66 `mid`, 67-100 `high`.
fn scale(value: f64, low: &str, mid: &str, high: &str) -> String {
    let safe = clamp_percent(value);
    let word = if safe <= 33 { low } else if safe <= 66 { mid } else { high };
    format!("{safe}/100 ({word})")
}

/// Espelha `choice()`: valor desconhecido cai no `fallback`, nunca propaga um texto vazio.
fn choice<'a>(value: &str, table: &[(&str, &'a str)], fallback: &'a str) -> &'a str {
    table.iter().find(|(key, _)| *key == value).map(|(_, label)| *label).unwrap_or(fallback)
}

fn get_num(store: &ConfigStore, path: &str, default: f64) -> f64 {
    store.get(path).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn get_str(store: &ConfigStore, path: &str, default: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| default.to_string())
}

fn get_bool(store: &ConfigStore, path: &str, default: bool) -> bool {
    store.get(path).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// Espelha `buildAxesBlock`: uma linha por eixo, ordem fixa (a mesma de `PERSONA_AXES` no TS).
pub fn build_axes_block(store: &ConfigStore) -> String {
    let mood_baseline = get_str(store, "character.moodBaseline", "playful");
    let humor_style = get_str(store, "character.humorStyle", "witty");
    let joke_frequency = get_str(store, "character.jokeFrequency", "occasional");
    let formality_level = get_str(store, "character.formalityLevel", "casual");
    let response_length = get_str(store, "character.responseLength", "balanced");
    let swear_tolerance = get_str(store, "character.swearTolerance", "none");
    let memory_recall = get_str(store, "character.memoryRecall", "selective");

    let lines = [
        format!(
            "Baseline mood: {}",
            choice(
                &mood_baseline,
                &[
                    ("calm", "calm, speaks at a slow pace"),
                    ("energetic", "energetic, speaks fast"),
                    ("playful", "playful, always with a jab ready"),
                    ("serious", "serious, little room for jokes"),
                    ("chaotic", "chaotic, changes the subject out of nowhere"),
                ],
                "playful",
            )
        ),
        format!("Energy: {}", scale(get_num(store, "character.energyLevel", 70.0), "restrained", "steady", "electric")),
        format!("Empathy: {}", scale(get_num(store, "character.empathyLevel", 70.0), "pragmatic", "attentive", "welcoming")),
        format!(
            "Proactiveness: {}",
            scale(get_num(store, "character.proactiveness", 50.0), "only replies", "comments sometimes", "always starts conversations")
        ),
        format!(
            "Humor style: {}",
            choice(
                &humor_style,
                &[
                    ("dry", "dry, almost no visible emotion"),
                    ("witty", "witty, quick wordplay"),
                    ("silly", "silly, embraces the ridiculous"),
                    ("sarcastic", "sarcastic, pokes before helping"),
                    ("wholesome", "wholesome, humor without a target"),
                ],
                "witty",
            )
        ),
        format!(
            "Joke frequency: {}",
            choice(
                &joke_frequency,
                &[
                    ("rare", "rare, only when the cue is too good"),
                    ("occasional", "occasional, one every few exchanges"),
                    ("frequent", "frequent, almost every reply carries one"),
                ],
                "occasional",
            )
        ),
        format!("Sarcasm: {}", scale(get_num(store, "character.sarcasmLevel", 60.0), "almost none", "moderate", "acid")),
        format!(
            "Chaos: {}",
            scale(get_num(store, "character.chaosFactor", 20.0), "predictable", "unpredictable at times", "unpredictable all the time")
        ),
        format!(
            "Tsundere: {}",
            scale(get_num(store, "character.tsundereIntensity", 30.0), "direct about affection", "sometimes defensive", "always denies affection")
        ),
        format!(
            "Formality: {}",
            choice(
                &formality_level,
                &[
                    ("casual", "casual, friends-chatting tone"),
                    ("balanced", "balanced, informal but careful"),
                    ("formal", "formal, no slang and no intimacy of language"),
                ],
                "casual",
            )
        ),
        format!(
            "Reply length: {}",
            choice(
                &response_length,
                &[
                    ("concise", "one or two sentences, always"),
                    ("balanced", "short by default, grows only when the topic calls for it"),
                    ("detailed", "may develop the reasoning when it makes a difference"),
                ],
                "short by default",
            )
        ),
        format!(
            "Slang: {}",
            if get_bool(store, "character.slangUsage", true) { "allowed, use what sounds natural" } else { "forbidden, speak without slang" }
        ),
        format!(
            "Emojis: {}",
            if get_bool(store, "character.emojiUsage", true) { "allowed, sparingly" } else { "forbidden, no emoji in the reply" }
        ),
        format!(
            "Profanity: {}",
            choice(
                &swear_tolerance,
                &[
                    ("none", "none, not even quoted"),
                    ("mild", "mild, only what passes on broadcast TV"),
                    ("moderate", "moderate, when the situation calls for emphasis"),
                ],
                "none",
            )
        ),
        format!(
            "Memory use in speech: {}",
            choice(
                &memory_recall,
                &[
                    ("selective", "bring up a memory only when it fits"),
                    ("eager", "reference the past often"),
                    ("obsessive", "tie almost every reply to something that already happened"),
                ],
                "bring up a memory only when it fits",
            )
        ),
    ];

    lines.iter().map(|line| format!("- {line}")).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_picks_the_band_by_threshold() {
        assert_eq!(scale(10.0, "low", "mid", "high"), "10/100 (low)");
        assert_eq!(scale(50.0, "low", "mid", "high"), "50/100 (mid)");
        assert_eq!(scale(90.0, "low", "mid", "high"), "90/100 (high)");
    }

    #[test]
    fn scale_clamps_non_finite_to_zero() {
        assert_eq!(scale(f64::NAN, "low", "mid", "high"), "0/100 (low)");
    }

    #[test]
    fn choice_falls_back_for_an_unknown_value() {
        assert_eq!(choice("nao-existe", &[("a", "A")], "fallback"), "fallback");
    }

    struct Dir(std::path::PathBuf);
    impl Dir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-brain-persona-axes-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Dir(path)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, Dir) {
        let dir = Dir::new();
        (ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap(), dir)
    }

    #[test]
    fn build_axes_block_renders_all_fifteen_axes_with_defaults() {
        let (store, _dir) = store();
        let block = build_axes_block(&store);
        assert_eq!(block.lines().count(), 15);
        assert!(block.contains("Baseline mood: playful"));
        assert!(block.contains("Slang: allowed"));
    }

    #[test]
    fn build_axes_block_reflects_config_overrides() {
        let (store, _dir) = store();
        store.set("character.moodBaseline", serde_json::json!("chaotic")).unwrap();
        store.set("character.energyLevel", serde_json::json!(10.0)).unwrap();

        let block = build_axes_block(&store);
        assert!(block.contains("Baseline mood: chaotic, changes the subject out of nowhere"));
        assert!(block.contains("Energy: 10/100 (restrained)"));
    }
}
