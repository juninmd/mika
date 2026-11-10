//! `mika.gamer-companion` — os métodos de `mika:plugin-rpc` que este plugin serve (os 7 portados de
//! `apps/desktop/src/features/gamer-companion/handlers/gamer-companion-rpc.ts::registerGamerCompanionRpc`,
//! mais `session-stats`/`steam:root`, que alimentam o card "Agora jogando" do painel de Configurações).
//!
//! `session-list`/`session-current` reaproveitam [`super::session_memory::SessionMemory`], já
//! existente, e `super::game_session_tool::session_json` (tornada `pub(super)` nesta rodada) para o
//! formato de wire — a UI (`renderer/widgets/game-hud.widget.tsx`) espera `startTime`/`endTime` como
//! STRING ISO 8601 e um `durationMs` numérico, e essa conversão já existia ali (porte de
//! `session-memory-rows.ts::rowToSession`); duplicá-la aqui teria sido a MESMA lógica em dois
//! lugares divergindo com o tempo.
//!
//! `steam:top-games`/`steam:game-art` reaproveitam [`super::steam_top_games`]/[`super::steam_art`]/
//! [`super::steam_local_stats`] — tudo já lido do DISCO, sem chave de API (medido nesta rodada).
//! `steam:game-art` valida `appId` com [`super::steam_library::is_valid_steam_app_id`] ANTES de
//! tocar o disco — mesma guarda do TS, reaproveitada, não reescrita.
//!
//! `mini-game:payload`/`mini-game:requested` reaproveitam [`super::play_minigame_tool`] (núcleo do
//! payload e a fila `miniGameQueue`, ambos remedidos nesta rodada — ver o cabeçalho de lá).
//!
//! `mini-game:result` **publica `GAME_MOMENT` desde 2026-09-22** — `EventKey::GameMoment` entrou
//! junto com o primeiro consumidor Rust real (`brain::awareness::tension`, o relógio de tensão que
//! `ProactiveEngine::should_speak` consulta). Dos 4 consumidores TS originais
//! (`game-moment-commentary.ts`, `slang-context-provider.ts`, `main/services/memory/experience.ts`,
//! `vision/processing/observation-publisher.ts`), só o efeito de tensão está portado — os outros 3
//! seguem sem porte, nomeados, não escondidos. Mesmo payload do TS: `{ game: label, type, description,
//! at }`, sem `tense` (o TS só manda `tense` a partir da visão, que não publica `GAME_MOMENT` aqui).

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::plugins::rpc::{registry, RpcHandler};

use super::game_session_tool::session_json;
use super::iso_time::now_ms;
use super::session_stats::{day_key, playtime_by_game, streak};
use super::session_types::GameSession;
use super::play_minigame_tool::{build_payload_data, next_requested, KNOWN_GAME_IDS};
use super::session_memory::SessionMemory;
use super::steam_art::read_game_art;
use super::steam_library::is_valid_steam_app_id;
use super::steam_local_stats::resolve_steam_root;
use super::steam_top_games::get_steam_top_games;

const PLUGIN_ID: &str = "mika.gamer-companion";

/// Espelha `boundedLimit` (`gamer-companion-rpc.ts`): não-número/infinito cai no `fallback`,
/// truncado (não arredondado) e prensado em `[1, maximum]`.
fn bounded_limit(args: &Value, maximum: i64, fallback: i64) -> i64 {
    match args.get("limit").and_then(Value::as_f64) {
        Some(raw) if raw.is_finite() => raw.trunc().clamp(1.0, maximum as f64) as i64,
        _ => fallback,
    }
}

fn configured_steam_path(config: &ConfigStore) -> Option<String> {
    config
        .get("gamer-companion.steamPath")
        .and_then(|v| v.as_str().map(str::to_string))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

struct SessionList {
    memory: Arc<SessionMemory>,
}

#[async_trait]
impl RpcHandler for SessionList {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let limit = bounded_limit(&args, 100, 10);
        let sessions = self.memory.history(limit)?;
        Ok(Value::Array(sessions.iter().map(session_json).collect()))
    }
}

