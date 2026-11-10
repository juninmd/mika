//! Um pedaço de `speaker/mod.rs` — só a síntese de UM chunk (cache lookup, chamada ao provedor,
//! entrega ao `SpeechSink`). Separado por tamanho de arquivo (AGENTS.md, <300 linhas), não por
//! camada nova: `speak_all_chunks` (em `mod.rs`) é o único chamador.

use std::path::Path;
use std::sync::Mutex;

use crate::config::store::ConfigStore;
use crate::modules::voice::tts::abort_registry::CancelHandle;
use crate::modules::voice::tts::cache::{supertonic_cache_key, CachedAudio, TtsCache};
use crate::modules::voice::tts::dispatch::generate_with_provider;
use crate::modules::voice::tts::preprocessor::sanitize_text;
use crate::modules::voice::tts::state::TtsState;

use super::{SpeechSink, TtsChunkAudio};

/// Antes desta rodada, todo provedor cortava em `supertonic_text_limit` (sempre 3000) — os
/// controles `voice.{elevenLabs,azureTts,googleTts}TextLimit` existiam só na UI, sem leitor. Cada
/// provedor remoto agora carrega o próprio `text_limit` (lido de `voice.*` em `from_config`,
/// `providers/remote/*.rs`); os demais (Supertonic, Piper, OpenAI, system) continuam no valor
/// fixo do schema, que é o único que declaram.
pub(super) fn text_limit(state: &TtsState) -> usize {
    match state.provider.as_str() {
        "elevenlabs" => state.eleven_labs.text_limit,
        "azure-tts" => state.azure_tts.text_limit,
        "google-tts" => state.google_tts.text_limit,
        _ => state.supertonic_text_limit,
    }
}

/// Espelha `TTSFactory#producesNoSpeech`: um chunk que sanitiza para vazio não é falado.
pub(super) fn produces_no_speech(state: &TtsState, text: &str) -> bool {
    sanitize_text(text, text_limit(state), state.preprocessor).is_empty()
}

/// Espelha `finishGenerate`/`buildCacheKey` para o único provedor com gerador real.
fn cache_key(state: &TtsState, sanitized: &str) -> Option<String> {
    (state.provider == "supertonic").then(|| {
        supertonic_cache_key(sanitized, &state.supertonic_voice, &state.supertonic_language, state.supertonic_steps, state.effective_speed(None))
    })
}

pub(super) struct ChunkOutcome {
    pub success: bool,
    pub error: Option<String>,
    pub cancelled: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn speak_chunk(
    chunk: &str,
    index: usize,
    total: usize,
    state: &TtsState,
    cache: &Mutex<TtsCache>,
    cache_dir: &Path,
    store: &ConfigStore,
    cancel: &CancelHandle,
    sink: &dyn SpeechSink,
) -> ChunkOutcome {
    if cancel.is_cancelled() {
        return ChunkOutcome { success: false, error: None, cancelled: true };
    }

    let sanitized = sanitize_text(chunk, text_limit(state), state.preprocessor);
    let key = cache_key(state, &sanitized);
    if let Some(key) = &key {
        if let Some(hit) = cache.lock().ok().and_then(|c| c.get(key)) {
            sink.on_chunk_ready(TtsChunkAudio {
                audio: hit.audio,
                mime_type: hit.mime_type,
                chunk_index: index,
                total_chunks: total,
                provider: state.provider.clone(),
            });
            return ChunkOutcome { success: true, error: None, cancelled: false };
        }
    }

    match generate_with_provider(state, &sanitized, cache_dir, store, cancel).await {
        Ok(Some(generated)) => {
            if cancel.is_cancelled() {
                return ChunkOutcome { success: false, error: None, cancelled: true };
            }
            if let Some(key) = key {
                if let Ok(mut c) = cache.lock() {
                    c.insert(key, CachedAudio { audio: generated.audio.clone(), mime_type: generated.mime_type });
                }
            }
            sink.on_chunk_ready(TtsChunkAudio {
                audio: generated.audio,
                mime_type: generated.mime_type,
                chunk_index: index,
                total_chunks: total,
                provider: state.provider.clone(),
            });
            ChunkOutcome { success: true, error: None, cancelled: false }
        }
        // Porte de `factory.ts` ("'system' or unknown: no runtime audio generation" -> `null`):
        // nenhum áudio, mas NÃO é falha — o chunk conta como falado, sem chegar ao `sink`.
        Ok(None) if cancel.is_cancelled() => ChunkOutcome { success: false, error: None, cancelled: true },
        Ok(None) => ChunkOutcome { success: true, error: None, cancelled: false },
        Err(error) => {
            if cancel.is_cancelled() {
                ChunkOutcome { success: false, error: None, cancelled: true }
            } else {
                ChunkOutcome { success: false, error: Some(error), cancelled: false }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regressão: antes desta rodada `text_limit` sempre devolvia `supertonic_text_limit` (3000),
    /// então `voice.{elevenLabs,azureTts,googleTts}TextLimit` — salvos pela UI — nunca afetavam o
    /// corte real de nenhum provedor remoto.
    #[test]
    fn text_limit_reads_the_active_remote_providers_own_config_instead_of_the_supertonic_default() {
        let mut state = TtsState::default();
        state.supertonic_text_limit = 3_000;

        state.provider = "elevenlabs".into();
        state.eleven_labs.text_limit = 500;
        assert_eq!(text_limit(&state), 500);

        state.provider = "azure-tts".into();
        state.azure_tts.text_limit = 750;
        assert_eq!(text_limit(&state), 750);

        state.provider = "google-tts".into();
        state.google_tts.text_limit = 1_200;
        assert_eq!(text_limit(&state), 1_200);

        // Provedor sem `textLimit` próprio no schema (Supertonic, Piper, OpenAI, system): continua
        // usando o teto fixo.
        state.provider = "supertonic".into();
        assert_eq!(text_limit(&state), 3_000);
    }
}
