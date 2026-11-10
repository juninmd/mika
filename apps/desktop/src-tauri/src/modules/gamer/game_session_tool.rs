//! Tool `game_session` — porte de `tools/index.ts::createGameSessionTool` +
//! `handlers/game-session-stats.ts`. `stats`/`playtime`/`streak`/`events` são portadas fielmente
//! sobre [`SessionMemory`], que já existia (aberta em `game_watcher.rs::start`) sem nenhum chamador
//! de produção antes desta tool.
//!
//! **Remedido 2026-09-22c: `describe` também é portado — a mesma premissa refutada.** A rodada
//! anterior marcou `describe` como "sem porte" pela MESMA premissa que `recommend_media_tool.rs` já
//! provou falsa: `turn::run_turn_llm_raw(&ConfigStore, ...)` é `pub`, sem `AppHandle`, e
//! `register_session_tools` já recebe `Arc<ConfigStore>` — o mesmo caminho, só um prompt e um
//! contexto diferentes. Porte de `handlers/commentary.ts::describeSession`: mesmo template
//! (`prompts/commentary.md`, `include_str!`), mesmos 6 placeholders, mesmo `errorCode: 'internal'`
//! no catch de geração.
//!
//! `eventsText` no TS (`handler-commentary.test.ts` confirma) NÃO tem teto — mapeia
//! `session.events` inteiro. Aqui os últimos [`MAX_EVENTS_IN_PROMPT`] eventos são usados, mesmo teto
//! que `session-summary.ts::summarizeSessionWithLLM` já usa (`.slice(-40)`) para o MESMO tipo de
//! contexto (eventos de sessão → prompt de LLM) dentro deste módulo — uma sessão de jogo de horas
//! pode acumular milhares de eventos, e um prompt sem teto é custo e correção, não só um detalhe de
//! porte; documentado no relatório desta rodada como desvio deliberado do TS, não um erro de porte.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::iso_time::{format_iso8601, now_ms};
use super::session_memory::SessionMemory;
use super::session_stats::{day_key, format_duration_ms, playtime_by_game, streak};
use super::session_types::{GameEvent, GameSession};
use crate::config::store::ConfigStore;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortHandle};
use crate::modules::tools::{ToolDefinition, ToolHandler};

/// Cópia literal do `.md` que `loadPrompt('commentary', ...)` carrega no TS.
const TEMPLATE: &str = include_str!("../../../../prompts/gamer-companion/commentary.md");

/// Mesmo teto de `session-summary.ts::summarizeSessionWithLLM` (`.slice(-40)`) — ver o cabeçalho.
const MAX_EVENTS_IN_PROMPT: usize = 40;

