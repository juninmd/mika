//! Motor proativo — port de `services/brain/awareness/{proactive-*.ts,awareness-idle-provider.ts,
//! awareness-game-provider.ts}`: a Mika puxa assunto sozinha (ocioso, jogo em andamento, ausência de
//! jogo, observação de tela), com gates de cadência (`gates.rs`) e reserva-antes-de-falar
//! (`engine.rs::speak`) para uma fala que falha não custar o cooldown de uma que deu certo.
//!
//! **Fora do escopo desta rodada** (nomeado, não escondido):
//! - `awareness-game.ts` (aprendizado de wiki via LLM/browser/memória vetorial) — o provider de jogo
//!   aqui comenta a sessão sem esse contexto extra.
//! - `awareness-vision.ts` inteiro (dedup por hash de análise, `startGamingLoop` a partir da visão,
//!   reconhecimento de pessoa) — só o campo `analysis`/`foregroundApp` de `vision:screen-observation`
//!   é consumido (`screen_provider.rs`).
//! - `awareness-event-handlers.ts` — de 7 reações auditadas, 5 agora têm publicador real e são
//!   portadas em `event_reactions.rs`: `EventKey::GameChanged` (steam-game, `game:{game}` prioridade
//!   9), `EventKey::ThemeChanged` (`system:theme` prioridade 4, double-wrap),
//!   `EventKey::PowerResume`/`PowerBattery` (`system:wake-up`/`system:battery`, prioridade 8 cada —
//!   publicador `modules/system_signals/power.rs`, um poller de 15s que revive parte da lacuna
//!   nomeada em `power.rs`), e desde 2026-09-22 `EventKey::ActiveWindowChanged` (`window:change`,
//!   prioridade 6 — publicador `modules/system_signals/active_window.rs`, que fechou a lacuna de
//!   dependência que este cabeçalho documentava: a crate `windows` com
//!   `Win32_UI_WindowsAndMessaging` já resolvia transitivamente via `sysinfo`/`xcap`, então entrar
//!   como dependência direta não ampliou a árvore). `EventKey::GameStopped` existe mas o handler TS
//!   correspondente não fala nada (só contabilidade, já coberta do lado Rust por
//!   `GameAbsenceProvider` lendo o banco direto — ver `event_reactions.rs::on_game_stopped`). As 2
//!   restantes (terminal-error; hardware alert ganhou publicador em 2026-09-27, `hardware_alert.rs`) continuavam SEM publicador Rust alcançável: nem
//!   `terminal:error` nem `hardware:alert` têm um EMISSOR sequer do lado TypeScript (grep em
//!   `apps/desktop/src` fora de `__tests__/` não acha nenhum — são handlers órfãos já no Electron).
//! - `Events.USER_MESSAGE` não existe em `events/keys.rs`; `IdleProvider` mede ociosidade via
//!   `crate::power::snapshot()` em vez disso — ver o cabeçalho de `providers.rs`.
//! - `isGameTense` — PORTADO desde 2026-09-22 (`tension.rs`, `GameTensionState`), alimentado por
//!   `EventKey::GameMoment` (hoje só publicado por `gamer::rpc::MiniGameResult`) e checado em
//!   `ProactiveEngine::should_speak` antes de qualquer outro gate, mesma ordem do TS.
//!   `ProviderSuspension` (o motor pausa quando um provedor de LLM de visão recusa de vez) continua
//!   sem equivalente Rust — é um estado do `vision::watcher`, não deste módulo.
//!
//! Horário silencioso usa `chrono::Local` (hora LOCAL de verdade, `character.quietHoursStart`/`End`
//! comparados contra `Local::now().hour()`) — `chrono` entrou como dependência DIRETA nesta rodada
//! (já estava presente transitivamente, mesma versão fixada — ver `Cargo.toml`), corrigindo o corte
//! anterior (hora UTC, que em UTC-3 deslocava a janela silenciosa em 3h).

pub mod bedtime;
pub mod body_need;
pub mod constants;
pub mod engine;
pub mod event_reactions;
pub mod gates;
pub mod hardware_alert;
pub mod interruption;
pub mod moves;
pub mod person_detected;
pub mod prompts;
pub mod providers;
pub mod screen_provider;
pub mod tension;
pub mod tick;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use chrono::Timelike;
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};

