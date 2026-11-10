//! Cache-aware streaming RNNT over the export's three graphs (NeMo layout, `genai_config.json`):
//! the encoder takes 56 new mel frames plus the 9 previous ones (`pre_encode_cache_size`) and
//! carries its attention/conv caches between chunks; greedy RNNT advances a frame on blank and
//! only steps the prediction network when a real token is emitted.

use ort::session::Session;
use ort::value::Tensor;

use super::features::N_MELS;

pub const BLANK: usize = 13_087;
const PRE_ENCODE: usize = 9;
const CHUNK: usize = 56;
const LAYERS: i64 = 24;
const LEFT_CONTEXT: i64 = 56;
const HIDDEN: usize = 1024;
const CONV_CONTEXT: i64 = 8;
const PRED_LAYERS: i64 = 2;
const PRED_HIDDEN: usize = 640;
const MAX_SYMBOLS_PER_FRAME: usize = 10;

pub struct Graphs {
    pub encoder: Session,
    pub decoder: Session,
    pub joiner: Session,
}

struct EncoderCache {
    channel: Vec<f32>,
    time: Vec<f32>,
    len: i64,
}

struct Prediction {
    out: Vec<f32>,
    h: Vec<f32>,
    c: Vec<f32>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Token ids for a whole utterance of log-mel frames (`frames × N_MELS`, row-major).
pub fn transcribe(graphs: &mut Graphs, mel: &[f32], lang_id: i64) -> Result<Vec<usize>, String> {
    let frames = mel.len() / N_MELS;
    let mut cache = EncoderCache {
        channel: vec![0.0; (LAYERS * LEFT_CONTEXT) as usize * HIDDEN],
        time: vec![0.0; LAYERS as usize * HIDDEN * CONV_CONTEXT as usize],
        len: 0,
    };
    let zero_state = vec![0.0; PRED_LAYERS as usize * PRED_HIDDEN];
    let mut prediction = predict(&mut graphs.decoder, BLANK, zero_state.clone(), zero_state)?;
    let mut context = vec![0.0f32; PRE_ENCODE * N_MELS];
    let mut tokens = Vec::new();
    for chunk in 0..frames.div_ceil(CHUNK) {
        let mut input = std::mem::take(&mut context);
        let new = &mel[(chunk * CHUNK * N_MELS).min(mel.len())..((chunk + 1) * CHUNK * N_MELS).min(mel.len())];
        input.extend_from_slice(new);
        input.resize((PRE_ENCODE + CHUNK) * N_MELS, 0.0);
        context = input[CHUNK * N_MELS..].to_vec();
        let encoded = encode(&mut graphs.encoder, input, &mut cache, lang_id)?;
        for frame in encoded.chunks_exact(HIDDEN) {
            decode_frame(graphs, frame, &mut prediction, &mut tokens)?;
        }
    }
    Ok(tokens)
}

fn encode(encoder: &mut Session, input: Vec<f32>, cache: &mut EncoderCache, lang_id: i64) -> Result<Vec<f32>, String> {
    let frames = (PRE_ENCODE + CHUNK) as i64;
    let outputs = encoder
        .run(ort::inputs![
            "audio_signal" => Tensor::from_array(([1i64, frames, N_MELS as i64], input)).map_err(err)?,
            "length" => Tensor::from_array(([1i64], vec![frames])).map_err(err)?,
            "cache_last_channel" => Tensor::from_array(([1i64, LAYERS, LEFT_CONTEXT, HIDDEN as i64], std::mem::take(&mut cache.channel))).map_err(err)?,
            "cache_last_time" => Tensor::from_array(([1i64, LAYERS, HIDDEN as i64, CONV_CONTEXT], std::mem::take(&mut cache.time))).map_err(err)?,
            "cache_last_channel_len" => Tensor::from_array(([1i64], vec![cache.len])).map_err(err)?,
            "lang_id" => Tensor::from_array(([1i64], vec![lang_id])).map_err(err)?,
        ])
        .map_err(|e| format!("encoder Nemotron falhou: {e}"))?;
    let (_, encoded) = outputs["outputs"].try_extract_tensor::<f32>().map_err(err)?;
    let (_, lengths) = outputs["encoded_lengths"].try_extract_tensor::<i64>().map_err(err)?;
    let valid = (lengths.first().copied().unwrap_or(0).max(0) as usize * HIDDEN).min(encoded.len());
    cache.channel = outputs["cache_last_channel_next"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec();
    cache.time = outputs["cache_last_time_next"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec();
    cache.len = outputs["cache_last_channel_len_next"].try_extract_tensor::<i64>().map_err(err)?.1.first().copied().unwrap_or(0);
    Ok(encoded[..valid].to_vec())
}

fn decode_frame(graphs: &mut Graphs, frame: &[f32], prediction: &mut Prediction, tokens: &mut Vec<usize>) -> Result<(), String> {
    for _ in 0..MAX_SYMBOLS_PER_FRAME {
        let token = join(&mut graphs.joiner, frame, &prediction.out)?;
        if token == BLANK {
            return Ok(());
        }
        tokens.push(token);
        let (h, c) = (std::mem::take(&mut prediction.h), std::mem::take(&mut prediction.c));
        *prediction = predict(&mut graphs.decoder, token, h, c)?;
    }
    Ok(())
}

fn join(joiner: &mut Session, frame: &[f32], prediction: &[f32]) -> Result<usize, String> {
    let outputs = joiner
        .run(ort::inputs![
            "encoder_output" => Tensor::from_array(([1i64, 1, HIDDEN as i64], frame.to_vec())).map_err(err)?,
            "decoder_output" => Tensor::from_array(([1i64, 1, PRED_HIDDEN as i64], prediction.to_vec())).map_err(err)?,
        ])
        .map_err(|e| format!("joiner Nemotron falhou: {e}"))?;
    let (_, logits) = outputs["joint_output"].try_extract_tensor::<f32>().map_err(err)?;
    Ok(logits.iter().enumerate().fold((BLANK, f32::MIN), |best, (i, &v)| if v > best.1 { (i, v) } else { best }).0)
}

fn predict(decoder: &mut Session, token: usize, h: Vec<f32>, c: Vec<f32>) -> Result<Prediction, String> {
    let state = [PRED_LAYERS, 1, PRED_HIDDEN as i64];
    let outputs = decoder
        .run(ort::inputs![
            "targets" => Tensor::from_array(([1i64, 1], vec![token as i64])).map_err(err)?,
            "h_in" => Tensor::from_array((state, h)).map_err(err)?,
            "c_in" => Tensor::from_array((state, c)).map_err(err)?,
        ])
        .map_err(|e| format!("decoder Nemotron falhou: {e}"))?;
    // `decoder_output` is [1, 640, 1]: with a single target the flat order already matches [1, 1, 640].
    Ok(Prediction {
        out: outputs["decoder_output"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec(),
        h: outputs["h_out"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec(),
        c: outputs["c_out"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec(),
    })
}
