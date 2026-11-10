//! Porte fiel de `apps/desktop/src/main/services/voice/services/auto-speak-turn.ts` — o estado de
//! UM turno de fala automática: o que já foi dito (`stream_spoken`/`stream_spoken_literal`), o que
//! ainda está no buffer esperando fechar frase, e se um barge-in derrubou o resto.
//!
//! **Dedup de fala (`SpeakGuard`) NÃO mora mais aqui.** `TurnState` guardava uma instância própria
//! antes desta rodada; `voice::speak_guard` agora expõe um tracker de PROCESSO (`global()`) que
//! `interruption_transcript.rs`/`vad_audio.rs` também leem (`spoken_prefix`/`looks_like_self_echo`
//! para o barge-in) — uma instância local aqui deixava esses dois lados cegos para tudo que a fala
//! automática dizia. `mod.rs` chama `speak_guard::mark_spoken`/`reset_turn` diretamente no tracker
//! global; `TurnState` cuida só do que É por turno de STREAM (buffer/spoken/aborted/escuta).
//! **O que NÃO foi portado, e por quê.** `ExpressionSyncService` inteira (a troca de EXPRESSÃO do
//! avatar por trás de `idle`/`listening`/`thinking`/`speaking`/`error`, com debounce de 800ms) fica
//! de fora: ela publica `Events.ACTIVITY_CHANGED`, uma chave que não existe em
//! `crate::events::keys` — criá-la está fora do escopo desta rodada (arquivo fechado a outros
//! times). `on_listening_started`/`on_listening_stopped` portam só a GUARDA de
//! `AutoSpeakTurn.onListeningStarted`/`onListeningStopped` ([`ListenState`]), sem publicar
//! expressão nenhuma — pronto para quem religar `ExpressionSyncService` reaproveitar em vez de
//! reinventar a guarda. `on_turn_start`/`on_barge_in` continuam sem tentar sincronizar avatar
//! nenhum; só o estado de fala em si. Ver o relatório final para o detalhe completo.

use crate::config::store::ConfigStore;

use super::speech_text::normalize_for_compare;

const VOICE_TTS_ENABLED: &str = "voice.ttsEnabled";
const VOICE_AUTO_SPEAK: &str = "voice.autoSpeak";

/// Porta de entrada da fala automática: `voice.ttsEnabled` E `voice.autoSpeak`, lidos AGORA — nunca
/// capturados uma vez no boot, porque o usuário pode desligar qualquer um dos dois no meio de um
/// turno em andamento.
pub fn gate_open(store: &ConfigStore) -> bool {
    let tts_enabled = store.get(VOICE_TTS_ENABLED).and_then(|v| v.as_bool()).unwrap_or(true);
    let auto_speak = store.get(VOICE_AUTO_SPEAK).and_then(|v| v.as_bool()).unwrap_or(true);
    tts_enabled && auto_speak
}

/// Subconjunto do `VoiceState` de `ExpressionSyncService` — só os dois valores que
/// `AutoSpeakTurn.onListeningStarted`/`onListeningStopped` de fato lêem/escrevem
/// (`state === 'idle' || state === 'offline'` / `state === 'listening'`). O TS também tem
/// `thinking`/`speaking`/`acting`/`confirming`/`error`, mas só a `ExpressionSyncService` real —
/// fora daqui, ver o parágrafo acima — os escreve; como nada neste módulo os produz, um
/// booleano-com-nome já reproduz fielmente a guarda "só liga escuta se estava ocioso" sem inventar
/// estado que ninguém popula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListenState {
    #[default]
    Idle,
    Listening,
}

#[derive(Default)]
pub struct TurnState {
    pub stream_buffer: String,
    pub stream_spoken: String,
    pub stream_spoken_literal: String,
    pub stream_aborted: bool,
    pub listen_state: ListenState,
}

impl TurnState {
    pub fn new() -> Self {
        Self::default()
    }

    /// `chat:thinking-start`: zera o estado do turno anterior.
    pub fn on_turn_start(&mut self) {
        self.stream_buffer.clear();
        self.stream_spoken.clear();
        self.stream_spoken_literal.clear();
        self.stream_aborted = false;
    }

    /// O usuário interrompeu: para o turno inteiro, não só a frase em voo (entre frases não há
    /// síntese ativa para abortar — o cancelamento de áudio em si é `abort_registry`, fora daqui).
    pub fn on_barge_in(&mut self) {
        self.stream_aborted = true;
        self.stream_buffer.clear();
    }

    /// Porte de `AutoSpeakTurn.onListeningStarted`: só liga "escutando" se estava ocioso — mesma
    /// guarda do TS (`state === 'idle' || state === 'offline'`), sem sobrescrever um estado ativo.
    pub fn on_listening_started(&mut self) {
        if self.listen_state == ListenState::Idle {
            self.listen_state = ListenState::Listening;
        }
    }

    /// Porte de `AutoSpeakTurn.onListeningStopped`: só desliga se o estado atual É "escutando" —
    /// mesma guarda do TS (`state === 'listening'`).
    pub fn on_listening_stopped(&mut self) {
        if self.listen_state == ListenState::Listening {
            self.listen_state = ListenState::Idle;
        }
    }

