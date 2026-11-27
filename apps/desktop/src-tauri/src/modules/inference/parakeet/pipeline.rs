//! ONNX side of Parakeet TDT: `nemo128` features, the Conformer encoder and the decoder_joint step.
//! The loop itself is in [`super::tdt`]; this file only shapes tensors and transposes the encoder
//! output to the frame-major layout that loop expects (same port as onnx-asr's `NemoConformerTdt`).

use ort::session::Session;
use ort::value::{Tensor, ValueType};

use super::tdt::{self, TdtState};

const N_MELS: usize = 128;

pub struct Graphs {
    pub preprocessor: Session,
    pub encoder: Session,
    pub decoder: Session,
    pub state_shape: [i64; 3],
    pub state_len: usize,
    pub vocab_size: usize,
    pub blank: usize,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub fn transcribe(graphs: &mut Graphs, samples: &[f32]) -> Result<Vec<usize>, String> {
    let Graphs { preprocessor, encoder, decoder, state_shape, state_len, vocab_size, blank } = graphs;
    let (features, features_len) = preprocess(preprocessor, samples)?;
    let (encoded, frames, hidden) = encode(encoder, features, features_len)?;
    tdt::greedy_decode(
        frames,
        *blank,
        *vocab_size,
        tdt::MAX_TOKENS_PER_STEP,
        TdtState::zeros(*state_len),
        |frame, previous, state| joint(decoder, &encoded, frame, hidden, previous, state, *state_shape),
    )
}

/// `features` keeps the preprocessor's `[1, 128, T]` layout; `features_lens` travels as the
/// encoder's `length`.
fn preprocess(session: &mut Session, samples: &[f32]) -> Result<(Vec<f32>, i64), String> {
    let length = samples.len() as i64;
    let outputs = session
        .run(ort::inputs![
            "waveforms" => Tensor::from_array(([1i64, length], samples.to_vec())).map_err(err)?,
            "waveforms_lens" => Tensor::from_array(([1i64], vec![length])).map_err(err)?,
        ])
        .map_err(|e| format!("preprocessador Parakeet falhou: {e}"))?;
    let (shape, features) = outputs["features"].try_extract_tensor::<f32>().map_err(err)?;
    let (_, lens) = outputs["features_lens"].try_extract_tensor::<i64>().map_err(err)?;
    if shape.len() != 3 || shape[1] != N_MELS as i64 {
        return Err(format!("preprocessador Parakeet devolveu formato inesperado: {shape:?}"));
    }
    if features.len() != N_MELS * shape[2] as usize {
        return Err(format!("features do Parakeet com tamanho inesperado: {} para {:?}", features.len(), shape));
    }
    Ok((features.to_vec(), lens.first().copied().unwrap_or(0)))
}

/// Encoder outputs `[1, D, T']`; the TDT loop wants frames `[T', D]`.
fn encode(session: &mut Session, features: Vec<f32>, features_len: i64) -> Result<(Vec<f32>, usize, usize), String> {
    let time = (features.len() / N_MELS) as i64;
    let outputs = session
        .run(ort::inputs![
            "audio_signal" => Tensor::from_array(([1i64, N_MELS as i64, time], features)).map_err(err)?,
            "length" => Tensor::from_array(([1i64], vec![features_len])).map_err(err)?,
        ])
        .map_err(|e| format!("encoder Parakeet falhou: {e}"))?;
    let (shape, encoded) = outputs["outputs"].try_extract_tensor::<f32>().map_err(err)?;
    let (_, lens) = outputs["encoded_lengths"].try_extract_tensor::<i64>().map_err(err)?;
    if shape.len() != 3 {
        return Err(format!("encoder Parakeet devolveu formato inesperado: {shape:?}"));
    }
    let (hidden, time_out) = (shape[1] as usize, shape[2] as usize);
    let frames = (lens.first().copied().unwrap_or(0).max(0) as usize).min(time_out);
    let mut transposed = Vec::with_capacity(frames * hidden);
    for frame in 0..frames {
        for channel in 0..hidden {
            transposed.push(encoded[channel * time_out + frame]);
        }
    }
    Ok((transposed, frames, hidden))
}

fn joint(
    decoder: &mut Session,
    encoded: &[f32],
    frame: usize,
    hidden: usize,
    previous: usize,
    state: &TdtState,
    state_shape: [i64; 3],
) -> Result<(Vec<f32>, TdtState), String> {
    let frame = &encoded[frame * hidden..(frame + 1) * hidden];
    let outputs = decoder
        .run(ort::inputs![
            "encoder_outputs" => Tensor::from_array(([1i64, hidden as i64, 1], frame.to_vec())).map_err(err)?,
            "targets" => Tensor::from_array(([1i64, 1], vec![previous as i32])).map_err(err)?,
            "target_length" => Tensor::from_array(([1i64], vec![1i32])).map_err(err)?,
            "input_states_1" => Tensor::from_array((state_shape, state.first.clone())).map_err(err)?,
            "input_states_2" => Tensor::from_array((state_shape, state.second.clone())).map_err(err)?,
        ])
        .map_err(|e| format!("decoder_joint Parakeet falhou: {e}"))?;
    let (_, logits) = outputs["outputs"].try_extract_tensor::<f32>().map_err(err)?;
    let next = TdtState {
        first: outputs["output_states_1"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec(),
        second: outputs["output_states_2"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec(),
    };
    Ok((logits.to_vec(), next))
}

/// The decoder_joint declares the LSTM state as `[2, -1, 640]`; the middle axis is always one step
/// here. The length comes from the graph, never a hardcoded 640.
pub fn state_shape(decoder: &Session) -> Result<([i64; 3], usize), String> {
    let outlet = decoder
        .inputs()
        .iter()
        .find(|input| input.name() == "input_states_1")
        .ok_or_else(|| "decoder_joint Parakeet sem input_states_1".to_string())?;
    let ValueType::Tensor { shape, .. } = outlet.dtype() else {
        return Err(format!("input_states_1 do Parakeet não é tensor: {:?}", outlet.dtype()));
    };
    if shape.len() != 3 || shape[0] <= 0 || shape[2] <= 0 {
        return Err(format!("formato inesperado de input_states_1 no Parakeet: {shape:?}"));
    }
    let state_shape = [shape[0], 1, shape[2]];
    let len = state_shape.iter().product::<i64>() as usize;
    Ok((state_shape, len))
}