use engine::{ProactiveEngine, ProactiveSender};
use providers::{GameAbsenceProvider, GameProvider, IdleProvider};
use screen_provider::ScreenProvider;
use tension::GameTensionState;
use types::ProactiveProvider;

use crate::modules::gamer::gaming_state::GamingState;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Roda `future` fora do laço do assinante — todo chamador abaixo termina, cedo ou tarde, num
/// `ProactiveEngine::request`/`speak`, que pode ficar até `PROACTIVE_TIMEOUT_SECS` (180s) esperando
/// um turno de LLM real (`ChatSender::send`). `.await`ado INLINE no `match` do laço principal, isso
/// travaria `sub.recv()` pelo mesmo tempo — `broadcast` descarta para quem não drena a tempo
/// (`events/bus.rs`), então um único candidato proativo em voo faria este assinante perder
/// `ThinkingStart`/`GameMoment`/etc. publicados enquanto isso. `ProactiveEngine`/`CommentaryState`
/// já garantem não sobrepor duas falas (guardas atômicos/mutex internos, ver `engine.rs::speak` e o
/// cabeçalho de `gamer::game_moment_commentary`), então disparar várias destas tasks concorrentemente
/// é seguro — mesma técnica de `gamer::game_learning::spawn`.
fn spawn_reaction<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tauri::async_runtime::spawn(future);
}

/// The user's wall clock (`user.timezone`, else the OS zone), re-read on every call — the same
/// clock the prompt's time block uses (`persona::user_clock`). Quiet hours are the user's night.
fn user_clock_now(store: &ConfigStore) -> chrono::NaiveDateTime {
    crate::modules::brain::persona::user_clock::user_now(store).local
}

fn user_hour_of_day(store: &ConfigStore) -> u32 {
    user_clock_now(store).hour()
}

fn nickname(store: &ConfigStore) -> String {
    store
        .get("user.nickname")
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
        .or_else(|| store.get("user.name").and_then(|v| v.as_str().map(str::to_string)))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "senpai".to_string())
}

fn quiet_hours(store: &ConfigStore) -> (u32, u32) {
    let start = store.get("character.quietHoursStart").and_then(|v| v.as_f64()).unwrap_or(23.0) as u32;
    let end = store.get("character.quietHoursEnd").and_then(|v| v.as_f64()).unwrap_or(7.0) as u32;
    (start, end)
}

fn proactiveness(store: &ConfigStore) -> f64 {
    store.get("character.proactiveness").and_then(|v| v.as_f64()).unwrap_or(50.0)
}

/// `gamer-companion.momentCooldowns[type]` (segundos) — mesmo override de `game-tension.ts`, que
/// deixa o usuário alargar/encurtar a janela de tensão por tipo de momento sem tocar em código.
fn moment_cooldown_override_ms(store: &ConfigStore, moment_type: &str) -> Option<u64> {
    store
        .get("gamer-companion.momentCooldowns")
        .and_then(|v| v.get(moment_type).and_then(|n| n.as_u64()))
        .map(|seconds| seconds * 1_000)
}

/// Sender de produção — delega a `modules::chat::proactive::send_awareness`, o turno real.
struct ChatSender {
    app: AppHandle,
    store: Arc<ConfigStore>,
}

#[async_trait]
impl ProactiveSender for ChatSender {
    async fn send(&self, prompt: &str) -> String {
        super::super::chat::proactive::send_awareness(&self.app, &self.store, prompt).await
    }

    /// Só `on_interruption` chama isto (ver o trait) — registra o abort sob `window_label` para que
    /// um SEGUNDO barge-in na mesma janela aborte esta resposta (defects.md "barge-in", item 2).
    async fn send_for_window(&self, prompt: &str, window_label: &str, barge_in_at_ms: Option<u64>) -> String {
        super::super::chat::proactive::send_interruption_reply(&self.app, &self.store, prompt, window_label, barge_in_at_ms).await
    }
}

