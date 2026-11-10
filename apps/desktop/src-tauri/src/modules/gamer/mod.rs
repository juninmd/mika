//! Módulo `gamer` em Rust — port do backend TypeScript correspondente
//! (`apps/desktop/src/features/gamer-companion/`, 4203 LOC).
//!
//! Virou diretório em 2026-09-21 (era um stub vazio) para caber a fatia real portada nesta
//! rodada: detecção de jogo Steam a partir do DISCO (sem chave de API — fato já verificado em
//! rodadas anteriores da migração), tempo de jogo local, arte da biblioteca, detecção de jogo por
//! processo em execução, e as tools `track_gamer_mood`/`steam`, registradas de verdade no
//! `ToolRegistry` nativo (`modules/tools.rs`, sem editar aquele arquivo).
//!
//! Nenhum canal de `mika_invoke` é reivindicado aqui — ver a investigação preservada abaixo sobre
//! `entertainment:play-sound`. Registrar tools no `ToolRegistry` é uma coisa DIFERENTE de
//! reivindicar um canal de IPC: o catálogo de tools é consultado por `brain::turn` via
//! `tools::registry().execute(...)`, não pela ponte `bridge.rs`↔sidecar.
//!
//! ## Remedido 2026-09-22: as 6 tools do TS medidas de novo (não presumidas) — 6 de 6 nativas agora
//!
//! Medição contra `apps/desktop/src/features/gamer-companion/tools/`: `game_session`,
//! `game_knowledge`, `play_minigame` (as 3 que `modules/tools/mod.rs` listava como faltando, MENOS
//! `recommend_media`, fechada numa rodada seguinte — ver abaixo) ganharam par nativo nesta
//! rodada — [`game_session_tool`], [`game_knowledge_tool`], [`play_minigame_tool`].
//! `track_gamer_mood`/`steam` já existiam.
//! - `game_session`: `stats`/`playtime`/`streak`/`events` são fiéis, sobre [`session_memory`]
//!   (que já existia sem nenhum chamador de produção antes desta rodada). `describe` foi remedido
//!   numa rodada seguinte — ver abaixo.
//! - `game_knowledge`: `memorize_location`/`get_location_guide` são fiéis, sobre [`game_locations`]
//!   (CRUD novo na MESMA `SessionDb`/arquivo que `session_db.rs` já abre). `ask` foi remedido numa
//!   rodada seguinte — ver abaixo.
//! - `play_minigame`: jokenpo/tictactoe/pong são fiéis (payload HTML estático, porte literal).
//!   truco é fiel na distribuição das cartas; a zoeira da Mika (`taunts`) fica sempre vazia pelo
//!   MESMO motivo (sem LLM) — o mesmo fallback que o próprio TS já usa quando a chamada falha, não
//!   uma simulação nova.
//! - `playtime`/`streak` agrupam por dia no fuso do SO (`chrono::Local`), não pelo
//!   `user.timezone` IANA do TS (`resolveUserTimeZone()`) — sem `chrono-tz`.
//!
//! ## Remedido 2026-09-22b: `recommend_media` tinha porte, sim — a premissa "sem cliente de LLM
//! ## alcançável daqui" estava errada
//!
//! `brain::tools::content::ContentHandler` (`brain/tools/content.rs`) já prova o caminho:
//! `turn::run_turn_ollama_raw(&ConfigStore, ...)` é `pub`, sem `AppHandle`, chamável de qualquer
//! módulo que feche sobre `Arc<ConfigStore>` — exatamente o que [`register_session_tools`] já
//! recebe. [`recommend_media_tool`] é o porte real (não degradação nomeada): mesmo prompt
//! (`prompts/recommend-media.md`, embutido por `include_str!`), mesmo parse de duas camadas
//! (JSON direto, depois regex de objetos soltos), mesmo `errorCode: 'internal'` no catch de
//! geração que o TS de origem usa (DIFERENTE do `'unavailable'` de `content.rs`, que é de outro
//! arquivo TS). Registrada com `game_session`/`game_knowledge` no mesmo ponto de boot — precisa de
//! `Arc<ConfigStore>`, que só chega aqui via [`game_watcher::start`].
//!
//! ## Remedido 2026-09-22c: `game_session` action=`describe` e `game_knowledge` action=`ask`
//! ## também tinham porte — as duas últimas ações "sem cliente de LLM" nomeadas acima
//!
//! Mesma premissa de 2026-09-22b, agora medida contra as DUAS ações que faltavam: nem `describe`
//! (`handlers/commentary.ts`) nem `ask` (`handlers/game-knowledge.ts`) tinham de fato uma lacuna de
//! LLM — só faltava passar o `Arc<ConfigStore>` que [`register_session_tools`] já recebe para
//! [`game_session_tool::GameSessionHandler`]/[`game_knowledge_tool::GameKnowledgeHandler`], que só
//! seguravam `Arc<SessionMemory>` até agora. As 6 tools do TS estão 6 de 6 nativas SEM nenhuma ação
//! degradada por falta de cliente de LLM — só as lacunas de outra classe seguem (tabela abaixo).
//!
//! **`session_memory.rs`** documentava a MESMA premissa errada para `buildSummary`/
//! `summarizeSessionWithLLM` (resumo automático ao ENCERRAR uma sessão, chamado de
//! `services/session-memory.ts`, DIFERENTE da tool `game_session` action=`describe` deste
//! parágrafo) — o cabeçalho de lá foi corrigido para não repetir a premissa refutada, mas essa
//! chamada específica (auto-resumo no `end_session`) segue usando [`session_types::fallback_summary`]
//! nesta rodada: é um call site diferente das duas ações desta seção, fora do escopo desta rodada,
//! nomeado como próximo passo no relatório, não como lacuna de LLM.
//!
//! ## Remedido 2026-09-22d: `steam` action=`achievements` despacha de verdade — saiu da primeira
//! ## passada de boot
//!
//! Mesma classe de premissa refutada de 2026-09-22b/c: a rodada anterior media "Steam Web API
//! precisa de `AppHandle`, que `ToolHandler::call` não recebe" como bloqueio definitivo, sem notar
//! que `register_session_tools` (abaixo) já resolvia exatamente isso para
//! `RecommendMediaHandler`/`GameSessionHandler`/`GameKnowledgeHandler`. `SteamToolHandler` agora
//! guarda `Arc<ConfigStore>` (`steam_tool.rs::SteamToolHandler::new`) e está cadastrada AQUI, não
//! mais em [`register_tools`] — `steam_achievements/` (porte de `handlers/steam-achievements.ts`)
//! chama `ISteamUserStats/GetPlayerAchievements/v1/` de verdade, atrás do mesmo chokepoint de rede
//! de `llm.rs`/`voice` (`privacy::assert_network_allowed`).
//!
//! ## O que mais NÃO foi portado nesta rodada, e por quê (tabela completa no relatório)
//! - **Janela em foco** (`findForegroundGame`/`getForegroundWindow`): o TS usa um addon nativo
//!   Windows-only. O equivalente Rust pede uma crate nova (`windows` com
//!   `Win32_UI_WindowsAndMessaging` no Windows, mais um caminho próprio de X11/Wayland/macOS para
//!   não regredir os outros dois SOs) — fora do que esta rodada pode acrescentar a `Cargo.toml`.
//!   `game_detection::find_active_game` cobre só a detecção por PROCESSO em execução, que já é o
//!   fallback que o próprio TS usa quando a janela em foco falha.
//! - **Steam Web API para jogos possuídos/recentes** (`getOwnedGames`/`getRecentGames`, distinto
//!   de `achievements`): mesmo `Arc<ConfigStore>` já disponível, não medido/portado nesta rodada —
//!   fora do pedido, não uma lacuna de arquitetura.
//! - **Watcher em background** (`SteamGameWatcher`, tick a cada N segundos): já sobe em produção
//!   desde a rodada anterior (`game_watcher::start`, chamado por
//!   `modules::register_config_bound_tools`) — não é mais uma lacuna, listado aqui só por histórico.

