//! Casos de `mod.rs`, extraídos por causa do teto de 300 linhas do AGENTS.md.

use super::assistant_message::{speech_request_text, was_already_spoken};
use super::*;
use crate::config::schemas;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// O tracker de fala e o mute de síntese são de PROCESSO; testes que os tocam correm em série.
static GLOBAL_STATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// O filtro do laço de fala. Chave errada nunca vira fala (o barramento entrega TUDO a todo
/// assinante), e texto vazio/só espaço não vale uma ida ao TTS.
#[test]
fn speech_request_text_only_fires_for_the_right_key_with_real_text() {
    assert_eq!(
        speech_request_text(EventKey::SpeechRequest, &serde_json::json!({ "text": "oi" })),
        Some("oi".to_string())
    );
    assert_eq!(
        speech_request_text(EventKey::SpeechRequest, &serde_json::json!({ "text": "  espaço nas bordas  " })),
        Some("espaço nas bordas".to_string())
    );
    assert_eq!(speech_request_text(EventKey::SpeechRequest, &serde_json::json!({ "text": "   " })), None);
    assert_eq!(speech_request_text(EventKey::SpeechRequest, &serde_json::json!({})), None);
    // A chave errada é o caso que mais importa: `AssistantMessage` também carrega `text`, e sem o
    // filtro de chave todo texto que passa pelo barramento viraria fala.
    assert_eq!(speech_request_text(EventKey::AssistantMessage, &serde_json::json!({ "text": "oi" })), None);
}

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-auto-speak-mod-test-{}-{unique}", std::process::id()));
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

/// Um canal descartável, do jeito que `spawn_auto_speak` monta um de verdade — sem worker
/// consumindo (`_rx` solto), então nada aqui jamais chama `run_speak`/sintetiza áudio de verdade.
/// Antes desta rodada os testes deste arquivo precisavam mutar `speech_mute` para evitar síntese
/// real (`run_speak` era chamado INLINE); agora isso é estruturalmente impossível — os handlers
/// só enfileiram, nunca sintetizam — então esse truque saiu daqui.
fn speak_queue() -> (UnboundedSender<SpeakJob>, AtomicU64) {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    (tx, AtomicU64::new(0))
}

/// O gap pedido pela coordenação: `chat:send` nunca publica `AssistantMessage` para um turno comum
/// (duplicaria a mensagem na janela), então `ThinkingStop` é o único fim-de-turno que uma resposta
/// sem pontuação final tem — sem isto a última frase ficava presa em `stream_buffer` para sempre.
///
/// `enqueue_speak` marca `speak_guard::mark_spoken` no MOMENTO do enfileiramento, não quando o
/// worker (que este teste nem sobe) eventualmente sintetiza — ver o cabeçalho de `mod.rs`. O
/// timestamp é capturado ANTES da chamada e reusado na asserção.
///
/// `was_already_spoken` lê `speak_guard::global()` — um tracker de PROCESSO, compartilhado com
/// QUALQUER outro teste do binário que fale a mesma frase no mesmo instante. A frase usada aqui é
/// específica o bastante (nome do teste embutido no texto) para a colisão ser desprezível; a
/// alternativa — instanciar um `SpeakGuard` isolado — não provaria que `mod.rs` de fato chama o
/// tracker global, que é exatamente o gap que esta rodada fechou.
#[tokio::test]
async fn thinking_stop_flushes_the_pending_buffer_and_marks_it_spoken() {
    let _serial = GLOBAL_STATE_LOCK.lock().await;
    let (tx, generation) = speak_queue();
    let mut turn = TurnState::new();
    let sentence = "frase sem ponto final do teste thinking-stop-flush";
    turn.stream_buffer = sentence.to_string();

    let before = now_millis();
    handle_thinking_stop(&mut turn, &tx, &generation);

    assert!(turn.stream_buffer.is_empty(), "o resto sem pontuação tinha que ser descarregado");
    assert!(was_already_spoken(before, sentence, before), "o tracker global devia ter marcado a frase");
}