fn config_str(store: &ConfigStore, path: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Espelha `getConfig('user.nickname') ?? getConfig('user.name') ?? 'senpai'` (`commentary.ts`).
fn nickname(store: &ConfigStore) -> String {
    let value = config_str(store, "user.nickname");
    if !value.is_empty() {
        return value;
    }
    let name = config_str(store, "user.name");
    if !name.is_empty() {
        return name;
    }
    "senpai".to_string()
}

/// Espelha `session.events.map(e => \`[${e.type}] ${e.description}${e.screenContext ? ...
/// : ''}\`).join('\n')`, com o teto de [`MAX_EVENTS_IN_PROMPT`] documentado no cabeçalho.
fn events_text(events: &[GameEvent]) -> String {
    if events.is_empty() {
        return "No events recorded.".to_string();
    }
    let start = events.len().saturating_sub(MAX_EVENTS_IN_PROMPT);
    events[start..]
        .iter()
        .map(|e| match e.screen_context.as_deref().filter(|s| !s.is_empty()) {
            Some(ctx) => format!("[{}] {} (context: {ctx})", e.kind, e.description),
            None => format!("[{}] {}", e.kind, e.description),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Espelha `session.endTime ? formatDurationMs(...) : 'ongoing'`.
fn duration_text(session: &GameSession) -> String {
    match session.duration_ms() {
        Some(ms) => format_duration_ms(Some(ms)),
        None => "ongoing".to_string(),
    }
}

/// Espelha os 6 placeholders de `commentary.md` (`loadPrompt('commentary', {...})`).
fn render_commentary_prompt(store: &ConfigStore, session: &GameSession, style: &str) -> String {
    TEMPLATE
        .replace("{{nickname}}", &nickname(store))
        .replace("{{style}}", style)
        .replace("{{gameName}}", &session.game_name)
        .replace("{{duration}}", &duration_text(session))
        .replace("{{eventsText}}", &events_text(&session.events))
        .replace("{{language}}", &crate::config::locale::app_language(store))
}

/// Porte de `...session` (spread) + `durationFormatted` só quando aplicável — `endTime`/`durationMs`/
/// `summary` ficam AUSENTES (não `null`) quando o TS os deixaria `undefined`, para casar com o que
/// `JSON.stringify` produziria do lado original. `pub(super)`: também é o que
/// `super::rpc::session-list`/`session-current` usam para o MESMO formato de wire — a UI
/// (`renderer/widgets/game-hud.widget.tsx`) espera `startTime`/`endTime` como string ISO e
/// `durationMs` numérico, não os dois `u64` epoch-ms crus de `GameSession`.
pub(super) fn session_json(session: &GameSession) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("id".into(), json!(session.id));
    obj.insert("gameId".into(), json!(session.game_id));
    obj.insert("gameName".into(), json!(session.game_name));
    obj.insert("startTime".into(), json!(format_iso8601(session.start_time_ms)));
    if let Some(end_ms) = session.end_time_ms {
        obj.insert("endTime".into(), json!(format_iso8601(end_ms)));
    }
    if let Some(duration) = session.duration_ms() {
        obj.insert("durationMs".into(), json!(duration));
    }
    obj.insert("events".into(), json!(session.events));
    if let Some(summary) = &session.summary {
        obj.insert("summary".into(), json!(summary));
    }
    Value::Object(obj)
}

fn stats_response(memory: &SessionMemory) -> Value {
    let Some(session) = memory.current_session() else {
        return json!({ "success": true, "data": { "session": null, "hasActiveSession": false } });
    };
    let mut payload = session_json(&session);
    let duration_ms = session.duration_ms().unwrap_or_else(|| now_ms().saturating_sub(session.start_time_ms));
    payload["durationFormatted"] = json!(format_duration_ms(Some(duration_ms)));
    json!({ "success": true, "data": { "session": payload, "hasActiveSession": true } })
}

fn playtime_response(memory: &SessionMemory) -> Value {
    let history = match memory.history(200) {
        Ok(h) => h,
        Err(error) => return json!({ "success": false, "error": error, "errorCode": "internal" }),
    };
    let today_key = day_key(now_ms(), &chrono::Local);
    let to_breakdown = |totals: &[(String, u64)]| -> Value {
        json!(
            totals
                .iter()
                .map(|(game, ms)| json!({ "game": game, "duration": format_duration_ms(Some(*ms)) }))
                .collect::<Vec<_>>()
        )
    };
    let sum_ms = |totals: &[(String, u64)]| totals.iter().map(|(_, ms)| *ms).sum::<u64>();

    let today_totals = playtime_by_game(&history, today_key, 0, &chrono::Local);
    let week_totals = playtime_by_game(&history, today_key, 6, &chrono::Local);
    json!({
        "success": true,
        "data": {
            "today": { "totalDuration": format_duration_ms(Some(sum_ms(&today_totals))), "byGame": to_breakdown(&today_totals) },
            "last7Days": { "totalDuration": format_duration_ms(Some(sum_ms(&week_totals))), "byGame": to_breakdown(&week_totals) },
        }
    })
}

fn streak_response(memory: &SessionMemory) -> Value {
    let history = match memory.history(500) {
        Ok(h) => h,
        Err(error) => return json!({ "success": false, "error": error, "errorCode": "internal" }),
    };
    let (current, longest) = streak(&history, now_ms(), &chrono::Local);
    json!({ "success": true, "data": { "currentStreakDays": current, "longestStreakDays": longest } })
}

fn events_response(memory: &SessionMemory, session_id: &str) -> Value {
    match memory.session_by_id(session_id) {
        Ok(Some(session)) => {
            json!({ "success": true, "data": { "sessionId": session_id, "gameName": session.game_name, "events": session.events } })
        }
        Ok(None) => json!({ "success": false, "error": format!("Session {session_id} not found."), "errorCode": "not_found" }),
        Err(error) => json!({ "success": false, "error": error, "errorCode": "internal" }),
    }
}

/// Porte de `describeSession`: busca a sessão, monta o prompt e gera o comentário — `not_found`
/// sem tocar o LLM quando a sessão não existe, `internal` nomeado se a geração falhar, nunca um
/// comentário fabricado.
async fn describe_response(store: &ConfigStore, memory: &SessionMemory, session_id: &str, style: &str) -> Value {
    let session = match memory.session_by_id(session_id) {
        Ok(Some(session)) => session,
        Ok(None) => return json!({ "success": false, "error": format!("Session {session_id} not found."), "errorCode": "not_found" }),
        Err(error) => return json!({ "success": false, "error": error, "errorCode": "internal" }),
    };

    let prompt = render_commentary_prompt(store, &session, style);
    let (_handle, signal) = TurnAbortHandle::new();
    match turn::run_turn_llm_raw(store, None, &prompt, signal, &NullObserver).await {
        Ok(result) => json!({
            "success": true,
            "data": { "sessionId": session_id, "gameName": session.game_name, "commentary": result.text.trim() }
        }),
        Err(error) => json!({ "success": false, "error": format!("Failed to generate commentary: {error}"), "errorCode": "internal" }),
    }
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "game_session".into(),
        description: Some(
            "The user's gaming sessions. stats: current session (id, game, duration); playtime: time played; streak: consecutive play days; events / describe: one session's timeline / narrated recap in a `style` (need `sessionId`). Use instead of guessing play numbers."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["stats", "playtime", "streak", "events", "describe"] },
                "sessionId": { "type": "string", "description": "Session id (from action=stats); required for events/describe." },
                "style": { "type": "string", "enum": ["funny", "strategic", "educational", "lore"] },
            },
            "required": ["action"],
        }),
        plugin_id: super::feature::GAMER_COMPANION_PLUGIN_ID.into(),
        requires_window: false,
    }
}

