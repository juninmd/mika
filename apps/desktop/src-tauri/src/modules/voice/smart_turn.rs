//! `voice:smart-turn` — a rota que liga o classificador de fim de turno ao runtime.
//!
//! Recebe PCM16 mono 16 kHz (o MESMO formato de `mika:voice-transcribe`) e devolve
//! `{ complete, probability, latencyMs, reason }`. O canal é opt-in (`voice.smartTurnEnabled`,
//! default `false`): desligado, responde `complete = true` / `reason = "disabled"` sem tocar em
//! disco, rede ou ONNX.
//!
//! **Nunca prende o usuário.** Modelo ausente, download falho, sessão quebrada ou inferência com
//! erro viram `complete = true` com `reason` de fallback — a decisão em si mora em
//! [`super::end_of_turn::decide_end_of_turn`], e o VAD acústico continua sendo o piso de segurança.
//!
//! O corpo binário chega por `bridge::mika_smart_turn_pcm` (`Uint8Array` no renderer); o caminho
//! JSON (`audioBase64`) existe pelo mesmo motivo do transcribe: tools/rotas legadas não passam pelo
//! corpo binário. Os dois convergem em [`run`].

use std::path::PathBuf;
use std::time::Instant;

use base64::Engine as _;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::modules::inference::{smart_turn, whisper};

use super::end_of_turn::{
    decide_end_of_turn, EndOfTurnDecision, EndOfTurnReason, DEFAULT_COMPLETION_THRESHOLD, MIN_AUDIO_SAMPLES,
};

/// `voice.smartTurnEnabled` — opt-in: o primeiro uso baixa ~8,7 MB e muda o comportamento de escuta.
const CFG_SMART_TURN_ENABLED: &str = "voice.smartTurnEnabled";

/// Teto do corpo binário: 60 s de PCM16 mono 16 kHz. O classificador só usa os últimos 8 s; aceitar
/// minutos de áudio gastaria memória sem mudar a decisão.
const MAX_AUDIO_BYTES: usize = 60 * 16_000 * 2;
/// Base64 do mesmo teto: `MAX_AUDIO_BYTES / 3 * 4` (arredondado para cima).
const MAX_AUDIO_BASE64_CHARS: usize = 2_560_000;