/// `ThinkingStop` também dispara depois de um ABORT (turno cancelado) — o barge-in já esvaziou
/// `stream_buffer` em `on_barge_in`; este teste prova que `flush_pending_buffer` respeita
/// `stream_aborted` e não fala nada mesmo que o buffer tivesse sido repopulado depois (e, portanto,
/// nunca chama `speak_guard::mark_spoken` para essa frase).
#[tokio::test]
async fn thinking_stop_after_an_abort_never_speaks_the_leftover() {
    let _serial = GLOBAL_STATE_LOCK.lock().await;
    let (tx, generation) = speak_queue();
    let mut turn = TurnState::new();
    turn.on_barge_in();
    // Simula texto que teria chegado depois do barge-in (o turno abortado ainda pode receber
    // deltas atrasados antes do `ThinkingStop` do lado do chat).
    let sentence = "resto do teste thinking-stop-abort que ninguem deve falar";
    turn.stream_buffer = sentence.to_string();

    handle_thinking_stop(&mut turn, &tx, &generation);

    assert_eq!(turn.stream_buffer, sentence, "aborted: flush não deve nem tocar o buffer");
    let now = now_millis();
    assert!(!was_already_spoken(now, sentence, now));
}

/// O gap central pedido pela coordenação: `voice::auto_speak` tinha seu próprio `SpeakGuard`,
/// então o tracker global (`speak_guard::global`) que o barge-in lê ficava sempre vazio para
/// qualquer coisa dita pela fala automática. Prova de ponta a ponta: uma frase falada via
/// `handle_stream_chunk` aparece em `speak_guard::spoken_prefix()`.
#[tokio::test]
async fn stream_chunk_feeds_the_global_tracker_that_barge_in_reads() {
    let _serial = GLOBAL_STATE_LOCK.lock().await;
    let (store, _dir) = store();
    let (tx, generation) = speak_queue();
    let mut turn = TurnState::new();
    handle_thinking_start(&mut turn);

    handle_stream_chunk(&mut turn, &store, &tx, &generation, "Marco global de prefixo do teste stream-chunk. ");

    assert!(
        speak_guard::spoken_prefix().contains("Marco global de prefixo do teste stream-chunk."),
        "spoken_prefix devia refletir o trecho falado pelo stream"
    );
}

/// `handle_thinking_start` reseta o PREFIXO global (não a janela de self-echo — ver o cabeçalho de
/// `speak_guard.rs`), então um turno novo não herda o prefixo do turno anterior.
#[tokio::test]
async fn thinking_start_resets_the_global_prefix_for_the_next_turn() {
    let _serial = GLOBAL_STATE_LOCK.lock().await;
    let (store, _dir) = store();
    let (tx, generation) = speak_queue();
    let mut turn = TurnState::new();
    handle_thinking_start(&mut turn);
    handle_stream_chunk(&mut turn, &store, &tx, &generation, "Turno velho que não deve sobrar. ");
    assert!(speak_guard::spoken_prefix().contains("Turno velho"));

    handle_thinking_start(&mut turn);

    assert!(
        !speak_guard::spoken_prefix().contains("Turno velho"),
        "o prefixo do turno anterior não pode vazar para o novo turno"
    );
}

// -- fila de fala: enfileirar nunca espera o worker, e barge-in cancela o que ainda não tocou -----

/// O ponto central desta rodada: enfileirar não é `async` — a assinatura de `handle_stream_chunk`/
/// `handle_thinking_stop`/`handle_assistant_message` provando isso em tempo de COMPILAÇÃO é mais
/// forte que qualquer asserção de tempo. Este teste prova a metade que o tipo não garante sozinho:
/// mesmo com um worker LENTO (ou nenhum) do outro lado do canal, `enqueue_speak` retorna na hora.
#[test]
fn enqueue_speak_never_blocks_even_without_a_worker_draining_the_channel() {
    let (tx, generation) = speak_queue();
    let started = std::time::Instant::now();
    for i in 0..50 {
        enqueue_speak(&tx, &generation, format!("frase {i}"), now_millis());
    }
    assert!(started.elapsed() < std::time::Duration::from_millis(50), "enfileirar 50 frases não pode demorar (fila sem limite, sem I/O)");
}