pub mod feature;
pub mod game_detection;
#[cfg(test)]
mod game_detection_live_tests;
pub mod game_knowledge_tool;
pub mod game_learning;
pub mod game_locations;
pub mod game_moment_commentary;
pub mod game_partners;
pub mod gaming_state;
pub mod game_session_tool;
pub mod game_watcher;
mod game_watcher_decision;
pub mod iso_time;
pub mod known_games;
/// Fixture compartilhado dos testes que alcançam o LLM — ver o cabeçalho do arquivo para o defeito
/// que ele conserta (teste que passa ou falha conforme Ollama esteja rodando na máquina).
#[cfg(test)]
mod llm_dead_port;
pub mod minigame_jokenpo;
pub mod minigame_pong;
pub mod minigame_pong_script;
pub mod minigame_shell;
pub mod minigame_tictactoe;
pub mod minigame_tictactoe_script;
pub mod mood_tracker;
pub mod mute_learning;
pub mod play_minigame_tool;
pub mod recommend_media_tool;
mod rpc;
pub mod session_db;
pub mod session_memory;
pub mod session_stats;
pub mod session_types;
pub mod steam_achievements;
mod plugin_test;
pub mod steam_art;
pub mod steam_library;
pub mod steam_local_stats;
mod steam_owned_games;
pub mod steam_tool;
pub mod steam_top_games;
pub mod test_support;
pub mod truco_deck;
pub mod truco_round;
pub mod vdf;

