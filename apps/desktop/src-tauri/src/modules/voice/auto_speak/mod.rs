//! Porte de `apps/desktop/src/main/services/voice/services/auto-speak.service.ts` +
//! `auto-speak-turn.ts` + `voice-auto-speak-events.ts` — a Mika passa a FALAR a própria resposta de
//! chat, não só escrevê-la. Até esta rodada `voice.autoSpeak` era um toggle de UI sem runtime
//! nenhum atrás: `chat::send` já publica `ThinkingStart`/`StreamChunk`/`ToolCall`/`ThinkingStop`
//! (ver `events/keys.rs`), e ninguém assinava para cortar frase e falar.
//!
//! **Um único assinante para chat E `SpeechRequest`.** `spawn_speech_request_listener`
//! (`voice/tts/bus_bridge.rs`) e a fala do turno de chat chamavam
//! [`crate::modules::voice::speak_tool::run_speak`] em DUAS tasks separadas — nada impedia as duas
//! de falar ao mesmo tempo, sobrepondo áudio. Esta função assina o barramento sozinha e processa
//! CADA evento sequencialmente dentro do mesmo laço `while let Some(envelope) = sub.recv().await`:
//! como só há uma task, `run_speak` de um evento sempre termina antes do próximo ser tratado — a
//! serialização vem de graça, sem fila explícita. `bus_bridge::spawn_speech_request_listener`
//! continua existindo só para quem ainda a chame fora deste módulo (nenhum chamador de produção
//! depois desta rodada — ver o relatório final para a linha exata a remover de `modules/mod.rs`).
//!
//! **O que NÃO foi portado** — ver o cabeçalho de `turn.rs` para `ExpressionSyncService`, e o
//! relatório final da rodada para a lista completa com o motivo de cada corte.
//!
//! **Síntese numa fila, fora do laço do assinante.** `run_speak` (TTS local) não é instantâneo —
//! `.await`á-lo INLINE no `match` travaria `sub.recv()` pela duração da síntese, e `broadcast`
//! descarta quem não drena a tempo (`events/bus.rs`). A ORDEM da fala importa (frases não podem
//! trocar de lugar nem sobrepor áudio), então isto não vira tasks concorrentes como
//! `awareness::spawn_reaction`: [`enqueue_speak`] só marca `speak_guard` (síncrono, na hora de
//! sempre) e empilha num `mpsc` de consumidor ÚNICO ([`spawn_speak_worker`]). Um barge-in incrementa
//! a geração compartilhada; o worker descarta qualquer job de geração anterior antes de sintetizar.

mod assistant_message;
pub mod speech_text;
pub mod stream_sentences;
pub mod turn;
mod turn_timing;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::store::ConfigStore;
use crate::events::{EventBus, EventKey};
use crate::modules::voice::speak_guard;

use speech_text::strip_emotion_tags;
use stream_sentences::{flush_speakable, take_complete_sentences, take_first_clause};
use turn::TurnState;
use turn_timing::TurnTiming;

/// Primeira frase do turno fecha com menos texto: o que importa ao vivo é o primeiro áudio, não o
/// tamanho do chunk — mesmo valor de `AutoSpeakService`.
const FIRST_SENTENCE_MIN_CHARS: usize = 12;

fn now_millis() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Frase marcada em `speak_guard` na fila de síntese; `timestamp_ms` renova o eco ao tocar.
struct SpeakJob {
    text: String,
    timestamp_ms: u64,
    /// Geração da fila no instante em que a frase entrou; o worker descarta o job se a geração já
    /// tiver avançado (barge-in) antes de chegar a vez dele.
    generation: u64,
}

/// Marca a frase no tracker GLOBAL agora (mesmo instante de sempre — nada muda para quem lê
/// `speak_guard`) e empilha para o worker sintetizar, SEM esperar a vez dela: enfileirar nunca
/// bloqueia o laço do assinante.
fn enqueue_speak(tx: &UnboundedSender<SpeakJob>, generation: &AtomicU64, text: String, timestamp_ms: u64) {
    speak_guard::mark_spoken(timestamp_ms, &text, now_millis());
    let _ = tx.send(SpeakJob { text, timestamp_ms, generation: generation.load(Ordering::SeqCst) });
}

