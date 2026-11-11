//! O pipeline de inferência em si — porte de
//! `node_modules/@huggingface/transformers/src/models/supertonic/modeling_supertonic.js`
//! (`SupertonicForConditionalGeneration::generate_speech`), a única fonte consultada para os nomes
//! de tensor e a ordem dos 3 grafos: não é um formato documentado publicamente, e adivinhar aqui
//! teria o mesmo risco que este repositório já pagou em canal reivindicado sem paridade real (ver
//! `project_canal_reivindicado_nao_e_migrado`). Config numérica (`SAMPLING_RATE`, `STYLE_DIM`,
//! `BASE_CHUNK_SIZE`, `CHUNK_COMPRESS_FACTOR`, `LATENT_DIM`) veio do `config.json` publicado do
//! modelo (`onnx-community/Supertonic-TTS-2-ONNX`, mesma revisão pinada de `assets.rs`) — constantes
//! fixas, não lidas em runtime, mesma escolha de `supertonic.runtime.ts` (`VOICE_FLOATS` hardcoded).
//!
//! **Sessão + tokenizer compartilhados por processo**, mesmo padrão e mesmo motivo de
//! `inference/vad.rs::SHARED_SESSION`: dois carregamentos concorrentes do mesmo arquivo já causaram
//! EACCES neste repositório (ver o cabeçalho de lá). O `Mutex` assíncrono colapsa cargas
//! concorrentes numa só; as demais reusam o `Arc`. Guarda o dispositivo que o plano de aceleração
//! (`inference::accel`) pediu: trocar Automática/GPU/CPU recarrega na próxima fala.
//!
//! **`padding`/`truncation` do `tokenizers` 1.0.0-rc.2.** Esta versão da crate é read-only e NÃO
//! aplica truncation automática (`lib.rs` da crate: "truncation and padding ... are not in this
//! release"). Como cada chamada é uma sentença só (`batch_size == 1`, ver `speaker.rs`), padding
//! nunca é necessário; truncation vira um corte manual em [`encode_text`] no mesmo limite do
//! `tokenizer_config.json` (`model_max_length: 1000`), defensivo — nenhuma sentença de TTS real
//! chega perto disso.

use std::path::Path;
use std::sync::Arc;

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::pipeline::{EncodeOptions, PipelineTokenizer};
use tokio::sync::Mutex as AsyncMutex;

use crate::modules::inference::accel::Model;
use crate::modules::inference::session::{Device, Load};

use crate::modules::voice::tts::abort_registry::CancelHandle;

use super::assets::SupertonicPaths;

const SAMPLING_RATE: u32 = 44_100;
const STYLE_DIM: usize = 128;
const BASE_CHUNK_SIZE: usize = 512;
const CHUNK_COMPRESS_FACTOR: usize = 6;
const LATENT_DIM: usize = 24;
const MODEL_MAX_LENGTH: usize = 1_000;

struct Sessions {
    text_encoder: AsyncMutex<Session>,
    latent_denoiser: AsyncMutex<Session>,
    voice_decoder: AsyncMutex<Session>,
    tokenizer: PipelineTokenizer,
}

static SHARED: AsyncMutex<Option<(Device, Arc<Sessions>)>> = AsyncMutex::const_new(None);
static LEASE: crate::modules::inference::Lease = crate::modules::inference::Lease::new();

/// Unloads the three sessions after `max_idle` without speech — the power governor's
/// `inference::evict_idle`, same policy as Whisper/Nemotron/e5. A synthesis in flight keeps its
/// own `Arc`; the next one reloads.
pub(crate) fn evict_if_idle(max_idle: std::time::Duration, now_ms: u64) -> bool {
    crate::modules::inference::evict_async_if_idle(&SHARED, &LEASE, max_idle, now_ms)
}

fn load_session(load: &mut Load, path: &Path, label: &str) -> Result<Session, String> {
    load.session(path).map_err(|e| format!("ort não carregou {label} ({}): {e}", path.display()))
}

