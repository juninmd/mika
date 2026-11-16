//! O pipeline de inferência do Supertonic 3 — porte de `TextToSpeech::_infer` do exemplo oficial
//! (`supertonic/rust/src/helper.rs`, commit `1e9799e`), a fonte dos nomes de tensor e da ordem dos
//! 4 grafos: `duration_predictor` (duração em segundos) → `text_encoder` (embedding do texto) →
//! `vector_estimator` (laço de remoção de ruído, um passo por chamada) → `vocoder` (forma de onda).
//! A config numérica (`sample_rate`, `base_chunk_size`, `chunk_compress_factor`, `latent_dim`) é
//! lida do `tts.json` fixado, não adivinhada.
//!
//! **Sessões compartilhadas por processo**, mesmo padrão e mesmo motivo de
//! `inference/vad.rs::SHARED_SESSION`: dois carregamentos concorrentes do mesmo arquivo já causaram
//! EACCES neste repositório. O `Mutex` assíncrono colapsa cargas concorrentes numa só; as demais
//! reusam o `Arc`. Guarda o dispositivo que o plano de aceleração (`inference::accel`) pediu: trocar
//! Automática/GPU/CPU recarrega na próxima fala.

use std::path::Path;
use std::sync::Arc;

use ort::session::Session;
use ort::value::Tensor;
use tokio::sync::Mutex as AsyncMutex;

use crate::modules::inference::accel::Model;
use crate::modules::inference::session::{Device, Load};

use crate::modules::voice::tts::abort_registry::CancelHandle;

use super::assets::{SupertonicPaths, VoiceStyle};
use super::text;

/// Os campos do `tts.json` que o pipeline usa.
#[derive(serde::Deserialize, Clone, Copy)]
struct AeConfig {
    sample_rate: u32,
    base_chunk_size: usize,
}

#[derive(serde::Deserialize, Clone, Copy)]
struct TtlConfig {
    chunk_compress_factor: usize,
    latent_dim: usize,
}

#[derive(serde::Deserialize, Clone, Copy)]
struct Config {
    ae: AeConfig,
    ttl: TtlConfig,
}

struct Sessions {
    duration_predictor: AsyncMutex<Session>,
    text_encoder: AsyncMutex<Session>,
    vector_estimator: AsyncMutex<Session>,
    vocoder: AsyncMutex<Session>,
    indexer: Vec<i64>,
    config: Config,
}

static SHARED: AsyncMutex<Option<(Device, Arc<Sessions>)>> = AsyncMutex::const_new(None);
static LEASE: crate::modules::inference::Lease = crate::modules::inference::Lease::new();

/// Unloads the four sessions after `max_idle` without speech — the power governor's
/// `inference::evict_idle`, same policy as Whisper/Nemotron/e5. A synthesis in flight keeps its
/// own `Arc`; the next one reloads.
pub(crate) fn evict_if_idle(max_idle: std::time::Duration, now_ms: u64) -> bool {
    crate::modules::inference::evict_async_if_idle(&SHARED, &LEASE, max_idle, now_ms)
}

fn load_session(load: &mut Load, path: &Path, label: &str) -> Result<AsyncMutex<Session>, String> {
    load.session(path)
        .map(AsyncMutex::new)
        .map_err(|e| format!("ort não carregou {label} ({}): {e}", path.display()))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("não leu {}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{} ilegível: {e}", path.display()))
}

fn build_sessions(paths: &SupertonicPaths) -> Result<(Device, Sessions), String> {
    let mut load = Load::new(
        Model::Supertonic,
        Some(crate::modules::system::governor::onnx_threads()),
    );
    let sessions = Sessions {
        duration_predictor: load_session(
            &mut load,
            &paths.duration_predictor,
            "duration_predictor",
        )?,
        text_encoder: load_session(&mut load, &paths.text_encoder, "text_encoder")?,
        vector_estimator: load_session(&mut load, &paths.vector_estimator, "vector_estimator")?,
        vocoder: load_session(&mut load, &paths.vocoder, "vocoder")?,
        indexer: read_json(&paths.unicode_indexer)?,
        config: read_json(&paths.config)?,
    };
    Ok((load.plan().device, sessions))
}

async fn shared_sessions(paths: SupertonicPaths) -> Result<Arc<Sessions>, String> {
    let mut shared = SHARED.lock().await;
    LEASE.touch();
    let device = crate::modules::inference::accel::plan(Model::Supertonic).device;
    if let Some((_, sessions)) = shared.as_ref().filter(|(loaded, _)| *loaded == device) {
        return Ok(Arc::clone(sessions));
    }
    // Solta a carga antiga antes de montar a nova: dois conjuntos de pesos na VRAM ao mesmo tempo.
    *shared = None;
    let (device, sessions) = tokio::task::spawn_blocking(move || build_sessions(&paths))
        .await
        .map_err(|e| format!("carga do Supertonic cancelada: {e}"))??;
    let sessions = Arc::new(sessions);
    *shared = Some((device, Arc::clone(&sessions)));
    Ok(sessions)
}

