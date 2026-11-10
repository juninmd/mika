//! Separa VOZ de DIGITAÇÃO antes de qualquer decisão que o Silero sozinho tomava — início de fala,
//! barge-in, a atividade que a proatividade vigia e o veredito que `transcribe.rs` consulta.
//!
//! **Onde o ruído de teclado vazava.** No motor `energy` (o default de `voice.vadEngine`), o renderer
//! grava qualquer trecho que passe o gate de NÍVEL (`speech-transcribe.ts`) e `transcribe.rs` não
//! tinha veto nenhum: digitação chegava ao STT inteira. No lado Silero, o risco é o classificador
//! por janela de 32 ms dar picos num clique ou num "thock" — e o barge-in
//! (`interruption_detector.rs`) disparava com 3 janelas seguidas (96 ms), `user_speech_activity`
//! na PRIMEIRA janela acima de 0,5, sem nada olhando a forma do sinal. Nos fixtures de
//! `e2e/fixtures/voice/noise/` o Silero real não chegou a disparar; os testes deste arquivo provam
//! o gate no pior caso (Silero dizendo 0,9 em toda janela de teclado).
//!
//! **Três medidas baratas por janela** ([`FrameAnalyzer::analyze`], ~25–30 µs em 512 amostras):
//! - `impulsive` / `low_impulsive` — pico/mediana da energia em blocos de 8 ms (esta janela e a
//!   anterior) na banda ALTA (o ataque do clique) e na BAIXA (o thock que decai). Vogal é envelope
//!   sustentado (≈ 1–2×); tecla sobe e cai dezenas de dB (> 6×).
//! - `low_band_ratio` — fração da energia abaixo de ~900 Hz (passa-baixa de 2 polos). Clique é
//!   banda-larga, com centróide alto (≈ 0,1–0,3); vogal concentra f0 e F1 ali (> 0,4).
//! - `voicing` — pico da autocorrelação normalizada, na banda baixa decimada a 8 kHz, entre os
//!   atrasos de 80 e 400 Hz (f0 de voz humana). Ruído de clique não tem periodicidade ali.
//!
//! **Decisão com memória** ([`SpeechGate`]): o início de fala exige [`ONSET_FRAMES`] janelas
//! QUALIFICADAS (Silero ≥ limiar e vozeada: periódica, grave e sustentada) com saldo positivo;
//! janela impulsiva sem voz é NEUTRA no saldo (nem soma nem zera) — é isso que deixa "voz +
//! digitação ao mesmo tempo" passar enquanto "só digitação" nunca soma. Confirmada a fala,
//! consoantes surdas seguem valendo pelo Silero; tecla não; o fim vem com [`RELEASE_FRAMES`]
//! janelas sem voz (histerese). A janela de tecla ([`FrameFeatures::keystroke`]) também alimenta
//! `user_speech_activity`, que é o que `transcribe.rs` pergunta para descartar um trecho só-teclado.
//!
//! **O que ficou de fora, de propósito.** Detector de ritmo de digitação (5–15 Hz): nenhuma janela
//! de teclado se qualifica, então o ritmo não precisa ser reconhecido. Sinal do SO de tecla
//! pressionada: exige hook global de teclado (Acessibilidade no macOS, impossível no Wayland) —
//! custo de permissão e privacidade desproporcional. RNNoise (`nnnoiseless`, BSD-3): opera a 48 kHz
//! em quadros de 10 ms (reamostragem ida e volta) e é treinado para ruído estacionário; clique
//! impulsivo é justamente o que ele menos atenua — custo sem ganho aqui.

/// Liga a rejeição de digitação — lida a cada janela (`vad_audio.rs`) e a cada trecho
/// (`transcribe.rs`), então mudar na tela vale na hora, sem recriar a sessão de captura.
pub const CFG_IGNORE_TYPING: &str = "voice.vadIgnoreTyping";

pub fn ignore_typing(store: &crate::config::store::ConfigStore) -> bool {
    store.get(CFG_IGNORE_TYPING).and_then(|v| v.as_bool()).unwrap_or(true)
}

