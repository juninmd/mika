//! Smart Turn v3.2 (pipecat-ai, BSD-2-Clause) — classificador de FIM DE TURNO sobre `ort`.
//!
//! O VAD (Silero) continua sendo o piso acústico: ele sabe que o usuário parou de emitir som, não
//! que a frase terminou. Smart Turn responde a segunda pergunta com um encoder Whisper-Tiny: os
//! últimos 8 s de áudio viram um log-mel 80×800 e o grafo devolve uma única probabilidade sigmoide
//! de "turno completo" (o `logits` do ONNX JÁ é probabilidade; não há softmax a aplicar aqui).
//!
//! **Reuse, não reimplementação.** O log-mel é o MESMO de `whisper.rs`
//! ([`super::whisper::log_mel_spectrogram_with`] + [`super::whisper::build_mel_filterbank`]), chamado
//! com `n_samples = 8 * 16000` e `n_frames = 800`. Duplicar a fórmula criaria duas verdades: um dia
//! uma delas muda e o classificador começa a ler o áudio por outro filtro em silêncio.
//!
//! **Pré-processamento (referência `_whisper_features.py`/`audio_utils.py` do pipecat):**
//! 1. mantém os ÚLTIMOS 8 s; áudio mais curto é preenchido com zeros NO INÍCIO (a fala fica colada
//!    no fim da janela, que é onde o modelo aprendeu a olhar);
//! 2. normaliza zero-média/variância unitária sobre a janela inteira: `(x - mean) / sqrt(var + 1e-7)`;
//! 3. log-mel Whisper (n_fft 400, hop 160, 80 mels slaney, reflect pad, log10 com piso 1e-10,
//!    max-8, `(x + 4) / 4`).
//!
//! **Download sob demanda, verificado.** O `.onnx` NÃO é rastreado no repositório (~8,7 MB):
//! [`ensure_model`] baixa da revisão fixa do Hugging Face, confere tamanho e SHA-256 e grava com o
//! mesmo funil atômico de `modules/desktop.rs` (chokepoint de privacidade incluso). Chamadas
//! concorrentes esperam o MESMO download — o `tokio::Mutex` cobre o `await` inteiro.
//!
//! **Sessão única no processo.** Como o VAD, abrir dois `ort::Session` do mesmo arquivo já custou
//! EACCES no boot; aqui o cache é um `Mutex<Option<Arc<Mutex<Session>>>>`, então a primeira carga
//! vence e as demais recebem o mesmo `Arc`. O modelo tem 8 MB e roda em CPU por escolha:
//! `Model::SmartTurn` é `Workload::Tiny` em `accel.rs`, onde GPU só somaria cópia.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ort::session::Session;
use ort::value::Tensor;

use crate::config::store::ConfigStore;

use super::accel::Model;
use super::whisper;

/// Arquivo publicado em `pipecat-ai/smart-turn-v3` na revisão fixa abaixo.
pub const SMART_TURN_MODEL_FILE: &str = "smart-turn-v3.2-cpu.onnx";
/// Revisão `f766f81d3cfdf7737ac64aad813d91bbfd56bf93` — commit, nunca branch/tag.
pub const SMART_TURN_REVISION: &str = "f766f81d3cfdf7737ac64aad813d91bbfd56bf93";
/// SHA-256 do arquivo de 8.679.182 bytes.
pub const SMART_TURN_MODEL_SHA256: &str = "2bb026316b14a660486a75b1733cd3fbab8c2fd0314dc9af7be49f8cca967e4f";
pub const SMART_TURN_MODEL_BYTES: u64 = 8_679_182;
pub const SMART_TURN_LICENSE: &str = "BSD-2-Clause";

const CACHE_DIR: &str = "smart-turn-cache";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const DOWNLOAD_TAG: &str = "smart-turn:model";

/// O contrato de áudio do módulo `inference`: PCM16 mono 16 kHz. Smart Turn só consome a mesma
/// janela de 8 s que o modelo foi treinado para ver.
pub const SAMPLE_RATE: u32 = 16_000;
pub const WINDOW_SAMPLES: usize = 8 * SAMPLE_RATE as usize;
/// 80 × 800 — a forma fixa que o grafo exporta e que `prepare_features` devolve achatada.
pub const N_MELS: usize = whisper::N_MELS;
pub const N_FRAMES: usize = 800;

