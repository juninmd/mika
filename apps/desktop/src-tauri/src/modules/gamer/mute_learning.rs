//! Aprende os momentos em que ela foi mandada calar e antecipa — specs/08-gamer-companion.md
//! ("Silêncio é feature", SHOULD). Antes desta rodada não havia rastreio nenhum de padrão de mute:
//! `game_learning.rs` existe, mas aprende FATOS do jogo (wiki via busca na web), não quando ficar
//! quieta.
//!
//! **Antecipa reusando o MESMO mecanismo de silêncio, não um segundo.** Em vez de ensinar
//! `game_moment_commentary`/`vision::watcher` a consultar mais um estado, [`MuteLearning`] publica
//! `EventKey::MuteSpeech{muted:true}` sozinha — o MESMO evento que
//! `shortcuts.rs::register_mute_shortcut` já publica na tecla de atalho — quando o histórico mostra
//! correlação forte entre um tipo de momento e um mute que veio logo depois dele. Todo consumidor
//! que já respeita mute (`game_moment_commentary::is_blocked`, e no futuro qualquer outro) passa a
//! respeitar a antecipação de graça, sem saber que ela existe.
//!
//! **Correlação, não causalidade.** Um `MuteSpeech{muted:true}` chegando dentro de
//! [`CORRELATION_WINDOW_MS`] do último momento OBSERVADO de um tipo credita esse tipo. Depois de
//! [`ANTICIPATE_THRESHOLD`] créditos, o PRÓXIMO momento do mesmo tipo já chega mutado, antes de
//! qualquer comentário ser gerado — "ela aprendeu que esse tipo de momento pede silêncio".

use std::collections::HashMap;
use std::sync::Mutex;

use crate::events::{bus, EventKey};

const CORRELATION_WINDOW_MS: u64 = 20_000;
const ANTICIPATE_THRESHOLD: u32 = 2;
/// Mesma ordem de grandeza do piso duro de comentário (`HARD_FLOOR_MS`, `game_moment_commentary.rs`)
/// — silencia só o bastante para não falar sobre ESTE momento, não a sessão inteira.
const ANTICIPATED_MUTE_MS: u64 = 20_000;

#[derive(Default)]
struct MomentStats {
    last_seen_at_ms: u64,
    mutes_within_window: u32,
}

pub struct MuteLearning {
    stats: Mutex<HashMap<String, MomentStats>>,
}

impl MuteLearning {
    pub fn new() -> Self {
        Self { stats: Mutex::new(HashMap::new()) }
    }

    /// Chamado a cada `EventKey::GameMoment`, ANTES de `game_moment_commentary` decidir se comenta.
    /// Sempre registra o instante (para correlacionar com um mute que venha em seguida) e devolve
    /// `true` quando o histórico já credita este tipo o bastante para antecipar o silêncio — a
    /// decisão fica pura/síncrona de propósito, sem tocar o barramento, para ser testável sem
    /// `tokio`/sem o singleton global de `bus()` (ver o aviso no topo de `events/bus.rs`).
    /// [`anticipate_mute`] é quem publica de verdade a partir do resultado.
    pub fn observe_moment(&self, moment_type: &str, now_ms: u64) -> bool {
        let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        let entry = stats.entry(moment_type.to_string()).or_default();
        entry.last_seen_at_ms = now_ms;
        entry.mutes_within_window >= ANTICIPATE_THRESHOLD
    }

    /// Chamado a cada `EventKey::MuteSpeech{muted:true}` real (atalho ou comando) — credita todo
    /// tipo de momento visto dentro de [`CORRELATION_WINDOW_MS`].
    pub fn observe_mute(&self, now_ms: u64) {
        let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        for entry in stats.values_mut() {
            if now_ms.saturating_sub(entry.last_seen_at_ms) <= CORRELATION_WINDOW_MS {
                entry.mutes_within_window += 1;
            }
        }
    }
}

impl Default for MuteLearning {
    fn default() -> Self {
        Self::new()
    }
}

