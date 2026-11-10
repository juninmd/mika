//! Port de `services/voice/services/expression-sync.ts::ExpressionSyncService`: traduz as
//! transições de voz do barramento (`ThinkingStart`/`ThinkingStop`/`SpeechStarted`/
//! `SpeechFinished`/`ListeningStarted`/`ListeningStopped`/`ChatError`, todas `EventKey` que já
//! existem) em `EventKey::ActivityChanged` (`idle`/`listening`/`thinking`/`speaking`/`error`), com
//! o MESMO debounce de 800ms para `idle` — o avatar não pisca de volta a `idle` entre duas frases
//! do mesmo turno.
//!
//! **Fora de escopo desta rodada:** `Events.EXPRESSION` por estado (`stateToExpression` no TS,
//! `idleExpression`/`listeningExpression`/etc. configuráveis) — já tem publicador próprio via a
//! tool `avatar` (`features::character::expression_bridge`, ver `events/keys.rs`); duplicar aqui
//! competiria pela mesma pose sem coordenação. `acting`/`confirming`/`offline` (mapeados para
//! `thinking`/`neutral` no TS) também ficam de fora — nenhum publicador Rust hoje emite esses
//! estados.
//!
//! `PureExpressionSync` é a máquina de estado pura (porta `setState`/`applyState`), testável sem
//! `EventBus` nem timer nenhum; [`spawn_expression_sync`] é o único ponto impuro (assina o
//! barramento, drena o debounce com `tokio::select!`, publica).

use std::time::Duration;

use serde_json::json;

use crate::events::{EventBus, EventKey};

const DEBOUNCE_MS: u64 = 800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceState {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Error,
}

impl VoiceState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Listening => "listening",
            Self::Thinking => "thinking",
            Self::Speaking => "speaking",
            Self::Error => "error",
        }
    }
}

/// Os 7 sinais do barramento que este serviço reage — mapeados 1:1 de `EventKey`, não de string
/// solta, para o `match` do assinante nunca divergir do vocabulário real.
#[derive(Debug, Clone, Copy)]
pub enum VoiceSignal {
    ThinkingStart,
    ThinkingStop,
    SpeechStarted,
    SpeechFinished,
    ListeningStarted,
    ListeningStopped,
    ChatError,
}

fn signal_for_key(key: EventKey) -> Option<VoiceSignal> {
    match key {
        EventKey::ThinkingStart => Some(VoiceSignal::ThinkingStart),
        EventKey::ThinkingStop => Some(VoiceSignal::ThinkingStop),
        EventKey::SpeechStarted => Some(VoiceSignal::SpeechStarted),
        EventKey::SpeechFinished => Some(VoiceSignal::SpeechFinished),
        EventKey::ListeningStarted => Some(VoiceSignal::ListeningStarted),
        EventKey::ListeningStopped => Some(VoiceSignal::ListeningStopped),
        EventKey::ChatError => Some(VoiceSignal::ChatError),
        _ => None,
    }
}

/// O que o chamador (o assinante async) precisa fazer depois de [`PureExpressionSync::on_signal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Publica este estado agora e cancela qualquer debounce de `idle` pendente.
    Apply(VoiceState),
    /// Agenda (ou reagenda) a volta a `idle` para daqui a `DEBOUNCE_MS` — `speaking` terminou faz
    /// pouco tempo.
    ScheduleIdle,
    /// Nenhuma troca de estado, mas um debounce pendente tem de ser cancelado (pedido de estado
    /// não-`idle` repetindo o estado atual — mesma ordem de `clearDebounce()` antes do `return`
    /// cedo no TS).
    ClearDebounce,
    /// Nada muda, debounce pendente (se algum) continua intocado.
    Unchanged,
}

/// Máquina de estado pura — porta `currentState`/`lastSpeakingTime`/`setState`/`applyState` do TS,
/// sem o timer (isso é responsabilidade do assinante async, ver [`spawn_expression_sync`]).
pub struct PureExpressionSync {
    current: VoiceState,
    last_speaking_ms: u64,
}

