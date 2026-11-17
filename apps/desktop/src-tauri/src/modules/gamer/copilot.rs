//! Modo co-piloto de jogo (`gamer-companion.copilotEnabled`, desligado por padrão): a visão
//! (`vision::watcher`, bloco `copilot` do `GAME_SIGNAL`) aponta um perigo ou um item que ficou para
//! trás, publica `EventKey::GameCopilotAlert`, e aqui se decide se vale interromper o jogo.
//!
//! **Sem atrapalhar** é o contrato: piso entre quaisquer dois avisos, cooldown por tipo, o MESMO
//! aviso (tipo + detalhe normalizado) nunca repete dentro de [`SAME_ALERT_MS`], mudo respeitado, e
//! item perdido cala em momento tenso (perigo, não: avisar no aperto é o ponto). O texto é curto e
//! gerado pelo LLM (`prompts/gamer-companion/copilot-alert.md`), que pode responder `[PASS]`.
//!
//! **Modo passageiro:** nada aqui toca disco — sem memória, sem histórico, sem estatística; o aviso
//! vive só como `SpeechRequest`. Também cala com o usuário falando (VAD), como a fala proativa. **Modo discreto do dock:** o aviso é só voz (`origin:
//! "game-copilot"`), nenhum cartão ou bolha; com tela cheia o dock continua escondido
//! (`dock-visibility.ts`: discreto vence "falando").
//!
//! Reserva-antes-do-LLM, mesma técnica de `game_moment_commentary`: dois avisos iguais chegando no
//! round-trip do LLM não passam os dois pelo gate.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

use super::game_moment_commentary::CommentGenerator;
use crate::config::store::ConfigStore;
use crate::modules::brain::awareness::tension::GameTensionState;
use crate::modules::chat::load_prompt_no_handle;
use crate::modules::voice::tts::speech_mute::is_muted;

const FLOOR_MS: u64 = 8_000;
const SAME_ALERT_MS: u64 = 180_000;
/// `detail` é texto lido da tela: entra no prompt curto e limitado.
const MAX_DETAIL_CHARS: usize = 80;

/// Os dois tipos que a visão pode pedir — qualquer outro valor é descartado na borda.
pub fn is_copilot_kind(kind: &str) -> bool {
    matches!(kind, "danger" | "missed_item")
}

fn cooldown_ms(kind: &str) -> u64 {
    // Um perigo que fica na tela volta a cada tick com outra paráfrase: o cooldown do tipo é o freio real.
    if kind == "danger" { 45_000 } else { 90_000 }
}

fn alert_key(kind: &str, detail: &str) -> String {
    let words: Vec<String> = detail.split_whitespace().map(str::to_lowercase).collect();
    format!("{kind}:{}", words.join(" "))
}

/// Quando cada tipo e cada aviso exato saíram por último — um por processo, injetado.
#[derive(Default)]
pub struct CopilotState {
    last: Mutex<HashMap<String, u64>>,
}

impl CopilotState {
    /// Gate puro sobre o próprio estado; `tense`/`muted` já resolvidos pelo chamador.
    fn is_blocked(&self, kind: &str, key: &str, tense: bool, muted: bool, now_ms: u64) -> bool {
        if muted || (tense && kind == "missed_item") {
            return true;
        }
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        last.retain(|_, at| now_ms.saturating_sub(*at) <= SAME_ALERT_MS);
        let since = |k: &str| last.get(k).map(|at| now_ms.saturating_sub(*at));
        let any = last.values().map(|at| now_ms.saturating_sub(*at)).min();
        any.is_some_and(|d| d < FLOOR_MS)
            || since(kind).is_some_and(|d| d < cooldown_ms(kind))
            || since(key).is_some_and(|d| d < SAME_ALERT_MS)
    }

    fn reserve(&self, kind: &str, key: &str, now_ms: u64) -> [Option<u64>; 2] {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        [last.insert(kind.to_string(), now_ms), last.insert(key.to_string(), now_ms)]
    }

    fn rollback(&self, kind: &str, key: &str, previous: [Option<u64>; 2]) {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        for (k, prev) in [kind, key].into_iter().zip(previous) {
            match prev {
                Some(at) => last.insert(k.to_string(), at),
                None => last.remove(k),
            };
        }
    }
}

