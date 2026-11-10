//! Estado explícito de "gaming" — specs/08-gamer-companion.md ("Estado de comportamento"). Antes
//! desta rodada não existia um único booleano de processo para "tem jogo rolando agora": só um
//! `EventKey::GameChanged`/`GameStopped` pontual (`game_watcher.rs`) e uma sessão em `SessionMemory`
//! que ninguém fora de `gamer/` lia.
//!
//! Mesmo desenho de [`super::super::brain::awareness::tension::GameTensionState`]: UMA instância de
//! processo, `Arc` injetado em quem precisa ler (`ProactiveEngine`), nunca um `static` global — ver
//! o cabeçalho daquele arquivo para o porquê. Alimentado pelo MESMO par de eventos que
//! `game_watcher.rs::apply` já publica (`GameChanged`/`GameStopped`), consumido no laço de bus de
//! `brain::awareness::spawn_proactive_engine`.
//!
//! **Escopo desta rodada:** proatividade reduzida para candidatos que NÃO são de jogo (ver
//! [`is_game_candidate_key`], consumido por `ProactiveEngine::should_speak`) e um accessor de
//! leitura (`is_active`) para outro código consultar depois — nomeadamente `vision::watcher`, que
//! ainda não foi editado nesta rodada (fora do escopo: intervalo de captura de tela é outra frente).
//! Respostas mais curtas: ver o cabeçalho de `game_moment_commentary.rs`/`providers.rs` para onde
//! esse sinal alcança prompt de LLM sem tocar `brain::turn` (fora do escopo, dono é outro agente).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Janela de validade do último sinal de visão — ver [`GamingState::vision_confirms_game`]. Maior
/// que o intervalo mínimo de captura em modo jogo (`vision::watcher::GAME_MIN_INTERVAL_MS`, 15s) e
/// que o tick do motor proativo (`awareness::constants::PROACTIVE_BASE_INTERVAL_MS`, 2min), para
/// tolerar um ciclo de análise lento/pulado sem apagar um sinal ainda válido.
pub const VISION_GAME_SIGNAL_MAX_AGE_MS: u64 = 3 * 60 * 1000;

pub struct GamingState {
    active: AtomicBool,
    /// Último `isLikelyGame` reportado por uma observação de tela REAL (`vision::watcher::run_tick`,
    /// campo `GameSignal::is_likely_game`) — nunca inferido do nome da sessão. Ver
    /// [`Self::vision_confirms_game`] para o porquê (smoke-baseline.md item 8/2: comentário de
    /// gameplay não pode confiar só no nome do jogo detectado por processo/janela).
    vision_says_game: AtomicBool,
    vision_signal_at_ms: AtomicU64,
}