impl PureExpressionSync {
    pub fn new() -> Self {
        Self { current: VoiceState::Idle, last_speaking_ms: 0 }
    }

    /// Porta `onListeningStarted`/`onListeningStopped` (guardas de estado) e o resto dos handlers
    /// (`setState` incondicional) do `AutoSpeakTurn`/`ExpressionSyncService` do TS.
    pub fn on_signal(&mut self, signal: VoiceSignal, now_ms: u64) -> Transition {
        match signal {
            VoiceSignal::ListeningStarted => {
                if self.current == VoiceState::Idle {
                    self.set_state(VoiceState::Listening, now_ms)
                } else {
                    Transition::Unchanged
                }
            }
            VoiceSignal::ListeningStopped => {
                if self.current == VoiceState::Listening {
                    self.set_state(VoiceState::Idle, now_ms)
                } else {
                    Transition::Unchanged
                }
            }
            VoiceSignal::ThinkingStart => self.set_state(VoiceState::Thinking, now_ms),
            VoiceSignal::ThinkingStop => self.set_state(VoiceState::Idle, now_ms),
            VoiceSignal::SpeechStarted => self.set_state(VoiceState::Speaking, now_ms),
            VoiceSignal::SpeechFinished => self.set_state(VoiceState::Idle, now_ms),
            VoiceSignal::ChatError => self.set_state(VoiceState::Error, now_ms),
        }
    }

    /// Porta `setState`: qualquer estado ativo cancela um `idle` adiado ANTES de checar se repete
    /// o estado atual (mesma ordem do TS — repetir um estado não-`idle` ainda limpa o debounce).
    fn set_state(&mut self, state: VoiceState, now_ms: u64) -> Transition {
        let clears_debounce = state != VoiceState::Idle;

        if state == self.current {
            return if clears_debounce { Transition::ClearDebounce } else { Transition::Unchanged };
        }

        if state == VoiceState::Idle && self.last_speaking_ms > 0 {
            let elapsed = now_ms.saturating_sub(self.last_speaking_ms);
            if elapsed < DEBOUNCE_MS {
                return Transition::ScheduleIdle;
            }
        }

        self.apply(state, now_ms);
        Transition::Apply(state)
    }

    fn apply(&mut self, state: VoiceState, now_ms: u64) {
        self.current = state;
        if state == VoiceState::Speaking {
            self.last_speaking_ms = now_ms;
        }
    }