fn model_url() -> String {
    format!("https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/{SMART_TURN_REVISION}/{SMART_TURN_MODEL_FILE}")
}

/// `<data>/smart-turn-cache/smart-turn-v3.2-cpu.onnx` — o mesmo layout `*-cache/` do VAD e do
/// Whisper, para o cache de modelo ser um conceito só.
pub fn smart_turn_model_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CACHE_DIR).join(SMART_TURN_MODEL_FILE)
}

/// Áudio de qualquer duração → exatamente [`WINDOW_SAMPLES`]. Mais longo perde o começo; mais curto
/// ganha zeros na frente (nunca atrás: o fim da fala é o sinal de que o modelo precisa).
fn last_eight_seconds(samples: &[f32]) -> Vec<f32> {
    let mut window = vec![0.0f32; WINDOW_SAMPLES];
    if samples.len() >= WINDOW_SAMPLES {
        window.copy_from_slice(&samples[samples.len() - WINDOW_SAMPLES..]);
    } else {
        window[WINDOW_SAMPLES - samples.len()..].copy_from_slice(samples);
    }
    window
}

/// Zero-média/variância unitária sobre a janela cheia (zeros do pad incluídos), com o mesmo epsilon
/// de `transformers.WhisperFeatureExtractor`. Em `f64` na soma: 128 mil `f32` acumulados perdem
/// precisão e mudariam o espectrograma por ruído de arredondamento, não por conteúdo.
fn normalize(window: &[f32]) -> Vec<f32> {
    let len = window.len() as f64;
    let mean = window.iter().map(|&v| f64::from(v)).sum::<f64>() / len;
    let variance = window.iter().map(|&v| (f64::from(v) - mean).powi(2)).sum::<f64>() / len;
    let scale = (variance + 1e-7).sqrt();
    window.iter().map(|&v| ((f64::from(v) - mean) / scale) as f32).collect()
}

/// Banco mel do Whisper montado uma vez por processo — a mesma matriz em toda fala, e derivá-la a
/// cada classificação custaria mais que a própria inferência de ~12 ms.
fn mel_filters() -> &'static Vec<Vec<f32>> {
    static FILTERS: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
    FILTERS.get_or_init(|| whisper::build_mel_filterbank(whisper::N_FFT / 2 + 1, N_MELS, f64::from(SAMPLE_RATE)))
}

/// `samples_16k` → `[80, 800]` achatado, pronto para o tensor `[1, 80, 800]` de `input_features`.
pub fn prepare_features(samples_16k: &[f32]) -> Vec<f32> {
    let window = last_eight_seconds(samples_16k);
    let normalized = normalize(&window);
    whisper::log_mel_spectrogram_with(&normalized, mel_filters(), WINDOW_SAMPLES, N_FRAMES)
}

/// Cache de sessão do processo. `SmartTurn::load` segura o lock enquanto monta a sessão; a segunda
/// chamada concorrente encontra o `Arc` pronto em vez de abrir um segundo `Session` do mesmo file.
static SESSION: Mutex<Option<Arc<Mutex<Session>>>> = Mutex::new(None);

/// Classificador carregado. `completion_probability` é síncrono (como `Session::run`): quem chama de
/// uma task async deve rodar dentro de `spawn_blocking`, o mesmo contrato de `whisper.rs`.
pub struct SmartTurn {
    session: Arc<Mutex<Session>>,
}

impl SmartTurn {
    /// Abre (ou reusa) a sessão a partir do `.onnx` já baixado e verificado por [`ensure_model`].
    /// Não baixa nada: modelo ausente é erro explícito, nunca um download disfarçado de carga.
    pub fn load(model_path: &Path) -> Result<Self, String> {
        if !model_path.is_file() {
            return Err(format!(
                "modelo do Smart Turn ausente em {}; chame inference::smart_turn::ensure_model antes de carregar",
                model_path.display()
            ));
        }

        let mut cache = SESSION.lock().map_err(|_| "cache da sessão do Smart Turn envenenado".to_string())?;
        if let Some(session) = cache.as_ref() {
            return Ok(Self { session: Arc::clone(session) });
        }

        let (session, _) = super::session::load(Model::SmartTurn, model_path, Some(1))
            .map_err(|e| format!("ort não carregou o Smart Turn ({}): {e}", model_path.display()))?;
        let session = Arc::new(Mutex::new(session));
        *cache = Some(Arc::clone(&session));
        Ok(Self { session })
    }

