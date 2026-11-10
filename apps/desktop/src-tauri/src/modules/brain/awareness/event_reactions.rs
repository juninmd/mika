//! Reações a eventos pontuais do barramento — port do recorte de `awareness-event-handlers.ts` que
//! agora TEM fonte Rust real: `EventKey::GameChanged`/`GameStopped` (`game_watcher.rs`),
//! `EventKey::ThemeChanged` (`windows.rs`), `EventKey::PowerResume`/`PowerBattery`
//! (`modules/system_signals/power.rs`) e, desde 2026-09-22, `EventKey::ActiveWindowChanged`
//! (`modules/system_signals/active_window.rs`). terminal-error segue sem publicador Rust; hardware
//! alert mora em `hardware_alert.rs` — ver o cabeçalho de `mod.rs`.
//!
//! Cada função aqui só monta o `ProactiveCandidate` a partir do payload e entrega a
//! `ProactiveEngine::request`, que aplica a MESMA cadência do tick — nenhuma fala sai fora dela.

use serde_json::Value;

use super::engine::ProactiveEngine;
use super::prompts;
use super::types::ProactiveCandidate;

/// Port de `registerHandler(Events.GAME_CHANGED, ...)`. Só a reação de "comentar o jogo que começou"
/// é portada (`key: game:{game}`, priority 9, `awareness/steam-game.md`) — a chamada irmã
/// `learnGameDetails` (wiki via LLM/browser, mesma chave, priority 8) e a checagem `initial` (que
/// suprime a fala na primeira detecção após o boot) NÃO são portadas: sem elas, o gate de
/// `initial: true` ainda é respeitado aqui, mas a variante de prioridade 8 nunca compete.
pub async fn on_game_changed(engine: &ProactiveEngine, payload: &Value, now_ms: u64) {
    let initial = payload.get("initial").and_then(Value::as_bool).unwrap_or(false);
    if initial {
        return;
    }
    let Some(game) = payload.get("game").and_then(Value::as_str).filter(|g| !g.is_empty()) else { return };

    // `previousSessionSummary` vem de `game_watcher.rs::apply` (`SessionMemory::session_for_game`
    // consultado ANTES de abrir a nova sessão) — presente só quando este mesmo jogo já tinha uma
    // sessão anterior encerrada. Puxa a sessão de volta à conversa em vez de deixá-la só no banco.
    let recall_ctx = payload
        .get("previousSessionSummary")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(|summary| format!(" Ele ja jogou isso antes; da ultima vez: {summary}"))
        .unwrap_or_default();

    let prompt = match prompts::steam_game_reaction(game, &recall_ctx) {
        Ok(p) => p,
        Err(error) => {
            log::error!("reação proativa 'steam game' sem prompt: {error}");
            return;
        }
    };

    engine
        .request(
            ProactiveCandidate { key: format!("game:{game}"), priority: 9, label: "steam game".to_string(), prompt },
            now_ms,
        )
        .await;
}

/// Port de `registerHandler(Events.GAME_STOPPED, ...)`: no TS, este handler não fala nada — só
/// contabilidade (`setLastPlayedGame`/`setLastGameStoppedAt`) que alimenta o cooldown de
/// `awayFromGamesProvider`. Essa contabilidade já existe do lado Rust em
/// `GameAbsenceProvider` (lê `end_time_ms` da última sessão direto do banco, ver `providers.rs`), e
/// não em estado mutável do engine — nada a portar aqui além de documentar a paridade.
pub fn on_game_stopped() {}

/// Port de `registerHandler(Events.THEME_CHANGED, ...)`: `key: "system:theme"`, priority 4,
/// double-wrap `awareness/system-theme-change.md` (var `mode`) dentro de `awareness/
/// system-generic-event.md` (var `content`) — mesma composição de `systemEvent()` no TS.
pub async fn on_theme_changed(engine: &ProactiveEngine, payload: &Value, now_ms: u64) {
    let is_dark = payload.get("isDark").and_then(Value::as_bool).unwrap_or(false);
    let mode = if is_dark { "dark" } else { "light" };

    let content = match prompts::system_theme_change(mode) {
        Ok(c) => c,
        Err(error) => {
            log::error!("reação proativa 'tema do sistema' sem prompt (system-theme-change): {error}");
            return;
        }
    };
    let prompt = match prompts::system_generic_event(&content) {
        Ok(p) => p,
        Err(error) => {
            log::error!("reação proativa 'tema do sistema' sem prompt (system-generic-event): {error}");
            return;
        }
    };

    engine
        .request(
            ProactiveCandidate { key: "system:theme".to_string(), priority: 4, label: "tema do sistema mudou".to_string(), prompt },
            now_ms,
        )
        .await;
}

/// Port de `registerHandler(Events.POWER_RESUME, ...)`: `key: "system:wake-up"`, priority 8,
/// mesmo double-wrap de `on_theme_changed` (`awareness/system-wake-up.md` dentro de
/// `awareness/system-generic-event.md`). Publicador real: `modules/system_signals/power.rs`.
pub async fn on_power_resume(engine: &ProactiveEngine, now_ms: u64) {
    let content = match prompts::system_wake_up() {
        Ok(c) => c,
        Err(error) => {
            log::error!("reação proativa 'sistema acordou' sem prompt (system-wake-up): {error}");
            return;
        }
    };
    let prompt = match prompts::system_generic_event(&content) {
        Ok(p) => p,
        Err(error) => {
            log::error!("reação proativa 'sistema acordou' sem prompt (system-generic-event): {error}");
            return;
        }
    };

    engine
        .request(
            ProactiveCandidate {
                key: "system:wake-up".to_string(),
                priority: 8,
                label: "sistema acordou".to_string(),
                prompt,
            },
            now_ms,
        )
        .await;
}