/// Janelas (32 ms cada) qualificadas para CONFIRMAR início de fala: 128 ms de voz de verdade.
pub const ONSET_FRAMES: u32 = 4;
/// Janelas seguidas sem voz para liberar uma fala confirmada: 256 ms — pausa entre palavras não
/// obriga a reconfirmar; fim de frase de verdade é decidido depois, pelo endpointing.
pub const RELEASE_FRAMES: u32 = 8;

const BLOCKS: usize = 4; // 8 ms cada numa janela de 512 @ 16 kHz
const IMPULSIVE_PEAK_OVER_MEDIAN: f32 = 6.0;
const MIN_VOICING: f32 = 0.5;
const MIN_LOW_BAND_RATIO: f32 = 0.35;
/// Abaixo disto (≈ -70 dBFS RMS) a janela é silêncio: nenhuma medida tem sentido.
const SILENCE_RMS: f32 = 10.0;
/// Coeficiente de um polo para fc ≈ 900 Hz @ 16 kHz (`1 - e^(-2π·fc/fs)`), aplicado duas vezes.
const LOW_PASS_ALPHA: f32 = 0.30;
const MIN_LAG: usize = 20; // 400 Hz @ 8 kHz
const MAX_LAG: usize = 100; // 80 Hz @ 8 kHz

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FrameFeatures {
    pub rms: f32,
    /// Transiente na banda alta (clique).
    pub impulsive: bool,
    /// Transiente também na banda baixa: o "thock" grave de um mecânico decai; a vogal não.
    pub low_impulsive: bool,
    pub low_band_ratio: f32,
    pub voicing: f32,
}

impl FrameFeatures {
    /// Estrutura de voz SUSTENTADA na banda baixa — um clique por cima (banda alta) não a desfaz.
    pub fn voiced(&self) -> bool {
        self.rms >= SILENCE_RMS
            && self.voicing >= MIN_VOICING
            && self.low_band_ratio >= MIN_LOW_BAND_RATIO
            && !self.low_impulsive
    }

    /// Transiente de tecla: impulsivo e sem a estrutura de voz — o que `transcribe.rs` pergunta.
    pub fn keystroke(&self) -> bool {
        self.rms >= SILENCE_RMS && self.impulsive && !self.voiced()
    }
}

/// Medidas por janela com a memória de UMA janela anterior — o bastante para a cauda de uma tecla
/// que atravessou a borda continuar sendo vista como impulsiva.
#[derive(Default)]
pub struct FrameAnalyzer {
    prev_blocks: [f32; BLOCKS],
    prev_low_blocks: [f32; BLOCKS],
    /// Estado do passa-baixa entre janelas: zerá-lo a cada janela jogava o sinal inteiro na banda
    /// alta do primeiro bloco — um "ataque" falso em toda borda de janela.
    low_pass_state: (f32, f32),
}

impl FrameAnalyzer {
    /// Uma janela PCM16 mono 16 kHz — ver o cabeçalho do arquivo para as três medidas.
    pub fn analyze(&mut self, samples: &[i16]) -> FrameFeatures {
        let x: Vec<f32> = samples.iter().map(|&s| f32::from(s)).collect();
        let low = low_pass(&x, &mut self.low_pass_state);
        let high: Vec<f32> = x.iter().zip(&low).map(|(v, l)| v - l).collect();
        let block_len = x.len() / BLOCKS;
        let blocks = block_energies(&high);
        let impulsive = is_impulsive(&self.prev_blocks, &blocks, block_len);
        self.prev_blocks = blocks;
        let low_blocks = block_energies(&low);
        let low_impulsive = is_impulsive(&self.prev_low_blocks, &low_blocks, block_len);
        self.prev_low_blocks = low_blocks;
        let energy: f32 = x.iter().map(|v| v * v).sum();
        let rms = (energy / x.len().max(1) as f32).sqrt();
        if rms < SILENCE_RMS {
            return FrameFeatures { rms, ..FrameFeatures::default() };
        }
        let low_energy: f32 = low.iter().map(|v| v * v).sum();
        FrameFeatures { rms, impulsive, low_impulsive, low_band_ratio: low_energy / energy, voicing: voicing(&low) }
    }
}

