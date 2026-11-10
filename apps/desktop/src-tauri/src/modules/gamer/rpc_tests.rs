//! Testes de `super::rpc` — todos via `rpc::dispatch` (não o handler direto), provando o despacho
//! REAL: guarda de identidade, roteamento por `pluginId`/`method`, e o embrulho `{success,data}`/
//! `{success:false,error}` que só `dispatch` aplica.
//!
//! `TEST_LOCK` (ver `super::TEST_LOCK`) é obrigatório em TODO teste que chama [`register`] — ele
//! sobrescreve chaves globais do registro de processo, e `cargo test` roda em paralelo.

use super::*;
use crate::modules::gamer::session_db::SessionDb;
use crate::modules::gamer::test_support::ScratchDir;
use crate::protocol::WindowType;
use std::path::Path;

fn store(dir: &Path) -> Arc<ConfigStore> {
    Arc::new(ConfigStore::open(dir, crate::config::schemas::registry()).unwrap())
}

fn memory() -> Arc<SessionMemory> {
    Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap()))
}

async fn call(method: &str, args: Value) -> Value {
    crate::modules::plugins::rpc::dispatch(
        WindowType::Main,
        None,
        &json!({ "pluginId": "mika.gamer-companion", "method": method, "args": args }),
    )
    .await
}

#[tokio::test]
async fn session_current_is_null_without_an_active_session() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-session-current-empty");
    register(memory(), store(scratch.path()));

    let result = call("session-current", Value::Null).await;
    assert_eq!(result["success"], json!(true));
    assert_eq!(result["data"], Value::Null);
}

#[tokio::test]
async fn session_list_and_current_report_iso_time_and_duration_like_the_renderer_expects() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-session-list");
    let mem = memory();
    mem.start_session("570", "Dota 2").unwrap();
    register(Arc::clone(&mem), store(scratch.path()));

    let current = call("session-current", Value::Null).await;
    let start_time = current["data"]["startTime"].as_str().expect("startTime deveria ser string ISO");
    assert!(start_time.ends_with('Z'), "esperava ISO 8601, veio: {start_time}");
    assert!(current["data"].get("durationMs").is_none(), "sessão em andamento não tem durationMs");

    mem.end_session().unwrap();
    let list = call("session-list", json!({ "limit": 5 })).await;
    let sessions = list["data"].as_array().expect("data deveria ser array");
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0]["endTime"].as_str().unwrap().ends_with('Z'));
    assert!(sessions[0]["durationMs"].as_u64().is_some());
}

#[tokio::test]
async fn steam_game_art_rejects_an_invalid_app_id_before_touching_disk() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-game-art-invalid");
    register(memory(), store(scratch.path()));

    let result = call("steam:game-art", json!({ "appId": "../etc" })).await;
    assert_eq!(result["success"], json!(false));
    assert_eq!(result["error"]["message"], json!("appId inválido"));
}

#[tokio::test]
async fn steam_game_art_reports_null_art_when_no_steam_root_resolves() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-game-art-no-root");
    let config = store(&scratch.path().join("config"));
    config.set("gamer-companion.steamPath", json!("Z:\\does-not-exist")).unwrap();
    register(memory(), config);

    let result = call("steam:game-art", json!({ "appId": "730" })).await;
    assert_eq!(result, json!({ "success": true, "data": { "art": null } }));
}

#[tokio::test]
async fn steam_top_games_reports_not_found_when_no_steam_root_resolves() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-top-games-no-root");
    let config = store(&scratch.path().join("config"));
    config.set("gamer-companion.steamPath", json!("Z:\\does-not-exist")).unwrap();
    register(memory(), config);

    let result = call("steam:top-games", Value::Null).await;
    assert_eq!(result["data"]["steamFound"], json!(false));
    assert_eq!(result["data"]["games"], json!([]));
}

#[tokio::test]
async fn mini_game_payload_rejects_an_unknown_game_by_name() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-minigame-unknown");
    register(memory(), store(scratch.path()));

    let result = call("mini-game:payload", json!({ "game": "chess" })).await;
    assert_eq!(result["success"], json!(false));
    assert_eq!(result["error"]["message"], json!("Unknown mini game: chess"));

    let missing = call("mini-game:payload", Value::Null).await;
    assert_eq!(missing["error"]["message"], json!("Unknown mini game: (missing)"));
}

#[tokio::test]
async fn mini_game_payload_builds_a_known_game_without_wrapping_twice() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-minigame-known");
    register(memory(), store(scratch.path()));

    let result = call("mini-game:payload", json!({ "game": "jokenpo" })).await;
    assert_eq!(result["success"], json!(true));
    assert_eq!(result["data"]["game"], json!("jokenpo"));
    assert!(result["data"]["html"].as_str().unwrap().contains("Pedra, Papel ou Tesoura"));
}

/// Prova a fila via a mesma trilha que a tool `play_minigame` usa — precisa das DUAS travas: o
/// registro (`TEST_LOCK`) e a fila em si (`play_minigame_tool::QUEUE_TEST_LOCK`).
#[tokio::test]
async fn mini_game_requested_returns_null_when_the_queue_is_empty() {
    let _registry_order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _queue_order = super::super::play_minigame_tool::QUEUE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    while next_requested().is_some() {}

    let scratch = ScratchDir::new("rpc-minigame-requested-empty");
    register(memory(), store(scratch.path()));

    let result = call("mini-game:requested", Value::Null).await;
    assert_eq!(result["data"]["game"], Value::Null);
}