/// Publica o `MuteSpeech` de antemão quando [`MuteLearning::observe_moment`] diz que deveria — único
/// ponto que toca `bus()`, chamado pelo assinante de `EventKey::GameMoment` em
/// `brain::awareness::spawn_proactive_engine` logo após `observe_moment`.
///
/// `"source": "mute_learning"` — sem isto, o assinante de `MuteSpeech` que credita
/// [`MuteLearning::observe_mute`] veria este mute ANTECIPADO como se fosse um pedido novo do usuário
/// e se auto-alimentaria (crédito crescendo sozinho a cada momento, nunca por um "cala a boca" de
/// verdade).
pub fn anticipate_mute(now_ms: u64) {
    bus().publish(EventKey::MuteSpeech, serde_json::json!({ "muted": true, "untilMs": now_ms + ANTICIPATED_MUTE_MS, "source": "mute_learning" }));
}

#[cfg(test)]
mod tests {
    use super::*;

    // `observe_moment`/`observe_mute` são puros e síncronos de propósito (ver o cabeçalho) — os
    // quatro testes abaixo não tocam `tokio` nem o singleton global de `bus()`, então não sofrem do
    // cross-talk que `cargo test` em paralelo causaria contra `bus()` (aviso no topo de
    // `events/bus.rs`). Só [`anticipate_mute`], o efeito colateral, precisa de um teste de
    // integração — é o único, mais abaixo.

    #[test]
    fn does_not_anticipate_before_the_threshold_is_reached() {
        let learning = MuteLearning::new();
        learning.observe_moment("boss", 0);
        learning.observe_mute(1_000); // 1º crédito para "boss"
        assert!(!learning.observe_moment("boss", 2_000), "só 1 crédito ainda: não deveria antecipar");
    }

    #[test]
    fn anticipates_after_the_threshold_is_reached() {
        let learning = MuteLearning::new();
        learning.observe_moment("boss", 0);
        learning.observe_mute(1_000); // crédito 1
        learning.observe_moment("boss", 2_000);
        learning.observe_mute(2_500); // crédito 2 — bate o piso
        assert!(learning.observe_moment("boss", 5_000), "2 créditos deveriam bastar para antecipar");
    }

    #[test]
    fn a_mute_outside_the_correlation_window_does_not_credit_a_stale_moment() {
        let learning = MuteLearning::new();
        learning.observe_moment("boss", 0);
        learning.observe_mute(CORRELATION_WINDOW_MS + 1); // longe demais do momento observado: não credita
        learning.observe_moment("boss", CORRELATION_WINDOW_MS + 2);
        learning.observe_mute(CORRELATION_WINDOW_MS + 3); // só este poderia contar (crédito 1)
        assert!(!learning.observe_moment("boss", CORRELATION_WINDOW_MS + 100), "só 1 crédito real: não deveria antecipar");
    }

    #[test]
    fn moment_types_do_not_share_credit() {
        let learning = MuteLearning::new();
        learning.observe_moment("boss", 0);
        learning.observe_mute(500);
        learning.observe_moment("boss", 1_000);
        learning.observe_mute(1_500); // "boss" bate o piso (2 créditos)
        assert!(!learning.observe_moment("cutscene", 100_000), "um tipo novo não deveria herdar o crédito de outro");
    }

    /// Único teste que toca `bus()` — integração mínima provando que `anticipate_mute` publica o
    /// payload que `game_moment_commentary`/`voice::tts::speech_mute` esperam. Filtra por
    /// `source: "mute_learning"` (só esta função grava essa tag) para não confundir com um
    /// `MuteSpeech` de outro teste concorrente.
    #[tokio::test]
    async fn anticipate_mute_publishes_a_named_mute_speech_event() {
        let mut sub = bus().subscribe();
        anticipate_mute(1_000);

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        let envelope = loop {
            let received = tokio::time::timeout_at(deadline, sub.recv()).await.ok().flatten().expect("MuteSpeech não publicado a tempo");
            if received.key == EventKey::MuteSpeech && received.payload.get("source").and_then(|v| v.as_str()) == Some("mute_learning") {
                break received;
            }
        };
        assert_eq!(envelope.payload["muted"], true);
        assert_eq!(envelope.payload["untilMs"], 1_000 + ANTICIPATED_MUTE_MS);
    }
}