fn block_energies(x: &[f32]) -> [f32; BLOCKS] {
    let mut out = [0.0f32; BLOCKS];
    let size = x.len().div_ceil(BLOCKS).max(1);
    for (slot, chunk) in out.iter_mut().zip(x.chunks(size)) {
        *slot = chunk.iter().map(|v| v * v).sum();
    }
    out
}

/// Pico sobre mediana da energia de UMA banda em 8 blocos de 8 ms (janela anterior + esta). Na
/// banda alta é o clique (onde ventoinha/zumbido não mora — o passeio lento do ruído grave de uma
/// ventoinha fazia a banda cheia parecer "pico"); na baixa, o thock que decai. Bloco de 8 ms
/// cobre um período inteiro de f0 ≥ 125 Hz: pulso glotal não vira pico, ataque de tecla vira.
fn is_impulsive(prev: &[f32; BLOCKS], cur: &[f32; BLOCKS], block_len: usize) -> bool {
    let mut all = [0.0f32; 2 * BLOCKS];
    all[..BLOCKS].copy_from_slice(prev);
    all[BLOCKS..].copy_from_slice(cur);
    let peak = all.iter().copied().fold(0.0, f32::max);
    if peak < SILENCE_RMS * SILENCE_RMS * block_len as f32 {
        return false; // banda alta em silêncio: nada a medir
    }
    all.sort_unstable_by(f32::total_cmp);
    let median = (all[BLOCKS - 1] + all[BLOCKS]) / 2.0;
    peak > IMPULSIVE_PEAK_OVER_MEDIAN * median.max(f32::EPSILON)
}

fn low_pass(x: &[f32], state: &mut (f32, f32)) -> Vec<f32> {
    let (mut a, mut b) = *state;
    let out = x
        .iter()
        .map(|&v| {
            a += LOW_PASS_ALPHA * (v - a);
            b += LOW_PASS_ALPHA * (a - b);
            b
        })
        .collect();
    *state = (a, b);
    out
}

/// Autocorrelação normalizada na banda baixa decimada por 2 (8 kHz): o pico entre 80 e 400 Hz.
fn voicing(low: &[f32]) -> f32 {
    let d: Vec<f32> = low.chunks(2).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
    let mean = d.iter().sum::<f32>() / d.len().max(1) as f32;
    let d: Vec<f32> = d.iter().map(|v| v - mean).collect();
    (MIN_LAG..MAX_LAG.min(d.len() / 2))
        .map(|lag| normalized_correlation(&d[..d.len() - lag], &d[lag..]))
        .fold(0.0, f32::max)
}

fn normalized_correlation(a: &[f32], b: &[f32]) -> f32 {
    let (mut ab, mut aa, mut bb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    ab / (aa * bb).sqrt().max(f32::EPSILON)
}

/// Estado de início/fim de fala por sessão de captura — ver o cabeçalho do arquivo.
pub struct SpeechGate {
    threshold: f32,
    reject_typing: bool,
    onset: u32,
    confirmed: bool,
    misses: u32,
}

impl SpeechGate {
    pub fn new(threshold: f32) -> Self {
        Self { threshold, reject_typing: true, onset: 0, confirmed: false, misses: 0 }
    }

    /// `voice.vadSpeechThreshold` mudou na UI.
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }

    /// `voice.vadIgnoreTyping` desligado: volta ao veredito cru do Silero, janela a janela.
    pub fn set_reject_typing(&mut self, reject: bool) {
        self.reject_typing = reject;
    }

    /// `true` = esta janela conta como voz do usuário.
    pub fn update(&mut self, probability: f32, features: &FrameFeatures) -> bool {
        let neural = probability >= self.threshold;
        if !self.reject_typing {
            return neural;
        }
        if self.confirmed {
            return self.sustain(neural && features.rms >= SILENCE_RMS && !features.keystroke());
        }
        self.onset = match (neural && features.voiced(), features.impulsive) {
            (true, _) => self.onset + 1,
            (false, true) => self.onset, // tecla por cima da fala: neutra, nem soma nem zera
            (false, false) => self.onset.saturating_sub(1),
        };
        self.confirmed = self.onset >= ONSET_FRAMES;
        self.confirmed
    }

    fn sustain(&mut self, voice: bool) -> bool {
        self.misses = if voice { 0 } else { self.misses + 1 };
        if self.misses >= RELEASE_FRAMES {
            self.confirmed = false;
            self.onset = 0;
            self.misses = 0;
        }
        voice
    }
}