pub struct GameSessionHandler {
    memory: Arc<SessionMemory>,
    store: Arc<ConfigStore>,
}

impl GameSessionHandler {
    pub fn new(memory: Arc<SessionMemory>, store: Arc<ConfigStore>) -> Self {
        Self { memory, store }
    }
}

#[async_trait]
impl ToolHandler for GameSessionHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        // Toggle mestre da feature (`gamer-companion.enabled`) — lido do `ConfigStore` REAL a cada
        // chamada, mesma técnica de `person_catalog::tool::catalog_enabled` (varredura de
        // controles de UI de 2026-09-27).
        if !super::feature::master_enabled(&self.store) {
            return Ok(super::feature::master_disabled_response());
        }
        let action = args.get("action").and_then(Value::as_str).unwrap_or("stats");
        let session_id = args.get("sessionId").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());

        Ok(match action {
            "stats" => stats_response(&self.memory),
            "playtime" => playtime_response(&self.memory),
            "streak" => streak_response(&self.memory),
            "events" => match session_id {
                Some(id) => events_response(&self.memory, id),
                None => json!({ "success": false, "error": "sessionId is required for action=events", "errorCode": "invalid_args" }),
            },
            "describe" => match session_id {
                Some(id) => {
                    let style = args.get("style").and_then(Value::as_str).unwrap_or("funny");
                    describe_response(&self.store, &self.memory, id, style).await
                }
                None => json!({ "success": false, "error": "sessionId is required for action=describe", "errorCode": "invalid_args" }),
            },
            other => json!({ "success": false, "error": format!("unknown action: {other}"), "errorCode": "invalid_args" }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::session_db::SessionDb;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-gamer-game-session-tool-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn handler() -> (GameSessionHandler, Arc<SessionMemory>, TempDir) {
        let memory = Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap()));
        let dir = TempDir::new();
        let store = Arc::new(ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap());
        super::super::llm_dead_port::point_the_llm_at_a_dead_port(&store);
        (GameSessionHandler::new(Arc::clone(&memory), store), memory, dir)
    }

    /// = [`handler`], mas com `gamer-companion.enabled` forçado — só para o teste de toggle mestre
    /// abaixo.
    fn handler_with_master_toggle(enabled: bool) -> (GameSessionHandler, Arc<SessionMemory>, TempDir) {
        let memory = Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap()));
        let dir = TempDir::new();
        let store = Arc::new(ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap());
        super::super::llm_dead_port::point_the_llm_at_a_dead_port(&store);
        store.set("gamer-companion.enabled", json!(enabled)).unwrap();
        (GameSessionHandler::new(Arc::clone(&memory), store), memory, dir)
    }

    /// Toggle mestre da feature (`gamer-companion.enabled=false`) — nenhuma das ações de
    /// `game_session` dispacha (varredura de controles de UI de 2026-09-27).
    #[tokio::test]
    async fn call_is_denied_when_the_master_toggle_is_off() {
        let (handler, _memory, _dir) = handler_with_master_toggle(false);
        let result = handler.call(json!({ "action": "stats" })).await.unwrap();
        assert_eq!(result["success"], false);
        assert_eq!(result["errorCode"], "denied");
    }

    #[tokio::test]
    async fn stats_reports_no_active_session_when_none_is_open() {
        let (handler, _memory, _dir) = handler();
        let result = handler.call(json!({ "action": "stats" })).await.unwrap();
        assert_eq!(result["data"]["hasActiveSession"], false);
        assert_eq!(result["data"]["session"], Value::Null);
    }

    #[tokio::test]
    async fn stats_reports_the_active_session_with_a_formatted_duration() {
        let (handler, memory, _dir) = handler();
        memory.start_session("570", "Dota 2").unwrap();
        let result = handler.call(json!({ "action": "stats" })).await.unwrap();
        assert_eq!(result["data"]["hasActiveSession"], true);
        assert_eq!(result["data"]["session"]["gameName"], "Dota 2");
        assert!(result["data"]["session"]["durationFormatted"].is_string());
        // Sessão em andamento: sem endTime/durationMs no wire, mesma ausência do TS.
        assert!(result["data"]["session"].get("endTime").is_none());
    }

    #[tokio::test]
    async fn events_without_a_session_id_is_a_named_invalid_args_error() {
        let (handler, _memory, _dir) = handler();
        let result = handler.call(json!({ "action": "events" })).await.unwrap();
        assert_eq!(result["errorCode"], "invalid_args");
    }

    #[tokio::test]
    async fn events_reports_not_found_for_an_unknown_session_id() {
        let (handler, _memory, _dir) = handler();
        let result = handler.call(json!({ "action": "events", "sessionId": "nope" })).await.unwrap();
        assert_eq!(result["errorCode"], "not_found");
    }

    #[tokio::test]
    async fn describe_without_a_session_id_is_a_named_invalid_args_error() {
        let (handler, _memory, _dir) = handler();
        let result = handler.call(json!({ "action": "describe" })).await.unwrap();
        assert_eq!(result["errorCode"], "invalid_args");
    }

    #[tokio::test]
    async fn describe_reports_not_found_for_an_unknown_session_id_without_calling_the_llm() {
        let (handler, _memory, _dir) = handler();
        let result = handler.call(json!({ "action": "describe", "sessionId": "nope" })).await.unwrap();
        assert_eq!(result["success"], false);
        assert_eq!(result["errorCode"], "not_found");
    }

    /// Sem servidor Ollama real no ambiente de teste, `describe` degrada honestamente a `internal`
    /// (código do TS de origem) — prova que a sessão foi encontrada e a geração foi alcançada de
    /// verdade, não a resposta antiga "sem porte nesta rodada".
    #[tokio::test]
    async fn describe_with_a_known_session_reaches_generation_and_degrades_internal_without_a_live_provider() {
        let (handler, memory, _dir) = handler();
        let started = memory.start_session("570", "Dota 2").unwrap();
        let result = handler.call(json!({ "action": "describe", "sessionId": started.id })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("internal"));
        let message = result["error"].as_str().unwrap();
        assert!(message.contains("commentary"), "mensagem deveria nomear a falha de geração: {message}");
    }

    #[tokio::test]
    async fn playtime_and_streak_round_trip_through_a_started_and_ended_session() {
        let (handler, memory, _dir) = handler();
        memory.start_session("570", "Dota 2").unwrap();
        memory.end_session().unwrap();

        let playtime = handler.call(json!({ "action": "playtime" })).await.unwrap();
        assert_eq!(playtime["success"], true);
        assert!(playtime["data"]["today"]["byGame"].as_array().unwrap().iter().any(|g| g["game"] == "Dota 2"));

        let streak = handler.call(json!({ "action": "streak" })).await.unwrap();
        assert_eq!(streak["data"]["currentStreakDays"], 1);
    }

    #[test]
    fn events_text_reports_the_fallback_note_when_there_are_no_events() {
        assert_eq!(events_text(&[]), "No events recorded.");
    }

    #[test]
    fn events_text_appends_screen_context_in_parentheses() {
        let events = vec![GameEvent::new(0, "combat", "lutou", Some("arena".to_string()))];
        assert_eq!(events_text(&events), "[combat] lutou (context: arena)");
    }

    #[test]
    fn events_text_keeps_only_the_most_recent_events_past_the_cap() {
        let events: Vec<GameEvent> =
            (0..(MAX_EVENTS_IN_PROMPT + 5)).map(|i| GameEvent::new(0, "combat", &format!("evento {i}"), None)).collect();
        let text = events_text(&events);
        assert_eq!(text.lines().count(), MAX_EVENTS_IN_PROMPT, "deveria manter só o teto de eventos mais recentes");
        assert!(text.contains(&format!("evento {}", MAX_EVENTS_IN_PROMPT + 4)), "o evento mais recente tem de sobreviver ao corte");
        assert!(!text.contains("evento 0"), "o evento mais antigo tem de ser cortado");
    }

    #[test]
    fn render_commentary_prompt_fills_every_placeholder_the_template_declares() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        let session = GameSession {
            id: "s1".into(),
            game_id: "570".into(),
            game_name: "Dota 2".into(),
            start_time_ms: 0,
            end_time_ms: None,
            events: vec![],
            summary: None,
        };
        let rendered = render_commentary_prompt(&store, &session, "funny");
        assert!(!rendered.contains("{{"), "placeholder não preenchido: {rendered}");
        assert!(rendered.contains("Dota 2"));
        assert!(rendered.contains("ongoing"));
        assert!(rendered.contains("funny"));
    }
}
