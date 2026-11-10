//! `SessionMemory` — porte de `services/session-memory.ts::GameSessionMemory`, sessão "atual" +
//! histórico via [`super::session_db::SessionDb`].
//!
//! **Sem `CircuitBreaker`.** O original envolve toda chamada de banco num `CircuitBreaker` (abre
//! depois de 3 falhas, 10s de cooldown) porque `@libsql/client` pode ficar preso num I/O lento sem
//! timeout embutido. `rusqlite` é síncrono sobre um arquivo local com `busy_timeout` (5s, ver
//! `session_db.rs`) — o timeout já existe um nível abaixo, e todo erro já sobe como `Result`. Uma
//! segunda camada de disjuntor aqui seria estado extra sem falha nova que ele cubra.
//!
//! **Resumo: [`fallback_summary`] na hora, LLM depois — portado em 2026-09-24.** `end_session`
//! continua síncrono e nunca bloqueia esperando o LLM (mesma razão de sempre: `apply()`, quem chama
//! `end_session`, roda dentro do tick do watcher). O que muda: [`super::game_watcher::GameWatcher`]
//! agora captura a sessão encerrada e dispara [`summarize_with_llm`] em segundo plano
//! (`tauri::async_runtime::spawn`, nunca dentro do `apply()` testado sem runtime); se o LLM responder
//! com texto não-vazio, [`SessionMemory::set_summary`] substitui o `fallback_summary` no SQLite. Uma
//! falha do LLM (offline, provedor fora) deixa o fallback determinístico valendo — nunca um resumo
//! fabricado no lugar dele. Mesmo caminho de `turn::run_turn_llm_raw` que
//! [`super::game_session_tool`] (`describe`) e [`super::game_knowledge_tool`] (`ask`) já usam.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use super::session_db::SessionDb;
use super::session_types::{fallback_summary, GameSession};
use crate::config::store::ConfigStore;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortSignal};

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub struct SessionMemory {
    db: SessionDb,
    current: Mutex<Option<GameSession>>,
}

impl SessionMemory {
    pub fn new(db: SessionDb) -> Self {
        Self { db, current: Mutex::new(None) }
    }

    /// Se já havia uma sessão em andamento, ela é encerrada primeiro — mesma regra de
    /// `startSession` no TS (nunca duas sessões concorrentes).
    pub fn start_session(&self, game_id: &str, game_name: &str) -> Result<GameSession, String> {
        self.end_session()?;

        let session = GameSession {
            id: Uuid::new_v4().to_string(),
            game_id: game_id.to_string(),
            game_name: game_name.to_string(),
            start_time_ms: now_ms(),
            end_time_ms: None,
            events: Vec::new(),
            summary: None,
        };
        self.db.insert(&session)?;

        let mut guard = self.current.lock().map_err(|_| "sessão de jogo envenenada".to_string())?;
        *guard = Some(session.clone());
        Ok(session)
    }

    pub fn end_session(&self) -> Result<Option<GameSession>, String> {
        let mut guard = self.current.lock().map_err(|_| "sessão de jogo envenenada".to_string())?;
        let Some(mut session) = guard.take() else { return Ok(None) };

        session.end_time_ms = Some(now_ms());
        session.summary = Some(fallback_summary(&session));
        self.db.finish(&session.id, session.end_time_ms.unwrap(), session.summary.as_deref(), &session.events)?;
        Ok(Some(session))
    }

