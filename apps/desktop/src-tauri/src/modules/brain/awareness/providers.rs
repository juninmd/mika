//! Providers do motor proativo — port de `awareness-idle-provider.ts` e `awareness-game-provider.ts`.
//! `awareness-game.ts` (aprendizado de wiki via LLM/browser) tem o lado de ESCRITA em
//! `gamer::game_learning` desde 2026-09-22; `GameProvider::due` lê de volta
//! (`game_learning::wiki_context_for`) para preencher `wikiCtx` em `gaming_loop_comment` — `memory`
//! é `Option` porque nem todo chamador de teste tem um `MemoryStore` à mão, e "sem memória" degrada
//! para `wikiCtx` vazio, nunca panica.

use std::sync::{Arc, Mutex};

use chrono::Timelike;

use super::constants::{GAME_ABSENCE_RENUDGE_MS, GAME_ABSENCE_THRESHOLD_MS, IDLE_THRESHOLD_MS};
use super::prompts;
use super::types::{ProactiveCandidate, ProactiveProvider};
use crate::modules::gamer::gaming_state::{GamingState, VISION_GAME_SIGNAL_MAX_AGE_MS};
use crate::modules::gamer::session_db::SessionDb;
use crate::modules::memory::MemoryStore;

// === Ocioso ======================================================================================

/// Port de `buildIdleProvider`, mas medido diferente: o TS mede ociosidade por
/// `lastUserMessageAt` (`Events.USER_MESSAGE`, que não existe em `events/keys.rs` — fora da minha
/// propriedade adicionar). `crate::power::snapshot()` já mede "segundos desde o último input do
/// usuário" via `GetLastInputInfo`/IOKit/XScreenSaver (ver o cabeçalho de `power.rs`, que documenta
/// justamente a proatividade por ocioso como o "candidato óbvio" de consumidor) — a MESMA pergunta
/// que `lastUserMessageAt` tentava responder por um caminho mais indireto (última mensagem de chat
/// em vez de último input de teclado/mouse). `snapshot` é síncrono e chamado a cada `due()`.
pub struct IdleProvider {
    nickname: String,
    /// Which `idle-nudge-*` variant the next nudge uses; advances on each one produced.
    variant: std::sync::atomic::AtomicUsize,
    /// The user's wall clock (`user.timezone`) — "madrugada" is theirs, not the machine's.
    clock: Box<dyn Fn() -> chrono::NaiveDateTime + Send + Sync>,
}

impl IdleProvider {
    pub fn new(nickname: String, clock: Box<dyn Fn() -> chrono::NaiveDateTime + Send + Sync>) -> Self {
        Self { nickname, variant: Default::default(), clock }
    }
}

/// "morning"/"afternoon"/"evening"/"madrugada" a partir da hora do USUÁRIO — mesmas 4 faixas de
/// `getTimeOfDay` no TS (`awareness.service.ts`).
fn time_of_day(hour: u32) -> &'static str {
    match hour {
        5..=11 => "morning",
        12..=17 => "afternoon",
        18..=23 => "evening",
        _ => "late night",
    }
}

impl ProactiveProvider for IdleProvider {
    fn id(&self) -> &'static str {
        "idle"
    }

    fn due(&self, _now_ms: u64) -> Option<ProactiveCandidate> {
        let snapshot = crate::power::snapshot().ok()?;
        let idle_seconds = snapshot.get("idleSeconds")?.as_u64()?;
        if idle_seconds * 1000 < IDLE_THRESHOLD_MS {
            return None;
        }
        let hour = (self.clock)().hour();
        let period = time_of_day(hour);
        let prompt = match prompts::idle_nudge(&self.nickname, period, period == "late night", self.variant.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) {
            Ok(prompt) => prompt,
            Err(error) => {
                log::error!("provider proativo 'idle': prompt não carregou: {error}");
                return None;
            }
        };
        Some(ProactiveCandidate { key: "idle".into(), priority: 3, label: "idle nudge".into(), prompt })
    }
}

// === Jogo sustentado =============================================================================

/// Port de `buildGameProvider`. Lê a sessão "atual" direto do `gamer-sessions.db` que
/// `gamer::game_watcher` já escreve — sem duplicar detecção de jogo, só consumo (mandato da tarefa).
///
/// **Aterrado em visão, não só no nome da sessão** (smoke-baseline.md item 8/2): uma sessão aberta
/// (`gamer::game_watcher`, detecção por processo/janela) já foi provada capaz de falso positivo
/// ("Stray" com o Windows Terminal em foco). `due()` só fala quando, ALÉM da sessão aberta, a
/// observação de tela MAIS RECENTE (`GamingState::vision_confirms_game`, alimentada por
/// `vision::watcher::run_tick`) também disse que há um jogo na tela — nunca o nome da sessão
/// sozinho.
pub struct GameProvider {
    db: SessionDb,
    memory: Option<&'static MemoryStore>,
    gaming: Arc<GamingState>,
}

