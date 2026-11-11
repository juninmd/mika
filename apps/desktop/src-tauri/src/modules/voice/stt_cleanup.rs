//! Corta as pontas sem som do trecho ANTES do STT local (Whisper e Nemotron) — o único funil por
//! onde passa todo áudio que um motor local ouve (`transcribe::transcribe_local`).
//!
//! **Por quê.** O renderer recorta a fala pelo nível (`speech-transcribe.ts`) e manda junto o
//! pré-roll e o `vadSilenceMs` de cauda (1,2 s por padrão). O Nemotron escala com a duração e
//! pagava essa cauda inteira; medido no smoke (`stt_smoke.rs`): fala baixa sobre ventoinha caiu de
//! 2,6 s para 1,2 s e passou a sair inteira ("Ela pra diferente" → "Ela tá diferente").
//!
//! **Energia, não `FrameFeatures::voiced()`.** O vozeamento de `speech_gate.rs` falha em boa parte
//! da fala real (vogais de `fala-usuario-pt-br.wav` com banda baixa < 0,35) e cortaria palavra.
//! Também NÃO abafa digitação aqui: testado no smoke, o ataque de "Ah" no início da fala real é
//! marcado como tecla, e abafá-lo trocou palavra no Nemotron sem tirar nenhuma alucinação — o
//! trecho só-teclado já é descartado antes, pelo veredito do VAD (`transcribe::neural_vad_rejects`).
//!
//! Sem nada acima do piso, não corta: decidir "sem fala" é do VAD e do `no_speech` do Whisper.

use crate::modules::inference::vad::FRAME_SAMPLES;

/// 384 ms de folga antes da primeira e depois da última janela ativa — consoante surda no fim de
/// palavra e o pré-roll do início moram aqui.
const MARGIN_FRAMES: usize = 12;
/// Janela ativa: 4× (+12 dB) o piso do trecho (percentil 20 da energia por janela)…
const ACTIVE_OVER_FLOOR: f32 = 4.0;
/// …e nunca abaixo de ≈ -50 dBFS RMS, para ruído de fundo baixíssimo não virar "atividade".
const MIN_ACTIVE_RMS: f32 = 100.0;

fn rms(frame: &[i16]) -> f32 {
    (frame.iter().map(|&s| f32::from(s).powi(2)).sum::<f32>() / frame.len().max(1) as f32).sqrt()
}

/// Faixa de amostras a manter: da primeira à última janela ativa, com [`MARGIN_FRAMES`] de folga.
fn active_span(samples: &[i16]) -> std::ops::Range<usize> {
    let levels: Vec<f32> = samples.chunks(FRAME_SAMPLES).map(rms).collect();
    let mut sorted = levels.clone();
    sorted.sort_unstable_by(f32::total_cmp);
    let floor = sorted.get(sorted.len() / 5).copied().unwrap_or(0.0);
    let gate = (floor * ACTIVE_OVER_FLOOR).max(MIN_ACTIVE_RMS);
    match (
        levels.iter().position(|&l| l > gate),
        levels.iter().rposition(|&l| l > gate),
    ) {
        (Some(first), Some(last)) => {
            let start = first.saturating_sub(MARGIN_FRAMES) * FRAME_SAMPLES;
            start..((last + 1 + MARGIN_FRAMES) * FRAME_SAMPLES).min(samples.len())
        }
        _ => 0..samples.len(),
    }
}

/// PCM16 mono 16 kHz sem as pontas abaixo do piso. Devolve quantas amostras saíram.
pub fn trim_silence(samples: &mut Vec<i16>) -> usize {
    let span = active_span(samples);
    let removed = samples.len() - span.len();
    samples.truncate(span.end);
    samples.drain(..span.start);
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: usize = 16_000;

    fn tone(len: usize, amp: f32) -> Vec<i16> {
        (0..len)
            .map(|i| (amp * (i as f32 * 0.05).sin()) as i16)
            .collect()
    }

    #[test]
    fn cuts_the_hangover_but_keeps_the_margin_around_the_speech() {
        let mut samples = vec![0i16; SECOND];
        samples.extend(tone(SECOND, 8_000.0));
        samples.extend(vec![0i16; 2 * SECOND]);
        let removed = trim_silence(&mut samples);
        let margin = MARGIN_FRAMES * FRAME_SAMPLES;
        assert!(
            removed > 2 * SECOND,
            "a cauda de 2 s tinha de sair (saíram {removed})"
        );
        assert!(
            samples.len() >= SECOND + margin,
            "a fala e a folga ficam: {}",
            samples.len()
        );
        assert!(samples.len() <= SECOND + 2 * margin + 2 * FRAME_SAMPLES);
    }

    #[test]
    fn audio_without_anything_above_the_floor_is_left_whole() {
        let mut silence = vec![0i16; 2 * SECOND];
        assert_eq!(trim_silence(&mut silence), 0);
        assert_eq!(silence.len(), 2 * SECOND);
        let mut hum = tone(2 * SECOND, 60.0); // abaixo de MIN_ACTIVE_RMS: zumbido, não fala
        assert_eq!(trim_silence(&mut hum), 0);
    }

    #[test]
    fn steady_noise_with_speech_on_top_keeps_the_speech() {
        let mut samples = tone(SECOND, 300.0); // ventoinha constante
        samples.extend(tone(SECOND, 6_000.0));
        samples.extend(tone(SECOND, 300.0));
        let before = samples.len();
        let removed = trim_silence(&mut samples);
        assert!(
            removed > 0 && samples.len() >= SECOND,
            "{removed} de {before}"
        );
    }

    #[test]
    fn empty_input_stays_empty() {
        let mut empty = Vec::new();
        assert_eq!(trim_silence(&mut empty), 0);
    }
}