/// Port de `registerHandler(Events.POWER_BATTERY, ...)`: `key: "system:battery"`, priority 8,
/// mesma composição de `on_power_resume`. Publicador real: `modules/system_signals/power.rs`.
pub async fn on_power_battery(engine: &ProactiveEngine, now_ms: u64) {
    let content = match prompts::system_on_battery() {
        Ok(c) => c,
        Err(error) => {
            log::error!("reação proativa 'saiu da tomada' sem prompt (system-battery): {error}");
            return;
        }
    };
    let prompt = match prompts::system_generic_event(&content) {
        Ok(p) => p,
        Err(error) => {
            log::error!("reação proativa 'saiu da tomada' sem prompt (system-generic-event): {error}");
            return;
        }
    };

    engine
        .request(
            ProactiveCandidate {
                key: "system:battery".to_string(),
                priority: 8,
                label: "saiu da tomada".to_string(),
                prompt,
            },
            now_ms,
        )
        .await;
}

/// Port de `registerHandler(Events.OS_ACTIVE_WINDOW_CHANGED, ...)`: `key: "window:change"`,
/// priority 6, `awareness/window-changed.md` (vars `title`/`owner`). Publicador real:
/// `modules/system_signals/active_window.rs`. O throttle extra de 2.5min que o TS tinha
/// (`lastWindowComment`) não é portado — a cadência genérica por chave que `should_speak` já aplica
/// (mesma que `game:{game}`/`system:theme` usam) cobre o mesmo papel, sem estado duplicado.
pub async fn on_window_changed(engine: &ProactiveEngine, payload: &Value, now_ms: u64) {
    let title = payload.get("title").and_then(Value::as_str).unwrap_or("");
    let owner = payload.get("owner").and_then(Value::as_str).unwrap_or("");
    if title.is_empty() && owner.is_empty() {
        return;
    }

    let prompt = match prompts::window_changed(title, owner) {
        Ok(p) => p,
        Err(error) => {
            log::error!("reação proativa 'janela ativa mudou' sem prompt (window-changed): {error}");
            return;
        }
    };

    engine
        .request(
            ProactiveCandidate { key: "window:change".to_string(), priority: 6, label: "janela ativa mudou".to_string(), prompt },
            now_ms,
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::brain::awareness::engine::ProactiveSender;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct RecordingSender {
        prompts: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl ProactiveSender for RecordingSender {
        async fn send(&self, prompt: &str) -> String {
            self.prompts.lock().unwrap().push(prompt.to_string());
            "falei".to_string()
        }
    }

    fn engine(sender: Arc<RecordingSender>) -> ProactiveEngine {
        struct ArcSender(Arc<RecordingSender>);
        #[async_trait]
        impl ProactiveSender for ArcSender {
            async fn send(&self, prompt: &str) -> String {
                self.0.send(prompt).await
            }
        }
        ProactiveEngine::new(
            vec![],
            Box::new(ArcSender(sender)),
            false,
            super::super::constants::PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
        )
    }

    #[tokio::test]
    async fn game_changed_speaks_with_key_and_priority_matching_ts() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_game_changed(&e, &serde_json::json!({ "game": "Elden Ring", "initial": false }), 10_000_000).await;
        assert_eq!(sender.prompts.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn game_changed_is_silent_on_the_initial_detection() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_game_changed(&e, &serde_json::json!({ "game": "Elden Ring", "initial": true }), 10_000_000).await;
        assert_eq!(sender.prompts.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn game_changed_is_silent_without_a_game_name() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_game_changed(&e, &serde_json::json!({ "initial": false }), 10_000_000).await;
        assert_eq!(sender.prompts.lock().unwrap().len(), 0);
    }

    /// SHOULD do spec (08-gamer-companion.md, "Sessão"): reabrir o mesmo jogo puxa a sessão anterior
    /// de volta à conversa em vez de deixá-la só no banco.
    #[tokio::test]
    async fn game_changed_mentions_the_previous_session_summary_when_present() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_game_changed(
            &e,
            &serde_json::json!({ "game": "Hades", "initial": false, "previousSessionSummary": "Session: 40min, 3 events (death, victory)." }),
            10_000_000,
        )
        .await;
        let prompts = sender.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].contains("Session: 40min, 3 events (death, victory)."), "prompt: {}", prompts[0]);
    }

    #[tokio::test]
    async fn theme_changed_speaks_the_double_wrapped_prompt() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_theme_changed(&e, &serde_json::json!({ "isDark": true }), 10_000_000).await;
        assert_eq!(sender.prompts.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn power_resume_speaks_the_wake_up_reaction() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_power_resume(&e, 10_000_000).await;
        assert_eq!(sender.prompts.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn power_battery_speaks_the_unplugged_reaction() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_power_battery(&e, 10_000_000).await;
        assert_eq!(sender.prompts.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn window_changed_speaks_with_key_and_priority_matching_ts() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_window_changed(&e, &serde_json::json!({ "title": "index.ts", "owner": "code.exe" }), 10_000_000).await;
        assert_eq!(sender.prompts.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn window_changed_is_silent_without_a_title_or_owner() {
        let sender = Arc::new(RecordingSender { prompts: Mutex::new(Vec::new()) });
        let e = engine(sender.clone());
        on_window_changed(&e, &serde_json::json!({}), 10_000_000).await;
        assert_eq!(sender.prompts.lock().unwrap().len(), 0);
    }
}
