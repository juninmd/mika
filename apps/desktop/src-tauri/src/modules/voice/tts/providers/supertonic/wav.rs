//! Porte de `apps/desktop/src/main/services/inference/wav.ts::encodeWavPcm16` — `f32` em `[-1, 1]`
//! para PCM16 WAV mono. `hound` já é dependência do crate (declarada para o pipeline de áudio de
//! saída); reusar em vez de escrever o cabeçalho RIFF à mão.

pub fn encode_wav_pcm16(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec { channels: 1, sample_rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };

    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| format!("wav writer: {e}"))?;
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let pcm = (clamped * i16::MAX as f32) as i16;
            writer.write_sample(pcm).map_err(|e| format!("wav write_sample: {e}"))?;
        }
        writer.finalize().map_err(|e| format!("wav finalize: {e}"))?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_a_valid_wav_header_that_hound_can_read_back() {
        let samples = [0.0f32, 0.5, -0.5, 1.0, -1.0];
        let bytes = encode_wav_pcm16(&samples, 44_100).unwrap();

        let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().sample_rate, 44_100);
        assert_eq!(reader.spec().channels, 1);
        let decoded: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(decoded.len(), samples.len());
    }

    #[test]
    fn clamps_out_of_range_samples_instead_of_wrapping() {
        let bytes = encode_wav_pcm16(&[2.0, -2.0], 16_000).unwrap();
        let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        let decoded: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(decoded, vec![i16::MAX, -i16::MAX]);
    }
}
