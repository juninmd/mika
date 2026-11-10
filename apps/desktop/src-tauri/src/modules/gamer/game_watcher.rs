//! `SteamGameWatcher` — porte de `services/steam-game-watcher.ts`. Inclui o ramo de janela em foco
//! (`findForegroundGame`), lido de `system_signals::active_window::last_known()` (Windows, Linux/X11
//! e macOS), com fallback multiplataforma por PROCESSO em execução via
//! `sysinfo` quando não há janela conhecida. A precedência exclusiva entre os dois ramos e, desde
//! 2026-09-22, a graça de `handleNoGame` (a sessão sobrevive a um alt-tab rápido enquanto o
//! processo do último jogo detectado continua rodando) estão implementadas em
//! [`super::game_watcher_decision`], que guarda toda a lógica pura testável.
//!
//! **Sessão direto, evento para o resto.** [`GameWatcher::apply`] abre/fecha a sessão chamando
//! `SessionMemory` DIRETO (o listener separado do TS, `game-event-subscriptions.ts`, só existia para
//! isso) e TAMBÉM publica `EventKey::GameChanged`/`GameStopped` para quem reage ao jogo.
//!
//! **Por que não há risco do bug "assinou dentro do `tokio::spawn`".** Esse bug é sobre assinar o
//! barramento de eventos; este watcher não assina bus nenhum — segura `Arc<SessionMemory>`/
//! `Arc<ConfigStore>` desde a CONSTRUÇÃO, antes do `tokio::spawn` em [`spawn_watcher`].

use crate::config::schemas::llm_defaults;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::config::store::ConfigStore;

use super::game_detection::{self, ActiveGameInfo};
use super::game_watcher_decision::{decide_transition, detect_active_game, Source, Transition};
use super::session_memory::SessionMemory;
use super::session_types::GameSession;
use super::steam_library;

const DEFAULT_INTERVAL_SECONDS: u64 = 30;
const MIN_INTERVAL_SECONDS: u64 = 10;
/// A biblioteca Steam (um `appmanifest_*.acf` por jogo instalado, em cada biblioteca) era relida do
/// disco a cada passada. Jogo instalado agora aparece em até este tempo.
const STEAM_LIBRARY_TTL: Duration = Duration::from_secs(5 * 60);
/// O default compartilhado (`llm_defaults`), só para rotular `token_usage::record` com o modelo real
/// quando nenhum foi configurado.
const OLLAMA_DEFAULT_MODEL: &str = llm_defaults::OLLAMA_MODEL;

/// Mesma ordem de `resolve_ollama_config` (`brain/turn.rs`, privado): `llm.ollamaModel`, depois
/// `llm.model`, depois o default — só para o rótulo de `token_usage::record`, nunca para resolver o
/// provedor de verdade (isso é `run_turn_ollama_raw` que já faz, dentro de `summarize_with_llm`).
fn resolved_ollama_model(store: &ConfigStore) -> String {
    let get_str = |path: &str| store.get(path).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty());
    get_str("llm.ollamaModel").or_else(|| get_str("llm.model")).unwrap_or_else(|| OLLAMA_DEFAULT_MODEL.to_string())
}

