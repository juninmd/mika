//! Porte fiel de `apps/desktop/src/main/services/voice/audio/speak-guard.ts` — a janela recente do
//! que a Mika acabou de falar, usada para reconhecer que uma "interrupção" transcrita é na verdade
//! o próprio microfone captando o alto-falante (eco), não o usuário.
//!
//! **Rastreador de processo, alimentado por fora.** `EventKey::TtsAudioReady` não carrega o texto
//! falado (só áudio, `tts/bus_bridge.rs`), então este arquivo não pode assinar o barramento
//! sozinho. Em vez disso expõe um tracker GLOBAL ([`global`]) que quem SABE o texto — o dono do
//! laço de fala (`voice::auto_speak`, já ligado) — alimenta chamando [`mark_spoken`] a cada trecho
//! falado, e [`reset_turn`] quando um novo turno começa. `interruption_transcript.rs` lê por
//! [`looks_like_self_echo`]/[`spoken_prefix`] no outro lado. Contrato de quem alimenta:
//! `mark_spoken(now_ms(), texto_do_trecho, now_ms())` por trecho e `reset_turn()` no início de cada
//! turno de fala — sem isso o guard nunca filtra nada (mesma degradação seguro-por-omissão dos
//! outros detectores deste módulo).
//!
//! **`spoken_prefix` é uma aproximação, não `AutoSpeakTurn.streamSpokenLiteral` fiel.** O TS
//! concatena os trechos do turno ATUAL, resetados em `onTurnStart`; aqui replicamos isso com
//! [`SpeakGuard::turn_prefix`] SE o chamador de fato chamar `reset_turn` no início do turno — sem
//! essa chamada, o prefixo cresce turno após turno (nunca fica staleado por TTL, ao contrário de
//! `recent`). Divergência aceita e não escondida: melhor um prefixo levemente velho do que nenhum.

use std::sync::{Mutex, OnceLock};

const TTL_MS: u64 = 10_000;
const NEAR_WINDOW_MS: u64 = 2_000;
const ECHO_WORD_OVERLAP: f64 = 0.6;
const ECHO_MIN_WORDS: usize = 3;

struct Entry {
    timestamp_ms: u64,
    text: String,
    words: Vec<String>,
    inserted_at_ms: u64,
}

/// Espelha o array de módulo `recent` do TS — aqui um struct instanciável em vez de estado global
/// direto, para os testes poderem exercitar uma instância ISOLADA (ver [`global`] para o singleton
/// de processo que a produção usa).
pub struct SpeakGuard {
    recent: Vec<Entry>,
    /// Concatenação dos textos RAW (não normalizados) marcados desde o último [`Self::reset_turn`]
    /// — ver o cabeçalho do arquivo ("`spoken_prefix` é uma aproximação").
    turn_prefix: String,
}

fn normalize(text: &str) -> String {
    text.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(500).collect()
}

