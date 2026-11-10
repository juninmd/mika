//! Mood follows the conversation. Each user turn is an interaction (affinity, throttled by
//! `mood.minAffinityInteractionTime`) — the `USER_MESSAGE` listener of the TS plugin
//! (`features/mood/index.ts::activate`) that the Rust port dropped, which left the mood panel frozen
//! for the whole chat. The emotion tag opening Mika's reply (`<|LAUGH|>`, `[SMUG]`) also pulls
//! valence/arousal toward that emotion, so the label moves with what she actually says.
//!
//! Proactive turns are skipped: no user message to count, same rule as `brain::post_turn`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};

use super::engine::engine;
use super::math::clamp_unit;
use super::persistence::PersistentAffinity;

const MIN_INTERVAL_PATH: &str = "mood.minAffinityInteractionTime";
const DEFAULT_MIN_INTERVAL_SECS: f64 = 30.0;
/// A leading tag fits well within this; past it the reply simply opened without one.
const TAG_SCAN_LIMIT: usize = 64;
/// Share of the gap to the emotion's target closed per reply: one reply nudges, a streak moves the label.
const BLEND: f64 = 0.35;

#[derive(Debug, PartialEq)]
enum Scan {
    Pending,
    Tag(String),
    NoTag,
}

/// One tag at the start of `text`: its name and the rest, `Err(Pending)` while streaming may still
/// complete it (`<|SM` + `UG|>`), `Err(NoTag)` once it plainly is not one.
fn split_tag(text: &str) -> Result<(String, &str), Scan> {
    let (open, close) = match text.chars().next() {
        None => return Err(Scan::Pending),
        Some('<') => ("<|", "|>"),
        Some('[') => ("[", "]"),
        Some(_) => return Err(Scan::NoTag),
    };
    if !text.starts_with(open) {
        return Err(if text.len() < open.len() { Scan::Pending } else { Scan::NoTag });
    }
    let Some(end) = text[open.len()..].find(close) else {
        return Err(if text.len() > TAG_SCAN_LIMIT { Scan::NoTag } else { Scan::Pending });
    };
    let name = text[open.len()..open.len() + end].split(':').next().unwrap_or_default().to_ascii_uppercase();
    Ok((name, &text[open.len() + end + close.len()..]))
}

/// First mood-bearing tag of the opening run: the model stacks them (`<|SITTING|> <|LAUGH|>`), and a
/// pose in front must not hide the emotion behind it. Tags later in the reply are prose, not mood.
fn scan_leading_tag(head: &str) -> Scan {
    let mut text = head.trim_start();
    loop {
        match split_tag(text) {
            Ok((name, _)) if emotion_target(&name).is_some() => return Scan::Tag(name),
            Ok((_, rest)) => text = rest.trim_start(),
            Err(scan) => return scan,
        }
    }
}

/// (valence, arousal) per tag, covering `VTuberExpression` and the body actions that read as a
/// feeling (`packages/sdk/src/contracts/types/avatar-types.ts`); pure poses carry no mood.
fn emotion_target(tag: &str) -> Option<(f64, f64)> {
    Some(match tag {
        "CELEBRATE" | "CHEER" | "VICTORY" | "CLAP" | "TADA" | "FIST-PUMP" | "BOUNCE" => (0.8, 0.7),
        "LAUGH" => (0.7, 0.6),
        "EXCITED" | "GROOVE" => (0.6, 0.6),
        "LOVE" | "HEART" => (0.7, 0.3),
        "HAPPY" | "JOY" | "THUMBS-UP" => (0.6, 0.2),
        "WINK" | "PEACE" => (0.5, 0.3),
        "SMUG" => (0.4, 0.2),
        "SHY" | "BLUSH" | "EMBARRASSED" => (0.3, 0.2),
        "RELAXED" => (0.4, -0.5),
        "EVIL" => (0.3, 0.5),
        "DETERMINED" | "FOCUS" => (0.3, 0.5),
        "CURIOUS" => (0.2, 0.4),
        "SURPRISED" => (0.1, 0.7),
        "SERIOUS" => (0.0, 0.2),
        "SLEEP" | "DOZE" => (0.0, -0.8),
        "BORED" => (-0.1, -0.5),
        "TIRED" => (-0.1, -0.6),
        "CONFUSED" => (-0.2, 0.2),
        "SIGH" => (-0.2, -0.3),
        "POUT" | "FACEPALM" => (-0.3, 0.1),
        "DISGUSTED" => (-0.5, 0.3),
        "SAD" | "SLUMP" => (-0.6, -0.2),
        "ANGRY" | "RAGE" => (-0.6, 0.7),
        "SCARED" | "GASP" => (-0.6, 0.7),
        "PANIC" => (-0.5, 0.8),
        _ => return None,
    })
}