fn enabled(store: &crate::config::store::ConfigStore) -> bool {
    store.get(CFG_SMART_TURN_ENABLED).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn to_value(decision: EndOfTurnDecision, latency_ms: u64) -> Value {
    json!({
        "complete": decision.complete,
        "probability": decision.probability,
        "latencyMs": latency_ms,
        "reason": decision.reason.as_str(),
    })
}

/// Decodifica o JSON legado (`audioBase64` + `mimeType`) para os dois campos que [`run`] usa.
fn parse_payload(payload: &Value) -> Result<(Vec<u8>, Option<String>), String> {
    let encoded = payload["audioBase64"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "'audioBase64' ausente ou vazio".to_string())?;
    if encoded.len() > MAX_AUDIO_BASE64_CHARS {
        return Err(format!("'audioBase64' excede o teto de {MAX_AUDIO_BASE64_CHARS} caracteres (~60s de PCM16 mono 16kHz)"));
    }
    let audio = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .map_err(|e| format!("'audioBase64' inválido: {e}"))?;
    Ok((audio, payload["mimeType"].as_str().map(str::to_string)))
}

/// Corpo binário (`mika_smart_turn_pcm`): mesmas regras de tamanho, sem base64.
fn request_from_bytes(audio: Vec<u8>, mime_type: Option<String>) -> Result<(Vec<u8>, Option<String>), String> {
    if audio.is_empty() {
        return Err("áudio ausente ou vazio".to_string());
    }
    if audio.len() > MAX_AUDIO_BYTES {
        return Err(format!("áudio excede o teto de {MAX_AUDIO_BYTES} bytes (~60s de PCM16 mono 16kHz)"));
    }
    Ok((audio, mime_type))
}

async fn run(app: &AppHandle, audio: Vec<u8>, mime_type: Option<String>) -> Result<Value, String> {
    let store = match crate::state::config_store(app) {
        Ok(store) => store,
        Err(error) => {
            log::warn!("Smart Turn sem config store: {error}");
            return Ok(to_value(EndOfTurnDecision::fallback(EndOfTurnReason::ModelUnavailable), 0));
        }
    };
    if !enabled(&store) {
        return Ok(to_value(EndOfTurnDecision::fallback(EndOfTurnReason::Disabled), 0));
    }

    let mime_type = mime_type.as_deref().unwrap_or("");
    let samples = whisper::pcm16_to_f32(&audio, super::transcribe::pcm_sample_rate(mime_type))?;
    // Curto demais não justifica baixar 8,7 MB: a própria regra pura devolve o fallback.
    if samples.len() < MIN_AUDIO_SAMPLES {
        return Ok(to_value(EndOfTurnDecision::fallback(EndOfTurnReason::ShortAudio), 0));
    }

    let data_dir = match crate::state::app_data_dir(app) {
        Ok(dir) => dir,
        Err(error) => {
            log::warn!("Smart Turn sem app_data_dir: {error}");
            return Ok(to_value(EndOfTurnDecision::fallback(EndOfTurnReason::ModelUnavailable), 0));
        }
    };

    let model_path: PathBuf = match smart_turn::ensure_model(&store, &data_dir).await {
        Ok(path) => path,
        Err(error) => {
            log::warn!("Smart Turn indisponível: {error}");
            return Ok(to_value(EndOfTurnDecision::fallback(EndOfTurnReason::ModelUnavailable), 0));
        }
    };

    // `latencyMs` mede a classificação (carga da sessão na primeira vez + inferência), nunca o
    // download de 8,7 MB: tempo de rede na resposta faria o número mentir sobre os ~12 ms do modelo.
    let classify_started = Instant::now();

    // `Session::run` é síncrono: o mesmo isolamento de `whisper.rs`/`vad.rs`, fora do worker que
    // também resolve `mika_invoke`.
    let decision = tokio::task::spawn_blocking(move || match smart_turn::SmartTurn::load(&model_path) {
        Ok(mut classifier) => decide_end_of_turn(&mut classifier, &samples, DEFAULT_COMPLETION_THRESHOLD),
        Err(error) => {
            log::warn!("Smart Turn não carregou: {error}");
            EndOfTurnDecision::fallback(EndOfTurnReason::ModelUnavailable)
        }
    })
    .await
    .unwrap_or_else(|error| {
        log::warn!("inferência do Smart Turn cancelada: {error}");
        EndOfTurnDecision::fallback(EndOfTurnReason::ClassifierFailed)
    });

    Ok(to_value(decision, classify_started.elapsed().as_millis() as u64))
}

/// Caminho JSON de `voice:smart-turn` (`audioBase64`). A guarda de superfície de
/// `voice/mod.rs::VoiceModule::invoke` já rodou antes daqui.
pub async fn handle(app: &AppHandle, payload: &Value) -> Result<Value, String> {
    let (audio, mime_type) = parse_payload(payload)?;
    run(app, audio, mime_type).await
}

/// Caminho binário (`bridge::mika_smart_turn_pcm`): fora do roteador de canais, então a guarda de
/// superfície é refeita aqui, como em `transcribe::handle_binary`/`vad_audio::handle_binary`.
pub async fn handle_binary(
    app: &AppHandle,
    ctx: &crate::protocol::InvokeContext,
    audio: Vec<u8>,
    mime_type: Option<String>,
) -> Result<Value, String> {
    if !crate::shell_actions::is_owner_surface(ctx.window_type) {
        return Err(format!("voice:smart-turn recusado para {:?}", ctx.window_type));
    }
    let (audio, mime_type) = request_from_bytes(audio, mime_type)?;
    run(app, audio, mime_type).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_binary_audio() {
        assert!(request_from_bytes(Vec::new(), None).is_err());
    }

    #[test]
    fn rejects_binary_audio_above_the_cap() {
        assert!(request_from_bytes(vec![0u8; MAX_AUDIO_BYTES + 2], None).is_err());
    }

    #[test]
    fn accepts_a_short_binary_utterance() {
        let (audio, mime) = request_from_bytes(vec![0u8; 2_000], Some("audio/pcm;rate=16000".into())).unwrap();
        assert_eq!(audio.len(), 2_000);
        assert_eq!(mime.as_deref(), Some("audio/pcm;rate=16000"));
    }

    #[test]
    fn json_payload_requires_audio_base64() {
        assert!(parse_payload(&json!({})).is_err());
        assert!(parse_payload(&json!({ "audioBase64": "" })).is_err());
    }

    #[test]
    fn json_payload_decodes_pcm_bytes_and_carries_the_mime_type() {
        let (audio, mime) =
            parse_payload(&json!({ "audioBase64": "AAAA", "mimeType": "audio/pcm;rate=16000" })).expect("payload válido");
        assert_eq!(audio, vec![0, 0, 0]);
        assert_eq!(mime.as_deref(), Some("audio/pcm;rate=16000"));
    }

    #[test]
    fn response_carries_exactly_the_four_contract_fields() {
        let value = to_value(
            EndOfTurnDecision { complete: false, probability: Some(0.25), reason: EndOfTurnReason::Probability },
            12,
        );
        assert_eq!(value["complete"], json!(false));
        assert_eq!(value["probability"], json!(0.25));
        assert_eq!(value["latencyMs"], json!(12));
        assert_eq!(value["reason"], json!("probability"));
    }
}