fn words(text: &str) -> Vec<String> {
    normalize(text)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

impl SpeakGuard {
    pub fn new() -> Self {
        Self { recent: Vec::new(), turn_prefix: String::new() }
    }

    fn cleanup(&mut self, now_ms: u64) {
        self.recent.retain(|e| now_ms.saturating_sub(e.inserted_at_ms) <= TTL_MS);
    }

    /// Porte de `markSpoken(timestamp, text)`. `now_ms` é o "agora" do chamador (`Date.now()` no
    /// TS), separado de `timestamp_ms` (quando aquele trecho foi de fato falado) pelo mesmo motivo
    /// do original: os dois só coincidem quando não há atraso entre falar e registrar. Também
    /// acumula `text` (RAW, sem normalizar) em [`Self::turn_prefix`] — ver [`Self::spoken_prefix`].
    pub fn mark_spoken(&mut self, timestamp_ms: u64, text: &str, now_ms: u64) {
        self.cleanup(now_ms);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            if !self.turn_prefix.is_empty() {
                self.turn_prefix.push(' ');
            }
            self.turn_prefix.push_str(trimmed);
        }
        let normalized = normalize(text);
        let entry_words = words(&normalized);
        self.recent.push(Entry { timestamp_ms, text: normalized, words: entry_words, inserted_at_ms: now_ms });
    }

    /// Renova a janela de self-echo quando a frase vai tocar (a fila de síntese passa do TTL).
    pub fn refresh_echo(&mut self, timestamp_ms: u64, text: &str, now_ms: u64) {
        self.cleanup(now_ms);
        let normalized = normalize(text);
        if let Some(entry) = self.recent.iter_mut().find(|e| e.text == normalized) {
            entry.inserted_at_ms = now_ms;
            return;
        }
        let entry_words = words(&normalized);
        self.recent.push(Entry { timestamp_ms, text: normalized, words: entry_words, inserted_at_ms: now_ms });
    }

    /// Zera o prefixo acumulado — o chamador chama isto no início de cada turno de fala (ver o
    /// cabeçalho do arquivo). NÃO mexe em `recent`/TTL: o self-echo continua valendo entre turnos.
    pub fn reset_turn(&mut self) {
        self.turn_prefix.clear();
    }

    /// Aproximação de `AutoSpeakTurn.spokenPrefix()` — ver o cabeçalho do arquivo.
    pub fn spoken_prefix(&self) -> String {
        self.turn_prefix.clone()
    }

    /// Porte de `wasSpokenInTurn`.
    pub fn was_spoken_in_turn(&mut self, timestamp_ms: u64, candidate: &str, now_ms: u64) -> bool {
        self.cleanup(now_ms);
        let norm = normalize(candidate);
        self.recent.iter().any(|e| {
            e.text == norm && timestamp_ms.abs_diff(e.timestamp_ms) <= NEAR_WINDOW_MS
        })
    }

    /// Porte de `looksLikeSelfEcho`: candidato com poucas palavras precisa bater 100% com uma fala
    /// recente; candidato maior basta ter 60% de sobreposição de palavras.
    pub fn looks_like_self_echo(&mut self, candidate: &str, now_ms: u64) -> bool {
        self.cleanup(now_ms);
        let candidate_words = words(candidate);
        if candidate_words.is_empty() {
            return false;
        }

        for entry in &self.recent {
            if entry.words.is_empty() {
                continue;
            }
            if candidate_words.len() < ECHO_MIN_WORDS {
                if candidate_words.iter().all(|w| entry.words.contains(w)) {
                    return true;
                }
                continue;
            }
            let hits = candidate_words.iter().filter(|w| entry.words.contains(w)).count();
            if hits as f64 / candidate_words.len() as f64 >= ECHO_WORD_OVERLAP {
                return true;
            }
        }
        false
    }
}

impl Default for SpeakGuard {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL: OnceLock<Mutex<SpeakGuard>> = OnceLock::new();

/// O tracker de processo que a produção usa — ver o cabeçalho do arquivo. Testes usam
/// `SpeakGuard::new()` isolado (mesmo motivo de `listening_state::global`: um `OnceLock`
/// compartilhado entre `#[test]`/`#[tokio::test]` rodando em paralelo seria flaky pelo vizinho).
pub fn global() -> &'static Mutex<SpeakGuard> {
    GLOBAL.get_or_init(|| Mutex::new(SpeakGuard::new()))
}

/// Lock envenenado degrada para NO-OP — mesma escolha de `listening_state.rs`/`AbortRegistry`:
/// um tracker informativo não pode derrubar o laço de fala por causa de um panic em outra thread.
///
/// Chamado por `voice::auto_speak` (`mod.rs`, `assistant_message.rs`) a cada trecho enfileirado para
/// falar — quem tem o texto de cada trecho é ele, não o barramento.
pub fn mark_spoken(timestamp_ms: u64, text: &str, now_ms: u64) {
    if let Ok(mut guard) = global().lock() {
        guard.mark_spoken(timestamp_ms, text, now_ms);
    }
}

/// Ver [`SpeakGuard::refresh_echo`].
pub fn refresh_echo(timestamp_ms: u64, text: &str, now_ms: u64) {
    if let Ok(mut guard) = global().lock() {
        guard.refresh_echo(timestamp_ms, text, now_ms);
    }
}

/// Zera o prefixo do turno (`auto_speak::mod.rs`, em `chat:thinking-start`) — a janela de eco fica.
pub fn reset_turn() {
    if let Ok(mut guard) = global().lock() {
        guard.reset_turn();
    }
}