    pub fn current_session(&self) -> Option<GameSession> {
        self.current.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn history(&self, limit: i64) -> Result<Vec<GameSession>, String> {
        self.db.history(limit)
    }

    pub fn session_for_game(&self, game_id: &str) -> Result<Option<GameSession>, String> {
        self.db.by_game_id(game_id)
    }

    /// Sobrescreve o resumo de uma sessão já persistida — usado quando [`summarize_with_llm`] chega
    /// depois do `fallback_summary` que `end_session` já salvou.
    pub fn set_summary(&self, id: &str, summary: &str) -> Result<(), String> {
        self.db.update_summary(id, summary)
    }

    pub fn session_by_id(&self, id: &str) -> Result<Option<GameSession>, String> {
        self.db.by_id(id)
    }

    /// Repassa para [`super::game_locations`] sobre a MESMA `SessionDb` — tool `game_knowledge`
    /// (`memorize_location`/`get_location_guide`) fecha sobre `SessionMemory`, não sobre o CRUD de
    /// localização direto, mesma simetria de todo o resto desta struct.
    pub fn save_location(
        &self,
        game_name: &str,
        location_name: &str,
        description: &str,
        connections: &[String],
        important_items: &[String],
        notes: Option<&str>,
    ) -> Result<super::game_locations::GameLocation, String> {
        super::game_locations::save_location(&self.db, game_name, location_name, description, connections, important_items, notes)
    }

    pub fn find_locations(&self, game_name: &str, query: Option<&str>) -> Result<Vec<super::game_locations::GameLocation>, String> {
        super::game_locations::find_locations(&self.db, game_name, query)
    }
}

/// Prompt mínimo do resumo por LLM — não é o template de comentário narrado (`commentary.md`, usado
/// pela tool `game_session`/`describe`, tom de personagem para o usuário ler); este é um resumo
/// factual curto para GRAVAR, mesmo teto de eventos (`.slice(-40)`) que `describe_response` já usa
/// para o mesmo tipo de contexto.
fn summary_prompt(session: &GameSession) -> String {
    const MAX_EVENTS: usize = 40;
    let events_text = if session.events.is_empty() {
        "No events recorded.".to_string()
    } else {
        let start = session.events.len().saturating_sub(MAX_EVENTS);
        session.events[start..].iter().map(|e| format!("[{}] {}", e.kind, e.description)).collect::<Vec<_>>().join("\n")
    };
    let duration_min = session.duration_ms().map(|ms| (ms as f64 / 60_000.0).round() as i64).unwrap_or(0);
    format!(
        "Summarize this gaming session of \"{}\" in one short sentence (max ~200 characters), same language as the events below. \
         Duration: {duration_min}min.\nEvents:\n{events_text}",
        session.game_name
    )
}

/// Resumo por LLM de uma sessão já encerrada — a PORTA que faltava (ver o cabeçalho do arquivo):
/// `end_session` continua síncrono e persiste [`fallback_summary`] na hora, NUNCA bloqueado
/// esperando o LLM; isto é chamado depois, em segundo plano (`game_watcher.rs::tick`), e o resultado
/// substitui o fallback via [`SessionMemory::set_summary`] só se o LLM responder com texto não-vazio.
/// Uma falha (rede/provedor fora) devolve `Err` e o fallback já salvo continua valendo — nunca um
/// resumo fabricado no lugar do template.
///
/// Devolve `usage` junto do texto (não só `r.text`) desde 2026-09-24: `game_watcher.rs::spawn_llm_summary`
/// precisa dele para gravar em `token_usage::record` — antes desta rodada esse turno de LLM real era
/// invisível ao painel de custo (`specs/10-observability.md`, "Custo").
pub async fn summarize_with_llm(store: &ConfigStore, session: &GameSession, abort: TurnAbortSignal) -> Result<(String, rig_core::completion::Usage), String> {
    let prompt = summary_prompt(session);
    turn::run_turn_llm_raw(store, None, &prompt, abort, &NullObserver)
        .await
        .map(|r| (r.text.trim().to_string(), r.usage))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::session_types::GameEvent;

    fn memory() -> SessionMemory {
        SessionMemory::new(SessionDb::open_in_memory().unwrap())
    }

    #[test]
    fn start_session_makes_it_the_current_session_and_persists_it() {
        let sm = memory();
        let started = sm.start_session("570", "Dota 2").unwrap();
        assert_eq!(sm.current_session().unwrap().id, started.id);
        assert_eq!(sm.history(10).unwrap().len(), 1);
    }

    #[test]
    fn starting_a_second_session_ends_the_first_one_first() {
        let sm = memory();
        let first = sm.start_session("570", "Dota 2").unwrap();
        let second = sm.start_session("730", "Counter-Strike 2").unwrap();

        assert_eq!(sm.current_session().unwrap().id, second.id);
        let ended_first = sm.session_by_id(&first.id).unwrap().unwrap();
        assert!(ended_first.end_time_ms.is_some(), "a primeira sessão deveria ter sido encerrada");
    }

    #[test]
    fn end_session_without_a_current_session_is_a_no_op() {
        let sm = memory();
        assert_eq!(sm.end_session().unwrap(), None);
    }

    #[test]
    fn end_session_sets_end_time_and_a_fallback_summary() {
        let sm = memory();
        sm.start_session("570", "Dota 2").unwrap();

        let ended = sm.end_session().unwrap().unwrap();
        assert!(ended.end_time_ms.is_some());
        assert!(ended.summary.unwrap().starts_with("Session:"));
        assert_eq!(sm.current_session(), None);
    }

    #[test]
    fn session_for_game_finds_the_most_recent_session_of_that_game() {
        let sm = memory();
        sm.start_session("570", "Dota 2").unwrap();
        sm.end_session().unwrap();
        let latest = sm.start_session("570", "Dota 2").unwrap();

        let found = sm.session_for_game("570").unwrap().unwrap();
        assert_eq!(found.id, latest.id);
    }

    #[test]
    fn set_summary_overwrites_a_persisted_summary() {
        let sm = memory();
        sm.start_session("570", "Dota 2").unwrap();
        let ended = sm.end_session().unwrap().unwrap();
        assert!(ended.summary.unwrap().starts_with("Session:"), "fallback deveria ter sido salvo primeiro");

        sm.set_summary(&ended.id, "Resumo gerado pelo LLM.").unwrap();
        let reloaded = sm.session_by_id(&ended.id).unwrap().unwrap();
        assert_eq!(reloaded.summary.as_deref(), Some("Resumo gerado pelo LLM."));
    }

    #[test]
    fn summary_prompt_includes_game_name_duration_and_event_lines() {
        let events = vec![GameEvent::new(0, "death", "morreu pro chefe", None)];
        let session = GameSession {
            id: "s1".into(),
            game_id: "570".into(),
            game_name: "Dota 2".into(),
            start_time_ms: 0,
            end_time_ms: Some(120_000),
            events,
            summary: None,
        };
        let prompt = summary_prompt(&session);
        assert!(prompt.contains("Dota 2"), "prompt: {prompt}");
        assert!(prompt.contains("2min"), "prompt: {prompt}");
        assert!(prompt.contains("[death] morreu pro chefe"), "prompt: {prompt}");
    }

    /// MUST do spec (08-gamer-companion.md, "Sessão"): o resumo é gerado por LLM, nunca fabricado no
    /// lugar do fallback quando o LLM falha. Porta morta = nunca toca rede/provedor pago
    /// (`llm_dead_port`, mesma convenção de `game_session_tool.rs`).
    #[tokio::test]
    async fn summarize_with_llm_fails_named_without_touching_a_real_provider_when_unreachable() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mika-gamer-session-summary-llm-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        super::super::llm_dead_port::point_the_llm_at_a_dead_port(&store);

        let session = GameSession {
            id: "s1".into(),
            game_id: "570".into(),
            game_name: "Dota 2".into(),
            start_time_ms: 0,
            end_time_ms: Some(60_000),
            events: vec![],
            summary: None,
        };
        let (_handle, signal) = turn::TurnAbortHandle::new();
        let result = summarize_with_llm(&store, &session, signal).await;
        assert!(result.is_err(), "sem provedor no ar, o resumo por LLM deveria falhar nomeado");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
