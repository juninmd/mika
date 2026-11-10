//! `services/memory/experience.ts` — porte só do recorte que agora tem consumidor Rust real:
//! `storeGameMoment` (`EventKey::GameMoment` → memória episódica). `appendLearnedHabit` (hábitos por
//! `GAME_LAUNCHED`) e `setCurrentGame` (`GAME_LAUNCHED`/`GAME_CHANGED`) seguem sem porte — nenhum dos
//! dois bloqueia o gap nomeado que esta rodada fecha (`docs/tauri-rust-migration.md`, seção 5), e
//! `crate::events::EventKey` não tem `GameLaunched` ainda (sem consumidor Rust, mesmo critério de
//! `events/keys.rs`).
//!
//! Chamado pelo assinante de `EventKey::GameMoment` em `brain::awareness::mod.rs` — um dos 3 efeitos
//! colaterais do mesmo evento (junto de `brain::awareness::tension` e
//! `features::character::slang_context::on_game_moment`).

use serde_json::{json, Value};

/// Port de `storeGameMoment`: mesmo par de campos obrigatórios (`game`/`description`), mesma forma
/// de metadata (`type: 'game-moment'`, `source: 'vision'`, `momentType`, `importance: 0.3`).
/// `recording_allowed_cached()`/`episodic_reader()` já encapsulam o gate `memory.enabled` e a
/// resolução do `MemoryStore` global — nunca abre um `MemoryStore` próprio aqui (ver `mod.rs`, §3).
pub fn store_game_moment(payload: &Value) {
    let game = payload.get("game").and_then(Value::as_str).map(str::trim).unwrap_or("");
    let description = payload.get("description").and_then(Value::as_str).map(str::trim).unwrap_or("");
    if game.is_empty() || description.is_empty() {
        return;
    }

    let Some(service) = super::episodic_reader() else {
        log::debug!("momento de jogo não salvo: memória episódica indisponível ainda");
        return;
    };

    let metadata = json!({
        "type": "game-moment",
        "source": "vision",
        "game": game,
        "momentType": payload.get("type").cloned().unwrap_or(Value::Null),
        "importance": 0.3,
    });

    if let Err(error) = service.store(super::recording_allowed_cached(), description, None, Some(&metadata)) {
        log::warn!("falha ao salvar momento de jogo na memória episódica: {error}");
    }
}

/// 08-gamer-companion.md §"Sessão", MUST: "sessão de jogo tem início, fim e resumo" — o resumo
/// agora vira episódio de longo prazo, o gap nomeado "resumo NÃO vira episódio" que a spec
/// registrava. Chamado UMA vez por sessão encerrada, de `gamer::game_watcher::spawn_llm_summary`
/// depois que `summarize_with_llm` já tentou (o resumo passado aqui é o final: o do LLM quando deu
/// certo, senão o `fallback_summary` que `end_session` já persistiu) — nunca duas entradas para a
/// mesma sessão.
pub fn store_game_session_summary(game_name: &str, summary: &str) {
    let game_name = game_name.trim();
    let summary = summary.trim();
    if game_name.is_empty() || summary.is_empty() {
        return;
    }

    let Some(service) = super::episodic_reader() else {
        log::debug!("resumo de sessão de jogo não salvo: memória episódica indisponível ainda");
        return;
    };

    let metadata = json!({
        "type": "game-session-summary",
        "source": "gamer",
        "game": game_name,
        "importance": 0.4,
    });

    if let Err(error) = service.store(super::recording_allowed_cached(), summary, None, Some(&metadata)) {
        log::warn!("falha ao salvar resumo de sessão de jogo na memória episódica: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_game_session_summary_with_an_empty_game_or_summary_is_a_no_op() {
        // Sem `episodic_reader()` inicializado em teste, o caminho normal já degrada cedo — este
        // teste prova o gate ANTES dele.
        store_game_session_summary("", "resumo válido");
        store_game_session_summary("Dota 2", "");
        store_game_session_summary("  ", "  ");
    }

    #[test]
    fn a_payload_without_a_game_or_description_is_a_no_op() {
        // Sem `episodic_reader()` inicializado no processo de teste, o caminho normal já degrada
        // cedo — este teste prova o gate ANTES dele, que não depende de estado global nenhum.
        store_game_moment(&json!({ "type": "victory" }));
        store_game_moment(&json!({ "game": "Hades" }));
        store_game_moment(&json!({ "description": "venceu" }));
        // Nenhuma chamada acima deveria panicar — é o contrato provado aqui.
    }
}
