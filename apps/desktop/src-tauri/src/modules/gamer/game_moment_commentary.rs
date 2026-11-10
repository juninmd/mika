//! `handlers/game-moment-commentary.ts::maybeCommentOnGameMoment` — porte fiel, não "através do
//! motor proativo": o TS emite `Events.SPEECH_REQUEST` DIRETO (bypassa `ProactiveEngine` de
//! propósito — um comentário de "backseat gaming" é reativo a um momento que já passou, não um
//! candidato que compete por cadência), com o PRÓPRIO cooldown por `momentType` + piso duro de 15s
//! entre comentários de tipos diferentes. Este arquivo faz o mesmo: `EventKey::SpeechRequest` (já
//! tem consumidor real, `voice::auto_speak`), gate próprio em [`CommentaryState`].
//!
//! **Gates portados, na MESMA ordem do TS `isCommentBlocked`:**
//! `gamer-companion.backseatGamingEnabled` (desligado por padrão — `false`, mesmo default do TS) →
//! tensão de jogo (`brain::awareness::tension::GameTensionState`, MESMA instância que o motor
//! proativo consulta) → `voice::tts::speech_mute::is_muted` → jogo competitivo fora de
//! vitória/derrota → piso de 15s entre QUALQUER comentário → cooldown por tipo
//! (`gamer-companion.momentCooldowns[type]` ou o default de `DEFAULT_MOMENT_COOLDOWNS`).
//!
//! **Reserva-antes-do-LLM**: mesma técnica de `engine.rs::speak` — grava o slot do tipo ANTES do
//! `await` da geração, desfaz se a geração falhar (`generate_or_rollback`), pela mesma razão do TS
//! (`generateCommentOrRollback`): duas ocorrências do MESMO tipo entrando durante o round-trip do
//! LLM não podem passar as duas por `is_blocked`.
//!
//! Testável sem LLM real: [`CommentGenerator`] é injetado.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;

use crate::config::store::ConfigStore;
use crate::modules::brain::awareness::tension::GameTensionState;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortHandle};
use crate::modules::chat::load_prompt_no_handle;
use crate::modules::gamer::gaming_state::{GamingState, VISION_GAME_SIGNAL_MAX_AGE_MS};
use crate::modules::gamer::known_games::find_known_game;
use crate::modules::voice::tts::speech_mute::is_muted;

const HARD_FLOOR_MS: u64 = 15_000;
const STALE_ENTRY_MS: u64 = 10 * 60_000;
const COMMENTED_TYPES: &[&str] = &["death", "victory", "defeat", "achievement"];

fn default_cooldown_ms(moment_type: &str) -> u64 {
    match moment_type {
        "achievement" | "victory" => 60_000,
        "death" | "defeat" => 45_000,
        _ => 0,
    }
}

fn competitive_game_pattern() -> &'static Regex {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b(valorant|counter-?strike|cs ?2|league of legends|dota ?2|overwatch|rainbow six|siege|apex legends|rocket league|teamfight tactics|starcraft|street fighter|tekken|call of duty|warzone|pubg|fortnite|marvel rivals)\b").unwrap()
    })
}

/// Port de `isCompetitiveGame`.
pub fn is_competitive_game(game: &str) -> bool {
    if competitive_game_pattern().is_match(game) {
        return true;
    }
    let key: String = game.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    find_known_game(&key).is_some_and(|known| competitive_game_pattern().is_match(known.display_name))
}

/// Body pose she strikes as she comments (a name from the `avatar` tool's `EXPRESSIONS`): winning
/// reads in the body before the line lands, a death gets the facepalm.
pub fn moment_pose(moment_type: &str) -> Option<&'static str> {
    match moment_type {
        "victory" => Some("celebrate"),
        "achievement" => Some("tada"),
        "death" => Some("facepalm"),
        "defeat" => Some("slump"),
        _ => None,
    }
}

/// How long the pose's face holds before easing back to neutral — same revert the `avatar` tool does.
const POSE_HOLD: std::time::Duration = std::time::Duration::from_secs(5);

