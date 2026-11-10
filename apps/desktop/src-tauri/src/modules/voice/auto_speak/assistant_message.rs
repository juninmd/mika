//! `chat:assistant-message`/`chat:speech-request` — os dois eventos que não vêm do stream comum de
//! chat. Separado de `mod.rs` só por tamanho (AGENTS.md, teto de 300 linhas).

use std::sync::atomic::AtomicU64;

use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::store::ConfigStore;
use crate::events::EventKey;
use crate::modules::voice::speak_guard;

use super::turn::{gate_open, TurnState};
use super::{enqueue_speak, now_millis, speech_text, strip_emotion_tags, SpeakJob};

/// `was_spoken_in_turn` não tem free function em `speak_guard` (só `mark_spoken`/`reset_turn`/
/// `spoken_prefix`/`looks_like_self_echo` — ver o cabeçalho daquele arquivo); lê o tracker global
/// travando o `Mutex` diretamente, mesma degradação segura dos wrappers de lá (lock envenenado ->
/// `false`, nunca dedup incorreto por pânico em outra thread).
pub(super) fn was_already_spoken(timestamp_ms: u64, text: &str, now_ms: u64) -> bool {
    speak_guard::global().lock().map(|mut guard| guard.was_spoken_in_turn(timestamp_ms, text, now_ms)).unwrap_or(false)
}

/// `chat:assistant-message` — o recap diário e cada turno do debate multi-agente (`content`/
/// `collaboration_session`) chegam por aqui, não por `StreamChunk`. Um turno de chat COMUM nunca
/// publica esta chave (duplicaria a mensagem na janela — ver `handle_thinking_stop` em `mod.rs`),
/// então este caminho só atende recap/debate.
pub(super) fn handle_assistant_message(
    turn: &mut TurnState,
    store: &ConfigStore,
    tx: &UnboundedSender<SpeakJob>,
    generation: &AtomicU64,
    payload: &Value,
) {
    let raw = payload.get("content").and_then(Value::as_str).unwrap_or("");
    let clean = strip_emotion_tags(raw);
    if clean.is_empty() || !speech_text::has_speakable_text(&clean) {
        return;
    }

    let timestamp = payload.get("timestamp").and_then(Value::as_u64).unwrap_or_else(now_millis);
    if !gate_open(store) {
        return;
    }
    if was_already_spoken(timestamp, &clean, now_millis()) {
        return;
    }

    let remainder = turn.remaining_to_speak(&clean);
    // Reseta o turno DEPOIS de calcular o restante — mesma ordem de `AutoSpeakService.onAssistantMessage`.
    turn.on_turn_start();

    if remainder.trim().is_empty() || !speech_text::has_speakable_text(&remainder) {
        return;
    }

    // Marca o texto FINAL inteiro (não só `remainder`) — mesma dupla marcação de
    // `AutoSpeakService.onAssistantMessage`/`synthesize` no TS: um `AssistantMessage` repetido com
    // o mesmo `clean` precisa colidir mesmo quando só o restante foi de fato sintetizado agora.
    speak_guard::mark_spoken(timestamp, &clean, now_millis());
    enqueue_speak(tx, generation, remainder, timestamp);
}

/// O texto de um `EventKey::SpeechRequest`, ou `None` quando não há o que falar — mesmo filtro que
/// `bus_bridge::spawn_speech_request_listener` tinha, agora servindo o laço único de `mod.rs`.
pub(super) fn speech_request_text(key: EventKey, payload: &Value) -> Option<String> {
    if key != EventKey::SpeechRequest {
        return None;
    }
    let text = payload.get("text").and_then(Value::as_str)?.trim();
    (!text.is_empty()).then(|| text.to_string())
}