/// Sobe o motor proativo inteiro: assina o barramento (ANTES de `spawn`, ver `AGENTS.md`/o gate de
/// `modules/mod.rs::nothing_the_boot_calls_uses_bare_tokio_spawn`), registra os providers e dispara o
/// laço de tick com `tauri::async_runtime::spawn` — nunca `tokio::spawn` (este código é chamado de
/// `lib.rs::setup`, fora do reator).
///
/// Chamador de produção: `modules/mod.rs::register_config_bound_tools`
/// (`brain::awareness::spawn_proactive_engine(app, Arc::clone(&store), Arc::clone(&game_tension),
/// Arc::clone(&gaming_state))`).
///
/// Devolve o `Arc<ProactiveEngine>` (desde 2026-09-22, para `brain::thought::spawn_thought_service`
/// injetar o MESMO motor — cadência/dedup vivem no estado dele; um segundo `ProactiveEngine` para
/// `EventKey::BrainProactiveThought` duplicaria esse estado em vez de compartilhá-lo). Chamadores
/// que só querem o efeito colateral (como o `modules/mod.rs` de hoje) seguem livres para descartar
/// o retorno.
/// `tension`: MESMA instância que `vision::spawn_screen_watcher` recebe — criada uma vez em
/// `modules/mod.rs` (ver `events/keys.rs`/o cabeçalho de `vision::watcher` para o motivo: o bug
/// original que este relógio único existe para nunca reintroduzir era exatamente DOIS `tenseUntil`
/// dessincronizados entre visão e motor proativo).
/// `gaming`: MESMA razão de `tension` — UMA instância de processo, criada em `modules/mod.rs`,
/// atualizada pelo assinante de `GameChanged`/`GameStopped` logo abaixo e consultada pelo motor para
/// reduzir proatividade fora do jogo (specs/08-gamer-companion.md, "Estado de comportamento").
pub fn spawn_proactive_engine(
    app: &AppHandle,
    store: Arc<ConfigStore>,
    tension: Arc<GameTensionState>,
    gaming: Arc<GamingState>,
) -> Arc<ProactiveEngine> {
    let gamer_db_path: Option<PathBuf> = crate::modules::gamer::session_db::db_path(app).ok();

    // `Ok` vira `Some(&'static MemoryStore)` para `GameProvider` ler `wikiCtx` de volta
    // (`game_learning::wiki_context_for`); `Err` degrada para `None` (wikiCtx vazio), nunca panica —
    // mesma política de "melhor esforço" de `memory_store` em si.
    let memory = crate::modules::memory::memory_store(app).ok();

    let bedtime_quiet = {
        let store = Arc::clone(&store);
        Box::new(move || quiet_hours(&store))
    };
    let user_clock = |store: &Arc<ConfigStore>| {
        let store = Arc::clone(store);
        Box::new(move || user_clock_now(&store))
    };
    let mut providers: Vec<Box<dyn ProactiveProvider>> = vec![
        Box::new(IdleProvider::new(nickname(&store), user_clock(&store))),
        Box::new(bedtime::BedtimeProvider::new(nickname(&store), bedtime_quiet, user_clock(&store))),
        Box::new(body_need::BodyNeedProvider::new(nickname(&store), Arc::clone(&store))),
    ];
    if let Some(path) = &gamer_db_path {
        match GameProvider::open(path, memory, Arc::clone(&gaming)) {
            Ok(p) => providers.push(Box::new(p)),
            Err(error) => log::error!("provider proativo 'game' não abriu {}: {error}", path.display()),
        }
        match GameAbsenceProvider::open(path, nickname(&store)) {
            Ok(p) => providers.push(Box::new(p)),
            Err(error) => log::error!("provider proativo 'game-absence' não abriu {}: {error}", path.display()),
        }
    } else {
        log::error!("providers proativos de jogo desligados: não resolveu o caminho de gamer-sessions.db");
    }
    let screen = Arc::new(ScreenProvider::new());
    providers.push(Box::new(ScreenProviderHandle(Arc::clone(&screen))));

    // `game-moment-commentary.ts` port: própria máquina de cooldown por tipo, MESMA instância de
    // tensão que o gate acima consulta — ver o cabeçalho de `gamer::game_moment_commentary`.
    let commentary_state = Arc::new(crate::modules::gamer::game_moment_commentary::CommentaryState::new());
    let commentary_generator = Arc::new(crate::modules::gamer::game_moment_commentary::LiveCommentGenerator::new(Arc::clone(&store)));
    // SHOULD da spec ("Silêncio é feature"): aprende os momentos em que foi mandada calar e antecipa
    // — ver o cabeçalho de `gamer::mute_learning`.
    let mute_learning = Arc::new(crate::modules::gamer::mute_learning::MuteLearning::new());

    let sender = Box::new(ChatSender { app: app.clone(), store: Arc::clone(&store) });
    let engine = Arc::new(ProactiveEngine::with_user_speech_gate(
        providers,
        sender,
        cfg!(any(test, feature = "e2e")) && std::env::var("MIKA_E2E").as_deref() == Ok("1"),
        constants::PROACTIVE_BASE_INTERVAL_MS,
        {
            let store = Arc::clone(&store);
            Box::new(move || quiet_hours(&store))
        },
        {
            let store = Arc::clone(&store);
            Box::new(move || proactiveness(&store))
        },
        {
            let store = Arc::clone(&store);
            Box::new(move || user_hour_of_day(&store))
        },
        {
            let tension = Arc::clone(&tension);
            Box::new(move || tension.is_tense(now_ms()))
        },
        {
            let gaming = Arc::clone(&gaming);
            Box::new(move || gaming.is_active())
        },
        // defects.md "barge-in", item 3: nunca começar uma fala proativa em cima do usuário — mesmo
        // tracker que `voice::vad_audio` alimenta a cada janela do VAD, Mika falando ou não.
        Box::new(|| crate::modules::voice::user_speech_activity::global().should_defer_proactive_speech(now_ms())),
    ));

    // Assina ANTES do spawn — `broadcast` não entrega retroativamente (mesmo aviso de
    // `voice::tts::bus_bridge::spawn_force_stop_listener`).
    let mut sub = bus().subscribe();
    let engine_for_bus = Arc::clone(&engine);
    let tension_for_bus = Arc::clone(&tension);
    let gaming_for_bus = Arc::clone(&gaming);
    let store_for_bus = Arc::clone(&store);
    let commentary_state_for_bus = Arc::clone(&commentary_state);
    let commentary_generator_for_bus = Arc::clone(&commentary_generator);
    let mute_learning_for_bus = Arc::clone(&mute_learning);
    let app_for_bus = app.clone();
    let person_cooldown = Arc::new(person_detected::PersonCooldown::default());
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            match envelope.key {
                // A proactive turn's stop must not clear a user turn still in flight.
                EventKey::ThinkingStart | EventKey::ThinkingStop if crate::modules::brain::post_turn::is_background_turn(&envelope.payload) => {}
                EventKey::ThinkingStart => engine_for_bus.on_thinking_start(),
                EventKey::ThinkingStop => engine_for_bus.on_thinking_stop(),
                // Reacts now, like TS's withProactive did; waiting for the 2-minute tick made
                // every screen reaction stale. Same should_speak cadence as the tick.
                EventKey::ScreenObservation => {
                    screen.offer(&envelope.payload);
                    let now = now_ms();
                    if let Some(candidate) = screen.due(now) {
                        let engine = Arc::clone(&engine_for_bus);
                        spawn_reaction(async move { engine.request(candidate, now).await });
                    }
                    // Waits on enrichment for the bio, so it runs apart from the bus loop.
                    let (app, engine, cooldown) = (app_for_bus.clone(), Arc::clone(&engine_for_bus), Arc::clone(&person_cooldown));
                    let payload = envelope.payload.clone();
                    spawn_reaction(async move { person_detected::react(&app, &engine, &cooldown, &payload, now).await });
                }
                EventKey::Interruption => {
                    let engine = Arc::clone(&engine_for_bus);
                    let payload = envelope.payload.clone();
                    spawn_reaction(async move { engine.on_interruption(&payload).await });
                }
                EventKey::GameChanged => {
                    // MUST da spec ("Estado de comportamento"): entrar em gaming reinicia os timers
                    // dependentes — só quando a transição é REAL (ociosa->jogando), nunca a cada
                    // troca de jogo enquanto já estava jogando (`set_active` só devolve `true` na
                    // borda, ver o cabeçalho de `gaming_state.rs`).
                    if gaming_for_bus.set_active(true) {
                        engine_for_bus.on_gaming_transition();
                    }
                    let (engine, payload, now) = (Arc::clone(&engine_for_bus), envelope.payload.clone(), now_ms());
                    spawn_reaction(async move { event_reactions::on_game_changed(&engine, &payload, now).await });
                }
                EventKey::GameStopped => {
                    if gaming_for_bus.set_active(false) {
                        engine_for_bus.on_gaming_transition();
                    }
                    event_reactions::on_game_stopped()
                }
                EventKey::ThemeChanged => {
                    let (engine, payload, now) = (Arc::clone(&engine_for_bus), envelope.payload.clone(), now_ms());
                    spawn_reaction(async move { event_reactions::on_theme_changed(&engine, &payload, now).await });
                }
                EventKey::PowerResume => {
                    let (engine, now) = (Arc::clone(&engine_for_bus), now_ms());
                    spawn_reaction(async move { event_reactions::on_power_resume(&engine, now).await });
                }
                EventKey::PowerBattery => {
                    let (engine, now) = (Arc::clone(&engine_for_bus), now_ms());
                    spawn_reaction(async move { event_reactions::on_power_battery(&engine, now).await });
                }
                EventKey::HardwareAlert => {
                    let (engine, payload, now) = (Arc::clone(&engine_for_bus), envelope.payload.clone(), now_ms());
                    spawn_reaction(async move { hardware_alert::on_hardware_alert(&engine, &payload, now).await });
                }
                EventKey::ActiveWindowChanged => {
                    let (engine, payload, now) = (Arc::clone(&engine_for_bus), envelope.payload.clone(), now_ms());
                    spawn_reaction(async move { event_reactions::on_window_changed(&engine, &payload, now).await });
                }
                EventKey::GameMoment => {
                    let moment_type = envelope.payload.get("type").and_then(serde_json::Value::as_str).unwrap_or("");
                    let tense_flag = envelope.payload.get("tense").and_then(serde_json::Value::as_bool).unwrap_or(false);
                    let override_ms = moment_cooldown_override_ms(&store_for_bus, moment_type);
                    tension_for_bus.apply_moment(moment_type, tense_flag, override_ms, now_ms());
                    // Segundo efeito colateral do MESMO evento (ver o cabeçalho de
                    // `features::character::slang_context`): só grava `(game, at)`, nunca reage.
                    crate::modules::features::character::slang_context::on_game_moment(&envelope.payload);
                    // Terceiro: memória episódica (`memory/experience.ts::storeGameMoment`).
                    crate::modules::memory::experience::store_game_moment(&envelope.payload);
                    // Quarto: `MuteLearning` (SHOULD, "Silêncio é feature") — ANTES do comentário, já
                    // que a antecipação pode publicar um `MuteSpeech` que o próprio comentário deste
                    // momento deve respeitar (ver o cabeçalho de `gamer::mute_learning`).
                    if mute_learning_for_bus.observe_moment(moment_type, now_ms()) {
                        crate::modules::gamer::mute_learning::anticipate_mute(now_ms());
                    }
                    // Quinto: comentário de "backseat gaming" (`game-moment-commentary.ts`) — publica
                    // o próprio `EventKey::SpeechRequest` quando os gates deixam (chama o LLM, ver o
                    // cabeçalho de `gamer::game_moment_commentary`) — task própria pela mesma razão
                    // das demais reações acima; `CommentaryState` já reserva o slot por tipo ANTES do
                    // `await` (mesma técnica de `engine.rs::speak`), então disparar concorrentemente
                    // é seguro.
                    let engine = Arc::clone(&engine_for_bus);
                    let (commentary_state, store, tension, gaming_for_commentary, generator, payload, now) = (
                        Arc::clone(&commentary_state_for_bus),
                        Arc::clone(&store_for_bus),
                        Arc::clone(&tension_for_bus),
                        Arc::clone(&gaming_for_bus),
                        Arc::clone(&commentary_generator_for_bus),
                        envelope.payload.clone(),
                        now_ms(),
                    );
                    spawn_reaction(async move {
                        let spoke = crate::modules::gamer::game_moment_commentary::maybe_comment_on_game_moment(
                            &commentary_state,
                            &store,
                            &tension,
                            &gaming_for_commentary,
                            &*generator,
                            &payload,
                            now,
                        )
                        .await;
                        // Conta para o intervalo mínimo do motor: sem isto, um comentário de morte e
                        // uma fala proativa sobre outro assunto saíam colados.
                        if spoke {
                            engine.note_spoken_elsewhere(now_ms());
                        }
                    });
                }
                EventKey::MuteSpeech => {
                    // SHOULD ("Silêncio é feature"): só um mute de VERDADE (`muted: true`, pedido
                    // pelo usuário) credita o aprendizado — nem um `unmute`, nem o próprio mute que
                    // `MuteLearning` antecipou (`source: "mute_learning"`, ver o cabeçalho daquele
                    // arquivo para o loop que isso evitaria).
                    let muted = envelope.payload.get("muted").and_then(serde_json::Value::as_bool).unwrap_or(false);
                    let from_learning = envelope.payload.get("source").and_then(serde_json::Value::as_str) == Some("mute_learning");
                    if muted && !from_learning {
                        mute_learning_for_bus.observe_mute(now_ms());
                    }
                }
                _ => {}
            }
        }
    });

    let engine_for_tick = Arc::clone(&engine);
    tauri::async_runtime::spawn(async move {
        let base = Duration::from_millis(constants::PROACTIVE_BASE_INTERVAL_MS);
        let mut shutdown_rx = crate::shutdown::subscribe();
        // Tick primeiro (o `interval` de antes disparava no instante 0), depois dorme o intervalo
        // escalado pelo governador de energia: ocioso/jogando, o motor acorda menos vezes.
        loop {
            engine_for_tick.tick(now_ms()).await;
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = tokio::time::sleep(crate::modules::system::governor::background(base)) => {}
            }
        }
    });

    engine
}

