//! Gatilho do pipeline pós-turno e da reflexão — assina os três eventos que `chat::send` já
//! publica (`ThinkingStart`/`StreamChunk`/`ThinkingStop`, ver `modules/chat/send.rs::turn_event_bus`)
//! e, a cada turno fechado, roda [`super::pipeline::run`] e [`reflection::run_reflection`] em
//! sequência — sem editar `chat/send.rs`/`events/keys.rs` (fora do escopo desta rodada; ver
//! `AGENTS.md` da tarefa).
//!
//! Mesmo padrão de `features::mood::affinity_notice::spawn_affinity_notice_listener`: assina o
//! barramento ANTES de `tauri::async_runtime::spawn` (broadcast não entrega retroativamente — ver
//! `events/bus.rs`), e a task só CONSOME o barramento; a única publicação que sai daqui é indireta,
//! via `reflection::run_reflection` gravando em memória episódica (não publica evento nenhum).
//!
//! `ReflectionCadence`/`HeavyCadence`/`TurnAssembler` vivem DENTRO da task — um só listener por
//! processo (chamado uma vez no boot, mesmo ponto que já chama `features::mood::register`), então
//! não precisam de `Mutex`/`OnceLock`: é o mesmo motivo por que `IpcTurnObserver` (`chat/send.rs`)
//! não precisa sincronizar nada, só que a vida útil aqui é o LISTENER inteiro, não um turno.

use std::sync::Arc;

use serde_json::Value;
use tauri::AppHandle;

use super::assembler::TurnAssembler;
use super::cadence::HeavyCadence;
use super::pipeline;
use super::session_accumulator;
use super::super::reflection::{self, ReflectionCadence, ReflectionOutcome};
use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};

fn text_field<'a>(payload: &'a Value, field: &str) -> &'a str {
    payload.get(field).and_then(Value::as_str).unwrap_or_default()
}

/// Loga o desfecho da reflexão no nível certo — `NotDue`/`Blank` são o caminho normal (silencioso
/// demais para `info`, barulhento demais para nunca aparecer), falha é sempre `warn`.
fn log_reflection_outcome(outcome: &ReflectionOutcome) {
    match outcome {
        ReflectionOutcome::NotDue | ReflectionOutcome::Blank | ReflectionOutcome::MemoryDisabled => {}
        ReflectionOutcome::PromptUnavailable(error) => log::warn!("reflexão: prompt indisponível: {error}"),
        ReflectionOutcome::ModelFailed(error) => log::warn!("reflexão: modelo falhou: {error}"),
        ReflectionOutcome::ParseFailed(error) => log::warn!("reflexão: resposta do modelo não era o JSON esperado: {error}"),
        ReflectionOutcome::Stored { memory_id } => {
            log::debug!("reflexão gravada (memory_id={memory_id:?})");
        }
    }
}

/// Um turno fechado (usuário + resposta), na fila para o worker de `run_post_turn_work` abaixo.
struct PostTurnJob {
    user: String,
    assistant: String,
    used_tools: bool,
}

/// `pipeline::run` (fatos, consolidação) e `reflection::run_reflection` chamam o LLM — não são
/// rápidos. Rodá-los DENTRO do laço que também drena `ThinkingStart`/`StreamChunk` travaria
/// `sub.recv()` pela duração do turno inteiro: `broadcast` descarta para quem não drena a tempo
/// (`events/bus.rs`), então um turno de chat pesado faria este assinante perder o `ThinkingStart`
/// do PRÓXIMO turno. O laço do barramento só monta o job e empilha num canal; este worker consome
/// em SÉRIE (um `tokio::sync::mpsc` de consumidor único preserva a ordem de chegada sem precisar de
/// lock) — mesma técnica de fila de `voice::auto_speak` para a síntese de fala.
async fn run_post_turn_work(
    app: AppHandle,
    store: Arc<ConfigStore>,
    mut jobs: tokio::sync::mpsc::UnboundedReceiver<PostTurnJob>,
) {
    let mut reflection_cadence = ReflectionCadence::new();
    let mut heavy_cadence = HeavyCadence::new();
    loop {
        let job = match tokio::time::timeout(IDLE_CONSOLIDATION, jobs.recv()).await {
            Ok(Some(job)) => job,
            Ok(None) => break,
            Err(_idle) => {
                consolidate_session(&app, &store).await;
                continue;
            }
        };
        pipeline::run(&app, &store, &mut heavy_cadence, &job.user, &job.assistant, job.used_tools).await;
        let outcome = reflection::run_reflection(&app, &store, &mut reflection_cadence, &job.user, &job.assistant).await;
        log_reflection_outcome(&outcome);
    }
}

/// A conversation idle this long is over: its short-term session facts consolidate into episodic
/// (long-term) memory and leave the prompt.
pub(super) const IDLE_CONSOLIDATION: std::time::Duration = std::time::Duration::from_secs(20 * 60);

