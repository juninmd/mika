//! O laço de debate de `collaboration` — porte de `CollaborationService`
//! (`apps/desktop/src/main/services/brain/services/collaboration.service.ts`).
//!
//! **Um slot só, processo inteiro** — mesmo desenho do TS (`this.activeSession: CollaborationSession
//! | null`, um campo só na instância singleton). Aqui é `SESSION: OnceLock<Mutex<Option<Session>>>`,
//! mesmo padrão de `chat::turns::registry()`. Uma sessão NOVA substitui a anterior sem cancelar seu
//! laço explicitamente — a antiga se desliga sozinha ao notar, no próximo checkpoint
//! (`is_current`), que seu `id` não é mais o da sessão ativa. É o mesmo comportamento do TS: `start`
//! só sobrescreve `this.activeSession`, nunca chama `stop` na sessão anterior.
//!
//! **Toda função de estado recebe `&Registry` explícito** (mesmo padrão de
//! `chat::turns::register_turn(&turns, ...)`), em vez de ler `registry()` (o `OnceLock` de
//! processo) por conta própria. `start`/`stop` — as únicas chamadas de produção — passam
//! `registry()`; os testes passam um `Registry` LOCAL. Sem isso, testar a guarda de sessão
//! (`is_current`/`clear_if_current`) exigiria tocar o `OnceLock` global, e ele é compartilhado com
//! `collaboration.rs` — dois arquivos de teste rodando em paralelo sobre o MESMO slot mutável e
//! exclusivo seria flake garantido, não só o risco de asserção negativa que `tools::registry()`/
//! `events::bus()` já aceitam (aqueles são registries ADITIVOS; este é um slot que se SOBRESCREVE).
//!
//! **`id: u64` crescente, não `randomUUID()`** — o TS usa UUID só como identidade opaca (nunca
//! compara valor, só igualdade); um contador atômico cumpre o mesmo contrato com menos código.
//!
//! **Por que cancelamento aqui é MAIS forte que o TS, não uma cópia.** `waitPacing`/`endPacing` no
//! TS só resolvem a promise entre turnos — uma chamada de `generateRaw` EM VOO não é abortada por
//! `stopCollaboration`. Aqui, `stop()` também chama `TurnAbortHandle::abort()`, que `run_turn`
//! já observa em `select!` (`turn.rs`, cabeçalho "Abort: por que corre em select!") — parar o
//! debate corta a chamada de rede em curso, não só a próxima. Motivo, não capricho: a mesma lacuna
//! já foi encontrada como bug real nesta migração ("abort tem que propagar", achado em revisão
//! anterior) — reproduzir o buraco do TS aqui seria copiar um defeito conhecido, não paridade.
//!
//! **Emissão no EventBus e gravação em `chat.db`, ambas fechadas nesta rodada.** Cada turno
//! (abertura inclusive) publica `EventKey::AssistantMessage` (`super::assistant_message::
//! publish_debate_turn`, mesmo `[agente]: fala` do TS) e grava `user`/`assistant` no `chat.db`
//! compartilhado (`super::collaboration_history::persist_debate_turn`) — mesma ordem do TS
//! (`nextTurn`): emite ANTES do `waitPacing`, grava depois que a geração seguinte termina com
//! sucesso. `chat_db_path` chega já resolvido pelo chamador (mesmo desenho de `content.rs`);
//! `None` só pula a persistência (log, ver `collaboration_history.rs`), o resto segue igual.
//!
//! **Paridade exata, inclusive a lacuna do TS: o ÚLTIMO turno gerado nunca aparece no chat.** O
//! laço emite a fala de `current_speaker` no TOPO da iteração — a que a iteração ANTERIOR acabou de
//! gerar — antes de pedir a próxima. Ao atingir [`MAX_DEBATE_TURNS`], o laço para ANTES dessa
//! emissão da última geração: ela foi gerada e GRAVADA (`persist_debate_turn` roda em todo `Ok`),
//! mas nunca publicada. Mesmo comportamento de `nextTurn` no TS (o corte de teto acontece antes do
//! `emit` da chamada recursiva seguinte) — reproduzido de propósito, não lacuna nova.
//!
//! **Log: nomes de agente, tópico, contagem de turnos — nunca o texto gerado.** Mesma disciplina do
//! TS (`this.logger.info(\`Starting debate between ${a} and ${b} on topic: ${topic}\`)`,
//! `this.logger.debug(\`Turn: ${cur} -> ${other}\`)`): quem lê o log sabe QUE turnos aconteceram,
//! nunca O QUE foi dito. As duas gravações novas (EventBus, `chat.db`) seguem a mesma regra — só o
//! ERRO de gravação é logado (`collaboration_history.rs`), nunca o conteúdo.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use tokio::sync::Notify;

use crate::config::store::ConfigStore;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortHandle, TurnAbortSignal};

use super::prompts;

