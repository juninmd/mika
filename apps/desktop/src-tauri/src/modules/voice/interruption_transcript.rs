//! Porte fiel de `packages/sdk/src/utils/transcript-plausibility.ts` (`isImplausibleTranscript`,
//! `hasDegenerateRepetition`) e de
//! `apps/desktop/src/main/services/voice/audio/interruption-transcript.ts`
//! (`processInterruptionAudio`) — o filtro que decide se o texto que o Whisper devolveu sobre o
//! áudio pós-barge-in é fala real ou alucinação/eco, e o que fazer com ele.
//!
//! **Por que este filtro existe.** O barge-in dispara com ~100ms de fala real, mas grava até 2,5s
//! (`barge_in_recorder.rs::BargeInRecorderConfig::post_barge_ms`); quando o resto da janela é
//! silêncio/ruído, o Whisper devolve frases padrão ("legendas pela comunidade amara.org", "tchau")
//! que, sem este filtro, disparariam um turno de LLM inteiro para uma interrupção que nunca
//! aconteceu de verdade.
//!
//! **Quem chama isto.** `vad_audio.rs::react_to_barge_in` inicia a captura; quando
//! `VadPipeline::process` devolve `barge_in_audio: Some(_)` (o teto de tempo/amostras foi
//! atingido), `vad_audio.rs` chama [`process_interruption_audio`] fora do caminho quente (via
//! `tauri::async_runtime::spawn`, nunca bloqueando a próxima janela do VAD) — mesma separação que
//! `barge-in-recorder.ts::flushCapture` tinha ao disparar `detector.processInterruptionAudio` sem
//! aguardar (`void ...catch(...)`).
//!
//! **`stt.isListening()` — porte fechado.** `createInterruptionTranscriber` (`voice-barge-in.ts`)
//! pula a transcrição quando o STT contínuo já está ouvindo (ele vai entregar a mesma fala;
//! transcrever aqui também duplicaria a resposta). `listening_state.rs` agora expõe
//! [`super::listening_state::global`] — o MESMO estado que `voice_tool.rs` usa para
//! `listen_start`/`listen_stop` — para este módulo poder ler o mesmo checkbox sem duplicar estado.
//!
//! **Self-echo — porte fechado via tracker de processo.** `looksLikeSelfEcho`
//! (`speak-guard.ts`) precisa do TEXTO que a Mika falou; `speak_guard::global()` é o tracker que
//! `voice::auto_speak` alimenta chamando `speak_guard::mark_spoken` a cada trecho sintetizado (ver
//! o cabeçalho de `speak_guard.rs`) — este módulo só LÊ por
//! `speak_guard::looks_like_self_echo`/`speak_guard::spoken_prefix`.


use super::listening_state;
use super::speak_guard;

/// Mesmos números de `transcript-plausibility.ts` — medidos num log de sessão real (chars/seg
/// separa limpo: alucinação ~0.09-0.52 c/s, fala real ~4.35-11.11 c/s).
const MIN_CHARS_PER_SECOND: f64 = 2.0;
/// Outro extremo: o decoder trava num laço repetindo o mesmo trecho até o limite.
const MAX_CHARS_PER_SECOND: f64 = 22.0;
/// Áudio curto demais para a densidade significar algo — dois caracteres a mais já move para 5 c/s.
const MIN_MEASURABLE_MS: u64 = 1_200;
/// Abaixo disto, texto curto demais para a densidade ser significativa (fala real curta também
/// pontua baixo).
const MIN_DENSITY_TEXT_CHARS: usize = 12;
const FILLER_MAX_CHARS_PER_SECOND: f64 = 6.0;

/// Cópia privada da mesma tabela fixa de `person_catalog/normalize.rs`/`memory/text.rs` — `mod
/// text` de `modules/memory` é privado, e este módulo não pode alcançá-lo sem editar
/// `modules/memory/mod.rs`, fora do escopo desta tarefa. Cobre acentos latinos comuns (pt-BR);
/// divergência aceita do original (`\p{Diacritic}` cobre QUALQUER script) já documentada nas
/// outras cópias.
fn strip_diacritics(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ý' | 'ÿ' => 'y',
            other => other,
        })
        .collect()
}

fn normalize(text: &str) -> String {
    let lowered = strip_diacritics(&text.to_lowercase());
    let kept: String = lowered.chars().filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace()).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn whisper_fillers() -> &'static [&'static str] {
    &[
        "e ai",
        "o que e isso",
        "tchau",
        "obrigado",
        "obrigada",
        "ate mais",
        "legendas pela comunidade amara org",
        "valeu",
        "ah",
        "oi",
    ]
}

/// Detecta um decoder Whisper travado repetindo o mesmo trecho.
pub fn has_degenerate_repetition(text: &str) -> bool {
    let lowered = text.to_lowercase();
    let words: Vec<&str> = lowered.split_whitespace().collect();
    if words.len() < 8 {
        return false;
    }
    for size in [1usize, 2, 3] {
        if words.len() < size * 4 {
            continue;
        }
        let unit = words[..size].join(" ");
        let mut repeats = 0;
        let mut i = 0;
        while i + size <= words.len() {
            if words[i..i + size].join(" ") != unit {
                break;
            }
            repeats += 1;
            i += size;
        }
        if repeats >= 4 {
            return true;
        }
    }
    false
}