struct SessionCurrent {
    memory: Arc<SessionMemory>,
}

#[async_trait]
impl RpcHandler for SessionCurrent {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        Ok(self.memory.current_session().map(|s| session_json(&s)).unwrap_or(Value::Null))
    }
}

/// "Agora jogando / Últimas sessões" do painel de Configurações: os mesmos números de
/// `game_session(action=playtime|streak)`, mas em ms/dias CRUS para a UI formatar no idioma dela
/// (a tool devolve texto pronto em pt-BR para o modelo).
struct SessionStats {
    memory: Arc<SessionMemory>,
}

#[async_trait]
impl RpcHandler for SessionStats {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        let history = self.memory.history(500)?;
        Ok(session_stats_json(&history, now_ms(), &chrono::Local))
    }
}

pub(super) fn session_stats_json<Tz: chrono::TimeZone>(history: &[GameSession], now: u64, tz: &Tz) -> Value {
    let today_key = day_key(now, tz);
    let total = |totals: &[(String, u64)]| totals.iter().map(|(_, ms)| *ms).sum::<u64>();
    let week = playtime_by_game(history, today_key, 6, tz);
    let (current, longest) = streak(history, now, tz);
    json!({
        "todayMs": total(&playtime_by_game(history, today_key, 0, tz)),
        "weekMs": total(&week),
        "currentStreakDays": current,
        "longestStreakDays": longest,
        "topGames": week.iter().take(3).map(|(game, ms)| json!({ "game": game, "ms": ms })).collect::<Vec<_>>(),
    })
}

/// A pasta da Steam que a detecção usa de fato: a configurada (se existir no disco) e a que a
/// autodetecção acharia sem ela — a tela mostra a segunda como dica em vez de um exemplo fixo.
struct SteamRoot {
    config: Arc<ConfigStore>,
}

#[async_trait]
impl RpcHandler for SteamRoot {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        let configured = configured_steam_path(&self.config);
        let resolved = resolve_steam_root(configured.as_deref());
        let detected = resolve_steam_root(None);
        Ok(json!({ "resolved": resolved, "detected": detected }))
    }
}

struct SteamTopGamesHandler {
    config: Arc<ConfigStore>,
}

#[async_trait]
impl RpcHandler for SteamTopGamesHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let limit = bounded_limit(&args, 12, 8) as usize;
        let configured = configured_steam_path(&self.config);
        Ok(serde_json::to_value(get_steam_top_games(configured.as_deref(), limit)).expect("SteamTopGamesResult sempre serializa"))
    }
}

struct SteamGameArtHandler {
    config: Arc<ConfigStore>,
}

#[async_trait]
impl RpcHandler for SteamGameArtHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let app_id = args.get("appId").and_then(Value::as_str).unwrap_or_default();
        if !is_valid_steam_app_id(app_id) {
            return Err("appId inválido".into());
        }
        let configured = configured_steam_path(&self.config);
        let Some(root) = resolve_steam_root(configured.as_deref()) else {
            return Ok(json!({ "art": null }));
        };
        let art = read_game_art(&root, app_id);
        Ok(json!({ "art": art.as_ref().map(|a| &a.data_uri), "kind": art.as_ref().map(|a| a.kind) }))
    }
}

struct MiniGamePayload;

#[async_trait]
impl RpcHandler for MiniGamePayload {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let raw_game = args.get("game").and_then(Value::as_str);
        let known = raw_game.filter(|g| KNOWN_GAME_IDS.contains(g));
        let Some(game) = known else {
            return Err(format!("Unknown mini game: {}", raw_game.unwrap_or("(missing)")));
        };
        Ok(build_payload_data(game).expect("game já validado contra KNOWN_GAME_IDS acima"))
    }
}

struct MiniGameRequested;

#[async_trait]
impl RpcHandler for MiniGameRequested {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        Ok(json!({ "game": next_requested() }))
    }
}