fn blend_toward(current: (f64, f64), target: (f64, f64)) -> (f64, f64) {
    let step = |from: f64, to: f64| clamp_unit(from + (to - from) * BLEND);
    (step(current.0, target.0), step(current.1, target.1))
}

fn interaction_due(last: Option<Instant>, now: Instant, min_interval: Duration) -> bool {
    last.is_none_or(|at| now.duration_since(at) >= min_interval)
}

#[derive(Debug, PartialEq)]
enum Effect {
    Interaction,
    Emotion((f64, f64)),
}

/// Per-listener turn state; decides effects without touching the engine, so it is testable alone.
#[derive(Default)]
struct TurnMood {
    last_interaction: Option<Instant>,
    head: String,
    scanning: bool,
}

impl TurnMood {
    fn on_event(&mut self, key: EventKey, payload: &Value, now: Instant, min_interval: Duration) -> Option<Effect> {
        if crate::modules::brain::post_turn::is_background_turn(payload) {
            return None;
        }
        match key {
            EventKey::ThinkingStart => self.on_turn_start(now, min_interval),
            EventKey::StreamChunk if self.scanning => self.on_chunk(payload.get("text").and_then(Value::as_str)?),
            _ => None,
        }
    }

    fn on_turn_start(&mut self, now: Instant, min_interval: Duration) -> Option<Effect> {
        self.head.clear();
        self.scanning = true;
        if !interaction_due(self.last_interaction, now, min_interval) {
            return None;
        }
        self.last_interaction = Some(now);
        Some(Effect::Interaction)
    }

    fn on_chunk(&mut self, text: &str) -> Option<Effect> {
        self.head.push_str(text);
        match scan_leading_tag(&self.head) {
            Scan::Pending => None,
            Scan::NoTag => {
                self.scanning = false;
                None
            }
            Scan::Tag(tag) => {
                self.scanning = false;
                emotion_target(&tag).map(Effect::Emotion)
            }
        }
    }
}

fn min_interval(store: &ConfigStore) -> Duration {
    let secs = store.get(MIN_INTERVAL_PATH).and_then(|v| v.as_f64()).unwrap_or(DEFAULT_MIN_INTERVAL_SECS);
    Duration::from_secs_f64(secs.max(0.0))
}

fn apply(effect: Effect, affinity: &PersistentAffinity) {
    match effect {
        Effect::Interaction => match affinity.add_interaction(1.0) {
            Ok((data, true)) => bus().publish(
                EventKey::MoodAffinityLevelUp,
                json!({ "level": data["level"], "label": data["label"], "score": data["score"] }),
            ),
            Ok(_) => {}
            Err(error) => log::warn!("mood: afinidade do turno não gravada: {error}"),
        },
        Effect::Emotion(target) => {
            let snapshot = engine().mood_snapshot();
            let current = (snapshot["valence"].as_f64().unwrap_or(0.0), snapshot["arousal"].as_f64().unwrap_or(0.0));
            let (valence, arousal) = blend_toward(current, target);
            bus().publish(EventKey::MoodChanged, engine().update(valence, arousal)["data"].clone());
        }
    }
}

