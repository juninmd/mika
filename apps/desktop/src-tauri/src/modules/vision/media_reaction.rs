//! Real-time reactions: while the owner watches a video or a livestream, or listens to music, Mika
//! drops a short comment now and then — like a friend on the couch, not a narrator.
//!
//! Opt-in (`vision.mediaReactions`, off by default) and bounded three times: the media must stay in
//! focus for two polls (an alt-tab does not count), one reaction per `vision.mediaReactionCooldownSec`,
//! and the same video or song at most once per three cooldowns. The line itself goes through
//! `EventKey::BrainProactiveThought`, so the `ProactiveEngine` cadence, hourly cap and quiet hours
//! still apply on top. Video and live frames go through the same privacy chokepoint as every
//! capture (`capture_screen_frames`: consent, paused senses, blocklist, offline mode); music uses
//! the window title only, no capture. System audio is not recorded: the title already names the
//! song, and recording what plays would hear calls and voices too.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::media_classify::{classify, Media, MediaKind};
use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};
use crate::modules::system_signals::active_window;

const POLL: Duration = Duration::from_secs(20);
const DEFAULT_COOLDOWN_SEC: u64 = 300;
/// The same video or song comes back to a reaction only after this many cooldowns.
const SAME_TITLE_COOLDOWNS: u64 = 3;
const FRAME_PROMPT: &str = "This is a frame of a video the user is watching. In up to two short sentences, say what is happening in it (scene, people, on-screen text). Ignore browser and player UI.";

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// What the gate remembers between polls.
#[derive(Default, Debug)]
pub(super) struct Gate {
    seen: Option<String>,
    last_at: Option<u64>,
    last_title: Option<String>,
}

impl Gate {
    /// `true` when this poll should react to `media`; records the reaction when it does.
    pub(super) fn admit(&mut self, media: Option<&Media>, now: u64, cooldown_ms: u64) -> bool {
        let Some(media) = media else {
            self.seen = None;
            return false;
        };
        let settled = self.seen.as_deref() == Some(media.title.as_str());
        self.seen = Some(media.title.clone());
        let since = self.last_at.map_or(u64::MAX, |at| now.saturating_sub(at));
        let repeat = self.last_title.as_deref() == Some(media.title.as_str());
        let wait = if repeat { cooldown_ms.saturating_mul(SAME_TITLE_COOLDOWNS) } else { cooldown_ms };
        if !settled || since < wait {
            return false;
        }
        self.last_at = Some(now);
        self.last_title = Some(media.title.clone());
        true
    }
}

fn enabled(store: &ConfigStore) -> bool {
    let flag = |path: &str| store.get(path).and_then(|v| v.as_bool()).unwrap_or(false);
    flag("vision.enabled") && flag("vision.mediaReactions") && crate::modules::privacy::is_screen_capture_allowed(store)
}

fn cooldown_ms(store: &ConfigStore) -> u64 {
    store.get("vision.mediaReactionCooldownSec").and_then(|v| v.as_u64()).unwrap_or(DEFAULT_COOLDOWN_SEC).clamp(60, 1800) * 1000
}

/// The event `ThoughtService` turns into her line, and the dock turns into a "watching" card.
pub(super) fn thought_payload(media: &Media, scene: Option<&str>) -> Value {
    let what = match media.kind {
        MediaKind::Video => format!("The user is watching a video: \"{}\".", media.title),
        MediaKind::Live => format!("The user is watching a livestream: \"{}\".", media.title),
        MediaKind::Music => format!("The user is listening to music: \"{}\".", media.title),
    };
    let scene = scene.map(|s| format!(" On screen right now: {s}")).unwrap_or_default();
    json!({
        "source": "media-reaction",
        "context": format!("{what}{scene} React in one short line, like a friend enjoying it together: a light comment, a joke or a fun fact. No spoilers, don't narrate the screen, don't ask them to stop."),
        "priority": 4,
        "media": { "kind": media.kind.as_str(), "title": media.title },
    })
}