/// Lido por `vad_audio.rs::spawn_interruption_transcription` para o campo `spokenPrefix` do
/// payload de `EventKey::Interruption` — chamador real, ver o cabeçalho do arquivo.
pub fn spoken_prefix() -> String {
    global().lock().map(|g| g.spoken_prefix()).unwrap_or_default()
}

/// Lido por `interruption_transcript.rs::process_interruption_audio` — chamador real.
pub fn looks_like_self_echo(candidate: &str, now_ms: u64) -> bool {
    global().lock().map(|mut g| g.looks_like_self_echo(candidate, now_ms)).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_sentence_that_waited_in_the_synthesis_queue_is_still_recognized_as_echo() {
        let mut guard = super::SpeakGuard::new();
        let line = "hoje a gente fecha esse boss de vez";
        guard.mark_spoken(0, line, 0);
        guard.refresh_echo(0, line, 15_000);
        assert!(guard.looks_like_self_echo(line, 16_000), "a fila passou do TTL; o mic ouviria a Mika como usuário");
        assert_eq!(guard.spoken_prefix(), line, "renovar não duplica o prefixo do turno");
    }

    use super::*;

    #[test]
    fn empty_candidate_is_never_an_echo() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(0, "oi tudo bem", 0);
        assert!(!g.looks_like_self_echo("", 100));
    }

    #[test]
    fn short_candidate_requires_full_overlap() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(0, "vamos jogar", 0);
        assert!(g.looks_like_self_echo("vamos jogar", 100), "todas as palavras batem");
        assert!(!g.looks_like_self_echo("vamos correr", 100), "só uma das duas bate");
    }

    #[test]
    fn longer_candidate_needs_only_sixty_percent_overlap() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(0, "eu acho que voce devia tentar de novo", 0);
        // 4 de 5 palavras batem = 80% >= 60%.
        assert!(g.looks_like_self_echo("acho que voce devia mesmo", 100));
    }

    #[test]
    fn entries_older_than_ttl_are_forgotten() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(0, "vamos jogar", 0);
        assert!(!g.looks_like_self_echo("vamos jogar", TTL_MS + 1));
    }

    #[test]
    fn was_spoken_in_turn_matches_only_within_the_near_window() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(1_000, "boa noite", 1_000);
        assert!(g.was_spoken_in_turn(2_500, "boa noite", 1_000));
        assert!(!g.was_spoken_in_turn(4_000, "boa noite", 1_000), "fora da janela de 2s");
    }

    #[test]
    fn normalize_collapses_whitespace_and_case() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(0, "  Boa   Noite  ", 0);
        assert!(g.was_spoken_in_turn(0, "boa noite", 0));
    }

    // --- spoken_prefix / reset_turn -----------------------------------------------------------

    #[test]
    fn spoken_prefix_starts_empty() {
        let g = SpeakGuard::new();
        assert_eq!(g.spoken_prefix(), "");
    }

    #[test]
    fn spoken_prefix_accumulates_raw_text_across_marks_in_the_same_turn() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(0, "Oi!", 0);
        g.mark_spoken(1_000, "Tudo bem?", 1_000);
        assert_eq!(g.spoken_prefix(), "Oi! Tudo bem?", "concatena RAW, sem normalizar (mesmo texto literal do TS)");
    }

    #[test]
    fn reset_turn_clears_the_prefix_but_not_the_self_echo_window() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(0, "vamos jogar", 0);
        g.reset_turn();
        assert_eq!(g.spoken_prefix(), "", "prefixo zera no início do novo turno");
        assert!(g.looks_like_self_echo("vamos jogar", 100), "self-echo continua valendo entre turnos");
    }

    #[test]
    fn mark_spoken_with_blank_text_does_not_add_a_stray_separator() {
        let mut g = SpeakGuard::new();
        g.mark_spoken(0, "oi", 0);
        g.mark_spoken(1_000, "   ", 1_000);
        assert_eq!(g.spoken_prefix(), "oi");
    }

    // --- global() ------------------------------------------------------------------------------

    /// Mesma prova de `abort_registry.rs`/`listening_state.rs` — só identidade, sem mutar estado,
    /// para não colidir com outro teste deste binário rodando em paralelo.
    #[test]
    fn global_returns_the_same_process_wide_instance() {
        assert!(std::ptr::eq(global(), global()));
    }
}
