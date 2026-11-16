//! `game_session` action `today` — "o que fizemos hoje": tempo jogado, sessões, conquistas, mortes e
//! streak do dia civil local, mais os resumos narrados de cada sessão. Modo passageiro
//! (`privacy::passenger`) devolve só as estatísticas do jogo: os resumos são memória e ficam de fora.
//!
//! Os momentos (`EventKey::GameMoment`, da visão) entram na sessão aberta por [`spawn_moment_recorder`]:
//! antes dele nenhum caminho de produção gravava evento de sessão, e conquistas/mortes eram sempre zero.

use chrono::{DateTime, TimeZone};
use serde_json::{json, Value};

use super::session_memory::SessionMemory;
use super::session_stats::{day_key, format_duration_ms, playtime_by_game, streak};
use super::session_types::{fallback_summary, GameEvent, GameSession};
use crate::config::store::ConfigStore;
use crate::events::EventKey;

/// Teto da lista de conquistas no payload: um dia de farm não vira um prompt de mil linhas.
const MAX_ACHIEVEMENTS: usize = 10;
/// Teto de eventos por sessão: uma maratona de horas não cresce a linha do SQLite sem limite.
const MAX_SESSION_EVENTS: usize = 500;
const ACHIEVEMENT_KINDS: &[&str] = &["achievement", "victory"];

/// Momento de visão como evento de sessão. O mini-game é a Mika contra o usuário, não o jogo aberto.
fn moment_event(payload: &Value, at_ms: u64) -> Option<GameEvent> {
    let text = |key: &str| payload.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    if text("source") == Some("minigame") {
        return None;
    }
    let at = payload.get("at").and_then(Value::as_u64).unwrap_or(at_ms);
    Some(GameEvent {
        timestamp: super::iso_time::format_iso8601(at),
        kind: text("type")?.to_string(),
        description: text("description").unwrap_or_default().to_string(),
        screen_context: None,
    })
}

/// Passageiro não grava: o que a visão descreveu da tela também é lembrança.
pub(super) fn record_moment(memory: &SessionMemory, store: &ConfigStore, payload: &Value, at_ms: u64) -> bool {
    if crate::modules::privacy::passenger::is_passenger_mode(store) {
        return false;
    }
    moment_event(payload, at_ms).is_some_and(|event| memory.push_event(event, MAX_SESSION_EVENTS))
}

pub fn spawn_moment_recorder(memory: std::sync::Arc<SessionMemory>, store: std::sync::Arc<ConfigStore>) {
    let mut sub = crate::events::bus().subscribe();
    tauri::async_runtime::spawn(async move {
        while let Some(envelope) = sub.recv().await {
            if envelope.key == EventKey::GameMoment {
                record_moment(&memory, &store, &envelope.payload, envelope.at_ms);
            }
        }
    });
}

/// Histórico do banco com a sessão em andamento (eventos ainda só na memória) fechada em `now_ms`.
fn with_current(history: &[GameSession], current: Option<&GameSession>, now_ms: u64) -> Vec<GameSession> {
    let mut sessions: Vec<GameSession> = history.iter().filter(|s| current.is_none_or(|c| c.id != s.id)).cloned().collect();
    if let Some(live) = current {
        let mut live = live.clone();
        live.end_time_ms.get_or_insert(now_ms);
        sessions.push(live);
    }
    sessions
}

fn achievements(today: &[&GameSession]) -> Vec<Value> {
    let events = today.iter().flat_map(|s| s.events.iter().map(move |e| (s, e)));
    events
        .filter(|(_, e)| ACHIEVEMENT_KINDS.contains(&e.kind.as_str()))
        .take(MAX_ACHIEVEMENTS)
        .map(|(s, e)| json!({ "game": s.game_name, "description": e.description }))
        .collect()
}

/// Resumos que o LLM escreveu; o `fallback_summary` mecânico não conta como lembrança.
fn highlights(today: &[&GameSession]) -> Vec<Value> {
    today
        .iter()
        .filter_map(|s| s.summary.as_deref().filter(|t| !t.trim().is_empty() && *t != fallback_summary(s)).map(|t| (s, t)))
        .map(|(s, t)| json!({ "game": s.game_name, "summary": t.trim() }))
        .collect()
}