/// Adapta `Arc<ScreenProvider>` a `ProactiveProvider` sem expor o `Arc` na lista de providers do
/// motor — o assinante do barramento guarda o próprio `Arc` clonado para escrever (`offer`).
struct ScreenProviderHandle(Arc<ScreenProvider>);

impl ProactiveProvider for ScreenProviderHandle {
    fn id(&self) -> &'static str {
        self.0.id()
    }
    fn due(&self, now_ms: u64) -> Option<types::ProactiveCandidate> {
        self.0.due(now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quiet hours and bedtime are the USER's night, not the machine's: a PC on UTC (or a traveller
    /// who set `user.timezone`) made "23h-7h" silence the wrong hours. UTC+14 and UTC-12 are 26 h
    /// apart, so the same instant reads 2 h apart on the clock — `chrono::Local` reads them equal.
    #[test]
    fn the_quiet_hours_clock_follows_the_users_timezone() {
        let dir = std::env::temp_dir().join(format!("mika-awareness-user-clock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        store.set("user.timezone", serde_json::json!("UTC+14")).unwrap();
        let east = user_hour_of_day(&store);
        store.set("user.timezone", serde_json::json!("UTC-12")).unwrap();
        let west = user_hour_of_day(&store);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!((east + 24 - west) % 24, 2, "east={east} west={west}");
    }

    /// O ponto central desta rodada: `spawn_reaction` (usado por CADA braço do `match` que termina
    /// em `ProactiveEngine::request`/`on_interruption`) nunca faz o chamador esperar `future`
    /// terminar — só dispara a task. Mesma prova de
    /// `gamer::game_learning::a_slow_game_changed_handler_does_not_block_the_next_bus_receive`, aqui
    /// para o helper que este módulo usa em 7 braços do laço principal.
    #[tokio::test]
    async fn spawn_reaction_never_blocks_its_caller_even_with_a_pending_future() {
        let started = std::time::Instant::now();
        spawn_reaction(std::future::pending::<()>());
        // Waiting on `pending()` never returns, so any finite bound catches it; 50 ms flaked under a loaded suite.
        assert!(started.elapsed() < std::time::Duration::from_secs(1), "spawn_reaction não pode esperar o future terminar");
    }
}