#[cfg(test)]
pub(crate) mod synth {
    //! Sinais sintéticos determinísticos (16 kHz) para os testes deste módulo e de `vad_pipeline.rs`.
    use std::f32::consts::TAU;

    pub const RATE: f32 = 16_000.0;

    pub struct Rng(u64);
    impl Rng {
        pub fn new(seed: u64) -> Self {
            Self(seed.max(1))
        }
        /// Uniforme em [-1, 1).
        pub fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        }
    }

    #[derive(Clone, Copy)]
    pub enum Key {
        /// Clique de membrana/tesoura: rajada de ruído com ênfase em agudos, decaimento τ ms.
        Click { tau_ms: f32 },
        /// "Thock" de mecânico: senoide amortecida grave + clique curto por cima.
        Thock { hz: f32, tau_ms: f32 },
    }

    /// Trem de teclas a `rate_hz` com ±30% de jitter no intervalo, cada tecla com pressão e
    /// "fundo de curso" (segunda rajada 4–9 ms depois), pico `amp` em escala PCM16.
    pub fn typing(seconds: f32, rate_hz: f32, amp: f32, key: Key, seed: u64) -> Vec<f32> {
        let n = (seconds * RATE) as usize;
        let mut out = vec![0.0f32; n];
        let mut rng = Rng::new(seed);
        let mut t = 0.02 * RATE;
        while (t as usize) < n {
            let strike = t as usize;
            add_key(&mut out, strike, amp, key, &mut rng);
            let bottom = strike + ((0.004 + 0.005 * (rng.next() * 0.5 + 0.5)) * RATE) as usize;
            add_key(&mut out, bottom, amp * 0.6, key, &mut rng);
            t += RATE / rate_hz * (1.0 + 0.3 * rng.next());
        }
        out
    }

    fn add_key(out: &mut [f32], at: usize, amp: f32, key: Key, rng: &mut Rng) {
        let mut prev = 0.0f32;
        let len = (0.04 * RATE) as usize;
        for i in 0..len.min(out.len().saturating_sub(at)) {
            let t = i as f32 / RATE;
            let white = rng.next();
            let hiss = white - prev; // diferenciação: ruído com centróide alto
            prev = white;
            out[at + i] += match key {
                Key::Click { tau_ms } => amp * hiss * (-t * 1000.0 / tau_ms).exp(),
                Key::Thock { hz, tau_ms } => {
                    amp * ((TAU * hz * t).sin() * (-t * 1000.0 / tau_ms).exp() + 0.5 * hiss * (-t * 1000.0 / 2.0).exp())
                }
            };
        }
    }

    /// Vogal sustentada: série harmônica de f0 (com vibrato leve) moldada por três formantes.
    pub fn vowel(seconds: f32, f0: f32, formants: [f32; 3], rms: f32) -> Vec<f32> {
        let n = (seconds * RATE) as usize;
        let mut phase = 0.0f32;
        let harmonics = (3_400.0 / f0) as usize;
        let gains: Vec<f32> = (1..=harmonics).map(|h| formant_gain(h as f32 * f0, formants) / h as f32).collect();
        let mut out: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / RATE;
                phase += TAU * f0 * (1.0 + 0.01 * (TAU * 5.0 * t).sin()) / RATE;
                gains.iter().enumerate().map(|(k, g)| g * ((k + 1) as f32 * phase).sin()).sum()
            })
            .collect();
        scale_to_rms(&mut out, rms);
        out
    }

    fn formant_gain(f: f32, formants: [f32; 3]) -> f32 {
        formants.iter().map(|&fc| 1.0 / (1.0 + ((f - fc) / 120.0).powi(2))).sum::<f32>() + 0.05
    }

    /// Fala sintética: vogais em sequência moduladas por envelope silábico de ~4 Hz.
    pub fn speech_like(seconds: f32, rms: f32) -> Vec<f32> {
        let vowels = [[700.0, 1_200.0, 2_600.0], [300.0, 2_300.0, 3_000.0], [500.0, 900.0, 2_400.0]];
        let per = (0.25 * RATE) as usize;
        let n = (seconds * RATE) as usize;
        let mut out = Vec::with_capacity(n);
        for (k, chunk) in (0..n).step_by(per).enumerate() {
            let f0 = 170.0 + 20.0 * (k % 3) as f32;
            let v = vowel(0.25, f0, vowels[k % 3], rms);
            out.extend(v.iter().take(n - chunk).enumerate().map(|(i, s)| {
                let env = 0.35 + 0.65 * (std::f32::consts::PI * i as f32 / per as f32).sin();
                s * env
            }));
        }
        out
    }

    pub struct Typing {
        pub label: String,
        pub rate: f32,
        pub amp: f32,
        pub pcm: Vec<i16>,
    }

    /// 4 ritmos × 3 volumes × 3 tipos de tecla = 36 conjuntos de 3 s.
    pub fn keyboards() -> Vec<Typing> {
        let mut sets = Vec::new();
        for (i, rate) in [6.0, 9.0, 12.0, 15.0].into_iter().enumerate() {
            for (j, amp) in [3_000.0, 12_000.0, 28_000.0].into_iter().enumerate() {
                let seed = (i * 10 + j + 1) as u64;
                for key in [Key::Click { tau_ms: 4.0 }, Key::Click { tau_ms: 9.0 }, Key::Thock { hz: 220.0, tau_ms: 8.0 }] {
                    let kind = match key {
                        Key::Click { tau_ms } => format!("clique τ{tau_ms}ms"),
                        Key::Thock { hz, .. } => format!("thock {hz}Hz"),
                    };
                    let pcm = to_pcm16(&typing(3.0, rate, amp, key, seed));
                    sets.push(Typing { label: format!("{rate} teclas/s, pico {amp}, {kind}"), rate, amp, pcm });
                }
            }
        }
        sets
    }

    pub fn scale_to_rms(x: &mut [f32], rms: f32) {
        let now = (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt().max(f32::EPSILON);
        x.iter_mut().for_each(|v| *v *= rms / now);
    }

    pub fn mix(a: &[f32], b: &[f32]) -> Vec<f32> {
        (0..a.len().max(b.len())).map(|i| a.get(i).unwrap_or(&0.0) + b.get(i).unwrap_or(&0.0)).collect()
    }

    pub fn to_pcm16(x: &[f32]) -> Vec<i16> {
        x.iter().map(|v| v.round().clamp(-32_768.0, 32_767.0) as i16).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::synth::*;
    use super::*;
    use crate::modules::inference::vad::FRAME_SAMPLES;

    fn frames(pcm: &[i16]) -> Vec<FrameFeatures> {
        let mut analyzer = FrameAnalyzer::default();
        pcm.chunks_exact(FRAME_SAMPLES).map(|w| analyzer.analyze(w)).collect()
    }

    /// Pior caso do Silero: probabilidade 0,9 em TODA janela. Só as medidas decidem.
    fn gate_fires(features: &[FrameFeatures]) -> bool {
        let mut gate = SpeechGate::new(0.35);
        features.iter().any(|f| gate.update(0.9, f))
    }

    #[test]
    fn typing_alone_never_opens_the_gate_even_if_silero_says_voice_on_every_frame() {
        for set in keyboards() {
            assert!(!gate_fires(&frames(&set.pcm)), "digitação sozinha abriu o gate: {}", set.label);
        }
    }

    /// Duas teclas na MESMA janela (15/s) podem somar um envelope que não parece impulsivo — por isso
    /// o gate exige saldo de várias janelas, e aqui a régua é "quase nenhuma", não "nenhuma".
    #[test]
    fn keyboard_frames_are_almost_never_voice_like_and_every_second_has_a_keystroke() {
        for set in keyboards() {
            let f = frames(&set.pcm);
            let loud = f.iter().filter(|f| f.rms >= 200.0).count();
            let voice_like = f.iter().filter(|f| f.voiced()).count();
            assert!(voice_like * 10 <= loud, "{}: {voice_like}/{loud} janelas com cara de voz", set.label);
            // O que `transcribe.rs` precisa: todo segundo de digitação tem tecla vista.
            let every_second = f.chunks(31).all(|second| second.iter().any(FrameFeatures::keystroke));
            assert!(every_second, "{}: um segundo inteiro de digitação sem tecla marcada", set.label);
        }
    }

    #[test]
    fn a_sustained_vowel_is_voiced_on_nearly_every_frame_from_85_to_260_hz() {
        let cases = [
            (85.0, [600.0, 1_000.0, 2_500.0]),
            (120.0, [700.0, 1_200.0, 2_600.0]),
            (180.0, [300.0, 2_300.0, 3_000.0]),
            (260.0, [500.0, 900.0, 2_400.0]),
        ];
        for (f0, formants) in cases {
            let f = frames(&to_pcm16(&vowel(1.0, f0, formants, 2_000.0)));
            let voiced = f.iter().filter(|f| f.voiced() && !f.impulsive).count();
            assert!(voiced * 10 >= f.len() * 9, "vogal f0={f0}: só {voiced}/{} janelas vozeadas", f.len());
        }
    }

    #[test]
    fn speech_like_audio_opens_the_gate_within_200ms() {
        let f = frames(&to_pcm16(&speech_like(1.5, 2_000.0)));
        let mut gate = SpeechGate::new(0.35);
        let first = f.iter().position(|f| gate.update(0.9, f)).expect("fala sintética nunca abriu o gate");
        assert!(first < 7, "abriu só na janela {first} (> 200 ms)");
    }

    /// Limite documentado: 15 teclas/s com pico ~21 dB acima do RMS da voz põe um ataque em TODA
    /// janela de 32 ms — nenhuma sobra limpa para confirmar. Todo o resto tem de abrir.
    #[test]
    fn voice_over_typing_still_opens_the_gate() {
        for set in keyboards().into_iter().filter(|s| !(s.rate >= 15.0 && s.amp >= 28_000.0)) {
            let keys: Vec<f32> = set.pcm.iter().map(|&s| f32::from(s)).collect();
            let f = frames(&to_pcm16(&mix(&speech_like(3.0, 2_500.0), &keys)));
            assert!(gate_fires(&f), "voz + digitação não abriu o gate: {}", set.label);
        }
    }

    #[test]
    fn a_confirmed_speech_releases_after_typing_takes_over() {
        let mut signal = speech_like(0.6, 2_000.0);
        signal.extend(typing(2.0, 10.0, 12_000.0, Key::Click { tau_ms: 5.0 }, 7));
        let f = frames(&to_pcm16(&signal));
        let mut gate = SpeechGate::new(0.35);
        let verdicts: Vec<bool> = f.iter().map(|f| gate.update(0.9, f)).collect();
        assert!(verdicts[..18].iter().any(|v| *v), "a fala do início devia confirmar");
        assert!(verdicts[40..].iter().all(|v| !v), "digitação depois da fala não pode manter a voz ligada");
    }

    #[test]
    fn with_typing_rejection_off_the_gate_is_the_raw_silero_threshold() {
        let mut gate = SpeechGate::new(0.35);
        gate.set_reject_typing(false);
        let click = FrameAnalyzer::default().analyze(&to_pcm16(&typing(0.032, 30.0, 12_000.0, Key::Click { tau_ms: 4.0 }, 3)));
        assert!(gate.update(0.9, &click));
        assert!(!gate.update(0.2, &FrameFeatures::default()));
    }

    #[test]
    fn digital_silence_has_no_features() {
        let f = FrameAnalyzer::default().analyze(&[0i16; FRAME_SAMPLES]);
        assert!(!f.voiced() && !f.keystroke());
    }
}
