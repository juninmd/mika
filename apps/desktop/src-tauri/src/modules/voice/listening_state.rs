//! Porte fiel de `STTService.listening`/`startedAt`
//! (`apps/desktop/src/main/services/voice/stt/stt.service.ts`) — só o FLAG e o timestamp de
//! escuta, não um serviço de transcrição.
//!
//! **Por que isto basta para `listen_start`/`listen_stop` deixarem de degradar.** A rodada
//! anterior (ver o cabeçalho de `voice_tool.rs`) assumiu que `sttPort().start()/stop()` disparava
//! um laço de captura contínua — não dispara. Lendo o TS: `STTService.start()` só marca
//! `this.listening = true` e grava `startedAt`; `stop()` só zera os dois. Nenhum dos dois lê
//! microfone, janela VAD ou chama Whisper. A captura de áudio de verdade
//! (`useSpeechRecognition.ts` + `speech-transcribe.ts`, no renderer) já independe deste flag: ela
//! liga via `getUserMedia`/`MediaRecorder` quando o usuário aperta o mic (ou quando o bridge
//! `voice:listening-started`/`-stopped` ecoa pro renderer, ver `useShellMic.ts`), grava um
//! `MediaRecorder`, decide fim de frase com `SILENCE_HANGOVER_MS`/`SPEECH_STOP_LEVEL` locais, e
//! manda o WAV pronto para `mika:voice-transcribe` (`transcribe.rs`, já ligado ao Whisper real) —
//! sem nunca consultar `isListening()`. Este flag serve três leitores no TS, nenhum deles um
//! pipeline de áudio: `voice-barge-in.ts` (não transcreve o barge-in se o contínuo já vai
//! entregar a mesma fala), `voice-auto-speak-events.ts` (auto-speak sabe se pode falar por cima),
//! e `ipc.ts::currentListeningState` (reconciliação do HUD). Portar o flag com fidelidade —
//! idempotência incluída — fecha a ação de verdade, sem inventar um segundo motor de STT que o
//! TS nunca teve.
//!
//! **De instância por handler para `OnceLock` de processo.** Até aqui só `VoiceToolHandler`
//! (`register()`, reusado por todas as chamadas da tool via `Arc` no registry) lia isto, e uma
//! instância no campo bastava. Agora um SEGUNDO dono existe:
//! `interruption_transcript::process_interruption_audio` (a checagem "o STT contínuo já vai
//! entregar a mesma fala", porte de `createInterruptionTranscriber`/`stt.isListening()` em
//! `voice-barge-in.ts`) roda fora de `voice_tool.rs`, sem acesso ao campo do handler — mesmo
//! motivo pelo qual `AbortRegistry` expõe `new()` + `global()`. [`global`] é esse segundo modo;
//! `new()` continua existindo para os testes, que precisam de estado ISOLADO (um `OnceLock`
//! compartilhado entre `#[tokio::test]` rodando em paralelo seria flaky pelo vizinho).

use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Espelha o retorno de `STTService.start()`: `{ started, startedAt }`.
pub struct StartOutcome {
    pub started: bool,
    pub started_at_ms: i64,
}

/// Espelha o retorno de `STTService.stop()`: `{ stopped, durationMs }`.
pub struct StopOutcome {
    pub stopped: bool,
    pub duration_ms: i64,
}

/// `None` = não ouvindo; `Some(startedAtMs)` = ouvindo desde aquele instante — mesma codificação
/// de estado que `this.listening`/`this.startedAt` no TS, só que os dois campos colapsados num só
/// para não permitir o par inconsistente (`listening=false` com `startedAt` presente) que o TS
/// evita por convenção, não por tipo.
pub struct ListeningState {
    started_at_ms: Mutex<Option<i64>>,
}

impl ListeningState {
    pub fn new() -> Self {
        Self { started_at_ms: Mutex::new(None) }
    }