pub fn today_summary<Tz: TimeZone>(
    history: &[GameSession],
    current: Option<&GameSession>,
    now_ms: u64,
    tz: &Tz,
    passenger: bool,
) -> Value {
    let sessions = with_current(history, current, now_ms);
    let today_key = day_key(now_ms, tz);
    let today: Vec<&GameSession> = sessions.iter().filter(|s| day_key(s.start_time_ms, tz) == today_key).collect();
    let by_game = playtime_by_game(&sessions, today_key, 0, tz);
    let total_ms: u64 = by_game.iter().map(|(_, ms)| ms).sum();
    let deaths = today.iter().flat_map(|s| &s.events).filter(|e| e.kind == "death").count();
    let day = DateTime::from_timestamp_millis(now_ms as i64).map(|d| d.with_timezone(tz).date_naive().to_string());
    let mut data = json!({
        "day": day,
        "totalMs": total_ms,
        "totalDuration": format_duration_ms(Some(total_ms)),
        "sessionCount": today.len(),
        "byGame": by_game.iter().map(|(game, ms)| json!({ "game": game, "duration": format_duration_ms(Some(*ms)) })).collect::<Vec<_>>(),
        "achievements": achievements(&today),
        "deaths": deaths,
        "currentStreakDays": streak(&sessions, now_ms, tz).0,
        "passengerMode": passenger,
    });
    if !passenger {
        data["highlights"] = json!(highlights(&today));
    }
    json!({ "success": true, "data": data })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::session_types::GameEvent;
    use chrono::FixedOffset;

    const HOUR: u64 = 3_600_000;
    const DAY: u64 = 24 * HOUR;

    fn brt() -> FixedOffset {
        FixedOffset::west_opt(3 * 3600).unwrap()
    }

    fn session(id: &str, game: &str, start: u64, end: Option<u64>, events: Vec<GameEvent>, summary: Option<&str>) -> GameSession {
        GameSession {
            id: id.into(),
            game_id: game.into(),
            game_name: game.into(),
            start_time_ms: start,
            end_time_ms: end,
            events,
            summary: summary.map(str::to_string),
        }
    }

    /// Dia 10, 20h BRT. Ontem 23h BRT (02h UTC de hoje) é ontem; a sessão viva ainda não tem fim no banco.
    fn day_fixture() -> (Vec<GameSession>, GameSession, u64) {
        let now = 10 * DAY + 23 * HOUR;
        let yesterday_late = 10 * DAY + 2 * HOUR;
        let history = vec![
            session("y", "Hades", yesterday_late, Some(yesterday_late + HOUR), vec![], Some("ontem ela zerou o Hades")),
            session(
                "a",
                "Hades",
                10 * DAY + 15 * HOUR,
                Some(10 * DAY + 16 * HOUR),
                vec![GameEvent::new(0, "death", "Megera", None), GameEvent::new(0, "achievement", "Fugiu do Tártaro", None)],
                Some("Foi uma run tensa contra a Megera."),
            ),
            session("live", "Celeste", 10 * DAY + 22 * HOUR, None, vec![], None),
        ];
        let live = session("live", "Celeste", 10 * DAY + 22 * HOUR, None, vec![GameEvent::new(0, "victory", "Capítulo 3", None)], None);
        (history, live, now)
    }

    #[test]
    fn today_counts_only_the_local_day_and_closes_the_live_session_at_now() {
        let (history, live, now) = day_fixture();
        let data = &today_summary(&history, Some(&live), now, &brt(), false)["data"];
        assert_eq!(data["day"], "1970-01-11", "dia civil em BRT, não em UTC");
        assert_eq!(data["sessionCount"], 2, "a sessão das 23h de ontem não é de hoje");
        assert_eq!(data["totalMs"], 2 * HOUR, "1h de Hades + 1h da sessão viva até agora");
        assert_eq!(data["deaths"], 1);
        let achievements: Vec<&str> = data["achievements"].as_array().unwrap().iter().map(|a| a["description"].as_str().unwrap()).collect();
        assert_eq!(achievements, ["Fugiu do Tártaro", "Capítulo 3"], "conquista da sessão viva vem da memória, não do banco");
        assert_eq!(data["currentStreakDays"], 2);
    }

    #[test]
    fn highlights_are_memories_and_passenger_mode_leaves_them_out() {
        let (history, live, now) = day_fixture();
        let normal = &today_summary(&history, Some(&live), now, &brt(), false)["data"];
        assert_eq!(normal["highlights"], json!([{ "game": "Hades", "summary": "Foi uma run tensa contra a Megera." }]));

        let passenger = &today_summary(&history, Some(&live), now, &brt(), true)["data"];
        assert_eq!(passenger["passengerMode"], true);
        assert!(passenger.get("highlights").is_none(), "passageiro: só estatística, sem lembrança");
        assert_eq!(passenger["totalMs"], normal["totalMs"], "as estatísticas do jogo continuam");
        assert_eq!(passenger["achievements"], normal["achievements"]);
    }

    #[test]
    fn the_mechanical_fallback_summary_is_not_a_highlight() {
        let mut s = session("a", "Hades", 10 * DAY + 15 * HOUR, Some(10 * DAY + 16 * HOUR), vec![], None);
        s.summary = Some(fallback_summary(&s));
        let data = &today_summary(&[s], None, 10 * DAY + 20 * HOUR, &brt(), false)["data"];
        assert_eq!(data["highlights"], json!([]));
    }

    #[test]
    fn a_vision_moment_lands_in_the_open_session_but_not_in_passenger_mode_or_from_a_minigame() {
        let dir = std::env::temp_dir().join(format!("mika-daily-summary-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        let memory = SessionMemory::new(crate::modules::gamer::session_db::SessionDb::open_in_memory().unwrap());
        let death = json!({ "game": "Hades", "type": "death", "description": "Megera", "source": "vision" });
        assert!(!record_moment(&memory, &store, &death, 1), "sem sessão aberta não há onde anotar");

        memory.start_session("1145360", "Hades").unwrap();
        assert!(record_moment(&memory, &store, &death, 1));
        let minigame = json!({ "game": "Jogo da Velha", "type": "victory", "description": "x", "source": "minigame" });
        assert!(!record_moment(&memory, &store, &minigame, 2));
        store.set("privacy.passengerMode", json!(true)).unwrap();
        assert!(!record_moment(&memory, &store, &death, 3));

        let ended = memory.end_session().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(ended.events.iter().map(|e| e.description.as_str()).collect::<Vec<_>>(), ["Megera"]);
    }

    #[test]
    fn an_empty_day_is_zero_not_an_error() {
        let data = &today_summary(&[], None, 10 * DAY, &brt(), false)["data"];
        assert_eq!(data["sessionCount"], 0);
        assert_eq!(data["totalDuration"], "0min");
        assert_eq!(data["currentStreakDays"], 0);
    }
}