pub struct GameWatcher {
    session_memory: Arc<SessionMemory>,
    config: Arc<ConfigStore>,
    /// Só para `token_usage::record` em [`Self::spawn_llm_summary`] — o resumo de sessão é o único
    /// turno de LLM real que este módulo dispara, e `token_usage::record` pede `&AppHandle` (mesmo
    /// motivo de `chat/send.rs`/`vision/watcher.rs`: `telemetry::token_usage_db(app)` resolve o
    /// caminho do SQLite na primeira chamada do processo). `Option` (não `AppHandle` puro) pelo mesmo
    /// motivo de `vision::watcher::run_tick` — `AppHandle<Wry>` (produção) e `AppHandle<MockRuntime>`
    /// (`tauri::test::mock_app()`) não compilam juntos, então os testes deste arquivo constroem
    /// `GameWatcher` sem app nenhum e nunca gravam uso de token, sem precisar montar um app Tauri
    /// real para testar o resto do watcher.
    app: Option<AppHandle>,
    last_game_name: Mutex<Option<String>>,
    /// Porte de `lastGameProcess` — alimenta a graça de `handleNoGame` em [`Self::apply`].
    last_game_process: Mutex<Option<String>>,
    /// `AtomicBool` (não `Mutex`) porque só precisa impedir reentrância do laço, nunca é lido em
    /// conjunto com outro campo — mesmo uso do `ticking` boolean simples do TS.
    ticking: AtomicBool,
    /// `(steamPath configurado, lido em, jogos)` — ver [`STEAM_LIBRARY_TTL`].
    steam_cache: Mutex<Option<(Option<String>, Instant, Vec<steam_library::SteamGame>)>>,
}

impl GameWatcher {
    pub fn new(session_memory: Arc<SessionMemory>, config: Arc<ConfigStore>, app: Option<AppHandle>) -> Self {
        Self {
            session_memory,
            config,
            app,
            last_game_name: Mutex::new(None),
            last_game_process: Mutex::new(None),
            ticking: AtomicBool::new(false),
            steam_cache: Mutex::new(None),
        }
    }

    /// `gamer-companion.enabled` (toggle mestre da feature) E `gameWatcherEnabled` (o sub-toggle
    /// próprio deste watcher) — o mestre desligado tinha de parar o watcher inteiro e não parava
    /// (varredura de controles de UI de 2026-09-27, `feature::master_enabled` é a mesma leitura
    /// que `game_moment_commentary`/as tools de sessão agora fazem).
    fn is_enabled(&self) -> bool {
        super::feature::master_enabled(&self.config)
            && self.config.get("gamer-companion.gameWatcherEnabled").and_then(|v| v.as_bool()).unwrap_or(true)
    }

    /// `gamer-companion.sessionSummaryEnabled` — "Mika comenta um resumo da sessão quando o jogo
    /// fecha" (i18n). Antes desta rodada [`Self::tick`] sempre disparava
    /// [`Self::spawn_llm_summary`] (chamada de LLM real + episódio de memória de longo prazo)
    /// mesmo com o campo desligado; o `fallback_summary` síncrono de `end_session` continua
    /// intacto — ele alimenta `previousSessionSummary` (recall ao reabrir o MESMO jogo), um
    /// comportamento de continuidade de sessão diferente do "comentário" que este campo controla.
    fn session_summary_enabled(&self) -> bool {
        self.config.get("gamer-companion.sessionSummaryEnabled").and_then(|v| v.as_bool()).unwrap_or(true)
    }

    fn interval_seconds(&self) -> u64 {
        let configured = self.config.get("gamer-companion.gameWatcherIntervalSeconds").and_then(|v| v.as_u64());
        configured.unwrap_or(DEFAULT_INTERVAL_SECONDS).max(MIN_INTERVAL_SECONDS)
    }

    fn steam_path(&self) -> Option<String> {
        self.config
            .get("gamer-companion.steamPath")
            .and_then(|v| v.as_str().map(str::to_string))
            .filter(|s| !s.trim().is_empty())
    }

