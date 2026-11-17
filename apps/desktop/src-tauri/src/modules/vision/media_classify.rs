//! What is on screen, from the focused window alone: a video, a livestream or music — or nothing
//! worth a reaction. Pure and OS-agnostic: it reads the same redacted title/owner that
//! `system_signals::active_window::last_known` publishes on Windows, X11 and macOS.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MediaKind {
    Video,
    Live,
    Music,
}

impl MediaKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Live => "live",
            Self::Music => "music",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Media {
    pub kind: MediaKind,
    /// The window title without browser and service names: "Lofi Girl", "Artist - Song".
    pub title: String,
}

const BROWSERS: &[&str] = &[
    "mozilla firefox", "firefox", "google chrome", "chrome", "chromium", "microsoft edge", "edge", "brave", "opera", "vivaldi", "safari", "arc",
];
const LIVE_SITES: &[&str] = &["twitch", "kick"];
const VIDEO_SITES: &[&str] =
    &["youtube", "netflix", "prime video", "disney+", "hbo max", "max", "crunchyroll", "globoplay", "vimeo", "star+", "apple tv+", "paramount+"];
const MUSIC_SITES: &[&str] = &["youtube music", "spotify", "deezer", "soundcloud", "tidal", "apple music", "amazon music"];
const VIDEO_APPS: &[&str] =
    &["vlc", "mpv", "mpc-hc", "mpc-be", "potplayer", "iina", "totem", "celluloid", "kodi", "stremio", "media player", "movies & tv", "quicktime player"];
const MUSIC_APPS: &[&str] =
    &["spotify", "deezer", "tidal", "music", "foobar2000", "rhythmbox", "audacious", "aimp", "winamp", "musicbee", "strawberry", "elisa", "amarok"];
const LIVE_MARKERS: &[&str] = &["🔴", "ao vivo", "(live)", "[live]", "live stream", "livestream"];

/// "(3) Lofi Girl - YouTube — Mozilla Firefox" → (["lofi girl"], ["youtube", "mozilla firefox"]):
/// what the window is about, and the trailing app/site names it carries.
fn split_title(title: &str) -> (Vec<String>, Vec<String>) {
    let title = strip_notification_count(title.trim());
    let normalized = title.replace('\u{a0}', " ").replace(" — ", " - ").replace(" | ", " - ");
    let mut parts: Vec<String> = normalized.split(" - ").map(str::trim).filter(|p| !p.is_empty()).map(String::from).collect();
    let mut sites = Vec::new();
    while let Some(last) = parts.last() {
        let lower = last.to_lowercase();
        let known = BROWSERS.contains(&lower.as_str()) || LIVE_SITES.contains(&lower.as_str()) || VIDEO_SITES.contains(&lower.as_str()) || MUSIC_SITES.contains(&lower.as_str());
        if !known {
            break;
        }
        sites.push(lower);
        parts.pop();
    }
    (parts, sites)
}

/// YouTube prefixes unread notifications: "(3) Title".
fn strip_notification_count(title: &str) -> &str {
    let Some(rest) = title.strip_prefix('(') else { return title };
    match rest.split_once(") ") {
        Some((digits, tail)) if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) => tail,
        _ => title,
    }
}

fn kind_from_sites(sites: &[String], subject: &str) -> Option<MediaKind> {
    let has = |list: &[&str]| sites.iter().any(|s| list.contains(&s.as_str()));
    if has(MUSIC_SITES) {
        return Some(MediaKind::Music);
    }
    if has(LIVE_SITES) {
        return Some(MediaKind::Live);
    }
    if has(VIDEO_SITES) {
        let lower = subject.to_lowercase();
        return Some(if LIVE_MARKERS.iter().any(|m| lower.contains(m)) { MediaKind::Live } else { MediaKind::Video });
    }
    None
}

fn owner_name(owner: &str) -> String {
    let owner = owner.trim().to_lowercase();
    owner.strip_suffix(".app").unwrap_or(&owner).to_string()
}

fn kind_from_owner(owner: &str) -> Option<MediaKind> {
    let owner = owner_name(owner);
    let owner = owner.as_str();
    if MUSIC_APPS.contains(&owner) {
        return Some(MediaKind::Music);
    }
    VIDEO_APPS.contains(&owner).then_some(MediaKind::Video)
}

/// The media in the focused window, or `None`. A service's home page ("YouTube", "Spotify
/// Premium") carries no subject, so it is browsing, not watching or listening.
pub(super) fn classify(title: &str, owner: &str) -> Option<Media> {
    let (parts, sites) = split_title(title);
    let subject = parts.join(" - ");
    let from_site = kind_from_sites(&sites, &subject);
    let kind = from_site.or_else(|| kind_from_owner(owner))?;
    // An idle player titles its window after itself: "Spotify Premium", "VLC media player".
    let idle_player = from_site.is_none() && subject.to_lowercase().starts_with(&owner_name(owner));
    if subject.is_empty() || idle_player {
        return None;
    }
    Some(Media { kind, title: subject })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(title: &str, owner: &str) -> Option<MediaKind> {
        classify(title, owner).map(|m| m.kind)
    }

    #[test]
    fn a_youtube_video_in_any_browser_is_a_video_named_by_its_title() {
        let media = classify("(3) Lofi Girl - beats to relax - YouTube — Mozilla Firefox", "firefox").unwrap();
        assert_eq!(media, Media { kind: MediaKind::Video, title: "Lofi Girl - beats to relax".into() });
        assert_eq!(kind("Trailer GTA VI - YouTube - Google Chrome", "chrome"), Some(MediaKind::Video));
    }

    #[test]
    fn twitch_and_youtube_live_markers_are_a_livestream() {
        assert_eq!(kind("gaules - Twitch - Google Chrome", "chrome"), Some(MediaKind::Live));
        assert_eq!(kind("🔴 Copa ao vivo - YouTube - Microsoft Edge", "msedge"), Some(MediaKind::Live));
    }

    #[test]
    fn music_players_and_music_sites_are_music() {
        assert_eq!(classify("Daft Punk - One More Time", "Spotify").unwrap().title, "Daft Punk - One More Time");
        assert_eq!(kind("Numb - YouTube Music - Brave", "brave"), Some(MediaKind::Music));
        assert_eq!(kind("Music for coding - YouTube - Brave", "brave"), Some(MediaKind::Video));
    }

    #[test]
    fn desktop_video_players_are_video() {
        assert_eq!(kind("episodio-01.mkv", "vlc"), Some(MediaKind::Video));
    }

    /// A false positive is the invasive case: she would comment on a page nobody is watching.
    #[test]
    fn home_pages_idle_players_and_unrelated_windows_are_not_media() {
        assert_eq!(kind("YouTube - Google Chrome", "chrome"), None);
        assert_eq!(kind("Spotify Premium", "Spotify"), None);
        assert_eq!(kind("VLC media player", "vlc"), None);
        assert_eq!(kind("main.rs - mika - Visual Studio Code", "code"), None);
        assert_eq!(kind("Ableton Live 12 Suite", "Ableton Live 12 Suite"), None);
        assert_eq!(kind("", ""), None);
    }
}
