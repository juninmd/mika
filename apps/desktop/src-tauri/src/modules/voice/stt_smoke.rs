//! Smoke test de escuta (STT + VAD) sobre os fixtures versionados em `e2e/fixtures/voice/`.
//!
//! Não roda no gate padrão: precisa dos pesos (Silero, Whisper base, Nemotron) num diretório de
//! dados com o mesmo layout do app (`vad-cache/`, `whisper-cache/whisper-base/`, `nemotron-cache/`).
//!
//! ```text
//! MIKA_STT_SMOKE_DIR=<dados> MIKA_STT_SMOKE_OUT=<relatorio.json> CARGO_PROFILE_TEST_OPT_LEVEL=2 \
//!   cargo test --lib stt_smoke -- --ignored --nocapture
//! ```
//!
//! `MIKA_STT_SMOKE_ONLY=<trecho do nome>` roda só os arquivos que o contêm.
//! ```text
//! ```
//!
//! Mede, por arquivo: o veredito do VAD (Silero + `SpeechGate`, o mesmo par de `vad_pipeline.rs`),
//! o texto e a latência de cada motor local. Em arquivo SEM fala o STT roda mesmo assim — é o pior
//! caso (motor `energy` sem tecla detectada deixa o trecho passar) e o que mede alucinação.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Value};

use super::speech_gate::{FrameAnalyzer, SpeechGate};
use super::transcribe::transcribe_local;
use crate::modules::inference::vad::{Pcm16Frame, SileroVad, FRAME_SAMPLES};

const RATE: u32 = 16_000;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/voice")
}

fn read_wav(path: &Path) -> Vec<i16> {
    let mut reader = hound::WavReader::open(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    assert_eq!(reader.spec().sample_rate, RATE, "{path:?} fora de 16 kHz");
    reader.samples::<i16>().map(Result::unwrap).collect()
}

/// `(segundos com voz confirmada pelo gate, início da primeira janela de voz)`.
async fn vad_verdict(vad: &mut SileroVad, samples: &[i16]) -> (f32, Option<f32>) {
    vad.reset();
    let (mut analyzer, mut gate) = (FrameAnalyzer::default(), SpeechGate::new(0.5));
    let (mut voiced, mut first) = (0usize, None);
    for (i, chunk) in samples.chunks_exact(FRAME_SAMPLES).enumerate() {
        let features = analyzer.analyze(chunk);
        let p = vad
            .probability(Pcm16Frame {
                samples: chunk,
                sample_rate: RATE,
                channels: 1,
            })
            .await
            .unwrap();
        if gate.update(p, &features) {
            voiced += 1;
            first.get_or_insert(i as f32 * FRAME_SAMPLES as f32 / RATE as f32);
        }
    }
    (voiced as f32 * FRAME_SAMPLES as f32 / RATE as f32, first)
}

async fn stt(provider: &str, data_dir: &Path, samples: &[i16]) -> (String, u128) {
    let pcm: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let started = Instant::now();
    let text = transcribe_local(
        provider,
        "onnx-community/whisper-base",
        data_dir.to_path_buf(),
        pcm,
        RATE,
        Some("pt".into()),
    )
    .await
    .unwrap_or_else(|e| panic!("{provider}: {e}"))
    .unwrap_or_default();
    (text, started.elapsed().as_millis())
}

fn cases() -> Vec<(String, PathBuf, bool)> {
    let dir = fixtures_dir();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("noise/vad-cases.json")).unwrap())
            .unwrap();
    let mut out: Vec<(String, PathBuf, bool)> = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let file = c["file"].as_str().unwrap();
            (
                format!("noise/{file}"),
                dir.join("noise").join(file),
                c["expectSpeech"].as_bool().unwrap(),
            )
        })
        .collect();
    out.push((
        "fala-usuario-pt-br.wav".into(),
        dir.join("fala-usuario-pt-br.wav"),
        true,
    ));
    let mut commands: Vec<_> = std::fs::read_dir(dir.join("commands"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    commands.sort();
    out.extend(commands.into_iter().map(|p| {
        (
            format!("commands/{}", p.file_name().unwrap().to_string_lossy()),
            p,
            true,
        )
    }));
    out
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "precisa dos pesos Silero/Whisper/Nemotron em MIKA_STT_SMOKE_DIR"]
async fn stt_smoke_report() {
    let data_dir = PathBuf::from(std::env::var("MIKA_STT_SMOKE_DIR").expect("MIKA_STT_SMOKE_DIR"));
    let mut vad = SileroVad::load(&data_dir.join("vad-cache/silero_vad.onnx"))
        .await
        .unwrap();
    let providers = ["local-whisper", "nemotron"];
    let warm = read_wav(&fixtures_dir().join("fala-usuario-pt-br.wav"));
    for p in providers {
        stt(p, &data_dir, &warm).await; // carga das sessões fora da medida
    }
    let mut rows = Vec::new();
    let only = std::env::var("MIKA_STT_SMOKE_ONLY").unwrap_or_default();
    for (name, path, expect_speech) in cases()
        .into_iter()
        .filter(|(name, ..)| name.contains(&only))
    {
        let samples = read_wav(&path);
        let (voiced_s, first) = vad_verdict(&mut vad, &samples).await;
        let mut row = json!({ "file": name, "expectSpeech": expect_speech, "audioMs": samples.len() as u64 * 1000 / u64::from(RATE), "vadVoicedS": voiced_s, "vadFirstS": first });
        for p in providers {
            let (text, ms) = stt(p, &data_dir, &samples).await;
            eprintln!("{name:<38} {p:<14} {ms:>6} ms  {text:?}");
            row[p] = json!({ "text": text, "ms": ms });
        }
        rows.push(row);
    }
    if let Ok(out) = std::env::var("MIKA_STT_SMOKE_OUT") {
        std::fs::write(out, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
    }
}