fn publish_moment_pose(moment_type: &str) {
    let Some(pose) = moment_pose(moment_type) else { return };
    let bus = crate::events::bus();
    bus.publish(crate::events::EventKey::Expression, serde_json::json!({ "expression": pose }));
    tokio::spawn(async move {
        tokio::time::sleep(POSE_HOLD).await;
        bus.publish(crate::events::EventKey::Expression, serde_json::json!({ "expression": "neutral" }));
    });
}

fn is_commented_moment_type(moment_type: &str) -> bool {
    COMMENTED_TYPES.contains(&moment_type)
}

/// Julga se uma fala nova pode sair — em produção, o guarda de repetição do processo.
pub type Approval = Box<dyn Fn(&str) -> bool + Send + Sync>;

/// What she said lately, oldest first — shown to the model so it can pass instead of repeating.
pub type RecentLines = Box<dyn Fn() -> Vec<String> + Send + Sync>;

/// Estado de cooldown por tipo de momento — um `Arc` por processo, injetado (nunca `static`).
pub struct CommentaryState {
    last_comment_by_type: Mutex<HashMap<String, u64>>,
    approve: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    recent: RecentLines,
}

impl CommentaryState {
    /// Mesmo guarda (lexical + semântico) e MESMO histórico das falas proativas: um comentário de
    /// jogo não repete o que ela já disse, e a próxima fala proativa sabe que ele foi dito.
    pub fn new() -> Self {
        Self::with_history(
            Box::new(crate::modules::brain::guards::approve_proactive),
            Box::new(|| crate::modules::brain::guards::repetition_guard().recent().to_vec()),
        )
    }

    pub fn with_history(approve: Approval, recent: RecentLines) -> Self {
        Self { last_comment_by_type: Mutex::new(HashMap::new()), approve: Arc::from(approve), recent }
    }

    /// `approve` embute o texto (`ort`, síncrono): roda fora do worker async, que também serve
    /// `mika_invoke`. Tarefa que falha (pânico) conta como "não aprovado" — na dúvida, cala.
    async fn approve(&self, comment: &str) -> bool {
        let approve = Arc::clone(&self.approve);
        let comment = comment.to_string();
        tokio::task::spawn_blocking(move || approve(&comment)).await.unwrap_or(false)
    }

    fn prune_stale(&self, guard: &mut HashMap<String, u64>, now_ms: u64) {
        guard.retain(|_, &mut at| now_ms.saturating_sub(at) <= STALE_ENTRY_MS);
    }