fn build_sessions(paths: &SupertonicPaths) -> Result<(Device, Sessions), String> {
    let mut load = Load::new(Model::Supertonic, Some(crate::modules::system::governor::onnx_threads()));
    let text_encoder = load_session(&mut load, &paths.text_encoder, "text_encoder")?;
    let latent_denoiser = load_session(&mut load, &paths.latent_denoiser, "latent_denoiser")?;
    let voice_decoder = load_session(&mut load, &paths.voice_decoder, "voice_decoder")?;

    let canonical = tokenizers::canonicalize_file(&paths.tokenizer)
        .map_err(|e| format!("tokenizer.json não converteu para o formato canônico: {e}"))?;
    let tokenizer =
        tokenizers::from_json(&canonical).map_err(|e| format!("tokenizer.json canônico não carregou: {e}"))?;

    let sessions = Sessions {
        text_encoder: AsyncMutex::new(text_encoder),
        latent_denoiser: AsyncMutex::new(latent_denoiser),
        voice_decoder: AsyncMutex::new(voice_decoder),
        tokenizer,
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

fn encode_text(tokenizer: &PipelineTokenizer, text: &str) -> Result<(Vec<i64>, Vec<i64>), String> {
    let mut encodings = tokenizer
        .encode(text, &EncodeOptions::default())
        .wait()
        .map_err(|e| format!("tokenizer não codificou o texto: {e}"))?;
    let encoding = encodings.pop().ok_or_else(|| "tokenizer devolveu zero encodings".to_string())?;

    let mut ids: Vec<i64> = encoding.ids().iter().map(|t| i64::from(t.id())).collect();
    ids.truncate(MODEL_MAX_LENGTH);
    if ids.is_empty() {
        return Err("texto sanitizado não produziu nenhum token".to_string());
    }
    let attention_mask = vec![1i64; ids.len()];
    Ok((ids, attention_mask))
}

/// Box-Muller — `randn` de `utils/tensor.js`, sem depender de `rand_distr` (não está no grafo do
/// crate; `rand::rng().random::<f32>()` já é suficiente para um par uniforme -> normal).
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

struct DenoiseInputs {
    style: Vec<f32>,
    style_rows: i64,
    encoder_outputs: Vec<f32>,
    encoder_dims: Vec<i64>,
    attention_mask: Vec<i64>,
    seq_len: i64,
}

/// O laço de remoção de ruído (§3 do JS). Roda inteiramente dentro do `spawn_blocking` do
/// chamador — cada `Session::run` é síncrono e `cancel` é checado entre passos porque não há outro
/// ponto de interrupção possível aqui (não é `.await`able).
///
/// Os cinco tensores que não mudam entre passos (estilo, máscaras, saída do encoder, total de
/// passos) são montados UMA vez e passados por referência: antes cada um dos 20 passos clonava a
/// saída inteira do encoder e o estilo, alocação e cópia pagas na frente do primeiro áudio. Os
/// latentes andam por move — a saída de um passo vira a entrada do próximo sem clone.
fn denoise(
    session: &mut Session,
    inputs: DenoiseInputs,
    max_latent_len: i64,
    latent_channels: i64,
    num_inference_steps: u32,
    cancel: &CancelHandle,
) -> Result<Vec<f32>, String> {
    let err = |e: ort::Error| e.to_string();
    let style = Tensor::from_array(([1i64, inputs.style_rows, STYLE_DIM as i64], inputs.style)).map_err(err)?;
    let mask = Tensor::from_array(([1i64, max_latent_len], vec![1i64; max_latent_len as usize])).map_err(err)?;
    let encoder_outputs = Tensor::from_array((inputs.encoder_dims, inputs.encoder_outputs)).map_err(err)?;
    let attention_mask = Tensor::from_array(([1i64, inputs.seq_len], inputs.attention_mask)).map_err(err)?;
    let steps_tensor = Tensor::from_array(([1i64], vec![num_inference_steps as f32])).map_err(err)?;
    let mut latents = randn((latent_channels * max_latent_len) as usize);

    for step in 0..num_inference_steps {
        if cancel.is_cancelled() {
            return Err("TTS cancelled".to_string());
        }
        let noisy_latents = Tensor::from_array(([1i64, latent_channels, max_latent_len], latents)).map_err(err)?;
        let timestep = Tensor::from_array(([1i64], vec![step as f32])).map_err(err)?;
        let outputs = session
            .run(ort::inputs![
                "style" => &style,
                "noisy_latents" => noisy_latents,
                "latent_mask" => &mask,
                "encoder_outputs" => &encoder_outputs,
                "attention_mask" => &attention_mask,
                "timestep" => timestep,
                "num_inference_steps" => &steps_tensor,
            ])
            .map_err(|e| format!("latent_denoiser falhou no passo {step}: {e}"))?;
        let (_, denoised) = outputs["denoised_latents"].try_extract_tensor::<f32>().map_err(err)?;
        latents = denoised.to_vec();
    }

    Ok(latents)
}

/// Porte de `generate_speech`. `speed`/`num_inference_steps` já vêm resolvidos pelo chamador
/// (`state::effective_speed`/`supertonic_steps`) — este arquivo não conhece prosódia.
fn generate_speech(
    sessions: &Sessions,
    text: &str,
    voice_embedding: &[f32],
    speed: f32,
    num_inference_steps: u32,
    cancel: &CancelHandle,
) -> Result<(Vec<f32>, u32), String> {
    let (input_ids, attention_mask) = encode_text(&sessions.tokenizer, text)?;
    let seq_len = input_ids.len() as i64;
    let style_rows = (voice_embedding.len() / STYLE_DIM) as i64;

    let (last_hidden_state, encoder_dims, durations_samples) = {
        let mut encoder = sessions.text_encoder.blocking_lock();
        let ids_tensor = Tensor::from_array(([1i64, seq_len], input_ids)).map_err(|e| e.to_string())?;
        let mask_tensor = Tensor::from_array(([1i64, seq_len], attention_mask.clone())).map_err(|e| e.to_string())?;
        let style_tensor = Tensor::from_array(([1i64, style_rows, STYLE_DIM as i64], voice_embedding.to_vec()))
            .map_err(|e| e.to_string())?;

        let outputs = encoder
            .run(ort::inputs!["input_ids" => ids_tensor, "attention_mask" => mask_tensor, "style" => style_tensor])
            .map_err(|e| format!("text_encoder falhou: {e}"))?;

        let (hs_shape, hs_data) = outputs["last_hidden_state"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        let (_, dur_data) = outputs["durations"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        let duration_samples = dur_data.first().copied().unwrap_or(0.0) / speed.max(0.01) * SAMPLING_RATE as f32;

        (hs_data.to_vec(), hs_shape.to_vec(), duration_samples)
    };

    if cancel.is_cancelled() {
        return Err("TTS cancelled".to_string());
    }

    let latent_size = (BASE_CHUNK_SIZE * CHUNK_COMPRESS_FACTOR) as f32;
    let max_latent_len = ((durations_samples / latent_size).ceil() as i64).max(1);
    let latent_channels = (LATENT_DIM * CHUNK_COMPRESS_FACTOR) as i64;

    let denoise_inputs = DenoiseInputs {
        style: voice_embedding.to_vec(),
        style_rows,
        encoder_outputs: last_hidden_state,
        encoder_dims,
        attention_mask,
        seq_len,
    };

    let final_latents = {
        let mut denoiser = sessions.latent_denoiser.blocking_lock();
        denoise(&mut denoiser, denoise_inputs, max_latent_len, latent_channels, num_inference_steps, cancel)?
    };

    if cancel.is_cancelled() {
        return Err("TTS cancelled".to_string());
    }

    let waveform = {
        let mut decoder = sessions.voice_decoder.blocking_lock();
        let latents_tensor =
            Tensor::from_array(([1i64, latent_channels, max_latent_len], final_latents)).map_err(|e| e.to_string())?;
        let outputs =
            decoder.run(ort::inputs!["latents" => latents_tensor]).map_err(|e| format!("voice_decoder falhou: {e}"))?;
        let (_, wave_data) = outputs["waveform"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        wave_data.to_vec()
    };

    let trim_len = (durations_samples.ceil() as usize).min(waveform.len());
    Ok((waveform[..trim_len].to_vec(), SAMPLING_RATE))
}

/// Ponto de entrada assíncrono: carrega (ou reusa) as sessões e roda a síntese inteira dentro de
/// `spawn_blocking` — mesma garantia de `inference/vad.rs`/`inference/whisper.rs` (`ort::Session`
/// síncrono nunca roda direto numa task async).
pub async fn synthesize(
    paths: SupertonicPaths,
    text: String,
    voice_embedding: Vec<f32>,
    speed: f32,
    num_inference_steps: u32,
    cancel: CancelHandle,
) -> Result<(Vec<f32>, u32), String> {
    let sessions = shared_sessions(paths).await?;
    tokio::task::spawn_blocking(move || {
        generate_speech(&sessions, &text, &voice_embedding, speed, num_inference_steps, &cancel)
    })
    .await
    .map_err(|e| format!("síntese do Supertonic cancelada: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(mean.abs() < 1.0, "média longe de 0 para uma normal padrão: {mean}");
        assert!(samples.iter().any(|&s| s != samples[0]), "todas as amostras iguais — gerador degenerado");
    }
}