/// Called once from [`super::persistence::register`]; subscribes BEFORE the spawn (broadcast does not replay).
pub(super) fn spawn_turn_mood_listener(store: Arc<ConfigStore>) {
    let mut sub = bus().subscribe();
    tauri::async_runtime::spawn(async move {
        let affinity = PersistentAffinity::new(Arc::clone(&store));
        let mut turn = TurnMood::default();
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            // Toggle mestre (`mood.enabled`) — lido fresco a cada evento, e realimenta o cache de
            // processo que `super::current_mood_label` consulta (ver o cabeçalho de
            // `super::refresh_master_enabled`; varredura de controles de UI de 2026-09-27:
            // desligar a aba não impedia afinidade/emoção do turno de continuar mudando o motor).
            if !super::refresh_master_enabled(&store) {
                continue;
            }
            if let Some(effect) = turn.on_event(envelope.key, &envelope.payload, Instant::now(), min_interval(&store)) {
                apply(effect, &affinity);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const THROTTLE: Duration = Duration::from_secs(30);

    fn chunk(text: &str) -> Value {
        json!({ "text": text })
    }

    #[test]
    fn a_user_turn_counts_as_an_interaction_once_per_throttle_window() {
        let mut turn = TurnMood::default();
        let t0 = Instant::now();
        assert_eq!(turn.on_event(EventKey::ThinkingStart, &json!({}), t0, THROTTLE), Some(Effect::Interaction));
        assert_eq!(turn.on_event(EventKey::ThinkingStart, &json!({}), t0 + Duration::from_secs(5), THROTTLE), None);
        let later = t0 + Duration::from_secs(31);
        assert_eq!(turn.on_event(EventKey::ThinkingStart, &json!({}), later, THROTTLE), Some(Effect::Interaction));
    }

    #[test]
    fn a_proactive_turn_is_not_a_user_interaction_nor_moves_the_mood() {
        let mut turn = TurnMood::default();
        let background = json!({ "origin": "background", "text": "<|LAUGH|> oi" });
        assert_eq!(turn.on_event(EventKey::ThinkingStart, &background, Instant::now(), THROTTLE), None);
        assert_eq!(turn.on_event(EventKey::StreamChunk, &background, Instant::now(), THROTTLE), None);
    }

    #[test]
    fn the_reply_tag_moves_the_mood_even_when_split_across_chunks() {
        let mut turn = TurnMood::default();
        let now = Instant::now();
        turn.on_event(EventKey::ThinkingStart, &json!({}), now, THROTTLE);
        assert_eq!(turn.on_event(EventKey::StreamChunk, &chunk("<|LA"), now, THROTTLE), None);
        assert_eq!(turn.on_event(EventKey::StreamChunk, &chunk("UGH|> kkk"), now, THROTTLE), Some(Effect::Emotion((0.7, 0.6))));
        // Only the opening tag counts: a later one in the same reply is prose, not her mood.
        assert_eq!(turn.on_event(EventKey::StreamChunk, &chunk(" <|ANGRY|>"), now, THROTTLE), None);
    }

    #[test]
    fn a_pose_in_front_does_not_hide_the_emotion_stacked_behind_it() {
        // Real Gemma openings from the smoke run: only the first counted before this.
        assert_eq!(scan_leading_tag("<|SHY|> <|BLUSH|> Eita..."), Scan::Tag("SHY".into()));
        assert_eq!(scan_leading_tag("<|SITTING|> <|READING|> [LAUGH] oi"), Scan::Tag("LAUGH".into()));
        assert_eq!(scan_leading_tag("<|SITTING|> <|CELEB"), Scan::Pending);
    }

    #[test]
    fn pose_tags_and_untagged_replies_leave_the_mood_alone() {
        assert_eq!(scan_leading_tag("<|SITTING|> ok"), Scan::NoTag);
        assert_eq!(emotion_target("SITTING"), None);
        assert_eq!(scan_leading_tag("Claro, bora!"), Scan::NoTag);
        assert_eq!(scan_leading_tag("[smug] minúsculo"), Scan::Tag("SMUG".into()));
        assert_eq!(scan_leading_tag("<|LAUGH:0.5|>"), Scan::Tag("LAUGH".into()));
        assert_eq!(scan_leading_tag("  "), Scan::Pending);
        assert_eq!(scan_leading_tag("<"), Scan::Pending);
        assert_eq!(scan_leading_tag("<b>html</b>"), Scan::NoTag);
        assert_eq!(scan_leading_tag(&format!("<|{}", "A".repeat(80))), Scan::NoTag);
    }

    #[test]
    fn a_streak_of_happy_replies_changes_the_label_but_one_reply_only_nudges() {
        let once = blend_toward((0.0, 0.0), (0.7, 0.6));
        assert_eq!(super::super::math::label_for(once.0, once.1), "neutral");
        let twice = blend_toward(once, (0.7, 0.6));
        assert_eq!(super::super::math::label_for(twice.0, twice.1), "excited");
        let tense = (0..4).fold((0.0, 0.0), |mood, _| blend_toward(mood, (-0.5, 0.8)));
        assert_eq!(super::super::math::label_for(tense.0, tense.1), "tense");
    }
}