    /// Atualiza o rastro do que já foi dito no turno (streamado) com uma frase JÁ LIMPA
    /// (`speech_text::strip_emotion_tags` roda no chamador, como no TS). Devolve `Some(clean)` se
    /// ela tiver conteúdo falável (deve ser enfileirada), `None` se sobrou só pontuação/espaço.
    pub fn record_streamed_sentence(&mut self, clean: &str) -> Option<String> {
        let normalized = normalize_for_compare(clean);
        self.stream_spoken =
            if self.stream_spoken.is_empty() { normalized } else { format!("{} {normalized}", self.stream_spoken) };
        self.stream_spoken_literal = if self.stream_spoken_literal.is_empty() {
            clean.to_string()
        } else {
            format!("{} {clean}", self.stream_spoken_literal)
        };

        if super::speech_text::has_speakable_text(clean) { Some(clean.to_string()) } else { None }
    }

    /// Porte de `AutoSpeakTurn.remainingToSpeak`: o que da mensagem final AINDA não foi dito pelo
    /// stream. Diverge para o buffer pendente (ainda não cortado em frase) se o prefixo streamado
    /// não bater com o texto final — mesma degradação do TS.
    pub fn remaining_to_speak(&self, clean: &str) -> String {
        if self.stream_aborted {
            return String::new();
        }
        if self.stream_spoken.is_empty() {
            return clean.to_string();
        }

        let final_norm = normalize_for_compare(clean);
        if let Some(rest) = final_norm.strip_prefix(self.stream_spoken.as_str()) {
            return rest.trim().to_string();
        }

        normalize_for_compare(&self.stream_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schemas;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-auto-speak-turn-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, schemas::registry()).unwrap();
        (store, dir)
    }

    #[test]
    fn gate_is_open_by_default() {
        let (store, _dir) = store();
        assert!(gate_open(&store));
    }

    #[test]
    fn gate_closes_when_tts_is_disabled() {
        let (store, _dir) = store();
        store.set(VOICE_TTS_ENABLED, serde_json::json!(false)).unwrap();
        assert!(!gate_open(&store));
    }

    #[test]
    fn gate_closes_when_auto_speak_is_disabled_even_with_tts_on() {
        let (store, _dir) = store();
        store.set(VOICE_AUTO_SPEAK, serde_json::json!(false)).unwrap();
        assert!(!gate_open(&store));
    }

    #[test]
    fn on_turn_start_resets_everything() {
        let mut turn = TurnState::new();
        turn.stream_buffer = "resto".into();
        turn.stream_spoken = "ja dito".into();
        turn.stream_aborted = true;

        turn.on_turn_start();

        assert!(turn.stream_buffer.is_empty());
        assert!(turn.stream_spoken.is_empty());
        assert!(!turn.stream_aborted);
    }

    #[test]
    fn barge_in_aborts_the_turn_and_drops_the_pending_buffer() {
        let mut turn = TurnState::new();
        turn.stream_buffer = "metade de uma frase".into();

        turn.on_barge_in();

        assert!(turn.stream_aborted);
        assert!(turn.stream_buffer.is_empty());
    }

    #[test]
    fn remaining_to_speak_is_empty_after_a_barge_in() {
        let mut turn = TurnState::new();
        turn.on_barge_in();
        assert_eq!(turn.remaining_to_speak("Frase um. Frase dois."), "");
    }

    #[test]
    fn remaining_to_speak_is_the_whole_text_when_nothing_streamed_yet() {
        let turn = TurnState::new();
        assert_eq!(turn.remaining_to_speak("resposta inteira"), "resposta inteira");
    }

    #[test]
    fn remaining_to_speak_subtracts_the_streamed_prefix() {
        let mut turn = TurnState::new();
        turn.record_streamed_sentence("Frase um.");
        assert_eq!(turn.remaining_to_speak("Frase um. Frase dois."), "Frase dois.");
    }

    #[test]
    fn record_streamed_sentence_rejects_punctuation_only_text() {
        let mut turn = TurnState::new();
        assert_eq!(turn.record_streamed_sentence("..."), None);
        // Mesmo texto sem conteúdo falável ainda atualiza o rastro (bookkeeping), como no TS.
        assert_eq!(turn.stream_spoken, "...");
    }

    #[test]
    fn listening_started_turns_on_from_idle() {
        let mut turn = TurnState::new();
        assert_eq!(turn.listen_state, ListenState::Idle);
        turn.on_listening_started();
        assert_eq!(turn.listen_state, ListenState::Listening);
    }

    #[test]
    fn listening_stopped_turns_off_only_from_listening() {
        let mut turn = TurnState::new();
        turn.on_listening_stopped();
        assert_eq!(turn.listen_state, ListenState::Idle, "já estava ocioso, não devia mudar nada");

        turn.on_listening_started();
        turn.on_listening_stopped();
        assert_eq!(turn.listen_state, ListenState::Idle);
    }

    #[test]
    fn listening_started_is_a_no_op_while_already_listening() {
        let mut turn = TurnState::new();
        turn.on_listening_started();
        turn.on_listening_started();
        assert_eq!(turn.listen_state, ListenState::Listening);
    }
}