/// One frame of the video, described by the vision model; `None` when capture or analysis is not
/// available (the reaction then rides on the title alone).
async fn describe_frame(store: &Arc<ConfigStore>) -> Option<String> {
    let capture_store = Arc::clone(store);
    let cfg = super::compression::read_compression_config(store);
    let quality = store.get("vision.captureQuality").and_then(|v| v.as_u64()).unwrap_or(80).clamp(1, 100) as u8;
    let frames = tokio::task::spawn_blocking(move || {
        super::screen_capture::capture_screen_frames(&capture_store).map(|f| super::compression::encode_frames_for_llm(f, &cfg, quality))
    })
    .await
    .ok()?
    .ok()?;
    let frame = frames.into_iter().take(1).collect::<Vec<_>>();
    if frame.is_empty() {
        return None;
    }
    match super::llm_client::analyze_with_usage(store, "vision:media-reaction", FRAME_PROMPT, &frame).await {
        Ok(outcome) => Some(outcome.text.trim().to_string()).filter(|t| !t.is_empty()),
        Err(error) => {
            log::warn!("reação a mídia sem descrição da cena: {error}");
            None
        }
    }
}

async fn poll(store: &Arc<ConfigStore>, gate: &mut Gate) {
    if !enabled(store) {
        gate.seen = None;
        return;
    }
    let media = active_window::last_known().and_then(|w| classify(w.title.as_deref().unwrap_or(""), w.owner.as_deref().unwrap_or("")));
    if !gate.admit(media.as_ref(), now_ms(), cooldown_ms(store)) {
        return;
    }
    let Some(media) = media else { return };
    let scene = if media.kind == MediaKind::Music { None } else { describe_frame(store).await };
    log::info!("Mika vai reagir ao que você assiste/ouve ({})", media.kind.as_str());
    bus().publish(EventKey::BrainProactiveThought, thought_payload(&media, scene.as_deref()));
}

/// Started once at boot, next to the screen watcher; idles while the toggle is off.
pub fn spawn_media_reactions(store: Arc<ConfigStore>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut gate = Gate::default();
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = tokio::time::sleep(POLL) => {}
            }
            poll(&store, &mut gate).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    fn video(title: &str) -> Media {
        Media { kind: MediaKind::Video, title: title.into() }
    }

    #[test]
    fn a_video_must_stay_in_focus_for_two_polls_before_she_reacts() {
        let mut gate = Gate::default();
        assert!(!gate.admit(Some(&video("a")), 0, 5 * MIN), "an alt-tab past a video is not watching it");
        assert!(gate.admit(Some(&video("a")), 20_000, 5 * MIN));
    }

    #[test]
    fn leaving_the_video_resets_the_settle_check() {
        let mut gate = Gate::default();
        gate.admit(Some(&video("a")), 0, 5 * MIN);
        gate.admit(None, 20_000, 5 * MIN);
        assert!(!gate.admit(Some(&video("a")), 40_000, 5 * MIN));
    }

    #[test]
    fn a_new_video_waits_for_the_cooldown_after_the_last_reaction() {
        let mut gate = Gate::default();
        gate.admit(Some(&video("a")), 0, 5 * MIN);
        assert!(gate.admit(Some(&video("a")), 1, 5 * MIN));
        gate.admit(Some(&video("b")), 2 * MIN, 5 * MIN);
        assert!(!gate.admit(Some(&video("b")), 3 * MIN, 5 * MIN), "inside the cooldown");
        assert!(gate.admit(Some(&video("b")), 5 * MIN + 1, 5 * MIN));
    }

    #[test]
    fn the_same_video_comes_back_only_after_three_cooldowns() {
        let mut gate = Gate::default();
        gate.admit(Some(&video("a")), 0, 5 * MIN);
        assert!(gate.admit(Some(&video("a")), 1, 5 * MIN));
        assert!(!gate.admit(Some(&video("a")), 10 * MIN, 5 * MIN));
        assert!(gate.admit(Some(&video("a")), 15 * MIN + 1, 5 * MIN));
    }

    #[test]
    fn the_payload_feeds_the_thought_service_and_names_the_media_for_the_dock() {
        let payload = thought_payload(&video("Trailer GTA VI"), Some("a car chase at night"));
        assert_eq!(payload["source"], "media-reaction");
        assert_eq!(payload["media"], json!({ "kind": "video", "title": "Trailer GTA VI" }));
        let context = payload["context"].as_str().unwrap();
        assert!(context.contains("Trailer GTA VI") && context.contains("a car chase at night"));
    }
}