/// Porte de `isImplausibleTranscript`. `duration_ms` ausente = "não dá para julgar" — nunca condena
/// sem ele. `min_filler_ms` deixa o chamador de janela fixa (o barge-in, 2,5s de captura) passar o
/// próprio piso em vez de herdar o do renderer, que essa janela nunca alcançaria.
pub fn is_implausible_transcript(text: &str, duration_ms: Option<u64>, min_filler_ms: u64) -> bool {
    if has_degenerate_repetition(text) {
        return true;
    }
    let Some(duration_ms) = duration_ms else { return false };
    if duration_ms < MIN_MEASURABLE_MS {
        return false;
    }

    let density = (text.chars().count() as f64 / duration_ms as f64) * 1000.0;
    if density > MAX_CHARS_PER_SECOND {
        return true;
    }

    if density < MIN_CHARS_PER_SECOND
        && (text.chars().count() >= MIN_DENSITY_TEXT_CHARS || duration_ms >= min_filler_ms)
    {
        return true;
    }

    whisper_fillers().contains(&normalize(text).as_str())
        && density < FILLER_MAX_CHARS_PER_SECOND
        && duration_ms >= min_filler_ms
}

/// Payload PCM16 mono a transcrever, já o resultado de `BargeInRecorder::flush` — decoupled de
/// `Vec<i16>` bruto para deixar [`process_interruption_audio`] testável sem depender do tipo do
/// recorder.
pub struct InterruptionAudio {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
}

/// Inverso de `inference::vad::pcm16_to_f32` — `transcribe_local` espera bytes PCM16 LE crus
/// (mesmo contrato que `transcribe.rs::transcribe_pcm` já decodifica de base64 antes de chamar).
fn i16_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

/// Mensagem da alucinação pós-barge-in, extraída para ser testável sem capturar `log::info!`:
/// nomeia o provedor REALMENTE ativo (`provider`) em vez de "Whisper" fixo — o motor local default
/// é Nemotron (`config/schemas/voice.rs`), então um log que sempre diz "Whisper" manda quem
/// depura na direção errada quando o provedor de verdade é outro.
fn hallucination_log_message(transcript: &str, duration_ms: u64, provider: &str) -> String {
    format!(
        "interruption ignored: transcript does not fit the audio — {provider} hallucination (\"{transcript}\", {} chars, {duration_ms}ms)",
        transcript.chars().count()
    )
}

/// Porte de `processInterruptionAudio`: transcreve, filtra alucinação/eco, devolve o texto pronto
/// para publicar em `EventKey::Interruption` — ou `None` quando nada deve ser publicado. Quem
/// publica o evento é `vad_audio.rs` (este módulo não tem `EventBus`, propositalmente: mantém o
/// filtro puro de I/O do barramento, só transcrição via `transcribe::transcribe_local`).
///
/// `min_filler_ms: 0` no chamador de produção — mesma escolha do TS (`{ minFillerMs: 0 }` na
/// linha 30 de `interruption-transcript.ts`): a janela do barge-in é curta e fixa, então mesmo uma
/// frase padrão CURTA já é suspeita, ao contrário do piso mais alto usado pelo STT contínuo.
pub async fn process_interruption_audio(
    data_dir: std::path::PathBuf,
    audio: InterruptionAudio,
    language: Option<String>,
    provider: &str,
    whisper_model: &str,
    min_filler_ms: u64,
) -> Option<String> {
    // Porte de `if (stt.isListening()) return ''` (`createInterruptionTranscriber`,
    // `voice-barge-in.ts`): o contínuo já vai entregar a mesma fala, transcrever aqui duplicaria a
    // resposta. Sai ANTES de chamar o Whisper — mesma economia que o TS ganhava ao devolver `''`
    // direto, sem rodar o modelo à toa.
    if listening_state::global().is_listening() {
        log::debug!("interruption without transcript: continuous STT will deliver the speech");
        return None;
    }

    let duration_ms = (audio.samples.len() as f64 / audio.sample_rate as f64 * 1000.0).round() as u64;
    let pcm = i16_to_le_bytes(&audio.samples);

    let transcript = match super::transcribe::transcribe_local(provider, whisper_model, data_dir, pcm, audio.sample_rate, language).await {
        Ok(Some(text)) => text.trim().to_string(),
        Ok(None) => return None,
        Err(e) => {
            log::warn!("interruption: transcription failed: {e}");
            return None;
        }
    };
    if transcript.is_empty() {
        return None;
    }

    if is_implausible_transcript(&transcript, Some(duration_ms), min_filler_ms) {
        log::info!("{}", hallucination_log_message(&transcript, duration_ms, provider));
        return None;
    }

    let now_ms = now_millis();
    if speak_guard::looks_like_self_echo(&transcript, now_ms) {
        log::debug!("interruption ignored: echo of its own speech ({} chars)", transcript.chars().count());
        return None;
    }

    Some(transcript)
}

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests;
