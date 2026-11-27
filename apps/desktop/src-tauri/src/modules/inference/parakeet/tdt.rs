//! Greedy TDT (token-and-duration transducer) decoding, pure over a step closure. The ONNX graphs
//! live in [`super::pipeline`]; this module only owns the loop so the state/duration rules can be
//! tested with a deterministic fake decoder, no model and no network.
//!
//! Faithful to onnx-asr's `_AsrWithTransducerDecoding._decoding`: the state only advances when a real
//! token is emitted, a positive duration skips frames, and a frame emits at most
//! [`MAX_TOKENS_PER_STEP`] tokens before moving on.

use std::fmt::Debug;

/// onnx-asr's `max_tokens_per_step` default for NeMo transducers.
pub const MAX_TOKENS_PER_STEP: usize = 10;

/// Two LSTM states of the decoder_joint graph (`input_states_1`/`input_states_2`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TdtState {
    pub first: Vec<f32>,
    pub second: Vec<f32>,
}

impl TdtState {
    pub fn zeros(len: usize) -> Self {
        Self { first: vec![0.0; len], second: vec![0.0; len] }
    }
}

fn argmax(values: &[f32]) -> usize {
    values.iter().enumerate().fold((0usize, f32::MIN), |best, (i, &v)| if v > best.1 { (i, v) } else { best }).0
}

/// Decodes `frames` encoder steps. `step(frame, previous_token, &state)` returns the joint logits
/// (token logits first, then duration logits) and the next state.
pub fn greedy_decode<State, Step>(
    frames: usize,
    blank: usize,
    vocab_size: usize,
    max_tokens_per_step: usize,
    initial: State,
    mut step: Step,
) -> Result<Vec<usize>, String>
where
    State: Clone + Debug,
    Step: FnMut(usize, usize, &State) -> Result<(Vec<f32>, State), String>,
{
    let mut t = 0usize;
    let mut emitted = 0usize;
    let mut previous = blank;
    let mut state = initial;
    let mut tokens = Vec::new();
    while t < frames {
        let (logits, next_state) = step(t, previous, &state)?;
        if logits.len() < vocab_size {
            return Err(format!("logits do decoder_joint curtos: {} < {vocab_size}", logits.len()));
        }
        let token = argmax(&logits[..vocab_size]);
        let duration = if logits.len() > vocab_size { argmax(&logits[vocab_size..]) } else { 0 };
        if token != blank {
            state = next_state;
            previous = token;
            tokens.push(token);
            emitted += 1;
        }
        if duration > 0 {
            t += duration;
            emitted = 0;
        } else if token == blank || emitted == max_tokens_per_step {
            t += 1;
            emitted = 0;
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOCAB: usize = 4;
    const DURATIONS: usize = 4;

    fn logits(token: usize, duration: usize) -> Vec<f32> {
        let mut values = vec![-10.0; VOCAB + DURATIONS];
        values[token] = 10.0;
        values[VOCAB + duration] = 10.0;
        values
    }

    #[test]
    fn a_positive_duration_skips_frames() {
        let mut visited = Vec::new();
        let tokens = greedy_decode(5, 0, VOCAB, MAX_TOKENS_PER_STEP, (), |frame, _, _| {
            visited.push(frame);
            let (token, duration) = match frame {
                0 => (1, 2),
                2 => (2, 3),
                _ => (0, 0),
            };
            Ok((logits(token, duration), ()))
        })
        .unwrap();

        assert_eq!(tokens, [1, 2]);
        assert_eq!(visited, [0, 2], "a duração 2 pulou o quadro 1; a duração 3 encerrou a fala");
    }

    #[test]
    fn blank_advances_exactly_one_frame() {
        let mut visited = Vec::new();
        let mut calls = 0;
        let tokens = greedy_decode(3, 0, VOCAB, MAX_TOKENS_PER_STEP, (), |frame, _, _| {
            visited.push(frame);
            calls += 1;
            Ok((logits(if calls == 2 { 1 } else { 0 }, 0), ()))
        })
        .unwrap();

        assert_eq!(tokens, [1]);
        assert_eq!(visited, [0, 1, 1, 2], "o blank andou 1; o token com duração 0 repetiu o quadro");
    }

    #[test]
    fn a_frame_emits_at_most_the_token_limit() {
        let mut calls = 0;
        let tokens = greedy_decode(1, 0, VOCAB, MAX_TOKENS_PER_STEP, (), |_, _, _| {
            calls += 1;
            Ok((logits(1, 0), ()))
        })
        .unwrap();

        assert_eq!(calls, MAX_TOKENS_PER_STEP);
        assert_eq!(tokens.len(), MAX_TOKENS_PER_STEP);
    }

    #[test]
    fn the_state_only_advances_when_a_token_is_emitted() {
        let mut seen = Vec::new();
        let mut calls = 0;
        let tokens = greedy_decode(2, 0, VOCAB, MAX_TOKENS_PER_STEP, 0usize, |frame, _, state| {
            seen.push(*state);
            calls += 1;
            let (token, duration, next) = match calls {
                1 => (2, 0, 1),
                2 => (0, 0, 2),
                _ => (3, 2, 3),
            };
            assert_eq!(frame, if calls < 3 { 0 } else { 1 });
            Ok((logits(token, duration), next))
        })
        .unwrap();

        assert_eq!(tokens, [2, 3]);
        assert_eq!(seen, [0, 1, 1], "o estado do blank (a chamada 2) foi descartado");
    }
}
