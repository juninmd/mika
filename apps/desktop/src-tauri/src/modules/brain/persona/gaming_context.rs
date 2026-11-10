//! === brain/engine/gaming-context.ts (fatia jogo+duração; ver "Escopo" abaixo) ==================
//!
//! Porte PULL-BASED, não o `GamingContextProvider` do TS (estado próprio, atualizado por
//! `GAME_CHANGED`/`GAME_STOPPED`). Em vez de assinar eventos, lê `SessionMemory::current_session()`
//! no instante em que o prompt é montado — a MESMA fonte que `gamer::game_watcher.rs` já mantém
//! consistente. `EventKey::GameChanged`/`GameStopped`/`GamingTick` não existem em
//! `crate::events::keys` hoje (mesma lacuna já documentada no cabeçalho de `game_watcher.rs`); o pull
//! evita precisar delas só para este bloco de prompt.
//!
//! ## Escopo menor que o TS, não escondido
//! Sem `gamePreferences` (a lista "O Que Sabemos Sobre Games" do TS): essas entradas vêm de
//! `chat/pipeline.ts::extractGamingInfo`, uma extração por LLM sem porte Rust ainda — nada aqui
//! inventa um substituto.

const MINUTE_MS: u64 = 60_000;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// `None`/`""` quando não há sessão de jogo ativa — o placeholder some do prompt em vez de aparecer
/// vazio (mesmo padrão dos blocos opcionais de `system_prompt::build_volatile_context`).
pub fn build_gaming_context_block() -> String {
    let Some(memory) = crate::modules::gamer::session_memory() else { return String::new() };
    let Some(session) = memory.current_session() else { return String::new() };

    let duration_min = now_ms().saturating_sub(session.start_time_ms) / MINUTE_MS;

    let mut lines = vec!["## Current Gaming Session".to_string(), format!("- Game: {}", session.game_name)];
    if duration_min > 0 {
        lines.push(format!("- Time in session: {duration_min}min"));
    }
    // 08-gamer-companion.md §"Estado de comportamento", MUST: "respostas mais curtas" enquanto uma
    // sessão de jogo está ativa — instrução de prompt, nunca fala pronta (AGENTS.md §2.6, toda fala
    // da Mika é gerada por LLM). Fica DENTRO deste bloco pull-based (não um parâmetro novo de
    // `run_turn_ollama`) para não mexer na assinatura de uma função com vários chamadores fora do
    // gamer.
    lines.push("- He is playing right now: answer in 1-2 short sentences, straight to the point, without breaking his game flow.".to_string());
    format!("{}\n", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::session_memory::SessionMemory;
    use crate::modules::gamer::session_db::SessionDb;
    use std::sync::Arc;

    /// Serializa os testes deste arquivo com os de `mod.rs`/`game_watcher.rs` que também escrevem no
    /// mesmo `OnceLock` de processo — mesma razão do `TOOL_REGISTRATION_ORDER` em `features::mood`.
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        crate::modules::gamer::SESSION_MEMORY_TEST_ORDER.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn build_gaming_context_block_reports_the_active_game_and_a_positive_duration() {
        let _guard = lock();
        let memory = Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap()));
        memory.start_session("570", "Dota 2").unwrap();
        crate::modules::gamer::set_session_memory(Arc::clone(&memory));

        let block = build_gaming_context_block();
        assert!(block.contains("## Current Gaming Session"));
        assert!(block.contains("- Game: Dota 2"));
        // MUST (08-gamer-companion.md §"Estado de comportamento"): respostas mais curtas com sessão
        // ativa — instrução no bloco de prompt, não um parâmetro extra em `run_turn_ollama`.
        assert!(block.contains("answer in 1-2 short sentences"), "{block}");
    }

    #[test]
    fn build_gaming_context_block_is_empty_when_no_session_is_current() {
        let _guard = lock();
        let memory = Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap()));
        crate::modules::gamer::set_session_memory(Arc::clone(&memory));

        assert_eq!(build_gaming_context_block(), "");
    }
}