/// Fecha a corrida que a fila introduziu: um barge-in incrementa a geração; um job enfileirado
/// ANTES dele carrega a geração ANTIGA, distinta da geração dos jobs enfileirados DEPOIS — é essa
/// diferença que `spawn_speak_worker` usa para descartar o job velho em vez de sintetizar uma frase
/// que o usuário já mandou calar (ver o `if job.generation != generation.load(...)` em `mod.rs`).
#[test]
fn a_barge_in_separates_queued_jobs_into_a_stale_and_a_fresh_generation() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SpeakJob>();
    let generation = AtomicU64::new(0);
    enqueue_speak(&tx, &generation, "frase antes do barge-in".to_string(), now_millis());

    generation.fetch_add(1, Ordering::SeqCst); // mesmo efeito de `ForceStopTts`/`Interruption` em mod.rs

    enqueue_speak(&tx, &generation, "frase depois do barge-in".to_string(), now_millis());

    let stale = rx.try_recv().expect("primeiro job devia estar na fila");
    let fresh = rx.try_recv().expect("segundo job devia estar na fila");
    assert_ne!(stale.generation, fresh.generation, "barge-in tem que separar os dois jobs em gerações diferentes");
}

// -- tool call: fala o que já estava pronto e NÃO inventa fala de enchimento ------------------------

#[tokio::test]
async fn a_tool_call_flushes_what_the_model_already_said_without_waiting_for_punctuation() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SpeakJob>();
    let generation = AtomicU64::new(0);
    let mut turn = TurnState::new();
    handle_thinking_start(&mut turn);
    turn.stream_buffer = "Deixa eu olhar isso pra você".into();

    flush_pending_buffer(&mut turn, &tx, &generation);

    let job = rx.try_recv().expect("o que já estava pronto devia ser enfileirado");
    assert_eq!(job.text, "Deixa eu olhar isso pra você");
}

/// Toda fala da Mika é gerada por LLM: tool chamada sem NENHUM texto antes dela fica em silêncio
/// em vez de ganhar uma frase de enchimento fixa (o filler antigo falava pt-BR em qualquer idioma).
#[tokio::test]
async fn a_tool_call_with_nothing_said_yet_stays_silent_instead_of_speaking_a_canned_phrase() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SpeakJob>();
    let generation = AtomicU64::new(0);
    let mut turn = TurnState::new();
    handle_thinking_start(&mut turn);

    flush_pending_buffer(&mut turn, &tx, &generation);

    assert!(rx.try_recv().is_err(), "nenhuma fala devia ser inventada");
}

/// Tempo até o primeiro áudio: a primeira oração do turno entra na fila na vírgula, sem esperar o
/// ponto final; depois que algo já foi dito, as orações seguintes voltam a esperar a frase fechar
/// (chunks maiores soam mais naturais, e aí o áudio anterior já está tocando).
#[tokio::test]
async fn the_first_clause_of_a_turn_is_queued_at_a_comma_but_later_clauses_wait_for_the_sentence() {
    let _serial = GLOBAL_STATE_LOCK.lock().await;
    let (store, _dir) = store();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let generation = AtomicU64::new(0);
    let mut turn = TurnState::new();
    handle_thinking_start(&mut turn);

    handle_stream_chunk(&mut turn, &store, &tx, &generation, "Nossa, que legal que você passou dessa fase, eu sabia");
    assert_eq!(rx.try_recv().expect("a primeira oração devia estar na fila").text, "Nossa, que legal que você passou dessa fase,");

    handle_stream_chunk(&mut turn, &store, &tx, &generation, " que ia conseguir, sério mesmo, parabéns");
    assert!(rx.try_recv().is_err(), "depois da primeira fala, oração solta não é corte");
}

/// `publish_silence_endpoint` também usa `EventKey::Interruption` (`is_silence_endpoint: true`, sem
/// transcrição): um pico de voz abaixo do streak de barge-in seguido de 1,5 s de silêncio publica
/// isso com a Mika ainda falando, e tratar como barge-in descartava o resto da resposta. O TS
/// (`voice-barge-in.ts`) ignorava o endpoint por silêncio.
#[test]
fn only_a_real_interruption_or_a_force_stop_counts_as_a_barge_in() {
    let real = serde_json::json!({ "transcript": "para", "is_interruption": true });
    let endpoint = serde_json::json!({ "transcript": "", "is_silence_endpoint": true, "timestamp": 1 });
    assert!(is_barge_in(EventKey::ForceStopTts, &serde_json::json!({})));
    assert!(is_barge_in(EventKey::Interruption, &real));
    assert!(!is_barge_in(EventKey::Interruption, &endpoint));
    assert!(!is_barge_in(EventKey::StreamChunk, &real));
}