    fn steam_games(&self) -> Vec<steam_library::SteamGame> {
        let path = self.steam_path();
        let mut cache = self.steam_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cached_path, at, games)) = cache.as_ref() {
            if *cached_path == path && at.elapsed() < STEAM_LIBRARY_TTL {
                return games.clone();
            }
        }
        let games = steam_library::list_steam_games(path.as_deref(), true);
        *cache = Some((path, Instant::now(), games.clone()));
        games
    }

    /// Aplica uma leitura de "jogo ativo agora" ao estado da sessão — separado de [`Self::tick`]
    /// para ser testável sem SO/Steam real. `is_last_process_running` vai direto para
    /// [`decide_transition`], que só a chama quando a graça de `handleNoGame` pode se aplicar.
    ///
    /// Devolve a sessão que acabou de ser ENCERRADA (por `Stopped` ou por um `Started` que trocou de
    /// jogo sem passar por `Stopped`), se houver — [`Self::tick`] usa isto para disparar o resumo por
    /// LLM em segundo plano (`Self::spawn_llm_summary`), nunca daqui: `apply` continua síncrono e
    /// testável sem runtime async (`tauri::async_runtime::spawn` dentro dele panicaria nos testes
    /// abaixo, que chamam `apply` direto, sem reator).
    fn apply(
        &self,
        active: Option<&ActiveGameInfo>,
        active_process_key: Option<&str>,
        source: Source,
        is_last_process_running: impl FnOnce() -> bool,
    ) -> Option<GameSession> {
        let last_name = self.last_game_name.lock().ok().and_then(|guard| guard.clone());
        let last_process = self.last_game_process.lock().ok().and_then(|guard| guard.clone());
        let transition =
            decide_transition(active, active_process_key, source, last_name.as_deref(), last_process.as_deref(), is_last_process_running);
        match transition {
            Transition::None => None,
            Transition::Started { game_id, game_name, game_process } => {
                if let Ok(mut guard) = self.last_game_name.lock() {
                    *guard = Some(game_name.clone());
                }
                if let Ok(mut guard) = self.last_game_process.lock() {
                    *guard = Some(game_process);
                }
                // SHOULD (specs/08-gamer-companion.md, "Sessão"): puxa a sessão anterior DO MESMO
                // jogo antes de abrir a nova — consultado ANTES de `start_session` fechar a corrente,
                // então uma sessão anterior deste mesmo `game_id` (se houver) ainda é a mais recente.
                let recall_summary = self
                    .session_memory
                    .session_for_game(&game_id)
                    .ok()
                    .flatten()
                    .and_then(|previous| previous.summary);
                // `start_session` encerra a sessão corrente sozinho, mas descarta o valor devolvido —
                // encerra aqui primeiro para CAPTURAR a sessão que estava rodando quando o jogo trocou
                // sem passar por `Stopped` (mesmo caso do teste
                // `apply_switches_sessions_when_the_game_changes_without_an_intermediate_stop`), e
                // repassar para o resumo por LLM em segundo plano.
                let ended = match self.session_memory.end_session() {
                    Ok(session) => session,
                    Err(error) => {
                        log::warn!("gamer: falha ao encerrar a sessão anterior antes de trocar de jogo: {error}");
                        None
                    }
                };
                if let Err(error) = self.session_memory.start_session(&game_id, &game_name) {
                    log::warn!("gamer: falha ao iniciar sessão para {game_name}: {error}");
                }
                crate::events::bus().publish(
                    crate::events::EventKey::GameChanged,
                    serde_json::json!({
                        "game": game_name, "appId": game_id, "source": "watcher", "initial": false,
                        "previousSessionSummary": recall_summary,
                    }),
                );
                ended
            }
            Transition::Stopped => {
                if let Ok(mut guard) = self.last_game_name.lock() {
                    *guard = None;
                }
                if let Ok(mut guard) = self.last_game_process.lock() {
                    *guard = None;
                }
                let ended = match self.session_memory.end_session() {
                    Ok(session) => session,
                    Err(error) => {
                        log::warn!("gamer: falha ao encerrar sessão: {error}");
                        None
                    }
                };
                crate::events::bus().publish(
                    crate::events::EventKey::GameStopped,
                    serde_json::json!({ "game": last_name.clone().unwrap_or_default(), "source": "watcher" }),
                );
                ended
            }
        }
    }

    /// Regenera o resumo em segundo plano depois que `apply()` já persistiu `fallback_summary` — o
    /// LLM nunca atrasa o encerramento da sessão nem o tick. Chamado só a partir de [`Self::tick`],
    /// que roda dentro do laço async de [`spawn_watcher`] (reator garantido); nunca a partir de
    /// `apply()` em teste, que não tem runtime.
    fn spawn_llm_summary(&self, session: GameSession) {
        let session_memory = Arc::clone(&self.session_memory);
        let config = Arc::clone(&self.config);
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let (_handle, signal) = crate::modules::brain::turn::TurnAbortHandle::new();
            let llm_result = super::session_memory::summarize_with_llm(&config, &session, signal).await;
            // `token_usage::record` (specs/10-observability.md, "Custo"): este é o único turno de
            // LLM real que este módulo dispara — antes desta rodada ficava invisível ao painel de
            // custo mesmo gastando tokens de verdade. Grava ANTES de decidir o texto final: o gasto
            // aconteceu independente do resumo vir vazio ou não.
            if let (Ok((_, usage)), Some(app)) = (&llm_result, &app) {
                let model = resolved_ollama_model(&config);
                crate::modules::token_usage::record(app, "gamer-summary", "ollama", &model, usage, Some(0.0));
            }
            let text_result = llm_result.map(|(summary, _)| summary);
            if let Ok(summary) = &text_result {
                if !summary.is_empty() {
                    if let Err(error) = session_memory.set_summary(&session.id, summary) {
                        log::warn!("gamer: falha ao salvar resumo por LLM da sessão {}: {error}", session.id);
                    }
                } else {
                    log::debug!("gamer: resumo por LLM da sessão {} veio vazio, mantendo o fallback", session.id);
                }
            } else if let Err(error) = &text_result {
                log::warn!("gamer: resumo por LLM falhou para a sessão {}, mantendo o fallback: {error}", session.id);
            }
            // 08-gamer-companion.md §"Sessão", MUST: o resumo vira episódio de longo prazo — sempre
            // o resumo FINAL desta sessão, seja o do LLM (quando deu certo) ou o `fallback_summary`
            // que `end_session` já persistiu (quando não deu), nunca as duas entradas.
            let fallback = session.summary.clone().unwrap_or_default();
            let final_summary = final_session_summary(text_result, &fallback);
            crate::modules::memory::experience::store_game_session_summary(&session.game_name, &final_summary);
        });
    }

    /// Uma passada real: lê a janela ativa, a biblioteca Steam do disco e, só se necessário (ver
    /// [`super::game_watcher_decision`]), os processos em execução — e aplica a transição.
    /// `ticking` evita uma segunda passada sobrepor a primeira se a leitura demorar mais que o
    /// intervalo configurado.
    pub fn tick(&self) {
        if !self.is_enabled() {
            return;
        }
        if self.ticking.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ticking = TickingGuard(&self.ticking);
        let games = self.steam_games();
        let window = crate::modules::system_signals::active_window::last_known();
        let custom = game_detection::custom_games(self.config.get("gamer-companion.customGames").as_ref());
        let (active, source, process_key) = detect_active_game(window.as_ref(), &games, &custom, game_detection::list_process_names);
        // Preguiça: `list_process_names()` só roda se a graça chegar a checar.
        let ended = self.apply(active.as_ref(), process_key.as_deref(), source, || {
            self.last_game_process
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
                .is_some_and(|p| game_detection::list_process_names().contains(&p))
        });
        if let Some(session) = ended {
            if self.session_summary_enabled() {
                self.spawn_llm_summary(session);
            }
        }
    }
}