use crate::config::store::ConfigStore;
use crate::native::NativeModule;
use std::sync::{Arc, Mutex, OnceLock};

use session_memory::SessionMemory;

/// Slot de processo do `Arc<SessionMemory>` real — `Mutex<Option<_>>`, não um `OnceLock<Arc<_>>`
/// direto, porque testes de `brain::persona::gaming_context` precisam TROCAR o valor entre casos
/// (um `OnceLock` só aceita `set` uma vez por processo, e `cargo test` roda tudo no mesmo binário).
/// Escrito uma única vez em produção, por [`register_session_tools`], logo depois que a instância
/// REAL é construída em [`game_watcher::start`] — pull-based, sem depender de
/// `EventKey::GameChanged`/`GameStopped` (que não existem hoje, ver o cabeçalho de `game_watcher.rs`).
static SESSION_MEMORY: OnceLock<Mutex<Option<Arc<SessionMemory>>>> = OnceLock::new();

fn session_memory_slot() -> &'static Mutex<Option<Arc<SessionMemory>>> {
    SESSION_MEMORY.get_or_init(|| Mutex::new(None))
}

/// Chamado uma vez em produção (ver [`register_session_tools`]); testes de outros módulos podem
/// chamar de novo para trocar o cenário — serializem contra [`SESSION_MEMORY_TEST_ORDER`].
pub fn set_session_memory(memory: Arc<SessionMemory>) {
    *session_memory_slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(memory);
}