impl GameProvider {
    pub fn open(db_path: &std::path::Path, memory: Option<&'static MemoryStore>, gaming: Arc<GamingState>) -> Result<Self, String> {
        Ok(Self { db: SessionDb::open(db_path)?, memory, gaming })
    }
}

impl ProactiveProvider for GameProvider {
    fn id(&self) -> &'static str {
        "game"
    }

    fn due(&self, now_ms: u64) -> Option<ProactiveCandidate> {
        let session = self.db.history(1).ok()?.into_iter().next()?;
        if session.end_time_ms.is_some() {
            return None; // sessão já encerrada — não é "jogando agora"
        }
        if !self.gaming.vision_confirms_game(now_ms, VISION_GAME_SIGNAL_MAX_AGE_MS) {
            return None; // sessão aberta, mas a tela não confirma jogo recentemente — nunca narra às cegas
        }
        let session_minutes = now_ms.saturating_sub(session.start_time_ms) / 60_000;
        let wiki_ctx = self
            .memory
            .map(|memory| crate::modules::gamer::game_learning::wiki_context_for(memory, &session.game_name))
            .unwrap_or_default();
        let prompt = match prompts::gaming_loop_comment(&session.game_name, session_minutes, &wiki_ctx) {
            Ok(prompt) => prompt,
            Err(error) => {
                log::error!("provider proativo 'game': prompt não carregou: {error}");
                return None;
            }
        };
        Some(ProactiveCandidate {
            key: "game-sustained".into(),
            priority: 7,
            label: format!("gaming loop: {}", session.game_name),
            prompt,
        })
    }
}

// === Ausência de jogo =============================================================================

/// Port de `buildAwayFromGamesProvider`. `last_nudged_at` é estado em memória do processo (mesma
/// semântica do `lastAbsenceNudgeAt` do TS — não persistido; reinicia com o app).
pub struct GameAbsenceProvider {
    db: SessionDb,
    nickname: String,
    last_nudged_at: Mutex<u64>,
}

impl GameAbsenceProvider {
    pub fn open(db_path: &std::path::Path, nickname: String) -> Result<Self, String> {
        Ok(Self { db: SessionDb::open(db_path)?, nickname, last_nudged_at: Mutex::new(0) })
    }
}