/// Solta `ticking` quando a passada sai — por qualquer caminho, inclusive panic. Sem isto, um panic
/// numa sonda do SO deixava `ticking` em `true` para sempre e o watcher nunca mais passava.
struct TickingGuard<'a>(&'a AtomicBool);

impl Drop for TickingGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Núcleo puro de [`GameWatcher::spawn_llm_summary`] — extraído para ser testável sem runtime
/// async: o resumo do LLM quando não-vazio, senão o `fallback_summary` que `end_session` já tinha
/// persistido. É este texto, e só ele, que vira episódio em
/// [`crate::modules::memory::experience::store_game_session_summary`] (08-gamer-companion.md
/// §"Sessão", MUST).
fn final_session_summary(llm_result: Result<String, String>, fallback: &str) -> String {
    match llm_result {
        Ok(summary) if !summary.is_empty() => summary,
        _ => fallback.to_string(),
    }
}

/// Chame UMA VEZ no boot (`lib.rs::setup`, depois que `AppHandle`/`ConfigStore` existem). Devolve
/// o `JoinHandle`; descartável com `let _ =` se o chamador não precisar abortar no shutdown.
pub fn spawn_watcher(
    session_memory: Arc<SessionMemory>,
    config: Arc<ConfigStore>,
    app: AppHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    let watcher = Arc::new(GameWatcher::new(session_memory, config, Some(app)));
    // `tauri::async_runtime::spawn`, NUNCA `tokio::spawn`: [`start`] é chamada de
    // `modules::register_config_bound_tools`, e essa roda em `lib.rs::setup` — FORA do contexto do
    // reator. `tokio::spawn` ali panica com "there is no reactor running" e o app morre em exit 101
    // antes de abrir janela, com a suíte inteira verde (`#[tokio::test]` fornece o runtime que o
    // boot real não tem). Ver o gate `nothing_the_boot_calls_uses_bare_tokio_spawn`.
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            // Varredura de processos + disco: fora do reator, que também resolve `mika_invoke`.
            let ticking = Arc::clone(&watcher);
            if let Err(error) = tokio::task::spawn_blocking(move || ticking.tick()).await {
                log::error!("gamer: a passada do watcher terminou em panic ({error}); a próxima segue no intervalo normal");
            }
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = tokio::time::sleep(Duration::from_secs(watcher.interval_seconds())) => {}
            }
        }
    })
}