/// Mesmo teto de `CollaborationService` (`MAX_DEBATE_TURNS = 8`).
const MAX_DEBATE_TURNS: u32 = 8;
/// Mesmo intervalo de `waitPacing(3000)`.
const PACING_MS: u64 = 3000;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Session {
    id: u64,
    abort: TurnAbortHandle,
    stop_notify: Arc<Notify>,
}

type Registry = Mutex<Option<Session>>;
static SESSION: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    SESSION.get_or_init(|| Mutex::new(None))
}

fn lock(registry: &Registry) -> MutexGuard<'_, Option<Session>> {
    registry.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn is_current(registry: &Registry, id: u64) -> bool {
    lock(registry).as_ref().map(|session| session.id == id).unwrap_or(false)
}

/// Encerra a sessão SÓ se `id` ainda for a ativa — uma sessão superada que termina (cap de turnos
/// ou falha de geração) não pode apagar a sessão NOVA que já assumiu o slot.
fn clear_if_current(registry: &Registry, id: u64) {
    let mut guard = lock(registry);
    if guard.as_ref().map(|session| session.id) == Some(id) {
        *guard = None;
    }
}

/// Porte de `stopCollaboration`: aborta a geração em voo (ver o cabeçalho do arquivo), libera
/// qualquer `waitPacing` pendente e limpa o slot — sempre seguro chamar sem sessão ativa (mesmo
/// `if (this.activeSession)` do TS, aqui `Option::take`).
fn stop_on(registry: &Registry) {
    if let Some(session) = lock(registry).take() {
        session.abort.abort();
        session.stop_notify.notify_waiters();
        log::info!("collaboration: sessão de debate encerrada");
    }
}

pub(super) fn stop() {
    stop_on(registry());
}

/// Registra a sessão nova no slot — parte SÍNCRONA e pura de `start` (sem `tokio::spawn`, sem
/// `ConfigStore`: só quem chama a geração precisa dele), para poder ser testada sem rede. Devolve
/// o `id` atribuído.
fn begin(registry: &Registry) -> (u64, TurnAbortSignal, Arc<Notify>) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (abort_handle, abort_signal) = TurnAbortHandle::new();
    let stop_notify = Arc::new(Notify::new());
    *lock(registry) = Some(Session { id, abort: abort_handle, stop_notify: Arc::clone(&stop_notify) });
    (id, abort_signal, stop_notify)
}

/// Porte de `startDebate` + o `void this.nextTurn(...)` fire-and-forget do TS: registra a sessão
/// (síncrono, ver [`begin`]), devolve na hora — o chamador em `collaboration.rs` já monta a
/// mensagem de sucesso — e roda o laço em background via `tokio::spawn`, sobre o registry GLOBAL
/// (`registry()`, `'static`, exigido por `tokio::spawn`).
pub(super) fn start(store: Arc<ConfigStore>, chat_db_path: Option<PathBuf>, agent_a: String, agent_b: String, topic: String) {
    let (id, abort_signal, stop_notify) = begin(registry());

    log::info!("collaboration: iniciando debate entre {agent_a} e {agent_b} sobre: {topic}");
    tokio::spawn(run_debate(registry(), store, chat_db_path, id, agent_a, agent_b, topic, abort_signal, stop_notify));
}

/// Espera `ms`, mas acorda na hora se `stop()` notificar (equivalente ao `resolve()` de
/// `endPacing` no TS) — sem isso, um `stop` durante a pausa entre turnos ainda esperaria os 3s
/// inteiros antes do laço notar que a sessão morreu.
async fn wait_pacing(ms: u64, stop_notify: &Notify) {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(ms)) => {}
        _ = stop_notify.notified() => {}
    }
}