    /// Port de `isCommentBlocked`, com `backseat_enabled`/`tense`/`muted`/`competitive` já
    /// resolvidos pelo chamador (mesma separação de `passes_cadence` em `gates.rs`: esta função
    /// permanece pura sobre o próprio estado).
    fn is_blocked(
        &self,
        moment_type: &str,
        backseat_enabled: bool,
        tense: bool,
        muted: bool,
        competitive_off_result: bool,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> bool {
        if !backseat_enabled || tense || muted || competitive_off_result {
            return true;
        }

        let mut guard = self.last_comment_by_type.lock().unwrap_or_else(|e| e.into_inner());
        self.prune_stale(&mut guard, now_ms);

        let last_any = guard.values().copied().max().unwrap_or(0);
        if now_ms.saturating_sub(last_any) < HARD_FLOOR_MS {
            return true;
        }

        let last_for_type = guard.get(moment_type).copied().unwrap_or(0);
        now_ms.saturating_sub(last_for_type) < cooldown_ms
    }

    /// Reserva o slot ANTES do `await` do LLM — ver o cabeçalho.
    fn reserve(&self, moment_type: &str, now_ms: u64) -> Option<u64> {
        let mut guard = self.last_comment_by_type.lock().unwrap_or_else(|e| e.into_inner());
        let previous = guard.insert(moment_type.to_string(), now_ms);
        previous
    }

    /// Desfaz a reserva quando a geração falha ou vem vazia.
    fn rollback(&self, moment_type: &str, now_ms: u64, previous: Option<u64>) {
        let mut guard = self.last_comment_by_type.lock().unwrap_or_else(|e| e.into_inner());
        if guard.get(moment_type) == Some(&now_ms) {
            match previous {
                Some(at) => {
                    guard.insert(moment_type.to_string(), at);
                }
                None => {
                    guard.remove(moment_type);
                }
            }
        }
    }
}

impl Default for CommentaryState {
    fn default() -> Self {
        Self::new()
    }
}

/// A única chamada de LLM real, injetada — mesma técnica de `ProactiveSender`/`GameResearch`.
#[async_trait]
pub trait CommentGenerator: Send + Sync {
    async fn generate(&self, prompt: &str) -> Result<String, String>;
}

pub struct LiveCommentGenerator {
    store: Arc<ConfigStore>,
}

impl LiveCommentGenerator {
    pub fn new(store: Arc<ConfigStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl CommentGenerator for LiveCommentGenerator {
    async fn generate(&self, prompt: &str) -> Result<String, String> {
        let (_handle, signal) = TurnAbortHandle::new();
        turn::run_turn_llm_raw(&self.store, None, prompt, signal, &NullObserver).await.map(|r| r.text).map_err(|e| e.to_string())
    }
}

fn config_bool(store: &ConfigStore, path: &str, default: bool) -> bool {
    store.get(path).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn config_f64(store: &ConfigStore, path: &str, default: f64) -> f64 {
    store.get(path).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn cooldown_override_ms(store: &ConfigStore, moment_type: &str) -> Option<u64> {
    store.get("gamer-companion.momentCooldowns").and_then(|v| v.get(moment_type).and_then(|n| n.as_u64())).map(|s| s * 1_000)
}

/// Port de `maybeCommentOnGameMoment`. `store` resolve config/locale, `tension` é a MESMA instância
/// que `ProactiveEngine::should_speak` consulta (injetada por `spawn`), `gaming` é a MESMA instância
/// que `vision::watcher::run_tick` alimenta a cada análise real (aterramento — ver o cabeçalho de
/// `GamingState::vision_confirms_game`), `generator` gera o comentário. Devolve `true` quando um
/// `EventKey::SpeechRequest` foi publicado.
pub async fn maybe_comment_on_game_moment(
    state: &CommentaryState,
    store: &ConfigStore,
    tension: &GameTensionState,
    gaming: &GamingState,
    generator: &dyn CommentGenerator,
    payload: &Value,
    now_ms: u64,
) -> bool {
    // Toggle mestre da feature (`gamer-companion.enabled`) — antes desta rodada só
    // `backseatGamingEnabled` era checado; desligar a feature inteira não impedia este comentário
    // reativo de sair (varredura de controles de UI de 2026-09-27, mesma leitura de
    // `feature::master_enabled` que `game_watcher`/as tools de sessão agora fazem).
    if !super::feature::master_enabled(store) {
        return false;
    }

    let moment_type_raw = payload.get("type").and_then(Value::as_str).unwrap_or("");
    let tense_flag = payload.get("tense").and_then(Value::as_bool).unwrap_or(false);
    // Mesma chamada que `brain::awareness::mod.rs` já faz para o relógio de tensão — aqui só LÊ o
    // resultado (o `apply_moment` de verdade já rodou naquele assinante; reaplicar aqui duplicaria
    // o efeito). `is_tense` pura, sem side effect.
    let tense = tension.is_tense(now_ms) || tense_flag;

    if !is_commented_moment_type(moment_type_raw) {
        return false;
    }
    let moment_type = moment_type_raw;

    let game = payload.get("game").and_then(Value::as_str).map(str::trim).unwrap_or("");
    let description = payload.get("description").and_then(Value::as_str).map(str::trim).unwrap_or("");
    if game.is_empty() || description.is_empty() {
        return false;
    }

    // Aterramento (smoke-baseline.md item 8/2) — só para momentos que VIERAM de visão
    // (`source: "vision"`, `vision::watcher::publish_game_moment`): um minigame local
    // (`gamer::rpc::MiniGameResult`, `source: "minigame"`) é a própria Mika jogando contra o
    // usuário, sempre real por definição — nunca precisou, e não deveria precisar, de uma
    // observação de tela para confirmar que o jogo aconteceu.
    let from_vision = payload.get("source").and_then(Value::as_str) == Some("vision");
    if from_vision && !gaming.vision_confirms_game(now_ms, VISION_GAME_SIGNAL_MAX_AGE_MS) {
        return false;
    }

    let backseat_enabled = config_bool(store, "gamer-companion.backseatGamingEnabled", false);
    let muted = is_muted(now_ms as i64);
    let competitive_blocks = is_competitive_game(game) && moment_type != "victory" && moment_type != "defeat";
    let cooldown_ms = cooldown_override_ms(store, moment_type).unwrap_or_else(|| default_cooldown_ms(moment_type));

    if state.is_blocked(moment_type, backseat_enabled, tense, muted, competitive_blocks, cooldown_ms, now_ms) {
        return false;
    }

    let previous = state.reserve(moment_type, now_ms);

    let aggressiveness = config_f64(store, "gamer-companion.backseatAggressiveness", 40.0);
    let boss_name = payload.get("bossName").and_then(Value::as_str).unwrap_or("");
    let boss_line = if boss_name.is_empty() { String::new() } else { format!("Inimigo/boss envolvido: {boss_name}") };
    let language = crate::config::locale::app_language(store);

    let vars: HashMap<String, String> = [
        ("game".to_string(), game.to_string()),
        ("type".to_string(), moment_type.to_string()),
        ("description".to_string(), description.to_string()),
        ("bossLine".to_string(), boss_line),
        ("aggressiveness".to_string(), aggressiveness.to_string()),
        ("language".to_string(), language),
    ]
    .into_iter()
    .collect();

    let prompt = match load_prompt_no_handle("gamer-companion", "game-moment-commentary", &vars) {
        Ok(p) => p,
        Err(error) => {
            log::error!("comentário de momento de jogo sem prompt (game-moment-commentary): {error}");
            state.rollback(moment_type, now_ms, previous);
            return false;
        }
    };

    // Same novelty contract as the awareness turn (`chat::proactive_novelty`): recent lines, and [PASS].
    let prompt = crate::modules::chat::proactive_novelty::with_recent_lines(&prompt, &(state.recent)());
    let text = match generator.generate(&prompt).await {
        Ok(text) => text,
        Err(error) => {
            log::warn!("geração de comentário de momento de jogo falhou: {error}");
            state.rollback(moment_type, now_ms, previous);
            return false;
        }
    };

    let comment = text.trim();
    if comment.is_empty() {
        state.rollback(moment_type, now_ms, previous);
        return false;
    }

    // Re-checa mute/tensão DEPOIS do round-trip do LLM — o mesmo gap de tempo que
    // `vision::observation_events::emit_screen_observation` já re-checa para `is_vision_enabled`.
    if is_muted(now_ms as i64) || tension.is_tense(now_ms) {
        state.rollback(moment_type, now_ms, previous);
        return false;
    }

    // The model chose silence: held back like a repeat — the reservation stays, nothing is recorded.
    if crate::modules::chat::proactive_novelty::is_pass(comment) {
        return false;
    }

    // Último gate: aprovar grava a fala no histórico. Repetida, ela ESCOLHE calar — a reserva fica,
    // como o `HELD_BACK` do motor, para não tentar outra paráfrase logo em seguida.
    if !state.approve(comment).await {
        return false;
    }

    publish_moment_pose(moment_type);
    // `origin` marca a fala como comentário de jogo (conquista Steam "primeiro jogo comentado").
    crate::events::bus().publish(
        crate::events::EventKey::SpeechRequest,
        serde_json::json!({ "text": comment, "origin": "game-moment" }),
    );
    true
}

#[cfg(test)]
impl CommentaryState {
    pub fn with_approval(approve: Approval) -> Self {
        Self::with_history(approve, Box::new(Vec::new))
    }
}

#[cfg(test)]
#[path = "game_moment_commentary_tests.rs"]
mod tests;