/// Assinatura exata para o boot ligar em uma linha: abre o SQLite de sessão, embrulha em
/// [`SessionMemory`], registra as tools `game_session`/`game_knowledge`
/// ([`super::register_session_tools`]) e spawna o watcher. Devolve os dois.
pub fn start(
    app: &AppHandle,
    config: Arc<ConfigStore>,
) -> Result<(Arc<SessionMemory>, tauri::async_runtime::JoinHandle<()>), String> {
    let path = super::session_db::db_path(app)?;
    let db = super::session_db::SessionDb::open(&path)?;
    let session_memory = Arc::new(SessionMemory::new(db));
    super::register_session_tools(Arc::clone(&session_memory), Arc::clone(&config));
    let handle = spawn_watcher(session_memory.clone(), config, app.clone());
    Ok((session_memory, handle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::{boolean, int_range, text};
    use crate::modules::gamer::session_db::SessionDb;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-gamer-watcher-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn config_store(enabled: bool) -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![
            Namespace {
                name: "gamer-companion",
                fields: vec![
                    boolean("enabled", true),
                    boolean("gameWatcherEnabled", enabled),
                    int_range("gameWatcherIntervalSeconds", 30, 10, 300),
                    text("steamPath", ""),
                    boolean("sessionSummaryEnabled", true),
                ],
            },
            // Só para `resolved_ollama_model_falls_back_to_the_same_default_as_chat` — o rótulo do
            // resumo de sessão em `token_usage::record` lê os mesmos dois campos que
            // `brain::turn::resolve_ollama_config` já lê para a conversa.
            Namespace { name: "llm", fields: vec![text("model", ""), text("ollamaModel", "")] },
        ]);
        (ConfigStore::open(&dir.0, schema).unwrap(), dir)
    }

    /// Devolve o `TempDir` junto — descartá-lo (como uma versão anterior deste helper fazia, mantendo
    /// só `_dir` local) apaga o diretório do `ConfigStore` do disco enquanto o `GameWatcher` devolvido
    /// ainda segura um `Arc<ConfigStore>` apontando pra ele; qualquer `.set()` posterior no config
    /// falhava com "caminho não encontrado" (Windows) — os testes que só liam config nunca pegavam
    /// isso, os que escrevem (abaixo) pegam.
    fn watcher(enabled: bool) -> (GameWatcher, TempDir) {
        let (store, dir) = config_store(enabled);
        let sm = Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap()));
        (GameWatcher::new(sm, Arc::new(store), None), dir)
    }

    /// Rótulo de `token_usage::record` (10-observability.md, "Custo") para o resumo de sessão: sem
    /// `llm.ollamaModel`/`llm.model` configurado, cai no MESMO default de
    /// `brain::turn::resolve_ollama_config` — os dois nunca podem divergir, senão o painel de custo
    /// rotula um modelo que a conversa nunca usou.
    #[test]
    fn resolved_ollama_model_falls_back_to_the_same_default_as_chat() {
        let (store, _dir) = config_store(true);
        assert_eq!(resolved_ollama_model(&store), OLLAMA_DEFAULT_MODEL);

        store.set("llm.model", serde_json::json!("mistral")).unwrap();
        assert_eq!(resolved_ollama_model(&store), "mistral");

        store.set("llm.ollamaModel", serde_json::json!("qwen3")).unwrap();
        assert_eq!(resolved_ollama_model(&store), "qwen3", "ollamaModel vence sobre o llm.model genérico");
    }

    /// Um panic no meio da passada não pode deixar `ticking` preso em `true`: o watcher nunca mais
    /// passaria (cada `tick` voltaria no `swap` logo na entrada).
    #[test]
    fn ticking_is_released_even_when_the_pass_panics() {
        let (w, _dir) = watcher(true);
        assert!(!w.ticking.swap(true, Ordering::SeqCst));

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ticking = TickingGuard(&w.ticking);
            panic!("sonda do SO quebrou");
        }));

        assert!(panicked.is_err());
        assert!(!w.ticking.load(Ordering::SeqCst), "o guard tem de soltar o flag no unwind");
    }

    fn game(name: &str, app_id: Option<&str>) -> ActiveGameInfo {
        ActiveGameInfo { name: name.to_string(), app_id: app_id.map(str::to_string) }
    }

    #[test]
    fn apply_starts_a_real_session_the_first_time_a_game_is_seen() {
        let (w, _dir) = watcher(true);
        w.apply(Some(&game("Hades", Some("1145360"))), Some("hades"), Source::ForegroundWindow, || false);
        let current = w.session_memory.current_session().expect("sessão deveria estar aberta");
        assert_eq!(current.game_name, "Hades");
        assert_eq!(current.game_id, "1145360");
    }

    #[test]
    fn apply_does_not_restart_the_session_while_the_same_game_keeps_running() {
        let (w, _dir) = watcher(true);
        w.apply(Some(&game("Hades", Some("1"))), Some("hades"), Source::ForegroundWindow, || false);
        let first_id = w.session_memory.current_session().unwrap().id;
        w.apply(Some(&game("Hades", Some("1"))), Some("hades"), Source::ForegroundWindow, || false);
        assert_eq!(w.session_memory.current_session().unwrap().id, first_id, "reabriu a mesma sessão");
    }

    #[test]
    fn apply_ends_the_session_when_the_game_stops() {
        let (w, _dir) = watcher(true);
        w.apply(Some(&game("Hades", Some("1"))), Some("hades"), Source::ForegroundWindow, || false);
        // Fonte process-watcher: a graça de handleNoGame não se aplica, sessão encerra de verdade.
        w.apply(None, None, Source::ProcessWatcher, || true);
        assert_eq!(w.session_memory.current_session(), None);
        assert_eq!(w.session_memory.history(10).unwrap()[0].game_name, "Hades");
    }

    /// A GRAÇA de `handleNoGame` na ponta real: fonte janela-em-foco sem match, mas o processo do
    /// último jogo detectado ainda está "rodando" (closure injetada) — a sessão continua aberta.
    #[test]
    fn apply_keeps_the_session_open_when_the_foreground_window_misses_but_the_last_process_still_runs() {
        let (w, _dir) = watcher(true);
        w.apply(Some(&game("Hades", Some("1145360"))), Some("hades"), Source::ForegroundWindow, || false);
        w.apply(None, None, Source::ForegroundWindow, || true);
        let current = w.session_memory.current_session().expect("a graça deveria manter a sessão aberta");
        assert_eq!(current.game_name, "Hades");
    }

    #[test]
    fn apply_switches_sessions_when_the_game_changes_without_an_intermediate_stop() {
        let (w, _dir) = watcher(true);
        w.apply(Some(&game("Hades", Some("1"))), Some("hades"), Source::ForegroundWindow, || false);
        w.apply(Some(&game("Dota 2", Some("570"))), Some("dota2"), Source::ForegroundWindow, || false);

        let current = w.session_memory.current_session().unwrap();
        assert_eq!(current.game_name, "Dota 2");
        let history = w.session_memory.history(10).unwrap();
        assert!(history.iter().any(|s| s.game_name == "Hades" && s.end_time_ms.is_some()));
    }

    #[test]
    fn apply_returns_the_ended_session_when_the_game_stops() {
        let (w, _dir) = watcher(true);
        w.apply(Some(&game("Hades", Some("1"))), Some("hades"), Source::ForegroundWindow, || false);
        let ended = w.apply(None, None, Source::ProcessWatcher, || true);
        assert_eq!(ended.map(|s| s.game_name), Some("Hades".to_string()), "tick usa isto para disparar o resumo por LLM");
    }

    #[test]
    fn apply_returns_the_ended_session_when_switching_games_without_an_intermediate_stop() {
        let (w, _dir) = watcher(true);
        w.apply(Some(&game("Hades", Some("1"))), Some("hades"), Source::ForegroundWindow, || false);
        let ended = w.apply(Some(&game("Dota 2", Some("570"))), Some("dota2"), Source::ForegroundWindow, || false);
        assert_eq!(ended.map(|s| s.game_name), Some("Hades".to_string()));
    }

    #[test]
    fn apply_returns_none_when_nothing_transitioned() {
        let (w, _dir) = watcher(true);
        assert_eq!(w.apply(None, None, Source::ProcessWatcher, || false), None);
    }

    /// SHOULD do spec (08-gamer-companion.md, "Sessão"): reabrir o MESMO jogo publica o resumo da
    /// sessão anterior no payload de `GameChanged`, para `event_reactions::on_game_changed` puxar a
    /// conversa de volta — ver `session_memory.rs::session_for_game`.
    #[tokio::test]
    async fn apply_publishes_the_previous_session_summary_when_the_same_game_reopens() {
        let (w, _dir) = watcher(true);
        w.apply(Some(&game("Hades", Some("1145360"))), Some("hades"), Source::ForegroundWindow, || false);
        // `end_session` grava o fallback primeiro (mesmo caminho síncrono de produção); só depois de
        // encerrada é que o resumo manual sobrescreve — mesma ordem de `spawn_llm_summary`.
        let ended = w.apply(None, None, Source::ProcessWatcher, || true).expect("sessão deveria ter encerrado");
        w.session_memory.set_summary(&ended.id, "Morreu pro primeiro chefe três vezes.").unwrap();

        // `bus()` é global e `cargo test` roda em paralelo (aviso no topo de `events/bus.rs`):
        // outros testes deste MESMO arquivo também publicam `GameChanged` para "Hades" — filtra pelo
        // resumo em si, uma string única desta chamada, não só pela chave do evento.
        let mut sub = crate::events::bus().subscribe();
        w.apply(Some(&game("Hades", Some("1145360"))), Some("hades"), Source::ForegroundWindow, || false);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        let envelope = loop {
            let received = tokio::time::timeout_at(deadline, sub.recv()).await.ok().flatten().expect("GameChanged não publicado a tempo");
            if received.key == crate::events::EventKey::GameChanged
                && received.payload.get("previousSessionSummary").and_then(serde_json::Value::as_str) == Some("Morreu pro primeiro chefe três vezes.")
            {
                break received;
            }
        };
        assert_eq!(envelope.payload["previousSessionSummary"], "Morreu pro primeiro chefe três vezes.");
    }

    #[test]
    fn tick_does_nothing_at_all_when_the_watcher_is_disabled() {
        let (w, _dir) = watcher(false);
        w.tick();
        assert_eq!(w.session_memory.current_session(), None);
        assert!(w.session_memory.history(10).unwrap().is_empty());
    }

    /// `gamer-companion.enabled=false` (toggle mestre) tem de parar o watcher mesmo com
    /// `gameWatcherEnabled=true` — antes desta rodada só o sub-toggle era lido (ver o cabeçalho de
    /// [`GameWatcher::is_enabled`]).
    #[test]
    fn tick_does_nothing_when_the_master_toggle_is_off_even_with_the_watcher_sub_toggle_on() {
        let (w, _dir) = watcher(true);
        w.config.set("gamer-companion.enabled", serde_json::json!(false)).unwrap();
        w.tick();
        assert_eq!(w.session_memory.current_session(), None);
        assert!(w.session_memory.history(10).unwrap().is_empty());
    }

    #[test]
    fn is_enabled_is_false_when_only_the_master_toggle_is_off() {
        let (w, _dir) = watcher(true);
        assert!(w.is_enabled());
        w.config.set("gamer-companion.enabled", serde_json::json!(false)).unwrap();
        assert!(!w.is_enabled());
    }

    /// `gamer-companion.sessionSummaryEnabled=false` — "Mika comenta um resumo da sessão quando o
    /// jogo fecha" (i18n) desligado. Antes desta rodada nada lia este campo (`tick` sempre
    /// disparava o resumo por LLM); a leitura em si é o comportamento observável testável sem
    /// runtime async (o gate de verdade em `tick` só soma uma chamada a este método já provado).
    #[test]
    fn session_summary_enabled_reads_the_config_field_live() {
        let (w, _dir) = watcher(true);
        assert!(w.session_summary_enabled(), "default é ligado, mesmo default do schema");
        w.config.set("gamer-companion.sessionSummaryEnabled", serde_json::json!(false)).unwrap();
        assert!(!w.session_summary_enabled());
    }

    #[test]
    fn interval_seconds_never_goes_below_the_floor_even_with_a_low_config_value() {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "gamer-companion",
            fields: vec![boolean("gameWatcherEnabled", true), int_range("gameWatcherIntervalSeconds", 1, 1, 300), text("steamPath", "")],
        }]);
        let store = ConfigStore::open(&dir.0, schema).unwrap();
        let sm = Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap()));
        let w = GameWatcher::new(sm, Arc::new(store), None);
        assert!(w.interval_seconds() >= MIN_INTERVAL_SECONDS);
    }

    // -- final_session_summary (08-gamer-companion.md §"Sessão", MUST: resumo vira episódio) --------

    #[test]
    fn final_session_summary_prefers_a_non_empty_llm_result() {
        assert_eq!(final_session_summary(Ok("resumo do LLM".to_string()), "fallback"), "resumo do LLM");
    }

    #[test]
    fn final_session_summary_falls_back_when_the_llm_result_is_empty() {
        assert_eq!(final_session_summary(Ok(String::new()), "fallback"), "fallback");
    }

    #[test]
    fn final_session_summary_falls_back_when_the_llm_call_failed() {
        assert_eq!(final_session_summary(Err("provedor fora".to_string()), "fallback"), "fallback");
    }
}