    /// Porta o `applyState('idle')` que o `setTimeout` do TS chama direto — sem guarda, sem
    /// checagem de debounce (o timer JÁ é a checagem).
    pub fn force_idle(&mut self, now_ms: u64) -> VoiceState {
        self.apply(VoiceState::Idle, now_ms);
        VoiceState::Idle
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn publish_activity(bus: &EventBus, state: VoiceState) {
    bus.publish(
        EventKey::ActivityChanged,
        json!({ "channel": "voice", "state": state.as_str(), "at": now_ms() }),
    );
}

/// Assina o barramento e publica `ActivityChanged` a cada transição — chamado de
/// `modules::voice::mod.rs`, ao lado de `spawn_auto_speak` (mesmo motivo: precisa existir desde o
/// boot, antes de qualquer `SpeechStarted` real).
///
/// Assina ANTES do `spawn` — `broadcast` não entrega retroativamente (mesmo aviso do resto do
/// crate). O debounce usa `tokio::select!` sobre um prazo opcional em vez de um `spawn` por
/// timer: um único task dono do estado evita corrida entre "novo evento chegou" e "debounce
/// venceu" que dois tasks concorrentes teriam.
pub fn spawn_expression_sync(bus: &'static EventBus) {
    let mut sub = bus.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut sync = PureExpressionSync::new();
        let mut debounce_deadline: Option<tokio::time::Instant> = None;
        let mut shutdown_rx = crate::shutdown::subscribe();

        loop {
            let sleep_until = debounce_deadline.unwrap_or_else(|| tokio::time::Instant::now() + Duration::from_secs(3600));
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                envelope = sub.recv() => {
                    let Some(envelope) = envelope else { break };
                    // A proactive turn nobody asked for must not light "thinking" on the HUD.
                    if crate::modules::brain::post_turn::is_background_turn(&envelope.payload) {
                        continue;
                    }
                    let Some(signal) = signal_for_key(envelope.key) else { continue };
                    match sync.on_signal(signal, now_ms()) {
                        Transition::Apply(state) => {
                            debounce_deadline = None;
                            publish_activity(bus, state);
                        }
                        Transition::ScheduleIdle => {
                            debounce_deadline = Some(tokio::time::Instant::now() + Duration::from_millis(DEBOUNCE_MS));
                        }
                        Transition::ClearDebounce => {
                            debounce_deadline = None;
                        }
                        Transition::Unchanged => {}
                    }
                }
                _ = tokio::time::sleep_until(sleep_until), if debounce_deadline.is_some() => {
                    debounce_deadline = None;
                    let state = sync.force_idle(now_ms());
                    publish_activity(bus, state);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_start_applies_immediately_from_idle() {
        let mut sync = PureExpressionSync::new();
        assert_eq!(sync.on_signal(VoiceSignal::ThinkingStart, 1_000), Transition::Apply(VoiceState::Thinking));
    }

    #[test]
    fn listening_started_is_ignored_outside_idle() {
        let mut sync = PureExpressionSync::new();
        sync.on_signal(VoiceSignal::ThinkingStart, 1_000);
        assert_eq!(sync.on_signal(VoiceSignal::ListeningStarted, 1_100), Transition::Unchanged);
    }

    #[test]
    fn listening_stopped_only_leaves_listening() {
        let mut sync = PureExpressionSync::new();
        assert_eq!(sync.on_signal(VoiceSignal::ListeningStopped, 1_000), Transition::Unchanged);
        sync.on_signal(VoiceSignal::ListeningStarted, 1_000);
        assert_eq!(sync.on_signal(VoiceSignal::ListeningStopped, 1_100), Transition::Apply(VoiceState::Idle));
    }

    #[test]
    fn idle_right_after_speaking_is_debounced() {
        let mut sync = PureExpressionSync::new();
        sync.on_signal(VoiceSignal::SpeechStarted, 1_000);
        assert_eq!(sync.on_signal(VoiceSignal::SpeechFinished, 1_100), Transition::ScheduleIdle);
    }

    #[test]
    fn idle_well_after_speaking_applies_immediately() {
        let mut sync = PureExpressionSync::new();
        sync.on_signal(VoiceSignal::SpeechStarted, 1_000);
        assert_eq!(sync.on_signal(VoiceSignal::SpeechFinished, 1_000 + DEBOUNCE_MS + 1), Transition::Apply(VoiceState::Idle));
    }

    #[test]
    fn a_new_speaking_turn_within_the_debounce_window_clears_it() {
        let mut sync = PureExpressionSync::new();
        sync.on_signal(VoiceSignal::SpeechStarted, 1_000);
        sync.on_signal(VoiceSignal::SpeechFinished, 1_100); // scheduled idle
        assert_eq!(sync.on_signal(VoiceSignal::ThinkingStart, 1_150), Transition::Apply(VoiceState::Thinking));
    }

    #[test]
    fn repeating_a_non_idle_state_still_clears_a_pending_debounce() {
        let mut sync = PureExpressionSync::new();
        sync.on_signal(VoiceSignal::ThinkingStart, 1_000);
        assert_eq!(sync.on_signal(VoiceSignal::ThinkingStart, 1_050), Transition::ClearDebounce);
    }

    #[test]
    fn chat_error_applies_from_any_state() {
        let mut sync = PureExpressionSync::new();
        sync.on_signal(VoiceSignal::ThinkingStart, 1_000);
        assert_eq!(sync.on_signal(VoiceSignal::ChatError, 1_050), Transition::Apply(VoiceState::Error));
    }

    #[test]
    fn force_idle_bypasses_every_guard() {
        let mut sync = PureExpressionSync::new();
        sync.on_signal(VoiceSignal::SpeechStarted, 1_000);
        assert_eq!(sync.force_idle(1_050), VoiceState::Idle);
    }
}