impl ProactiveProvider for GameAbsenceProvider {
    fn id(&self) -> &'static str {
        "game-absence"
    }

    fn due(&self, now_ms: u64) -> Option<ProactiveCandidate> {
        let session = self.db.history(1).ok()?.into_iter().next()?;
        let stopped_at = session.end_time_ms?;
        if now_ms.saturating_sub(stopped_at) < GAME_ABSENCE_THRESHOLD_MS {
            return None;
        }
        {
            let mut last_nudged = self.last_nudged_at.lock().ok()?;
            if now_ms.saturating_sub(*last_nudged) < GAME_ABSENCE_RENUDGE_MS {
                return None;
            }
            *last_nudged = now_ms;
        }
        let days = now_ms.saturating_sub(stopped_at) / (24 * 60 * 60 * 1000);
        let prompt = match prompts::game_absence_nudge(&self.nickname, &session.game_name, days) {
            Ok(prompt) => prompt,
            Err(error) => {
                log::error!("provider proativo 'game-absence': prompt não carregou: {error}");
                return None;
            }
        };
        Some(ProactiveCandidate { key: "game-absence".into(), priority: 4, label: format!("game absence: {days}d"), prompt })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::session_types::GameSession;

    fn db_with_session(end_time_ms: Option<u64>, start_time_ms: u64) -> SessionDb {
        let db = SessionDb::open_in_memory().unwrap();
        db.insert(&GameSession {
            id: "s1".into(),
            game_id: "g1".into(),
            game_name: "Hades".into(),
            start_time_ms,
            end_time_ms: None,
            events: vec![],
            summary: None,
        })
        .unwrap();
        if let Some(end) = end_time_ms {
            db.finish("s1", end, None, &[]).unwrap();
        }
        db
    }

    /// `IdleProvider::due` mede ociosidade via `power::snapshot()` (input real do SO) — não dá para
    /// controlar o tempo em teste (o motivo de `crate::power` não ter chamador antes desta rodada,
    /// ver o cabeçalho do arquivo). O que se prova aqui é o CONTRATO: nunca panica, e quando decide
    /// falar, fala com a chave/prioridade certas — a decisão liga-desliga em si já está coberta pelos
    /// testes puros de `gates.rs`/`tick.rs` e pelo teste de `prompts::idle_nudge`.
    #[test]
    fn idle_provider_never_panics_and_uses_the_right_key_and_priority_when_it_does_speak() {
        let provider = IdleProvider::new("Jun".into(), Box::new(|| chrono::Local::now().naive_local()));
        if let Some(candidate) = provider.due(0) {
            assert_eq!(candidate.key, "idle");
            assert_eq!(candidate.priority, 3);
            assert!(candidate.prompt.contains("Jun"));
        }
    }

    /// `at_ms` é o instante do SINAL de visão, não o instante de `due()` — o chamador escolhe algo
    /// dentro de `VISION_GAME_SIGNAL_MAX_AGE_MS` do `now_ms` que vai passar para `due()`, senão o
    /// próprio helper produziria um sinal EXPIRADO e o teste falharia pelo motivo errado (já
    /// aconteceu aqui: `due(10 * 60_000)` contra um sinal gravado em `0` é mais velho que os 3
    /// minutos de validade).
    fn grounded_gaming(at_ms: u64) -> Arc<GamingState> {
        let gaming = Arc::new(GamingState::new());
        gaming.record_vision_signal(true, at_ms);
        gaming
    }

    #[test]
    fn game_provider_fires_only_while_the_session_is_still_open() {
        let db = db_with_session(None, 0);
        let now = 10 * 60_000;
        let provider = GameProvider { db, memory: None, gaming: grounded_gaming(now) };
        let candidate = provider.due(now).unwrap();
        assert_eq!(candidate.key, "game-sustained");
        assert_eq!(candidate.priority, 7);
        assert!(candidate.prompt.contains("Hades"));
    }

    #[test]
    fn game_provider_is_silent_once_the_session_ended() {
        let db = db_with_session(Some(5_000), 0);
        let provider = GameProvider { db, memory: None, gaming: grounded_gaming(10_000) };
        assert!(provider.due(10_000).is_none());
    }

    /// O gate central desta rodada (smoke-baseline.md item 8/2): uma sessão aberta sozinha nunca é
    /// motivo suficiente para comentar — sem uma observação de tela recente confirmando o jogo,
    /// `due()` fica em silêncio mesmo com a sessão real aberta.
    #[test]
    fn game_provider_is_silent_when_vision_never_confirmed_a_game_even_with_an_open_session() {
        let db = db_with_session(None, 0);
        let provider = GameProvider { db, memory: None, gaming: Arc::new(GamingState::new()) };
        assert!(provider.due(10 * 60_000).is_none(), "sem sinal de visão, nunca deveria narrar às cegas");
    }

    /// Mesmo gate, agora com um sinal de visão VELHO demais (expirou) — não conta como confirmação.
    #[test]
    fn game_provider_is_silent_when_the_last_vision_confirmation_is_stale() {
        let db = db_with_session(None, 0);
        let gaming = Arc::new(GamingState::new());
        gaming.record_vision_signal(true, 0);
        let provider = GameProvider { db, memory: None, gaming };
        let stale_now = VISION_GAME_SIGNAL_MAX_AGE_MS + 10 * 60_000;
        assert!(provider.due(stale_now).is_none());
    }

    #[test]
    fn game_absence_provider_waits_out_the_threshold_then_nudges_once_per_renudge_window() {
        let db = db_with_session(Some(0), 0);
        let provider = GameAbsenceProvider { db, nickname: "Jun".into(), last_nudged_at: Mutex::new(0) };

        assert!(provider.due(GAME_ABSENCE_THRESHOLD_MS - 1).is_none());

        let first = provider.due(GAME_ABSENCE_THRESHOLD_MS + 1).unwrap();
        assert_eq!(first.key, "game-absence");

        // Ainda dentro da janela de renudge — não repete.
        assert!(provider.due(GAME_ABSENCE_THRESHOLD_MS + 2).is_none());

        // Passou a janela de renudge — nudge de novo.
        assert!(provider.due(GAME_ABSENCE_THRESHOLD_MS + GAME_ABSENCE_RENUDGE_MS + 1).is_some());
    }
}
