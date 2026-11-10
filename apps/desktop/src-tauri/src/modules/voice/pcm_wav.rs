//! Porte de `apps/desktop/src/main/services/voice/stt/pcm-wav.ts::pcm16ToWav` — envolve PCM16 cru
//! (já em bytes little-endian) num cabeçalho RIFF/WAVE mono, sem recodificar amostra nenhuma.
//! Distinto de `voice/tts/providers/supertonic/wav.rs::encode_wav_pcm16` (que parte de `f32` e
//! CONVERTE pra PCM16): aqui a entrada já É PCM16, só falta o cabeçalho — mesma separação que o TS
//! mantém entre `pcm16ToWav` e `encodeWavPcm16`, nunca a mesma função.

const HEADER_BYTES: usize = 44;

pub fn pcm16_to_wav(pcm: &[u8], sample_rate: u32) -> Vec<u8> {
    let mut wav = Vec::with_capacity(HEADER_BYTES + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate (mono, 16 bit)
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(pcm);
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_pcm_bytes_in_a_valid_riff_wave_header() {
        let pcm = vec![1u8, 0, 2, 0, 3, 0]; // 3 amostras i16 LE
        let wav = pcm16_to_wav(&pcm, 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), HEADER_BYTES + pcm.len());
        assert_eq!(&wav[HEADER_BYTES..], &pcm[..]);
    }

    #[test]
    fn header_declares_mono_16bit_at_the_given_sample_rate() {
        let wav = pcm16_to_wav(&[0u8; 4], 44_100);
        let sample_rate = u32::from_le_bytes(wav[24..28].try_into().unwrap());
        let channels = u16::from_le_bytes(wav[22..24].try_into().unwrap());
        let bits = u16::from_le_bytes(wav[34..36].try_into().unwrap());
        assert_eq!(sample_rate, 44_100);
        assert_eq!(channels, 1);
        assert_eq!(bits, 16);
    }

    #[test]
    fn empty_pcm_still_produces_a_readable_header() {
        let wav = pcm16_to_wav(&[], 16_000);
        assert_eq!(wav.len(), HEADER_BYTES);
        let data_len = u32::from_le_bytes(wav[40..44].try_into().unwrap());
        assert_eq!(data_len, 0);
    }
}