/// Leitura pull-based do jogo ativo, para quem monta o prompt (`brain::persona::gaming_context`).
/// `None` antes do primeiro `set_session_memory` (nunca acontece em produção: o boot chama
/// [`register_session_tools`] antes de qualquer turno) ou em teste que não registrou nada.
pub fn session_memory() -> Option<Arc<SessionMemory>> {
    session_memory_slot().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Serializa testes de OUTROS módulos (`brain::persona::gaming_context`) que escrevem em
/// [`SESSION_MEMORY`] — mesmo padrão de `features::mood::TOOL_REGISTRATION_ORDER`.
#[cfg(test)]
pub(crate) static SESSION_MEMORY_TEST_ORDER: Mutex<()> = Mutex::new(());

/// Ponto de extensão descrito em `modules/tools.rs`: registra as tools deste módulo no
/// `ToolRegistry` de processo sem editar aquele arquivo. `modules()` roda uma vez no boot
/// (`ShellState::new()` → `native::registry()` → `crate::modules::all()`), então isto não
/// reexecuta por invoke nem duplica entradas.
///
/// Só as tools CONFIG-LIVRES cabem aqui — `game_session`/`game_knowledge` precisam de
/// `Arc<SessionMemory>`, e `steam` (desde 2026-09-22d) de `Arc<ConfigStore>` para
/// `action=achievements`; nenhum dos dois existe antes do `AppHandle` (ver
/// [`register_session_tools`]).
fn register_tools() {
    let registry = crate::modules::tools::registry();
    registry.register(mood_tracker::definition(), Arc::new(mood_tracker::MoodTrackerHandler));
    registry.register(play_minigame_tool::definition(), Arc::new(play_minigame_tool::PlayMinigameHandler));
}

/// Segunda passada de registro deste módulo — chamada por [`game_watcher::start`] logo depois que
/// `SessionMemory` é construída, no MESMO ponto que já tem `AppHandle` (`register_config_bound_tools`
/// em `modules/mod.rs`, arquivo de outro dono nesta rodada — por isso a chamada mora aqui, e não lá,
/// nem uma linha a mais naquele arquivo compartilhado por três frentes).
///
/// `recommend_media` entrou aqui, e não em [`register_tools`], porque precisa de
/// `Arc<ConfigStore>` (persona/preferências do prompt e o cliente Ollama de
/// `turn::run_turn_ollama_raw`) — inexistente na primeira passada, sem `AppHandle`. Desde
/// 2026-09-22c, `game_session`/`game_knowledge` também recebem o mesmo `Arc<ConfigStore>` — suas
/// ações `describe`/`ask` precisam dele para a mesma chamada de LLM. Desde 2026-09-22d, `steam`
/// também — `action=achievements` precisa da mesma credencial para chamar a API Web da Steam.
pub(crate) fn register_session_tools(memory: Arc<SessionMemory>, config: Arc<ConfigStore>) {
    set_session_memory(Arc::clone(&memory));

    let registry = crate::modules::tools::registry();
    registry.register(
        game_session_tool::definition(),
        Arc::new(game_session_tool::GameSessionHandler::new(Arc::clone(&memory), Arc::clone(&config))),
    );
    registry.register(
        game_knowledge_tool::definition(),
        Arc::new(game_knowledge_tool::GameKnowledgeHandler::new(Arc::clone(&memory), Arc::clone(&config))),
    );
    registry.register(
        recommend_media_tool::definition(),
        Arc::new(recommend_media_tool::RecommendMediaHandler::new(Arc::clone(&config))),
    );
    registry.register(steam_tool::definition(), Arc::new(steam_tool::SteamToolHandler::new(Arc::clone(&config))));

    // O teste de conexão de `mika:plugin-test` — mesmo `Arc<ConfigStore>` que `steam_tool`/`rpc`
    // já recebem, lido fresco a cada chamada (ver o cabeçalho de `plugin_test.rs`).
    plugin_test::register(Arc::clone(&config));

    // Mesmo ponto: os 7 métodos de `mika:plugin-rpc` (`rpc::register`) precisam do MESMO
    // `Arc<SessionMemory>`/`Arc<ConfigStore>` que as tools acima já recebem.
    rpc::register(memory, config);
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    register_tools();
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sonda negativa preservada da versão anterior deste arquivo (2026-09-21): se algum dia
    /// `entertainment:play-sound` virar um invoke real, este teste vermelho é o lembrete para
    /// revisar a decisão, não para reimplementar em silêncio. Hoje ele é um EVENTO
    /// backend→renderer (`DESKTOP_BRIDGED_EVENT_CHANNELS.PlaySound`), sintetizado no renderer via
    /// `AudioContext`/`OscillatorNode` — sem arquivo, sem `rodio`/`cpal`.
    #[test]
    fn play_sound_is_not_an_invoke_channel_today() {
        let contract =
            include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        let invoke_block = contract
            .split("DESKTOP_INVOKE_CHANNELS")
            .nth(1)
            .and_then(|rest| rest.split("as const").next())
            .expect("bloco DESKTOP_INVOKE_CHANNELS não encontrado — leitura do contrato quebrada");
        assert!(
            !invoke_block.contains("entertainment:play-sound"),
            "'entertainment:play-sound' virou um invoke channel — reveja o relato no topo de gamer/mod.rs, \
             ele documentava esta ausência"
        );
    }

    #[test]
    fn no_invoke_channels_claimed_yet() {
        assert!(modules().is_empty());
    }

    /// A view de plugin só executa tool cujo `plugin_id` é igual à identidade tirada do label da
    /// janela (`tools::access`). As tools diziam `"gamer-companion"` e a janela `mika.gamer-companion`:
    /// a Biblioteca Steam recebia "plugin can only execute its own tools" e nunca carregava.
    #[test]
    fn plugin_view_identity_owns_every_gamer_tool() {
        let label = crate::windows::plugin_view_label(feature::GAMER_COMPANION_PLUGIN_ID, "gamer-companion.steam-library")
            .expect("label válido");
        let caller = crate::windows::plugin_id_from_label(&label).expect("identidade do label");
        for def in [
            steam_tool::definition(),
            game_session_tool::definition(),
            game_knowledge_tool::definition(),
            recommend_media_tool::definition(),
            play_minigame_tool::definition(),
            mood_tracker::definition(),
        ] {
            assert_eq!(def.plugin_id, caller, "tool `{}` não pertence à view do próprio plugin", def.name);
        }
    }

    #[test]
    fn registers_its_config_free_tools_into_the_shared_process_wide_registry() {
        modules();
        let all = crate::modules::tools::registry().get_all();
        assert!(all.iter().any(|t| t.name == "track_gamer_mood" && t.plugin_id == feature::GAMER_COMPANION_PLUGIN_ID));
        assert!(all.iter().any(|t| t.name == "play_minigame" && t.plugin_id == feature::GAMER_COMPANION_PLUGIN_ID));
    }

    /// Prova que `register_session_tools` (chamada por `game_watcher::start` em produção, fora deste
    /// arquivo) de fato cadastra as quatro tools desta segunda passada — as duas que dependem de
    /// `SessionMemory`, e `recommend_media`/`steam`, que dependem só de `ConfigStore`. `steam`
    /// saiu de [`register_tools`] nesta rodada (2026-09-22d) — `action=achievements` precisa da
    /// mesma credencial que as outras três. Só asserção POSITIVA — `tools::registry()` é
    /// `OnceLock` de processo e `cargo test` roda em paralelo.
    ///
    /// Toma `rpc::TEST_LOCK`: desde que `register_session_tools` também registra os 7 métodos de
    /// `mika:plugin-rpc` (`rpc::register`, adicionado nesta rodada), este teste PRECISA cooperar com
    /// a mesma trava que `rpc_tests.rs` usa — senão sobrescreveria o `SessionMemory`/`ConfigStore`
    /// que um teste de RPC concorrente acabou de registrar, no meio do `dispatch` dele.
    #[test]
    fn register_session_tools_registers_game_session_game_knowledge_recommend_media_and_steam() {
        let _rpc_order = rpc::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let memory = std::sync::Arc::new(session_memory::SessionMemory::new(session_db::SessionDb::open_in_memory().unwrap()));
        let dir = test_support::ScratchDir::new("register-session-tools");
        let store = std::sync::Arc::new(ConfigStore::open(dir.path(), crate::config::schemas::registry()).unwrap());
        register_session_tools(memory, store);
        let all = crate::modules::tools::registry().get_all();
        assert!(all.iter().any(|t| t.name == "game_session" && t.plugin_id == feature::GAMER_COMPANION_PLUGIN_ID));
        assert!(all.iter().any(|t| t.name == "game_knowledge" && t.plugin_id == feature::GAMER_COMPANION_PLUGIN_ID));
        assert!(all.iter().any(|t| t.name == "recommend_media" && t.plugin_id == feature::GAMER_COMPANION_PLUGIN_ID));
        assert!(all.iter().any(|t| t.name == "steam" && t.plugin_id == feature::GAMER_COMPANION_PLUGIN_ID));
    }
}