/// Espelha `MINI_GAME_DISPLAY_NAME` — só jokenpo/tictactoe têm rótulo (pong/truco nunca viram
/// `GAME_MOMENT`, mesmo comportamento do TS).
fn mini_game_display_name(game: &str) -> Option<&'static str> {
    match game {
        "jokenpo" => Some("Pedra, Papel ou Tesoura"),
        "tictactoe" => Some("Jogo da Velha"),
        _ => None,
    }
}

struct MiniGameResult;

#[async_trait]
impl RpcHandler for MiniGameResult {
    /// Ver o cabeçalho do módulo: computa o mesmo rótulo/tipo/descrição do TS e publica
    /// `EventKey::GameMoment` no barramento global do processo — mesmo chokepoint de
    /// `vision::observation_events::emit_screen_observation` (publica direto em
    /// `crate::events::bus()`, sem `EventBus` injetado).
    async fn call(&self, args: Value) -> Result<Value, String> {
        let game = args.get("game").and_then(Value::as_str).unwrap_or_default();
        let result = args.get("result").and_then(Value::as_str).unwrap_or_default();
        let label = mini_game_display_name(game);
        let moment_type = match result {
            "win" => Some("victory"),
            "lose" => Some("defeat"),
            _ => None,
        };
        let (Some(label), Some(moment_type)) = (label, moment_type) else { return Ok(Value::Null) };

        let description = if moment_type == "victory" {
            format!("Você venceu uma rodada de {label} contra a Mika.")
        } else {
            format!("Mika venceu uma rodada de {label} contra você.")
        };
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        crate::events::bus().publish(
            crate::events::EventKey::GameMoment,
            // `source: "minigame"` (ver o cabeçalho de `vision::watcher::publish_game_moment`):
            // nunca passa pelo aterramento de visão de `game_moment_commentary` — é a própria Mika
            // jogando contra o usuário, sempre real.
            json!({ "game": label, "type": moment_type, "description": description, "at": at, "source": "minigame" }),
        );
        Ok(Value::Null)
    }
}

/// Chamado por `super::register_session_tools` — mesmo ponto que já tem `Arc<SessionMemory>` e
/// `Arc<ConfigStore>`, os dois que as 7 RPCs precisam ao todo.
pub(super) fn register(memory: Arc<SessionMemory>, config: Arc<ConfigStore>) {
    let registry = registry();
    registry.register(PLUGIN_ID, "session-list", Arc::new(SessionList { memory: Arc::clone(&memory) }));
    registry.register(PLUGIN_ID, "session-current", Arc::new(SessionCurrent { memory: Arc::clone(&memory) }));
    registry.register(PLUGIN_ID, "session-stats", Arc::new(SessionStats { memory }));
    registry.register(PLUGIN_ID, "steam:root", Arc::new(SteamRoot { config: Arc::clone(&config) }));
    registry.register(PLUGIN_ID, "steam:top-games", Arc::new(SteamTopGamesHandler { config: Arc::clone(&config) }));
    registry.register(PLUGIN_ID, "steam:game-art", Arc::new(SteamGameArtHandler { config }));
    registry.register(PLUGIN_ID, "mini-game:payload", Arc::new(MiniGamePayload));
    registry.register(PLUGIN_ID, "mini-game:requested", Arc::new(MiniGameRequested));
    registry.register(PLUGIN_ID, "mini-game:result", Arc::new(MiniGameResult));
}

/// Serializa os testes que chamam [`register`] — ele SOBRESCREVE as mesmas 7 chaves globais de
/// `plugins::rpc::registry()` a cada chamada, e `cargo test` roda em paralelo: sem isto, o teste A
/// registraria seu `SessionMemory`/`ConfigStore`, o teste B sobrescreveria com os DELE antes de A
/// chamar `dispatch`, e A leria o estado de B. Mesmo padrão de
/// `modules::features::mood::TOOL_REGISTRATION_ORDER`.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
