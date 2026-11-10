//! Agenda as chamadas de LLM do processo: o turno de chat nunca espera; tarefa de fundo (extração
//! de fatos/pessoas, reflexão, aprendizado de jogo, fala proativa — tudo que passa por
//! `run_turn_llm_raw`) roda UMA de cada vez e só quando nenhum turno de chat está em voo.
//!
//! **O incidente.** Medido contra um `llama-server` local (Gemma 4, KV unificado de 16k, 4 slots):
//! cada mensagem disparava 2-3 requisições simultâneas — o chat (~9,4k tokens de prompt) e
//! extrações de ~4,2k cada. A soma estourava o contexto e o servidor derrubava TODAS com
//! `Context size has been exceeded`, inclusive a resposta ao usuário (4 de 10 turnos, o primeiro
//! com histórico vazio). Serializar tudo (`-np 1`) matava o erro, mas deixava o chat 70 s na fila
//! atrás das extrações. Daí a assimetria: fundo cede ao chat, chat não cede a ninguém.
//!
//! **Ceding includes the call already in flight** ([`LlmGate::run_background`]): waiting for the
//! slot was not enough — an extraction that had just started streaming kept the single server slot
//! and the reply queued behind it. A chat turn that starts mid-call drops that request (freeing the
//! server slot) and the work runs again after the chat ([`Preempted::Requeue`]), except proactive
//! speech, which is stale once the user speaks ([`Preempted::Drop`]).
//!
//! Chamada "de fundo" feita DE DENTRO do turno de chat (uma tool como `content`/`game_knowledge`
//! pede texto ao LLM no meio do laço) faz parte do turno: esperar o chat acabar seria esperar a si
//! mesma. O `task_local` marca esse escopo e a chamada aninhada passa direto.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use std::time::Duration;

use tokio::sync::{watch, Semaphore, SemaphorePermit};

use super::{TurnAbortSignal, TurnError};

static GATE: LazyLock<LlmGate> = LazyLock::new(LlmGate::new);

/// Teto de uma chamada de fundo COM o slot em mãos. Os clientes HTTP não têm timeout próprio: sem
/// este teto, um servidor que trava uma requisição prendia a fila inteira (visão, fatos, reflexão,
/// fala proativa) até reiniciar o app.
pub(super) const BACKGROUND_TIMEOUT: Duration = Duration::from_secs(180);

/// What a background call does when a chat turn starts while it holds the slot. Either way the
/// in-flight request is dropped at once — on a single-slot local server it was the reply's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Preempted {
    /// Run it again from scratch after the chat (extraction, reflection, summaries, vision).
    Requeue,
    /// The result would already be stale once the user spoke (proactive speech): `Aborted`.
    Drop,
}

tokio::task_local! {
    static IN_CHAT_TURN: ();
}

struct LlmGate {
    /// Quantos turnos de chat estão em voo agora.
    foreground: watch::Sender<usize>,
    /// Turnos de chat já iniciados — quem esperou na fila compara antes/depois para saber se o
    /// usuário falou no meio (uma fala proativa gerada antes disso já nasceu velha).
    started: AtomicU64,
    /// Epoch ms em que o último turno de chat TERMINOU (0 = nenhum ainda). Cobre o defeito major da
    /// revisão de código de `chat::proactive`: `started` só pega um turno real que COMEÇA depois de
    /// `turns_before` ser capturado — um turno que já tinha COMEÇADO E TERMINADO antes disso (a
    /// resposta de interrupção do Rust decodifica STT bem mais devagar que o hangover de silêncio do
    /// STT contínuo do renderer) nunca aparecia na diferença. Comparar este timestamp contra o
    /// instante em que o barge-in foi DETECTADO (bem mais cedo que `turns_before` é capturado hoje)
    /// pega essa janela também.
    completed_at_ms: AtomicU64,
    background: Semaphore,
}

/// Mantém o turno de chat marcado como "em voo" até sair de escopo.
struct ForegroundGuard<'a>(&'a LlmGate);