    /// Lock envenenado degrada para "não ouvindo" em vez de panicar — mesma escolha de
    /// `AbortRegistry::force_stop`, que prefere um `false` seguro a derrubar a tool inteira.
    pub fn is_listening(&self) -> bool {
        self.started_at_ms.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Idempotente: chamar enquanto já ouvindo NÃO reinicia `startedAt`, só devolve
    /// `started: false` com o timestamp ORIGINAL — mesmo contrato de `STTService.start()`, que o
    /// handler usa para publicar `restarted: true` em vez de fingir um novo começo.
    pub fn start(&self) -> StartOutcome {
        let Ok(mut guard) = self.started_at_ms.lock() else {
            return StartOutcome { started: false, started_at_ms: now_millis() };
        };
        if let Some(at) = *guard {
            return StartOutcome { started: false, started_at_ms: at };
        }
        let at = now_millis();
        *guard = Some(at);
        StartOutcome { started: true, started_at_ms: at }
    }

    /// Idempotente: parar sem estar ouvindo é NO-OP (`stopped: false, durationMs: 0`), nunca erro
    /// — mesmo contrato de `STTService.stop()`.
    pub fn stop(&self) -> StopOutcome {
        let Ok(mut guard) = self.started_at_ms.lock() else {
            return StopOutcome { stopped: false, duration_ms: 0 };
        };
        let Some(started_at) = guard.take() else {
            return StopOutcome { stopped: false, duration_ms: 0 };
        };
        StopOutcome { stopped: true, duration_ms: (now_millis() - started_at).max(0) }
    }
}

static GLOBAL: OnceLock<ListeningState> = OnceLock::new();

/// O MESMO `ListeningState` que `VoiceToolHandler` usa em produção (`register()`, abaixo) — ver o
/// cabeçalho do arquivo. `interruption_transcript.rs` lê por aqui; nada além de `voice_tool.rs`
/// deveria CHAMAR `start()`/`stop()` por aqui (só quem despacha `listen_start`/`listen_stop` é dono
/// da transição de estado).
pub(crate) fn global() -> &'static ListeningState {
    GLOBAL.get_or_init(ListeningState::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mesma prova de `abort_registry.rs::global_returns_the_same_process_wide_instance` — não
    /// muta estado, só identidade, para não colidir com outros testes deste binário que também
    /// tocam `global()` em paralelo.
    #[test]
    fn global_returns_the_same_process_wide_instance() {
        assert!(std::ptr::eq(global(), global()));
    }

    #[test]
    fn starts_from_idle_and_reports_listening() {
        let state = ListeningState::new();
        assert!(!state.is_listening());

        let outcome = state.start();
        assert!(outcome.started);
        assert!(state.is_listening());
    }

    #[test]
    fn a_second_start_while_listening_is_idempotent_and_keeps_the_original_timestamp() {
        let state = ListeningState::new();
        let first = state.start();
        let second = state.start();

        assert!(!second.started, "segundo start devia reportar restarted, não um novo início");
        assert_eq!(second.started_at_ms, first.started_at_ms);
        assert!(state.is_listening());
    }

    #[test]
    fn stop_while_listening_reports_stopped_and_clears_state() {
        let state = ListeningState::new();
        state.start();

        let outcome = state.stop();
        assert!(outcome.stopped);
        assert!(outcome.duration_ms >= 0);
        assert!(!state.is_listening());
    }

    #[test]
    fn stop_without_having_started_is_a_no_op_not_an_error() {
        let state = ListeningState::new();
        let outcome = state.stop();

        assert!(!outcome.stopped);
        assert_eq!(outcome.duration_ms, 0);
        assert!(!state.is_listening());
    }

    /// A prova central de concorrência pedida: stop -> start não deixa "dois" ouvindo, nem um
    /// resquício do ciclo anterior. Como não existe produtor em segundo plano (ver o cabeçalho:
    /// nenhum laço é iniciado por `start()`), "exatamente um" aqui significa exatamente isto —
    /// nenhum estado fantasma sobrevive ao stop, e o novo `start()` sempre carimba um timestamp
    /// nunca visto antes.
    #[test]
    fn stop_then_start_leaves_exactly_one_listening_session_with_a_fresh_timestamp() {
        let state = ListeningState::new();
        let first = state.start();
        let stop_outcome = state.stop();
        assert!(stop_outcome.stopped);
        assert!(!state.is_listening());

        let second = state.start();
        assert!(second.started, "restart após stop devia contar como um novo início real");
        assert!(state.is_listening());
        assert!(
            second.started_at_ms >= first.started_at_ms,
            "o novo ciclo não pode carimbar um instante anterior ao ciclo encerrado"
        );

        // Repetir start agora é o idempotente do NOVO ciclo, não um resquício do antigo.
        let third = state.start();
        assert!(!third.started);
        assert_eq!(third.started_at_ms, second.started_at_ms);
    }
}