#[tokio::test]
async fn mini_game_result_never_fails_for_an_unlabeled_pair() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-minigame-result");
    register(memory(), store(scratch.path()));

    let pong_win = call("mini-game:result", json!({ "game": "pong", "result": "win" })).await;
    assert_eq!(pong_win["success"], json!(true));

    let draw = call("mini-game:result", json!({ "game": "jokenpo", "result": "draw" })).await;
    assert_eq!(draw["success"], json!(true));

    let labeled = call("mini-game:result", json!({ "game": "jokenpo", "result": "win" })).await;
    assert_eq!(labeled["success"], json!(true), "par válido também responde sucesso, mesmo sem publicar o evento");
}

/// Positivo contra o barramento GLOBAL do processo — mesma técnica de
/// `vision::observation_events::tests` (filtra por `EventKey::GameMoment`, ignora eventos de outro
/// teste concorrente no mesmo processo). Prova que `mini-game:result` publica de verdade, não só
/// responde `success`.
#[tokio::test]
async fn mini_game_result_publishes_game_moment_for_a_labeled_pair() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-minigame-result-moment");
    register(memory(), store(scratch.path()));

    let mut sub = crate::events::bus().subscribe();
    call("mini-game:result", json!({ "game": "jokenpo", "result": "win" })).await;

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match tokio::time::timeout_at(deadline, sub.recv()).await {
            Ok(Some(envelope)) if envelope.key == crate::events::EventKey::GameMoment => {
                assert_eq!(envelope.payload["game"], json!("Pedra, Papel ou Tesoura"));
                assert_eq!(envelope.payload["type"], json!("victory"));
                assert!(envelope.payload["description"].as_str().unwrap().contains("venceu"));
                return;
            }
            Ok(Some(_)) => continue,
            _ => panic!("GameMoment não publicado dentro do prazo"),
        }
    }
}

/// Espelha `mini_game_result_never_fails_for_an_unlabeled_pair`: um par sem rótulo/tipo (pong, ou
/// `draw`) não deveria publicar `GameMoment` nenhum.
#[tokio::test]
async fn mini_game_result_publishes_nothing_for_an_unlabeled_pair() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-minigame-result-no-moment");
    register(memory(), store(scratch.path()));

    let mut sub = crate::events::bus().subscribe();
    call("mini-game:result", json!({ "game": "pong", "result": "win" })).await;
    // Publica outro evento rastreável depois, só para ter um marco de "chegou até aqui" sem esperar
    // um timeout inteiro por ausência.
    crate::events::bus().publish(crate::events::EventKey::GameStopped, json!({ "marker": "rpc-minigame-result-no-moment" }));

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match tokio::time::timeout_at(deadline, sub.recv()).await {
            Ok(Some(envelope)) if envelope.key == crate::events::EventKey::GameMoment => {
                panic!("GameMoment não deveria ser publicado para um par sem rótulo/tipo (pong/win)");
            }
            Ok(Some(envelope))
                if envelope.key == crate::events::EventKey::GameStopped
                    && envelope.payload["marker"] == json!("rpc-minigame-result-no-moment") =>
            {
                return;
            }
            Ok(Some(_)) => continue,
            _ => panic!("marco não chegou dentro do prazo"),
        }
    }
}

/// O card "Agora jogando" formata no idioma da tela: os números saem crus (ms/dias), somando só o
/// que é de hoje/da semana, e o top 3 vem da semana.
#[test]
fn session_stats_are_raw_numbers_for_today_week_and_streak() {
    use crate::modules::gamer::session_types::GameSession;
    const DAY: u64 = 86_400_000;
    let session = |game: &str, start: u64, minutes: u64| GameSession {
        id: "s".into(),
        game_id: "g".into(),
        game_name: game.into(),
        start_time_ms: start,
        end_time_ms: Some(start + minutes * 60_000),
        events: vec![],
        summary: None,
    };
    let now = 10 * DAY + 20 * 3_600_000;
    let history = vec![session("Hades", 10 * DAY, 60), session("Dota 2", 9 * DAY, 30), session("Hades", 2 * DAY, 90)];
    let stats = session_stats_json(&history, now, &chrono::Utc);
    assert_eq!(stats["todayMs"], json!(3_600_000));
    assert_eq!(stats["weekMs"], json!(5_400_000));
    assert_eq!(stats["currentStreakDays"], json!(2));
    assert_eq!(stats["topGames"][0], json!({ "game": "Hades", "ms": 3_600_000 }));
}

#[tokio::test]
async fn session_stats_and_steam_root_are_served_over_plugin_rpc() {
    let _order = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = ScratchDir::new("rpc-session-stats");
    register(memory(), store(scratch.path()));

    let stats = call("session-stats", Value::Null).await;
    assert_eq!(stats["success"], json!(true));
    assert_eq!(stats["data"]["weekMs"], json!(0));
    let root = call("steam:root", Value::Null).await;
    assert_eq!(root["success"], json!(true));
    assert!(root["data"].get("detected").is_some());
}