/// Só carrega (ou reusa) as sessões — o pré-carregamento do boot.
pub async fn load(paths: SupertonicPaths) -> Result<(), String> {
    shared_sessions(paths).await.map(drop)
}

/// Box-Muller — sem depender de `rand_distr` (não está no grafo do crate).
fn randn(len: usize) -> Vec<f32> {
    use rand::RngExt;
    let mut rng = rand::rng();
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let u1: f32 = rng.random::<f32>().max(f32::EPSILON);
        let u2: f32 = rng.random::<f32>();
        let radius = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * u2;
        out.push(radius * theta.cos());
        if out.len() < len {
            out.push(radius * theta.sin());
        }
    }
    out
}

fn tensor<T: ort::value::PrimitiveTensorElementType + std::fmt::Debug + Clone + 'static>(
    dims: impl Into<Vec<i64>>,
    data: Vec<T>,
) -> Result<Tensor<T>, String> {
    Tensor::from_array((dims.into(), data)).map_err(|e| e.to_string())
}

/// Entradas do texto que valem para os três primeiros grafos.
struct TextInputs {
    ids: Tensor<i64>,
    mask: Tensor<f32>,
    len: i64,
}

fn text_inputs(sessions: &Sessions, tagged: &str) -> Result<TextInputs, String> {
    let ids = text::to_ids(&sessions.indexer, tagged);
    let len = ids.len() as i64;
    Ok(TextInputs {
        ids: tensor([1, len], ids)?,
        mask: tensor([1, 1, len], vec![1.0f32; len as usize])?,
        len,
    })
}

/// Duração da fala em segundos, já dividida pela velocidade.
fn predict_duration(
    sessions: &Sessions,
    text: &TextInputs,
    style: &VoiceStyle,
    speed: f32,
) -> Result<f32, String> {
    let mut session = sessions.duration_predictor.blocking_lock();
    let style_dp = tensor(style.dp_dims, style.dp.clone())?;
    let outputs = session
        .run(ort::inputs!["text_ids" => &text.ids, "style_dp" => style_dp, "text_mask" => &text.mask])
        .map_err(|e| format!("duration_predictor falhou: {e}"))?;
    let (_, duration) = outputs["duration"]
        .try_extract_tensor::<f32>()
        .map_err(|e| e.to_string())?;
    Ok(duration.first().copied().unwrap_or(0.0) / speed.max(0.01))
}

fn encode_text(
    sessions: &Sessions,
    text: &TextInputs,
    style_ttl: &Tensor<f32>,
) -> Result<(Vec<i64>, Vec<f32>), String> {
    let mut session = sessions.text_encoder.blocking_lock();
    let outputs = session
        .run(ort::inputs!["text_ids" => &text.ids, "style_ttl" => style_ttl, "text_mask" => &text.mask])
        .map_err(|e| format!("text_encoder falhou: {e}"))?;
    let (shape, data) = outputs["text_emb"]
        .try_extract_tensor::<f32>()
        .map_err(|e| e.to_string())?;
    Ok((shape.to_vec(), data.to_vec()))
}

/// Forma do latente: `[1, canais, comprimento]` para `duration` segundos de áudio.
fn latent_shape(config: &Config, duration: f32) -> (i64, i64) {
    let wav_len = (duration * config.ae.sample_rate as f32) as usize;
    let chunk = config.ae.base_chunk_size * config.ttl.chunk_compress_factor;
    let len = wav_len.div_ceil(chunk).max(1);
    (
        (config.ttl.latent_dim * config.ttl.chunk_compress_factor) as i64,
        len as i64,
    )
}