    /// Probabilidade de turno completo, em `[0, 1]` — o `logits` do modelo já é sigmoide.
    pub fn completion_probability(&mut self, samples_16k: &[f32]) -> Result<f32, String> {
        let features = prepare_features(samples_16k);
        let mut session = self.session.lock().map_err(|_| "sessão do Smart Turn envenenada".to_string())?;

        let input = Tensor::from_array(([1usize, N_MELS, N_FRAMES], features)).map_err(|e| e.to_string())?;
        let outputs = session
            .run(ort::inputs!["input_features" => input])
            .map_err(|e| format!("inferência do Smart Turn falhou: {e}"))?;
        let (_, data) = outputs["logits"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        let probability = *data.first().ok_or_else(|| "Smart Turn não devolveu logit".to_string())?;

        if !probability.is_finite() {
            return Err(format!("Smart Turn devolveu probabilidade não finita ({probability})"));
        }
        Ok(probability.clamp(0.0, 1.0))
    }
}

/// Download do modelo, uma vez por processo. O `await` inteiro (rede + hash + gravação) roda com o
/// lock tomado: duas chamadas concorrentes não baixam 8,7 MB duas vezes, a segunda espera e recebe o
/// caminho já preparado. Falha não fica cacheada — a próxima chamada tenta de novo.
static PREPARED: tokio::sync::Mutex<Option<PathBuf>> = tokio::sync::Mutex::const_new(None);
static IN_FLIGHT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn is_prepared(path: &Path) -> bool {
    let prepared = PREPARED.lock().await;
    prepared.as_deref() == Some(path) && path.is_file()
}

async fn mark_prepared(path: &Path) {
    *PREPARED.lock().await = Some(path.to_path_buf());
}

async fn download_and_store(store: &ConfigStore, path: &Path) -> Result<(), String> {
    let bytes = crate::modules::desktop::download_bytes(store, DOWNLOAD_TAG, &model_url(), DOWNLOAD_TIMEOUT).await?;
    if bytes.len() as u64 != SMART_TURN_MODEL_BYTES {
        return Err(format!("modelo do Smart Turn com {} bytes, esperado {SMART_TURN_MODEL_BYTES}", bytes.len()));
    }
    let digest = crate::modules::desktop::sha256_hex(&bytes);
    if digest != SMART_TURN_MODEL_SHA256 {
        return Err(format!("modelo do Smart Turn com digest inesperado: {digest}"));
    }
    crate::modules::desktop::ensure_parent_dir(path)?;
    crate::modules::desktop::write_model_file_atomic(path, &bytes)
}

pub async fn ensure_model(store: &ConfigStore, data_dir: &Path) -> Result<PathBuf, String> {
    let path = smart_turn_model_path(data_dir);
    if is_prepared(&path).await {
        return Ok(path);
    }

    let _in_flight = IN_FLIGHT.lock().await;
    if is_prepared(&path).await {
        return Ok(path);
    }
    if !crate::modules::desktop::file_matches_sha256(&path, SMART_TURN_MODEL_SHA256) {
        download_and_store(store, &path).await?;
    }
    mark_prepared(&path).await;
    log::info!("Smart Turn v3.2 ({SMART_TURN_LICENSE}) pronto em {}", path.display());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn model_path_lives_in_the_smart_turn_cache() {
        let path = smart_turn_model_path(Path::new("/data"));
        assert_eq!(path, PathBuf::from("/data/smart-turn-cache/smart-turn-v3.2-cpu.onnx"));
    }

    #[test]
    fn prepared_features_have_the_shape_the_onnx_input_requires() {
        let features = prepare_features(&vec![0.1f32; WINDOW_SAMPLES]);
        assert_eq!(features.len(), N_MELS * N_FRAMES);
        assert!(features.iter().all(|v| v.is_finite()), "log-mel com NaN/infinito");
    }

    #[test]
    fn short_audio_is_padded_with_zeros_at_the_start() {
        let window = last_eight_seconds(&[0.5, 1.0, 1.5]);
        assert_eq!(window.len(), WINDOW_SAMPLES);
        assert_eq!(&window[..WINDOW_SAMPLES - 3], &vec![0.0f32; WINDOW_SAMPLES - 3]);
        assert_eq!(&window[WINDOW_SAMPLES - 3..], &[0.5, 1.0, 1.5]);
    }

    #[test]
    fn long_audio_keeps_the_tail_not_the_head() {
        let samples: Vec<f32> = (0..WINDOW_SAMPLES + 4).map(|i| i as f32).collect();
        let window = last_eight_seconds(&samples);
        assert_eq!(window.len(), WINDOW_SAMPLES);
        assert_eq!(window[0], 4.0);
        assert_eq!(window[WINDOW_SAMPLES - 1], (WINDOW_SAMPLES + 3) as f32);
    }

    #[test]
    fn normalization_is_zero_mean_unit_variance() {
        let normalized = normalize(&[1.0, 2.0, 3.0, 4.0]);
        let mean = normalized.iter().sum::<f32>() / normalized.len() as f32;
        let variance = normalized.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / normalized.len() as f32;
        assert!(mean.abs() < 1e-5, "média devia ser ~0, deu {mean}");
        assert!((variance - 1.0).abs() < 1e-3, "variância devia ser ~1, deu {variance}");
    }

    #[test]
    fn load_fails_loudly_when_the_model_file_is_missing() {
        let missing = std::env::temp_dir().join("mika-smart-turn-definitely-missing.onnx");
        let _ = std::fs::remove_file(&missing);
        let err = SmartTurn::load(&missing).err().expect("modelo ausente é erro");
        assert!(err.contains("ausente"), "erro devia dizer que o modelo está ausente: {err}");
    }

    fn read_wav(path: &Path) -> Vec<f32> {
        let mut reader = hound::WavReader::open(path).unwrap_or_else(|e| panic!("abrir {}: {e}", path.display()));
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, SAMPLE_RATE, "fixture não está em 16 kHz: {}", path.display());
        assert_eq!(spec.channels, 1, "fixture não é mono: {}", path.display());
        reader
            .samples::<i16>()
            .map(|s| s.expect("amostra PCM16 válida") as f32 / 32768.0)
            .collect()
    }