/// O laço inteiro: abertura + turnos alternados até o teto ou uma falha de geração. Iterativo (não
/// recursivo como o `nextTurn` do TS) só para não precisar de `Box::pin` — o comportamento
/// observável é o mesmo: teto de [`MAX_DEBATE_TURNS`], pausa cancelável entre turnos, guarda de
/// sessão (`is_current`) antes e depois de cada geração. `registry: &'static Registry` explícito
/// (não `registry()` lido por conta própria aqui dentro) — só `start` decide qual registry esta
/// task usa, mesmo padrão de dependência explícita do resto do arquivo.
async fn run_debate(
    registry: &'static Registry,
    store: Arc<ConfigStore>,
    chat_db_path: Option<PathBuf>,
    id: u64,
    agent_a: String,
    agent_b: String,
    topic: String,
    signal: TurnAbortSignal,
    stop_notify: Arc<Notify>,
) {
    let opening_directive = prompts::render(prompts::COLLABORATION_OPENING, &[("agentA", &agent_a), ("agentB", &agent_b), ("topic", &topic)]);

    let mut last_message = match turn::run_turn_llm_raw(&store, Some(&opening_directive), &topic, signal.clone(), &NullObserver).await {
        Ok(result) => {
            // Porte de `startDebate`'s `generateRaw(topic, { systemPrompt: openingDirective,
            // saveToHistory: true })` — o TS grava (user: topic, assistant: openingLine) assim que a
            // abertura termina, antes mesmo do primeiro `nextTurn`.
            super::collaboration_history::persist_debate_turn(&chat_db_path, &topic, &result.text);
            result.text
        }
        Err(error) => {
            log::warn!("collaboration: debate nao comecou: {error}");
            clear_if_current(registry, id);
            return;
        }
    };
    if !is_current(registry, id) {
        return;
    }

    let mut current_speaker = agent_a.clone();
    let mut turns_taken = 0u32;

    loop {
        if turns_taken >= MAX_DEBATE_TURNS {
            log::info!("collaboration: debate encerrado no teto de turnos ({turns_taken}), tópico: {topic}");
            clear_if_current(registry, id);
            return;
        }
        turns_taken += 1;

        let other_speaker = if current_speaker == agent_a { agent_b.clone() } else { agent_a.clone() };
        log::debug!("collaboration: turno {current_speaker} -> {other_speaker}");

        // Porte de `nextTurn`'s `eventBusPort()?.emit(Events.ASSISTANT_MESSAGE, { content:
        // \`[${currentAgent}]: ${lastMessage}\`, ... })` — emitido ANTES do `waitPacing`, mesma
        // ordem do TS: é a fala que ACABOU de ser gerada (abertura ou turno anterior), não a que
        // este laço está prestes a pedir.
        super::assistant_message::publish_debate_turn(crate::events::bus(), &format!("[{current_speaker}]: {last_message}"));

        wait_pacing(PACING_MS, &stop_notify).await;
        if !is_current(registry, id) {
            return;
        }

        let task_directive =
            prompts::render(prompts::COLLABORATION_TURN, &[("speaker", &other_speaker), ("listener", &current_speaker), ("topic", &topic)]);

        let input_for_turn = last_message.clone();
        last_message = match turn::run_turn_llm_raw(&store, Some(&task_directive), &input_for_turn, signal.clone(), &NullObserver).await {
            Ok(result) => {
                // Porte de `generateRaw(lastMessage, { systemPrompt: taskDirective, saveToHistory:
                // true })` — o TS grava (user: input do turno, assistant: resposta nova) a cada
                // volta, com sucesso ou sem, independente do turno ainda virar `ASSISTANT_MESSAGE`
                // (o último gerado antes do teto de turnos nunca chega a ser emitido — ver o
                // cabeçalho do arquivo — mas ainda assim é gravado, igual ao TS).
                super::collaboration_history::persist_debate_turn(&chat_db_path, &input_for_turn, &result.text);
                result.text
            }
            Err(error) => {
                log::warn!("collaboration: debate interrompido por falha na geração: {error}");
                clear_if_current(registry, id);
                return;
            }
        };
        if !is_current(registry, id) {
            return;
        }
        current_speaker = other_speaker;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Todo teste deste módulo usa um `Registry` LOCAL — nunca `registry()` (o `OnceLock` global,
    /// compartilhado com `collaboration.rs` e exclusivo por natureza: só uma sessão "atual" por
    /// vez). Ver o cabeçalho do arquivo para o motivo.
    fn local_registry() -> Registry {
        Mutex::new(None)
    }

    #[test]
    fn begin_makes_its_own_session_current_immediately() {
        let registry = local_registry();
        let (id, ..) = begin(&registry);
        assert!(is_current(&registry, id));
    }

    #[test]
    fn a_second_begin_supersedes_the_first_without_clearing_it_explicitly() {
        let registry = local_registry();
        let (first_id, ..) = begin(&registry);
        let (second_id, ..) = begin(&registry);

        assert_ne!(first_id, second_id);
        assert!(is_current(&registry, second_id));
        assert!(!is_current(&registry, first_id), "a sessão nova deveria suceder a antiga no MESMO slot");
    }

    #[test]
    fn clear_if_current_only_clears_the_matching_id() {
        let registry = local_registry();
        let (id, ..) = begin(&registry);

        clear_if_current(&registry, id + 1_000); // id que nunca existiu neste registry local
        assert!(is_current(&registry, id), "id diferente não deveria limpar a sessão atual");

        clear_if_current(&registry, id);
        assert!(!is_current(&registry, id), "id igual deveria limpar a sessão atual");
    }

    #[test]
    fn stop_on_clears_an_active_session_and_is_a_no_op_when_none_is_active() {
        let registry = local_registry();
        let (id, ..) = begin(&registry);

        stop_on(&registry);
        assert!(!is_current(&registry, id));
        // Chamar de novo sem sessão ativa não deveria panicar.
        stop_on(&registry);
    }

    #[test]
    fn stop_on_aborts_the_turn_signal_of_the_session_it_clears() {
        let registry = local_registry();
        let (id, signal, _stop_notify) = begin(&registry);
        assert!(!signal.is_aborted());

        stop_on(&registry);
        assert!(!is_current(&registry, id));
        assert!(signal.is_aborted(), "stop deveria abortar a geração em voo, não só limpar o slot");
    }
}