impl GamingState {
    pub fn new() -> Self {
        Self { active: AtomicBool::new(false), vision_says_game: AtomicBool::new(false), vision_signal_at_ms: AtomicU64::new(0) }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Devolve `true` só quando o valor de fato MUDOU — quem chama usa isto para saber quando
    /// reiniciar timers dependentes (MUST da spec: "entrar e sair do estado reinicia os timers
    /// dependentes"), em vez de reagir a todo `GameChanged` (troca de jogo SEM sair do estado
    /// "gaming" não deveria reiniciar nada — só a transição ocioso->jogando ou jogando->ocioso).
    pub fn set_active(&self, active: bool) -> bool {
        self.active.swap(active, Ordering::SeqCst) != active
    }

    /// Registra o que a ÚLTIMA análise de tela de verdade viu — chamado de `vision::watcher::run_tick`
    /// a cada ciclo bem-sucedido, independente de `auto_comment`/tensão (o sinal de aterramento
    /// existe mesmo quando o comentário em si fica suprimido).
    pub fn record_vision_signal(&self, is_likely_game: bool, at_ms: u64) {
        self.vision_says_game.store(is_likely_game, Ordering::SeqCst);
        self.vision_signal_at_ms.store(at_ms, Ordering::SeqCst);
    }

    /// Só `true` quando a observação de tela MAIS RECENTE, dentro de [`VISION_GAME_SIGNAL_MAX_AGE_MS`],
    /// disse de verdade que havia um jogo na tela. `false` antes de qualquer observação chegar
    /// (fail-closed: sem visão ainda, sem comentário de gameplay) e `false` quando o último sinal já
    /// envelheceu (um "sim" de 20 minutos atrás não aterra um comentário agora). Esta é a checagem
    /// que falta em `providers::GameProvider`/`gamer::game_moment_commentary`: os dois hoje comentam
    /// só porque uma SESSÃO está aberta (detectada por processo/janela), nunca porque a tela
    /// realmente mostra um jogo — a causa do "comentário de parkour de gato" com Stray falsamente
    /// detectado (smoke-baseline.md item 8/2).
    pub fn vision_confirms_game(&self, now_ms: u64, max_age_ms: u64) -> bool {
        let at = self.vision_signal_at_ms.load(Ordering::SeqCst);
        if at == 0 || now_ms.saturating_sub(at) > max_age_ms {
            return false;
        }
        self.vision_says_game.load(Ordering::SeqCst)
    }
}

impl Default for GamingState {
    fn default() -> Self {
        Self::new()
    }
}

/// Um candidato de jogo é qualquer `ProactiveCandidate::key` que já usa o prefixo `"game"` — `game:
/// {game}` (`event_reactions::on_game_changed`), `game-sustained` (`providers.rs::GameProvider`) e
/// `game-absence` (`providers.rs::GameAbsenceProvider`). Os três representam o jogo em si, não outro
/// assunto qualquer competindo por atenção enquanto ele joga.
pub fn is_game_candidate_key(key: &str) -> bool {
    key.starts_with("game")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_inactive() {
        assert!(!GamingState::new().is_active());
    }

    #[test]
    fn set_active_reports_whether_the_value_actually_changed() {
        let state = GamingState::new();
        assert!(state.set_active(true), "false -> true é uma transição real");
        assert!(!state.set_active(true), "true -> true não é transição nenhuma");
        assert!(state.set_active(false), "true -> false é uma transição real");
    }

    #[test]
    fn is_game_candidate_key_matches_every_real_game_provider_key() {
        assert!(is_game_candidate_key("game:Hades"));
        assert!(is_game_candidate_key("game-sustained"));
        assert!(is_game_candidate_key("game-absence"));
        assert!(!is_game_candidate_key("idle"));
        assert!(!is_game_candidate_key("system:theme"));
    }

    // -- vision_confirms_game (grounded commentary, smoke-baseline.md item 8/2) -------------------

    #[test]
    fn vision_confirms_game_is_false_before_any_observation_ever_arrives() {
        let state = GamingState::new();
        assert!(!state.vision_confirms_game(1_000_000, VISION_GAME_SIGNAL_MAX_AGE_MS), "sem visão ainda, fail-closed");
    }

    #[test]
    fn vision_confirms_game_reflects_the_most_recent_recorded_signal() {
        let state = GamingState::new();
        state.record_vision_signal(true, 1_000_000);
        assert!(state.vision_confirms_game(1_000_100, VISION_GAME_SIGNAL_MAX_AGE_MS));

        state.record_vision_signal(false, 1_000_200);
        assert!(!state.vision_confirms_game(1_000_300, VISION_GAME_SIGNAL_MAX_AGE_MS), "última observação disse que não há jogo");
    }

    #[test]
    fn vision_confirms_game_expires_a_stale_signal() {
        let state = GamingState::new();
        state.record_vision_signal(true, 1_000_000);
        let expired_at = 1_000_000 + VISION_GAME_SIGNAL_MAX_AGE_MS + 1;
        assert!(!state.vision_confirms_game(expired_at, VISION_GAME_SIGNAL_MAX_AGE_MS), "sinal velho não deveria aterrar mais nada");
    }
}