/// O laço de remoção de ruído. Os tensores constantes entre passos são montados UMA vez; o latente
/// anda por move — a saída de um passo vira a entrada do próximo sem clone. `cancel` é checado entre
/// passos porque `Session::run` síncrono não tem outro ponto de interrupção.
fn denoise(
    sessions: &Sessions,
    text: &TextInputs,
    text_emb: (Vec<i64>, Vec<f32>),
    style_ttl: &Tensor<f32>,
    (channels, len): (i64, i64),
    total_steps: u32,
    cancel: &CancelHandle,
) -> Result<Vec<f32>, String> {
    let text_emb = tensor(text_emb.0, text_emb.1)?;
    let latent_mask = tensor([1, 1, len], vec![1.0f32; len as usize])?;
    let total = tensor([1], vec![total_steps as f32])?;
    let mut latent = randn((channels * len) as usize);
    let mut session = sessions.vector_estimator.blocking_lock();
    for step in 0..total_steps {
        if cancel.is_cancelled() {
            return Err("TTS cancelled".to_string());
        }
        let outputs = session
            .run(ort::inputs![
                "noisy_latent" => tensor([1, channels, len], latent)?,
                "text_emb" => &text_emb,
                "style_ttl" => style_ttl,
                "latent_mask" => &latent_mask,
                "text_mask" => &text.mask,
                "current_step" => tensor([1], vec![step as f32])?,
                "total_step" => &total,
            ])
            .map_err(|e| format!("vector_estimator falhou no passo {step}: {e}"))?;
        let (_, denoised) = outputs["denoised_latent"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        latent = denoised.to_vec();
    }
    Ok(latent)
}

fn vocode(
    sessions: &Sessions,
    latent: Vec<f32>,
    (channels, len): (i64, i64),
) -> Result<Vec<f32>, String> {
    let mut session = sessions.vocoder.blocking_lock();
    let outputs = session
        .run(ort::inputs!["latent" => tensor([1, channels, len], latent)?])
        .map_err(|e| format!("vocoder falhou: {e}"))?;
    let (_, wav) = outputs["wav_tts"]
        .try_extract_tensor::<f32>()
        .map_err(|e| e.to_string())?;
    Ok(wav.to_vec())
}

fn check(cancel: &CancelHandle) -> Result<(), String> {
    if cancel.is_cancelled() {
        return Err("TTS cancelled".to_string());
    }
    Ok(())
}

/// `tagged` já vem com a tag do idioma ([`text::preprocess`]); `speed`/`steps` já resolvidos pelo
/// chamador (`state::effective_speed`/`supertonic_steps`).
fn generate_speech(
    sessions: &Sessions,
    tagged: &str,
    style: &VoiceStyle,
    speed: f32,
    steps: u32,
    cancel: &CancelHandle,
) -> Result<(Vec<f32>, u32), String> {
    let text = text_inputs(sessions, tagged)?;
    if text.len == 0 {
        return Err("texto sanitizado não produziu nenhum caractere".to_string());
    }
    let duration = predict_duration(sessions, &text, style, speed)?;
    check(cancel)?;
    let style_ttl = tensor(style.ttl_dims, style.ttl.clone())?;
    let text_emb = encode_text(sessions, &text, &style_ttl)?;
    check(cancel)?;
    let shape = latent_shape(&sessions.config, duration);
    let latent = denoise(sessions, &text, text_emb, &style_ttl, shape, steps, cancel)?;
    check(cancel)?;
    let mut wav = vocode(sessions, latent, shape)?;
    let sample_rate = sessions.config.ae.sample_rate;
    wav.truncate((duration * sample_rate as f32) as usize);
    Ok((wav, sample_rate))
}

/// Ponto de entrada assíncrono: carrega (ou reusa) as sessões e roda a síntese inteira dentro de
/// `spawn_blocking` (`ort::Session` síncrono nunca roda direto numa task async).
pub async fn synthesize(
    paths: SupertonicPaths,
    tagged: String,
    style: VoiceStyle,
    speed: f32,
    steps: u32,
    cancel: CancelHandle,
) -> Result<(Vec<f32>, u32), String> {
    let sessions = shared_sessions(paths).await?;
    tokio::task::spawn_blocking(move || {
        generate_speech(&sessions, &tagged, &style, speed, steps, &cancel)
    })
    .await
    .map_err(|e| format!("síntese do Supertonic cancelada: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A conta do exemplo oficial: 1 s a 44,1 kHz com blocos de 512×6 amostras = 15 posições
    /// (arredonda para cima), 24×6 canais.
    #[test]
    fn latent_shape_rounds_the_audio_up_to_whole_chunks() {
        let config = Config {
            ae: AeConfig {
                sample_rate: 44_100,
                base_chunk_size: 512,
            },
            ttl: TtlConfig {
                chunk_compress_factor: 6,
                latent_dim: 24,
            },
        };
        assert_eq!(latent_shape(&config, 1.0), (144, 15));
        assert_eq!(
            latent_shape(&config, 0.0),
            (144, 1),
            "nunca um latente vazio"
        );
    }

    #[test]
    fn randn_produces_the_requested_length_with_both_even_and_odd_counts() {
        assert_eq!(randn(4).len(), 4);
        assert_eq!(randn(5).len(), 5);
        assert_eq!(randn(1).len(), 1);
        assert_eq!(randn(0).len(), 0);
    }

    /// Não é um teste estatístico rigoroso (amostra pequena de propósito, para rodar em
    /// milissegundos) — só uma rede de segurança contra um Box-Muller invertido/degenerado (ex.:
    /// sempre devolvendo `NaN` ou uma constante).
    #[test]
    fn randn_is_not_degenerate() {
        let samples = randn(200);
        assert!(samples.iter().all(|s| s.is_finite()));
        let mean: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(
            mean.abs() < 1.0,
            "média longe de 0 para uma normal padrão: {mean}"
        );
        assert!(
            samples.iter().any(|&s| s != samples[0]),
            "todas as amostras iguais — gerador degenerado"
        );
    }
}