    /// Modelo REAL, fora do repositório (~8,7 MB). Sem `SMART_TURN_MODEL` (ou sem os WAVs ao lado
    /// dele) o teste se declara pulado com `eprintln!`, nunca verde otimista: o `assert` de
    /// separação só roda contra os bytes de verdade que a produção baixa.
    /// Os WAVs são frases pt-BR: `completa.wav` terminada e `hesitacao_pausa.wav` cortada no meio,
    /// antes de "corrida", com 600 ms de silêncio.
    ///   SMART_TURN_MODEL=/caminho/para/smart-turn-v3.2-cpu.onnx \
    ///     cargo test --lib inference::smart_turn::tests::real_model_separates_complete_from_hesitant_audio
    #[test]
    fn real_model_separates_complete_from_hesitant_audio() {
        let Ok(model) = std::env::var("SMART_TURN_MODEL") else {
            eprintln!("SMART_TURN_MODEL não definido; teste do modelo real do Smart Turn pulado");
            return;
        };
        let model = PathBuf::from(model);
        if !model.is_file() {
            eprintln!("{} não existe; teste do modelo real do Smart Turn pulado", model.display());
            return;
        }
        let dir = model.parent().unwrap_or_else(|| Path::new("."));
        let complete = dir.join("completa.wav");
        let hesitant = dir.join("hesitacao_pausa.wav");
        if !complete.is_file() || !hesitant.is_file() {
            eprintln!("WAVs completa.wav/hesitacao_pausa.wav ausentes em {}; teste pulado", dir.display());
            return;
        }

        let mut turn = SmartTurn::load(&model).expect("carregar Smart Turn real");
        let complete_probability = turn.completion_probability(&read_wav(&complete)).expect("inferência (completa)");
        let hesitant_probability = turn.completion_probability(&read_wav(&hesitant)).expect("inferência (hesitação)");
        eprintln!("Smart Turn: completa={complete_probability:.6} hesitacao_pausa={hesitant_probability:.6}");

        assert!(
            complete_probability > 0.5,
            "fala completa devia passar de 0,5, deu {complete_probability}"
        );
        assert!(
            hesitant_probability < 0.5,
            "pausa de hesitação devia ficar abaixo de 0,5, deu {hesitant_probability}"
        );
    }
}