/// Consome a fila de fala em SÉRIE, uma frase de cada vez — a mesma serialização que o laço único
/// já dava de graça antes desta rodada, só que agora numa task própria, longe do `sub.recv()` do
/// barramento. Loga, nunca panica — mesma política de `spawn_speech_request_listener` que este
/// módulo substituiu.
async fn spawn_speak_worker(
    store: Arc<ConfigStore>,
    cache_dir: PathBuf,
    bus: &'static EventBus,
    generation: Arc<AtomicU64>,
    timing: TurnTiming,
    mut jobs: UnboundedReceiver<SpeakJob>,
) {
    while let Some(job) = jobs.recv().await {
        if job.generation != generation.load(Ordering::SeqCst) {
            log::debug!("frase descartada da fila de fala: barge-in aconteceu antes da vez dela");
            continue;
        }
        speak_guard::refresh_echo(job.timestamp_ms, &job.text, now_millis());
        let result = crate::modules::voice::speak_tool::run_speak(&store, &cache_dir, bus, &job.text).await;
        if result.get("success").and_then(Value::as_bool) == Some(true) {
            // Observabilidade (specs/10-observability.md): só a PRIMEIRA síntese do turno vira
            // linha — `note_audio_ready` já garante isso (ver o cabeçalho de `turn_timing.rs`).
            if let Some(elapsed_ms) = timing.note_audio_ready(now_millis()) {
                log::info!("turno: primeiro áudio em {elapsed_ms}ms");
                crate::modules::llm::turn_metrics::record(crate::modules::llm::turn_metrics::Mark::FirstAudio, elapsed_ms);
            }
        } else {
            let reason = result.get("error").and_then(Value::as_str).unwrap_or("motivo não informado");
            log::warn!("auto-speak não virou áudio: {reason}");
        }
    }
}

/// `ForceStopTts` sempre; `Interruption` só a de fala real — o endpoint por silêncio
/// (`is_silence_endpoint`, sem `is_interruption`) é "o usuário parou de falar", não um corte.
fn is_barge_in(key: EventKey, payload: &Value) -> bool {
    match key {
        EventKey::ForceStopTts => true,
        EventKey::Interruption => {
            payload.get("is_interruption").and_then(Value::as_bool) == Some(true)
                && payload.get("is_silence_endpoint").and_then(Value::as_bool) != Some(true)
        }
        _ => false,
    }
}

/// `chat:thinking-start`: zera o turno local E o tracker GLOBAL (`speak_guard::reset_turn`) — pedido
/// explícito da coordenação: sem a segunda chamada, `spoken_prefix()` (lido pelo barge-in em
/// `interruption_transcript.rs`) cresceria turno após turno em vez de refletir só o que a Mika já
/// disse NESTE turno.
fn handle_thinking_start(turn: &mut TurnState) {
    turn.on_turn_start();
    speak_guard::reset_turn();
}

/// `chat:stream-chunk`: acumula o delta, corta frases prontas (`stream_sentences`) e ENFILEIRA cada
/// uma (nunca sintetiza aqui — ver o cabeçalho do módulo). Síncrona: nada aqui espera o TTS.
fn handle_stream_chunk(
    turn: &mut TurnState,
    store: &ConfigStore,
    tx: &UnboundedSender<SpeakJob>,
    generation: &AtomicU64,
    text: &str,
) {
    if text.is_empty() || turn.stream_aborted || !turn::gate_open(store) {
        return;
    }

    turn.stream_buffer.push_str(text);
    let first = turn.stream_spoken.is_empty();
    let mut cut = take_complete_sentences(&turn.stream_buffer, false, first.then_some(FIRST_SENTENCE_MIN_CHARS));
    // Nada falado ainda e a frase não fechou: a primeira oração já basta para o primeiro áudio.
    if first && cut.ready.is_empty() {
        cut = take_first_clause(&turn.stream_buffer);
    }
    turn.stream_buffer = cut.rest;

    for sentence in cut.ready {
        let clean = strip_emotion_tags(&sentence);
        if clean.is_empty() {
            continue;
        }
        if let Some(text) = turn.record_streamed_sentence(&clean) {
            enqueue_speak(tx, generation, text, now_millis());
        }
    }
}

/// Descarrega o que sobrou no buffer sem esperar pontuação seguida de espaço — mesma mecânica de
/// `flushSpeakable` do TS, compartilhada por `chat:tool-call` e `chat:thinking-stop` (ver os dois
/// chamadores abaixo para o porquê de cada um precisar disto).
fn flush_pending_buffer(turn: &mut TurnState, tx: &UnboundedSender<SpeakJob>, generation: &AtomicU64) {
    if turn.stream_aborted || turn.stream_buffer.trim().is_empty() {
        return;
    }
    let cut = flush_speakable(&turn.stream_buffer);
    turn.stream_buffer = cut.rest;

    for sentence in cut.ready {
        let clean = strip_emotion_tags(&sentence);
        if clean.is_empty() {
            continue;
        }
        if let Some(text) = turn.record_streamed_sentence(&clean) {
            enqueue_speak(tx, generation, text, now_millis());
        }
    }
}