/// Mudo, o usuário falando agora (VAD), ou vídeo/live/música em foco — o jogo de uma live não é o
/// dele: ela não fala por cima.
fn silenced(now_ms: u64) -> bool {
    is_muted(now_ms as i64)
        || crate::modules::voice::user_speech_activity::global().should_defer_proactive_speech(now_ms)
        || crate::modules::vision::is_watching(crate::modules::system_signals::active_window::last_known().as_ref())
}

fn enabled(store: &ConfigStore) -> bool {
    super::feature::master_enabled(store) && store.get("gamer-companion.copilotEnabled").and_then(|v| v.as_bool()).unwrap_or(false)
}

fn build_prompt(store: &ConfigStore, game: &str, kind: &str, detail: &str) -> Result<String, String> {
    let vars: HashMap<String, String> = [
        ("game", game.to_string()),
        ("kind", kind.replace('_', " ")),
        ("detail", detail.to_string()),
        ("language", crate::config::locale::app_language(store)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    load_prompt_no_handle("gamer-companion", "copilot-alert", &vars).map_err(|e| e.to_string())
}

async fn generate_line(generator: &dyn CommentGenerator, prompt: &str) -> Option<String> {
    match generator.generate(prompt).await {
        Ok(text) => Some(text.trim().to_string()).filter(|t| !t.is_empty()),
        Err(error) => {
            log::warn!("co-piloto: geração do aviso falhou: {error}");
            None
        }
    }
}

/// O `SpeechRequest` do aviso: só voz, marcado como co-piloto.
fn speech_payload(line: &str) -> Value {
    serde_json::json!({ "text": line, "origin": "game-copilot" })
}

/// Consome um `GameCopilotAlert` (`{ game, kind, detail }`). Devolve `true` quando publicou o
/// `SpeechRequest` do aviso.
pub async fn maybe_alert(
    state: &CopilotState,
    store: &ConfigStore,
    tension: &GameTensionState,
    generator: &dyn CommentGenerator,
    payload: &Value,
    now_ms: u64,
) -> bool {
    let Some(line) = decide(state, store, tension, generator, payload, now_ms).await else { return false };
    crate::events::bus().publish(crate::events::EventKey::SpeechRequest, speech_payload(&line));
    true
}

/// O gate inteiro e a geração: `Some(fala)` quando o aviso deve sair.
async fn decide(
    state: &CopilotState,
    store: &ConfigStore,
    tension: &GameTensionState,
    generator: &dyn CommentGenerator,
    payload: &Value,
    now_ms: u64,
) -> Option<String> {
    let field = |name: &str| payload.get(name).and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
    let (game, kind) = (field("game"), field("kind"));
    let detail: String = field("detail").chars().take(MAX_DETAIL_CHARS).collect();
    if !enabled(store) || !is_copilot_kind(&kind) || game.is_empty() || detail.is_empty() {
        return None;
    }
    let key = alert_key(&kind, &detail);
    // A flag do próprio tick: o `GameMoment` do mesmo tick pode ainda não ter tensionado o relógio.
    let tense = tension.is_tense(now_ms) || payload.get("tense").and_then(Value::as_bool).unwrap_or(false);
    if state.is_blocked(&kind, &key, tense, silenced(now_ms), now_ms) {
        return None;
    }
    let previous = state.reserve(&kind, &key, now_ms);
    let prompt = match build_prompt(store, &game, &kind, &detail) {
        Ok(prompt) => prompt,
        Err(error) => {
            log::error!("co-piloto sem prompt (copilot-alert): {error}");
            state.rollback(&kind, &key, previous);
            return None;
        }
    };
    let Some(line) = generate_line(generator, &prompt).await else {
        state.rollback(&kind, &key, previous);
        return None;
    };
    // O modelo escolheu calar: a reserva fica, para não insistir no mesmo aviso logo em seguida.
    let silent = crate::modules::chat::proactive_novelty::is_pass(&line) || silenced(now_ms);
    (!silent).then_some(line)
}

#[cfg(test)]
#[path = "copilot_tests.rs"]
mod tests;
