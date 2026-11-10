//! Log-mel features for Nemotron 3.5 ASR — the NeMo `AudioToMelSpectrogramPreprocessor` recipe that
//! `genai_config.json` of the pinned export declares: preemphasis 0.97, zero-padded centered STFT
//! (`n_fft` 512, hop 160, symmetric Hann of 400 centered in the frame), power spectrum, slaney mel
//! 128 bins, `ln(mel + 2^-24)`, no normalization. Validated against the numpy reference extractor.

use std::f32::consts::PI;
use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};

pub const SAMPLE_RATE: u32 = 16_000;
pub const N_MELS: usize = 128;
const N_FFT: usize = 512;
const HOP: usize = 160;
const WIN: usize = 400;
const PREEMPHASIS: f32 = 0.97;
const LOG_GUARD: f32 = 5.960_464_5e-8; // 2^-24

pub struct LogMel {
    window: Vec<f32>,
    /// Each triangular filter as `(first bin, weights)`: the dense rows are ~95% zeros.
    filters: Vec<(usize, Vec<f32>)>,
    fft: Arc<dyn RealToComplex<f32>>,
}

impl LogMel {
    pub fn new() -> Self {
        // `torch.hann_window(400, periodic=False)` zero-padded to the centre of the 512 frame.
        let pad = (N_FFT - WIN) / 2;
        let mut window = vec![0.0f32; N_FFT];
        for i in 0..WIN {
            window[pad + i] = 0.5 - 0.5 * (2.0 * PI * i as f32 / (WIN - 1) as f32).cos();
        }
        // A direct DFT here cost 2.3 s per 10 s of audio in a debug build; the planned FFT is ms.
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(N_FFT);
        let filters = super::super::whisper::build_mel_filterbank(N_FFT / 2 + 1, N_MELS, SAMPLE_RATE as f64)
            .into_iter()
            .map(nonzero_span)
            .collect();
        Self { window, filters, fft }
    }

    /// Frames of `N_MELS` values, row-major (`frames × N_MELS`), for the whole utterance.
    pub fn compute(&self, samples: &[f32]) -> Vec<f32> {
        if samples.is_empty() {
            return Vec::new();
        }
        let mut signal = vec![0.0f32; N_FFT / 2];
        signal.push(samples[0]);
        signal.extend(samples.windows(2).map(|w| w[1] - PREEMPHASIS * w[0]));
        signal.extend(std::iter::repeat_n(0.0, N_FFT / 2));
        let frames = 1 + (signal.len() - N_FFT) / HOP;
        let mut out = Vec::with_capacity(frames * N_MELS);
        let mut windowed = self.fft.make_input_vec();
        let mut spectrum = self.fft.make_output_vec();
        let mut scratch = self.fft.make_scratch_vec();
        let mut power = vec![0.0f32; spectrum.len()];
        for f in 0..frames {
            for (i, w) in windowed.iter_mut().enumerate() {
                *w = signal[f * HOP + i] * self.window[i];
            }
            self.fft.process_with_scratch(&mut windowed, &mut spectrum, &mut scratch).expect("buffers sized by the plan");
            for (p, c) in power.iter_mut().zip(&spectrum) {
                *p = c.norm_sqr();
            }
            out.extend(self.filters.iter().map(|(start, row)| (row.iter().zip(&power[*start..]).map(|(a, b)| a * b).sum::<f32>() + LOG_GUARD).ln()));
        }
        out
    }
}

fn nonzero_span(row: Vec<f32>) -> (usize, Vec<f32>) {
    let start = row.iter().position(|&w| w != 0.0).unwrap_or(0);
    let end = row.iter().rposition(|&w| w != 0.0).map_or(start, |i| i + 1);
    (start, row[start..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference values from the numpy extractor (`LogMelExtractor(center=True)`) on 0.1 s of a
    // 440 Hz sine at amplitude 0.5.
    #[test]
    fn matches_the_numpy_reference_extractor() {
        let samples: Vec<f32> = (0..1600).map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / 16_000.0).sin()).collect();
        let mel = LogMel::new().compute(&samples);
        assert_eq!(mel.len(), 11 * N_MELS);
        let at = |frame: usize, bin: usize| mel[frame * N_MELS + bin];
        for (got, want) in [(at(0, 0), -8.261_191), (at(0, 3), -7.050_059), (at(5, 12), -9.581_101), (at(10, 20), -1.482_739)] {
            assert!((got - want).abs() < 2e-3, "log-mel {got} != reference {want}");
        }
    }

    #[test]
    fn empty_input_yields_no_frames() {
        assert!(LogMel::new().compute(&[]).is_empty());
    }
}