/// `chat:thinking-stop`: fim do turno. `chat::send` NÃO publica `AssistantMessage` para um turno
/// comum (duplicaria a mensagem na janela via `assistant_bridge` — só `content`/
/// `collaboration_session` publicam aquela chave), então este é o único sinal de "acabou" que um
/// turno de chat normal tem. Sem isto, uma resposta cuja última frase não termina em pontuação
/// (`take_complete_sentences` exige `[.!?…]` ou `\n` para cortar) nunca seria falada — ficaria presa
/// em `stream_buffer` para sempre, porque não há próximo `StreamChunk` que a feche.
///
/// `ThinkingStop` também é publicado depois de um ABORT — `flush_pending_buffer` já respeita
/// `stream_aborted` (barge-in já zerou o buffer em `on_barge_in`), então um turno cancelado não
/// enfileira a cauda que sobrou.
fn handle_thinking_stop(turn: &mut TurnState, tx: &UnboundedSender<SpeakJob>, generation: &AtomicU64) {
    flush_pending_buffer(turn, tx, generation);
}

/// Assina o barramento e liga fala automática de chat + `SpeechRequest` num único laço serial de
/// LEITURA (a ORDEM em que uma frase entra na fila continua vindo daqui, de graça); a síntese em si
/// mora no worker de [`spawn_speak_worker`] — ver o cabeçalho do módulo.
pub fn spawn_auto_speak(store: Arc<ConfigStore>, cache_dir: PathBuf, bus: &'static EventBus) {
    // Assina ANTES do spawn — `broadcast::Sender` não entrega retroativamente, mesmo aviso do
    // cabeçalho de `voice/tts/bus_bridge.rs::spawn_force_stop_listener`.
    let mut sub = bus.subscribe();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<SpeakJob>();
    let generation = Arc::new(AtomicU64::new(0));
    let timing = TurnTiming::new();

    // `tauri::async_runtime::spawn`, NUNCA `tokio::spawn`: chamado de `lib.rs::setup`, fora do
    // contexto do reator — ver o mesmo gate documentado em `modules/mod.rs`.
    tauri::async_runtime::spawn(spawn_speak_worker(Arc::clone(&store), cache_dir, bus, Arc::clone(&generation), timing.clone(), rx));

    tauri::async_runtime::spawn(async move {
        let mut turn = TurnState::new();
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            match envelope.key {
                EventKey::ThinkingStart => {
                    handle_thinking_start(&mut turn);
                    // The turn was accepted (before prompt composition) — see `turn_timing.rs`.
                    timing.mark_turn_started(now_millis());
                }
                EventKey::StreamChunk => {
                    let text = envelope.payload.get("text").and_then(Value::as_str).unwrap_or("");
                    handle_stream_chunk(&mut turn, &store, &tx, &generation, text);
                }
                // O modelo parou para chamar uma ferramenta: enfileira AGORA o que já está pronto no
                // buffer, sem esperar o resultado (a pontuação seguida de espaço só viria depois
                // dele). Nada é inventado para preencher o silêncio — toda fala da Mika é gerada
                // por LLM, e uma frase de enchimento fixa não é.
                EventKey::ToolCall => flush_pending_buffer(&mut turn, &tx, &generation),
                EventKey::ThinkingStop => handle_thinking_stop(&mut turn, &tx, &generation),
                EventKey::AssistantMessage => {
                    assistant_message::handle_assistant_message(&mut turn, &store, &tx, &generation, &envelope.payload);
                }
                // Ambos sinalizam barge-in do usuário — `ForceStopTts` já cala o ÁUDIO em voo
                // (`bus_bridge::spawn_force_stop_listener`); aqui é a metade que falta: descartar o
                // que ainda estava no buffer (`on_barge_in`) E qualquer frase já enfileirada mas
                // ainda não sintetizada (`generation`, ver o cabeçalho do módulo) — sem o segundo
                // passo, frases enfileiradas ANTES do barge-in ainda seriam ditas depois dele.
                EventKey::ForceStopTts | EventKey::Interruption => {
                    if is_barge_in(envelope.key, &envelope.payload) {
                        turn.on_barge_in();
                        generation.fetch_add(1, Ordering::SeqCst);
                    }
                }
                // Porte de `AutoSpeakTurn.onListeningStarted`/`onListeningStopped` — ver o cabeçalho
                // de `turn.rs` para o porquê de guardar só o guard idle/listening (sem
                // `ExpressionSyncService` inteira, que é quem de fato usaria isto para trocar a
                // expressão do avatar; fora do escopo desta rodada).
                EventKey::ListeningStarted => turn.on_listening_started(),
                EventKey::ListeningStopped => turn.on_listening_stopped(),
                EventKey::SpeechRequest => {
                    if let Some(text) = assistant_message::speech_request_text(envelope.key, &envelope.payload) {
                        enqueue_speak(&tx, &generation, text, now_millis());
                    }
                }
                _ => {}
            }
        }
    });
}

// Teto de 300 linhas do AGENTS.md: os casos de teste moram em `mod_tests.rs`.
#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