async fn consolidate_session(app: &AppHandle, store: &ConfigStore) {
    let accumulator = session_accumulator::accumulator();
    if accumulator.lines().is_empty() {
        return;
    }
    let Ok(mem) = crate::modules::memory::memory_store(app) else { return };
    let recording_allowed = crate::modules::memory::memory_recording_allowed(store);
    // `flush` grava em SQLite (síncrono): fora do worker async.
    let flushed = tauri::async_runtime::spawn_blocking(move || accumulator.flush(mem, recording_allowed)).await;
    match flushed {
        Ok(Ok(())) => accumulator.clear(),
        Ok(Err(error)) => log::warn!("post_turn: idle consolidation of the session memory failed: {error}"),
        Err(error) => log::warn!("post_turn: idle consolidation of the session memory aborted: {error}"),
    }
}

/// Chamado uma vez no boot (ver o cabeçalho do módulo `post_turn` para a linha exata de
/// `modules/mod.rs::register_config_bound_tools`).
pub fn spawn_post_turn_listener(app: AppHandle, store: Arc<ConfigStore>) {
    // Assina ANTES do spawn — ver o cabeçalho.
    let mut sub = bus().subscribe();
    let (jobs_tx, jobs_rx) = tokio::sync::mpsc::unbounded_channel();
    tauri::async_runtime::spawn(run_post_turn_work(app, store, jobs_rx));

    tauri::async_runtime::spawn(async move {
        let mut assembler = TurnAssembler::new();

        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if super::is_background_turn(&envelope.payload) {
                continue;
            }
            match envelope.key {
                EventKey::ThinkingStart => assembler.on_thinking_start(text_field(&envelope.payload, "userContent")),
                EventKey::StreamChunk => assembler.on_stream_chunk(text_field(&envelope.payload, "text")),
                EventKey::ToolCall => assembler.on_tool_call(),
                EventKey::ThinkingStop => {
                    let Some((user, assistant, used_tools)) = assembler.on_thinking_stop() else { continue };

                    // Porte de `applyExtraction` (`chat/pipeline.ts`) — alimenta o acumulador de
                    // sessão a cada turno fechado, mesmo gatilho do TS. A injeção no prompt mora em
                    // `turn::build_raw_system_prompt` (`session_accumulator.rs` documenta o porquê).
                    // Rápido/síncrono (regex em memória) — continua aqui, não no worker.
                    let (preferences, entities, topics) = session_accumulator::local_extract(&user, &assistant);
                    session_accumulator::accumulator().add_facts(&preferences, &entities, &topics);

                    // `pipeline::run`/`reflection::run_reflection` (LLM) ficam no worker — ver o
                    // cabeçalho de `run_post_turn_work`. Canal sem limite: enfileirar nunca bloqueia
                    // este laço; perder o envio só é possível se o worker já tiver encerrado (app
                    // fechando), caso em que o turno não teria onde ser processado mesmo síncrono.
                    let _ = jobs_tx.send(PostTurnJob { user, assistant, used_tools });
                }
                _ => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_field_devolve_string_vazia_para_campo_ausente_ou_nao_texto() {
        assert_eq!(text_field(&json!({}), "userContent"), "");
        assert_eq!(text_field(&json!({ "userContent": 42 }), "userContent"), "");
    }

    #[test]
    fn a_proactive_turn_never_reaches_the_memory_pipeline() {
        assert!(super::super::is_background_turn(&json!({ "userContent": "", "origin": "background" })));
        assert!(!super::super::is_background_turn(&json!({ "userContent": "oi" })));
    }

    #[test]
    fn text_field_le_o_campo_pedido() {
        assert_eq!(text_field(&json!({ "text": "olá" }), "text"), "olá");
    }

    /// O ponto central desta rodada: enfileirar um `PostTurnJob` nunca espera o worker (LLM) — só
    /// depois disso o laço do assinante fica livre para o PRÓXIMO `ThinkingStart` sem esperar o
    /// turno anterior terminar de refletir. Prova sem `AppHandle` (que `run_post_turn_work` exige):
    /// um consumidor que nunca drena ainda assim recebe todos os `send`s sem bloquear quem manda.
    #[tokio::test]
    async fn queuing_a_post_turn_job_never_waits_for_a_stuck_worker() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PostTurnJob>();

        // Ninguém drena `rx` ainda — simula o worker preso num turno anterior de LLM.
        for i in 0..8 {
            tx.send(PostTurnJob { user: format!("turno {i}"), assistant: "resposta".into(), used_tools: false })
                .expect("enfileirar não pode falhar nem bloquear com o worker ocupado");
        }

        let first = rx.recv().await.expect("o primeiro job enfileirado deve estar disponível assim que alguém drena");
        assert_eq!(first.user, "turno 0", "a ordem de chegada tem que ser preservada (FIFO)");
    }
}