impl Drop for ForegroundGuard<'_> {
    fn drop(&mut self) {
        self.0.foreground.send_modify(|n| *n = n.saturating_sub(1));
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl LlmGate {
    fn new() -> Self {
        Self {
            foreground: watch::channel(0).0,
            started: AtomicU64::new(0),
            completed_at_ms: AtomicU64::new(0),
            background: Semaphore::new(1),
        }
    }

    async fn in_foreground<F: Future>(&self, turn: F) -> F::Output {
        self.run_foreground(turn, |_| true).await
    }

    /// A barge-in aborts the turn in flight; that abort answered nothing, so it must not stamp
    /// `completed_at_ms` or the interruption reply is dropped as already answered.
    async fn in_chat_turn<T>(&self, turn: impl Future<Output = Result<T, TurnError>>) -> Result<T, TurnError> {
        self.run_foreground(turn, |output| !matches!(output, Err(TurnError::Aborted))).await
    }

    async fn run_foreground<F: Future>(&self, turn: F, answered: impl FnOnce(&F::Output) -> bool) -> F::Output {
        self.started.fetch_add(1, Ordering::SeqCst);
        self.foreground.send_modify(|n| *n += 1);
        let _guard = ForegroundGuard(self);
        let output = IN_CHAT_TURN.scope((), turn).await;
        if answered(&output) {
            self.completed_at_ms.store(now_ms(), Ordering::SeqCst);
        }
        output
    }

    async fn enter_background(&self, abort: &TurnAbortSignal) -> Result<Option<SemaphorePermit<'_>>, TurnError> {
        if IN_CHAT_TURN.try_with(|_| ()).is_ok() {
            return Ok(None);
        }
        let mut abort = abort.clone();
        tokio::select! {
            permit = self.wait_for_background_slot() => Ok(Some(permit)),
            _ = abort.wait_aborted() => Err(TurnError::Aborted),
        }
    }

    /// Background work that yields to chat even mid-call: `attempt` builds a fresh future per try,
    /// raced against a chat turn starting; if the chat wins, the attempt is dropped (cancelling its
    /// request), the slot released, and `on_preempt` decides between waiting to run it again and
    /// giving up. Nested inside a chat turn it just runs — the turn already owns the slot.
    async fn run_background<Fut: Future>(
        &self,
        abort: &TurnAbortSignal,
        on_preempt: Preempted,
        mut attempt: impl FnMut() -> Fut,
    ) -> Result<Fut::Output, TurnError> {
        loop {
            let Some(slot) = self.enter_background(abort).await? else {
                return Ok(attempt().await);
            };
            let mut chat = self.foreground.subscribe();
            tokio::select! {
                output = attempt() => return Ok(output),
                _ = chat.wait_for(|n| *n > 0) => {}
            }
            drop(slot);
            log::info!("llm gate: chat turn started — background call cancelled ({on_preempt:?})");
            if on_preempt == Preempted::Drop {
                return Err(TurnError::Aborted);
            }
        }
    }

    async fn wait_for_background_slot(&self) -> SemaphorePermit<'_> {
        let permit = self.background.acquire().await.expect("o semáforo do gate nunca é fechado");
        let _ = self.foreground.subscribe().wait_for(|n| *n == 0).await;
        permit
    }
}

/// Roda o turno de chat marcado como em voo — tarefas de fundo esperam até ele terminar.
pub(super) async fn in_foreground<F: Future>(turn: F) -> F::Output {
    GATE.in_foreground(turn).await
}

/// `in_foreground` for the chat turn itself: an aborted turn never counts as completed.
pub(super) async fn in_chat_turn<T>(turn: impl Future<Output = Result<T, TurnError>>) -> Result<T, TurnError> {
    GATE.in_chat_turn(turn).await
}

/// Chamada de fundo que não passa por `run_turn_llm_raw` (visão manda imagem direto ao provedor)
/// na mesma fila — senão visão + extração ocupavam os 2 slots do servidor e o chat esperava.
/// `attempt` builds the call again when a chat turn preempts it (see [`Preempted::Requeue`]).
/// `None` = passou de [`BACKGROUND_TIMEOUT`] e foi cancelada (o slot volta para a fila).
pub(super) async fn as_background<F: Future>(mut attempt: impl FnMut() -> F) -> Option<F::Output> {
    let (_never_aborted, signal) = super::TurnAbortHandle::new();
    GATE.run_background(&signal, Preempted::Requeue, || tokio::time::timeout(BACKGROUND_TIMEOUT, attempt())).await.ok()?.ok()
}

/// A background call on the shared queue, preempted by chat turns (see [`LlmGate::run_background`]).
/// Each attempt gets [`BACKGROUND_TIMEOUT`] with the slot in hand.
pub(super) async fn run_background<T, F: Future<Output = Result<T, TurnError>>>(
    abort: &TurnAbortSignal,
    on_preempt: Preempted,
    mut attempt: impl FnMut() -> F,
) -> Result<T, TurnError> {
    let timed = || {
        let call = tokio::time::timeout(BACKGROUND_TIMEOUT, attempt());
        async move {
            call.await.unwrap_or_else(|_| {
                Err(TurnError::Provider(format!("tarefa de fundo passou de {}s e foi cancelada", BACKGROUND_TIMEOUT.as_secs())))
            })
        }
    };
    GATE.run_background(abort, on_preempt, timed).await?
}

