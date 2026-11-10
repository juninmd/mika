//! Tipos de sessão de jogo — porte de `services/game-session-types.ts`.
//!
//! `GameEvent::timestamp` fica em ISO 8601 (via [`super::iso_time`]), não epoch-ms: é o formato que
//! `JSON.stringify` já produz para um `Date` embutido no TS (serialização `toJSON()` implícita), e a
//! coluna `events` (SQLite) já guarda esse JSON no disco do usuário — trocar o formato quebraria a
//! leitura das linhas antigas.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameEvent {
    pub timestamp: String,
    /// Um dos 7 tipos de `GameEvent['type']` no TS (`cutscene`, `combat`, `exploration`,
    /// `dialogue`, `death`, `achievement`, `screenshot`). Não é um enum Rust fechado: a coluna já
    /// tem linhas gravadas por versões futuras/alternativas do produto, e um `serde(other)` perderia
    /// o valor original ao regravar — `String` preserva byte a byte o que já está no disco.
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    #[serde(rename = "screenContext", default, skip_serializing_if = "Option::is_none")]
    pub screen_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GameSession {
    pub id: String,
    #[serde(rename = "gameId")]
    pub game_id: String,
    #[serde(rename = "gameName")]
    pub game_name: String,
    #[serde(rename = "startTime")]
    pub start_time_ms: u64,
    #[serde(rename = "endTime", skip_serializing_if = "Option::is_none")]
    pub end_time_ms: Option<u64>,
    pub events: Vec<GameEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl GameSession {
    pub fn duration_ms(&self) -> Option<u64> {
        self.end_time_ms.map(|end| end.saturating_sub(self.start_time_ms))
    }
}

/// Porte de `session-memory-rows.ts::fallbackSummary`. Único caminho de resumo disponível em
/// Rust hoje — ver o cabeçalho de `session_memory.rs` para por que o resumo por LLM
/// (`summarizeSessionWithLLM`) não é chamável a partir daqui.
pub fn fallback_summary(session: &GameSession) -> String {
    let Some(duration_ms) = session.duration_ms() else { return String::new() };
    let duration_min = (duration_ms as f64 / 60_000.0).round() as i64;
    let mut types: Vec<&str> = Vec::new();
    for event in &session.events {
        if !types.contains(&event.kind.as_str()) {
            types.push(&event.kind);
        }
    }
    format!("Session: {duration_min}min, {} events ({}).", session.events.len(), types.join(", "))
}

#[cfg(test)]
impl GameEvent {
    pub fn new(now_ms: u64, kind: &str, description: &str, screen_context: Option<String>) -> Self {
        Self { timestamp: super::iso_time::format_iso8601(now_ms), kind: kind.to_string(), description: description.to_string(), screen_context }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(start_ms: u64, end_ms: Option<u64>, events: Vec<GameEvent>) -> GameSession {
        GameSession {
            id: "s1".into(),
            game_id: "g1".into(),
            game_name: "Hollow Knight".into(),
            start_time_ms: start_ms,
            end_time_ms: end_ms,
            events,
            summary: None,
        }
    }

    #[test]
    fn duration_is_none_while_the_session_is_ongoing() {
        assert_eq!(session(0, None, vec![]).duration_ms(), None);
    }

    #[test]
    fn duration_is_the_gap_between_start_and_end() {
        assert_eq!(session(1_000, Some(61_000), vec![]).duration_ms(), Some(60_000));
    }

    #[test]
    fn fallback_summary_is_empty_for_an_ongoing_session() {
        assert_eq!(fallback_summary(&session(0, None, vec![])), "");
    }

    #[test]
    fn fallback_summary_lists_duration_count_and_distinct_types_in_first_seen_order() {
        let events = vec![
            GameEvent::new(0, "death", "morreu pro chefe", None),
            GameEvent::new(0, "achievement", "desbloqueou algo", None),
            GameEvent::new(0, "death", "morreu de novo", None),
        ];
        let s = session(0, Some(120_000), events);
        assert_eq!(fallback_summary(&s), "Session: 2min, 3 events (death, achievement).");
    }
}