/// Leva o contexto do turno (marcador de chat em voo + trace id) para `work` rodar em OUTRA task
/// (`tool_step` executa a tool numa task própria para poder parar de esperá-la). Sem o marcador, uma
/// tool que chama o LLM esperaria a fila de fundo, que espera o próprio turno terminar.
pub(super) fn carry_turn_context<F>(work: F) -> impl Future<Output = F::Output> + Send + 'static
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let in_chat_turn = IN_CHAT_TURN.try_with(|_| ()).is_ok();
    let trace_id = crate::modules::system::log_trace::current();
    async move {
        let work = async move {
            if in_chat_turn {
                IN_CHAT_TURN.scope((), work).await
            } else {
                work.await
            }
        };
        match trace_id {
            Some(id) => crate::modules::system::log_trace::scoped(id, work).await,
            None => work.await,
        }
    }
}

/// Quantos turnos de chat já começaram neste processo.
pub(super) fn chat_turns_started() -> u64 {
    GATE.started.load(Ordering::SeqCst)
}

/// `true` quando o último turno de chat em primeiro plano TERMINOU no mesmo instante ou depois de
/// `since_ms` — ver o comentário de `LlmGate::completed_at_ms`. `since_ms` é epoch ms, mesmo relógio
/// (`SystemTime`) que `now_ms` aqui.
pub(super) fn chat_turn_completed_since(since_ms: u64) -> bool {
    GATE.completed_at_ms.load(Ordering::SeqCst) >= since_ms
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::modules::brain::turn::TurnAbortHandle;

    // Relógio virtual (`start_paused`): `SHORT` é tempo de simulação, avança sozinho quando todas as
    // tasks estão paradas — nenhuma asserção depende do relógio de parede da máquina.
    const SHORT: Duration = Duration::from_millis(50);

    /// Segura um turno de chat "em voo" numa task até o sender ser descartado.
    fn hold_chat(gate: &Arc<LlmGate>) -> tokio::sync::oneshot::Sender<()> {
        let (release, held) = tokio::sync::oneshot::channel::<()>();
        let gate = Arc::clone(gate);
        tokio::spawn(async move { gate.in_foreground(async { let _ = held.await; }).await });
        release
    }

    #[tokio::test(start_paused = true)]
    async fn background_waits_while_a_chat_turn_is_in_flight() {
        let gate = Arc::new(LlmGate::new());
        let (_handle, signal) = TurnAbortHandle::new();
        let chat = hold_chat(&gate);
        tokio::time::sleep(SHORT).await;

        let blocked = tokio::time::timeout(SHORT, gate.enter_background(&signal)).await;
        assert!(blocked.is_err(), "extração não pode disputar o contexto com a resposta ao usuário");

        drop(chat);
        let permit = tokio::time::timeout(SHORT, gate.enter_background(&signal)).await;
        assert!(matches!(permit, Ok(Ok(Some(_)))), "fim do chat libera a fila de fundo");
    }

    #[tokio::test(start_paused = true)]
    async fn background_tasks_run_one_at_a_time() {
        let gate = LlmGate::new();
        let (_handle, signal) = TurnAbortHandle::new();
        let first = gate.enter_background(&signal).await.expect("fila livre");

        let second = tokio::time::timeout(SHORT, gate.enter_background(&signal)).await;
        assert!(second.is_err(), "duas extrações somadas já estouravam o KV compartilhado");

        drop(first);
        assert!(matches!(tokio::time::timeout(SHORT, gate.enter_background(&signal)).await, Ok(Ok(Some(_)))));
    }

    #[tokio::test(start_paused = true)]
    async fn chat_never_waits_for_background_work() {
        let gate = LlmGate::new();
        let (_handle, signal) = TurnAbortHandle::new();
        let _busy = gate.enter_background(&signal).await.expect("fila livre");

        let chat = tokio::time::timeout(SHORT, gate.in_foreground(async { "resposta" })).await;
        assert_eq!(chat, Ok("resposta"), "o chat entra na hora, mesmo com tarefa de fundo em voo");
        assert_eq!(gate.started.load(Ordering::SeqCst), 1, "fala proativa que esperou sabe que o usuário falou");
    }

    /// O ponto central do defeito major da revisão: `completed_at_ms` marca o instante do FIM do
    /// turno (não do início), e só depois que ele sai de escopo — para que comparar contra o
    /// instante de detecção de um barge-in prove que um turno JÁ TERMINOU antes da resposta de
    /// interrupção sequer começar a esperar o LLM (`chat::proactive::run_proactive_turn`).
    #[tokio::test]
    async fn completed_at_ms_only_updates_once_the_chat_turn_ends() {
        let gate = LlmGate::new();
        assert_eq!(gate.completed_at_ms.load(Ordering::SeqCst), 0, "sem turno nenhum ainda, nunca terminou");

        let before = now_ms();
        gate.in_foreground(async {}).await;
        let after = gate.completed_at_ms.load(Ordering::SeqCst);

        assert!(after >= before, "o turno termina no `Drop` do guard, depois de `in_foreground` retornar");
    }

    /// Smoke de voz (2026-09-26): o barge-in aborta o turno de chat em voo, e contar esse abort como
    /// "terminou" fazia `run_proactive_turn` descartar a resposta de interrupção como já respondida —
    /// e o renderer, que cede a ela, também calava. Ninguém respondia.
    #[tokio::test]
    async fn an_aborted_chat_turn_never_counts_as_completed() {
        let gate = LlmGate::new();
        let _ = gate.in_chat_turn(async { Err::<(), _>(TurnError::Aborted) }).await;
        assert_eq!(gate.completed_at_ms.load(Ordering::SeqCst), 0, "turno abortado não respondeu nada");
        let _ = gate.in_chat_turn(async { Ok::<(), TurnError>(()) }).await;
        assert!(gate.completed_at_ms.load(Ordering::SeqCst) > 0, "turno que respondeu conta");
    }

    #[tokio::test(start_paused = true)]
    async fn a_tool_inside_the_chat_turn_does_not_wait_for_its_own_turn() {
        let gate = LlmGate::new();
        let (_handle, signal) = TurnAbortHandle::new();

        let nested = gate.in_foreground(async { tokio::time::timeout(SHORT, gate.enter_background(&signal)).await.map(|r| r.map(|p| p.is_some())) }).await;
        assert!(matches!(nested, Ok(Ok(false))), "esperar o fim do chat de dentro dele seria deadlock");
        assert_eq!(*gate.foreground.borrow(), 0, "o guard sai junto com o turno");
    }

    /// Incident: a fact extraction already streaming from a single-slot local server held that slot,
    /// and the reply to the user queued behind it. A chat turn that starts mid-call cancels the
    /// background attempt (dropping the request frees the server slot) and the work runs again
    /// from scratch once the chat is done.
    #[tokio::test(start_paused = true)]
    async fn a_chat_turn_preempts_background_work_in_flight_and_requeues_it() {
        let gate = Arc::new(LlmGate::new());
        let (_handle, signal) = TurnAbortHandle::new();
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let chat_done = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let worker = {
            let (gate, attempts, chat_done) = (Arc::clone(&gate), Arc::clone(&attempts), Arc::clone(&chat_done));
            tokio::spawn(async move {
                gate.run_background(&signal, Preempted::Requeue, || {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    let chat_done = Arc::clone(&chat_done);
                    async move {
                        tokio::time::sleep(SHORT * 2).await;
                        (attempt, chat_done.load(Ordering::SeqCst))
                    }
                })
                .await
            })
        };
        tokio::time::sleep(SHORT).await;
        let chat = hold_chat(&gate);
        tokio::time::sleep(SHORT).await;
        chat_done.store(true, Ordering::SeqCst);
        drop(chat);

        let outcome = tokio::time::timeout(SHORT * 6, worker).await.expect("requeued work finishes").unwrap();
        assert!(matches!(outcome, Ok((2, true))), "first attempt cancelled, second ran after the chat: {outcome:?}");
    }

    /// Proactive speech is stale the moment the user speaks: preempting it must not replay it.
    #[tokio::test(start_paused = true)]
    async fn preempted_work_that_would_be_stale_is_dropped_not_requeued() {
        let gate = Arc::new(LlmGate::new());
        let (_handle, signal) = TurnAbortHandle::new();
        let worker = {
            let gate = Arc::clone(&gate);
            tokio::spawn(async move { gate.run_background(&signal, Preempted::Drop, || tokio::time::sleep(SHORT * 4)).await })
        };
        tokio::time::sleep(SHORT).await;
        let _chat = hold_chat(&gate);

        let outcome = tokio::time::timeout(SHORT * 2, worker).await.expect("dropped at once, not after the chat").unwrap();
        assert!(matches!(outcome, Err(TurnError::Aborted)));
    }

    #[tokio::test(start_paused = true)]
    async fn abort_releases_a_waiting_background_task() {
        let gate = Arc::new(LlmGate::new());
        let (handle, signal) = TurnAbortHandle::new();
        let _chat = hold_chat(&gate);
        tokio::time::sleep(SHORT).await;

        let waiter = Arc::clone(&gate);
        let waiting = tokio::spawn(async move { waiter.enter_background(&signal).await.map(|_| ()) });
        tokio::time::sleep(SHORT).await;
        handle.abort();

        let outcome = tokio::time::timeout(SHORT, waiting).await.expect("abort não pode ficar preso na fila");
        assert!(matches!(outcome, Ok(Err(TurnError::Aborted))));
    }
}
