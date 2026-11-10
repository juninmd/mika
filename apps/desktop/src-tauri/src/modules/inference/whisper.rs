//! STT (Whisper) sobre `ort`. Destrava `mika:voice-transcribe`, hoje recusado em
//! `modules/voice.rs` porque transcrição que devolve string vazia é pior que não migrar.
//!
//! **Estado real, verificado nesta rodada (2026-09-21):** o pipeline inteiro — espectrograma
//! log-mel, encoder ONNX, laço autoregressivo do decoder com cache KV, tokenizer — roda de ponta a
//! ponta contra `apps/desktop/e2e/fixtures/voice/fala-usuario-pt-br.wav` (fala real em pt-BR) e
//! produz `" Ah, ela voltou! A pra diferente ela, eu tô mudando ela."`, que bate com a referência
//! documentada em `e2e/fixtures/voice/README.md` (`"Ah, ela voltou!"` ... `"...ela, eu estou
//! mudando ela"`, com o mesmo trecho central corrompido que o próprio README atribui ao modelo
//! `base`). Isso não é um teste automatizado deste arquivo — rodar de novo exige os pesos ONNX e o
//! `tokenizer.json` baixados manualmente, o que a suíte padrão não faz (ver `transcribe_matches_known_reference_wav`,
//! `#[ignore]`, com o caminho para reproduzir).
//!
//! **Atualizado nesta rodada (2026-09-26):** `voice/transcribe.rs::transcribe_local` já chama
//! `transcribe()` — o canal `mika:voice-transcribe` está ligado, não é mais um caminho morto.
//! `WHISPER_MODELS` (abaixo) é a tabela única tamanho -> (id, revisão, sha256); `modules/desktop.rs`
//! a importa em vez de manter sua própria cópia. `download_bytes`/`ConfigStore` (privados a
//! `desktop.rs`) continuam sendo o único chokepoint de rede — nenhum `reqwest` solto aqui.
//! `require_asset` abaixo falha alto e explícito quando um arquivo não está no cache.
//!
//! **O que continua best-effort:** detecção de idioma quando `language` vem `None` (um
//! forward-pass extra do decoder com só o token inicial, argmax restrito aos 99 tokens de idioma)
//! — mesma ideia do `detect_language` do Whisper original, não necessariamente byte-a-byte igual
//! ao que `transformers.js` faria.
//!
//! `ort` é a ÚNICA opção de ONNX aqui — `tract-onnx` está reprovado e documentado, não reabrir.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use ort::session::Session;

use super::accel::Model;
use ort::value::Tensor;
use tokenizers::pipeline::PipelineTokenizer;

// --- Modelo selecionável — espelha `modules/desktop.rs::prepare_whisper` -------------------------
//
// Tabela ÚNICA (não duplicada): `desktop.rs` importa `WHISPER_MODELS` daqui em vez de manter sua
// própria cópia de ids/revisões/sha256 — um tamanho errado duplicado em dois arquivos é o tipo de
// divergência que só aparece em produção (baixa um arquivo, verifica contra o hash do outro).
// `voice.whisperModel` (`config/schemas/voice.rs`) é o enum de tamanhos; cada entrada aqui é um
// espelho de `https://huggingface.co/api/models/<id>?blobs=true` (campo `sha` = revisão, `lfs.sha256`
// por arquivo — NÃO o ETag final do Hugging Face, que é hash do CDN Xet, não SHA-256; ver o
// cabeçalho de `modules/desktop.rs`). Verificado nesta rodada contra a API ao vivo em 2026-09-26.

pub const DEFAULT_WHISPER_MODEL_ID: &str = "onnx-community/whisper-base";

/// `tokenizer.json` não é rastreado como LFS num destes três repos (arquivo pequeno, blob git comum)
/// — a API não devolve `lfs.sha256` para ele. O `blobId` (sha1 do git) reportado pela API é
/// IDÊNTICO nos três (`onnx-community/whisper-{tiny,base,small}`): mesmo conteúdo byte-a-byte, os
/// três tamanhos compartilham o mesmo vocabulário multilíngue de 51865 tokens. Por isso o SHA-256
/// abaixo (medido uma vez, sobre o arquivo de `whisper-base` — o único já baixado e verificado em
/// produção) vale para os três: um hash errado só faria o download falhar alto (`sha256_hex !=
/// expected`), nunca aceitar um arquivo adulterado — a mesma direção seguro-por-falha que o
/// cabeçalho de `desktop.rs` já documenta.
const SHARED_TOKENIZER_SHA256: &str = "27fc476bfe7f17299480be2273fc0608e4d5a99aba2ab5dec5374b4482d1a566";

pub struct WhisperModelSpec {
    pub id: &'static str,
    pub revision: &'static str,
    pub encoder_sha256: &'static str,
    pub decoder_sha256: &'static str,
    pub tokenizer_sha256: &'static str,
}

pub const WHISPER_MODELS: [WhisperModelSpec; 3] = [
    WhisperModelSpec {
        id: "onnx-community/whisper-tiny",
        revision: "ff4177021cc41f7db950912b73ea4fdf7d01d8e7",
        encoder_sha256: "2af4a414ca47aa30f61246017e5fe82b0a8d229281d1255ba666a2a7f6b84d19",
        decoder_sha256: "25e807a962b6349356d0ea5d0dfe530b7e5bf0e2a484aeca0359d03143faddd3",
        tokenizer_sha256: SHARED_TOKENIZER_SHA256,
    },
    WhisperModelSpec {
        id: DEFAULT_WHISPER_MODEL_ID,
        revision: "1846881b6b3a3024392c1eea3ad983695bc23925",
        encoder_sha256: "5862993336bf33acd23736071aae2b32261d3b1b2f37780194460d4ef974dd46",
        decoder_sha256: "fa3ef9902734ce5ae6f9ef2bdb2ba9a6c4b5785b09f4f420ce036573dc9d090b",
        tokenizer_sha256: SHARED_TOKENIZER_SHA256,
    },
    WhisperModelSpec {
        id: "onnx-community/whisper-small",
        revision: "36050c46d777d46dc4b5f43f6d90574fc38f8732",
        encoder_sha256: "a43a83f3c5361cd591cfa7c36f14b43cf7cb22f47a415cc14a8d557be800fa92",
        decoder_sha256: "ec07c3cbb64172c39791e26ee870a65ac22b458c36722bfe2776b3dbf741e0c9",
        tokenizer_sha256: SHARED_TOKENIZER_SHA256,
    },
];

/// Nunca falha: um `model_id` desconhecido (config corrompida, valor de antes de um tamanho sair
/// do enum) cai no default em vez de derrubar a transcrição inteira.
pub fn whisper_model_spec(model_id: &str) -> &'static WhisperModelSpec {
    WHISPER_MODELS
        .iter()
        .find(|m| m.id == model_id)
        .or_else(|| WHISPER_MODELS.iter().find(|m| m.id == DEFAULT_WHISPER_MODEL_ID))
        .expect("DEFAULT_WHISPER_MODEL_ID precisa estar em WHISPER_MODELS")
}

/// Último segmento do id (`whisper-tiny`/`whisper-base`/`whisper-small`) — nome do subdiretório de
/// cache por tamanho, para trocar de modelo não pisar nos arquivos de outro já baixado.
pub fn whisper_model_slug(model_id: &str) -> &str {
    model_id.rsplit('/').next().unwrap_or(model_id)
}

// --- Contrato de assets — espelha `modules/desktop.rs::whisper_cache_path`/`prepare_whisper` -----

const WHISPER_CACHE_DIR: &str = "whisper-cache";
const ENCODER_FILE: &str = "encoder_model_quantized.onnx";
const DECODER_FILE: &str = "decoder_model_merged_quantized.onnx";
const TOKENIZER_FILE: &str = "tokenizer.json";

/// Recebe `&WhisperModelSpec`, nunca `model_id: &str` cru — o path tem de vir SEMPRE do allowlist
/// resolvido por `whisper_model_spec` (ver a regressão de segurança no cabeçalho de
/// `transcribe_blocking`). `spec.id` é sempre um dos IDs fixos de `WHISPER_MODELS`.
fn whisper_cache_path(data_dir: &Path, spec: &WhisperModelSpec, file: &str) -> PathBuf {
    data_dir.join(WHISPER_CACHE_DIR).join(whisper_model_slug(spec.id)).join(file)
}

/// Falha alta e explícita se um asset não estiver no cache — nunca um `Option` silencioso, porque
/// "modelo ausente" e "modelo vazio" são erros diferentes e quem chama precisa saber qual é qual.
fn require_asset(data_dir: &Path, spec: &WhisperModelSpec, file: &str) -> Result<PathBuf, String> {
    let path = whisper_cache_path(data_dir, spec, file);
    if !path.is_file() {
        return Err(format!(
            "asset do Whisper ausente: {path:?} — rode o wizard (`mika:wizard-prepare-asset`, branch \"whisper\") \
             para baixar {}",
            spec.id
        ));
    }
    Ok(path)
}

// --- Contrato de áudio — Main só aceita PCM16 mono 16 kHz (AGENTS.md §2 "Áudio & Inferência") ---

const REQUIRED_SAMPLE_RATE: u32 = 16_000;
/// Espelha `MIN_SAMPLES` de `whisper.runtime.ts`: menos de 100ms não é fala, é ruído de borda —
/// devolver `None` (não erro) para esse caso é o mesmo contrato do lado TS.
const MIN_SAMPLES: usize = REQUIRED_SAMPLE_RATE as usize / 10;

/// PCM16LE mono para `f32` em `[-1, 1]` — mesma divisão de `pcm16ToFloat32` em `whisper.runtime.ts`.
/// Erro alto (não conversão silenciosa) quando a entrada não é o que o contrato promete.
fn pcm16_to_f32(pcm: &[u8], sample_rate: u32) -> Result<Vec<f32>, String> {
    if sample_rate != REQUIRED_SAMPLE_RATE {
        return Err(format!("Whisper espera {REQUIRED_SAMPLE_RATE} Hz e recebeu {sample_rate} Hz"));
    }
    if pcm.len() % 2 != 0 {
        return Err(format!("PCM16 mal formado: {} bytes não é múltiplo de 2 (mono 16-bit)", pcm.len()));
    }
    Ok(pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect())
}

// --- Espectrograma log-mel — espelha `WhisperFeatureExtractor` (preprocessor_config.json do
// modelo: n_fft=400, hop=160, 80 mels, 16 kHz, 30s/3000 frames) ----------------------------------

const N_FFT: usize = 400;
const HOP_LENGTH: usize = 160;
const N_MELS: usize = 80;
const N_SAMPLES_30S: usize = 480_000;
const N_FRAMES_30S: usize = 3000;
const SAMPLE_RATE_F64: f64 = REQUIRED_SAMPLE_RATE as f64;

/// Escala mel "Slaney" (linear abaixo de 1 kHz, log acima) — mesma fórmula de
/// `transformers.audio_utils.hertz_to_mel(..., mel_scale="slaney")`, não a variante HTK.
fn hz_to_mel_slaney(hz: f64) -> f64 {
    const MIN_LOG_HZ: f64 = 1000.0;
    const MIN_LOG_MEL: f64 = 15.0;
    let logstep = 27.0 / 6.4f64.ln();
    if hz >= MIN_LOG_HZ {
        MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() * logstep
    } else {
        3.0 * hz / 200.0
    }
}

fn mel_to_hz_slaney(mel: f64) -> f64 {
    const MIN_LOG_HZ: f64 = 1000.0;
    const MIN_LOG_MEL: f64 = 15.0;
    let logstep = 6.4f64.ln() / 27.0;
    if mel >= MIN_LOG_MEL {
        MIN_LOG_HZ * (logstep * (mel - MIN_LOG_MEL)).exp()
    } else {
        200.0 * mel / 3.0
    }
}

/// Banco de filtros triangulares mel, normalizado por área ("slaney") — `[n_mels][n_freq_bins]`.
/// Mesma matemática de `transformers.audio_utils.mel_filter_bank(norm="slaney", mel_scale="slaney")`;
/// não há vetor pré-computado embarcado (o `mel_filters.npz` do whisper original é um asset binário
/// que este projeto não tem como baixar/verificar aqui), então o filtro é derivado da fórmula.
pub(super) fn build_mel_filterbank(n_freq_bins: usize, n_mels: usize, sample_rate: f64) -> Vec<Vec<f32>> {
    let mel_min = hz_to_mel_slaney(0.0);
    let mel_max = hz_to_mel_slaney(sample_rate / 2.0);
    let mel_points: Vec<f64> = (0..n_mels + 2).map(|i| mel_min + (mel_max - mel_min) * (i as f64) / ((n_mels + 1) as f64)).collect();
    let filter_freqs: Vec<f64> = mel_points.iter().map(|&m| mel_to_hz_slaney(m)).collect();
    let fft_freqs: Vec<f64> = (0..n_freq_bins).map(|i| (sample_rate / 2.0) * (i as f64) / ((n_freq_bins - 1) as f64)).collect();
    let filter_diff: Vec<f64> = filter_freqs.windows(2).map(|w| w[1] - w[0]).collect();

    let mut filters = vec![vec![0.0f32; n_freq_bins]; n_mels];
    for (m, row) in filters.iter_mut().enumerate() {
        for (k, &f) in fft_freqs.iter().enumerate() {
            let down = -(filter_freqs[m] - f) / filter_diff[m];
            let up = (filter_freqs[m + 2] - f) / filter_diff[m + 1];
            row[k] = down.min(up).max(0.0) as f32;
        }
        let area_norm = (2.0 / (filter_freqs[m + 2] - filter_freqs[m])) as f32;
        for v in row.iter_mut() {
            *v *= area_norm;
        }
    }
    filters
}

/// Hann periódico (`periodic=True` do `torch.stft`): `w[0] == 0`, não `w[n-1] == 0` também — a
/// variante "simétrica" repetiria a primeira amostra no fim e desalinha o STFT do Whisper.
fn hann_window_periodic(n: usize) -> Vec<f32> {
    (0..n).map(|i| (0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()) as f32).collect()
}

/// Reflect-pad manual — equivalente ao `center=True` (padding "reflect") do `torch.stft`, sem
/// depender de nenhuma crate de DSP (nenhuma está no grafo de dependências deste projeto).
fn reflect_pad(samples: &[f32], pad: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(samples.len() + 2 * pad);
    for i in (1..=pad).rev() {
        out.push(samples[i.min(samples.len() - 1)]);
    }
    out.extend_from_slice(samples);
    let n = samples.len();
    for i in 1..=pad {
        out.push(samples[n.saturating_sub(1 + i)]);
    }
    out
}

/// DFT direta (O(n²)) do espectro de potência de um frame já janelado — `n_fft` é pequeno (400),
/// e nenhuma crate de FFT está no grafo de dependências deste projeto (adicionar uma está fora do
/// escopo deste arquivo). Retorna só os `n_fft/2 + 1` bins não redundantes (sinal real).
fn dft_power_spectrum(windowed_frame: &[f32]) -> Vec<f32> {
    let n_fft = windowed_frame.len();
    let n_bins = n_fft / 2 + 1;
    let mut out = vec![0.0f32; n_bins];
    for (k, bin) in out.iter_mut().enumerate() {
        let mut re = 0.0f64;
        let mut im = 0.0f64;
        for (n, &sample) in windowed_frame.iter().enumerate() {
            let angle = -2.0 * PI * (k as f64) * (n as f64) / (n_fft as f64);
            re += sample as f64 * angle.cos();
            im += sample as f64 * angle.sin();
        }
        *bin = (re * re + im * im) as f32;
    }
    out
}

/// Log-mel de UM frame — a unidade que o teste rápido exercita. O espectrograma completo (3000
/// frames) só roda em produção e no teste `#[ignore]` com áudio real: 3000 × 201 × 400 DFTs
/// ingênuas por chamada é caro demais para entrar no gate padrão.
fn frame_mel_energies(padded_signal: &[f32], frame_idx: usize, window: &[f32], filters: &[Vec<f32>]) -> Vec<f32> {
    let start = frame_idx * HOP_LENGTH;
    let windowed: Vec<f32> = (0..N_FFT).map(|i| padded_signal[start + i] * window[i]).collect();
    let power = dft_power_spectrum(&windowed);

    filters.iter().map(|row| row.iter().zip(power.iter()).map(|(f, p)| f * p).sum()).collect()
}

/// Espectrograma log-mel completo, parametrizado em duração para permitir um teste barato (poucos
/// frames) sem duplicar a fórmula. Produção sempre chama com `n_samples`/`n_frames` = 30s/3000,
/// porque o encoder ONNX exporta `last_hidden_state` com comprimento fixo (1500 = 3000/2).
fn log_mel_spectrogram_with(samples: &[f32], filters: &[Vec<f32>], n_samples: usize, n_frames: usize) -> Vec<f32> {
    let mut padded_audio = vec![0.0f32; n_samples];
    let n = samples.len().min(n_samples);
    padded_audio[..n].copy_from_slice(&samples[..n]);

    let stft_input = reflect_pad(&padded_audio, N_FFT / 2);
    let window = hann_window_periodic(N_FFT);

    let mut mel = vec![0.0f32; N_MELS * n_frames];
    for frame_idx in 0..n_frames {
        let energies = frame_mel_energies(&stft_input, frame_idx, &window, filters);
        for (m, &e) in energies.iter().enumerate() {
            mel[m * n_frames + frame_idx] = e;
        }
    }

    let mut log_spec: Vec<f32> = mel.iter().map(|&v| v.max(1e-10).log10()).collect();
    let max_val = log_spec.iter().cloned().fold(f32::MIN, f32::max);
    for v in log_spec.iter_mut() {
        *v = v.max(max_val - 8.0);
        *v = (*v + 4.0) / 4.0;
    }
    log_spec
}

fn log_mel_spectrogram(samples: &[f32], filters: &[Vec<f32>]) -> Vec<f32> {
    log_mel_spectrogram_with(samples, filters, N_SAMPLES_30S, N_FRAMES_30S)
}

// --- Tokens especiais do Whisper multilíngue — de `generation_config.json` do modelo alvo
// (`onnx-community/whisper-base`, revisão fixada em `WHISPER_MODELS` acima). Hardcoded porque são
// constantes da FAMÍLIA whisper (tiny/base/small compartilham o mesmo tokenizer e os mesmos ids —
// só os pesos do encoder/decoder mudam de tamanho), e baixar `generation_config.json` teria o
// mesmo problema de chokepoint de rede que os pesos ONNX têm (ver cabeçalho do módulo).

const DECODER_START_TOKEN: i64 = 50258;
const TASK_TRANSCRIBE_TOKEN: i64 = 50359;
const NO_TIMESTAMPS_TOKEN: i64 = 50363;
const EOS_TOKEN: i64 = 50257;
const MAX_NEW_TOKENS: usize = 224;

/// `<|lang|>` → id, os 99 idiomas do Whisper multilíngue. Copiado ao vivo de
/// `generation_config.json.lang_to_id` (`onnx-community/whisper-base`) — não é uma tabela
/// aproximada, é a mesma que o checkpoint usa.
const LANG_TO_ID: &[(&str, i64)] = &[
    ("af", 50327), ("am", 50334), ("ar", 50272), ("as", 50350), ("az", 50304), ("ba", 50355),
    ("be", 50330), ("bg", 50292), ("bn", 50302), ("bo", 50347), ("br", 50309), ("bs", 50315),
    ("ca", 50270), ("cs", 50283), ("cy", 50297), ("da", 50285), ("de", 50261), ("el", 50281),
    ("en", 50259), ("es", 50262), ("et", 50307), ("eu", 50310), ("fa", 50300), ("fi", 50277),
    ("fo", 50338), ("fr", 50265), ("gl", 50319), ("gu", 50333), ("haw", 50352), ("ha", 50354),
    ("he", 50279), ("hi", 50276), ("hr", 50291), ("ht", 50339), ("hu", 50286), ("hy", 50312),
    ("id", 50275), ("is", 50311), ("it", 50274), ("ja", 50266), ("jw", 50356), ("ka", 50329),
    ("kk", 50316), ("km", 50323), ("kn", 50306), ("ko", 50264), ("la", 50294), ("lb", 50345),
    ("ln", 50353), ("lo", 50336), ("lt", 50293), ("lv", 50301), ("mg", 50349), ("mi", 50295),
    ("mk", 50308), ("ml", 50296), ("mn", 50314), ("mr", 50320), ("ms", 50282), ("mt", 50343),
    ("my", 50346), ("ne", 50313), ("nl", 50271), ("nn", 50342), ("no", 50288), ("oc", 50328),
    ("pa", 50321), ("pl", 50269), ("ps", 50340), ("pt", 50267), ("ro", 50284), ("ru", 50263),
    ("sa", 50344), ("sd", 50332), ("si", 50322), ("sk", 50298), ("sl", 50305), ("sn", 50324),
    ("so", 50326), ("sq", 50317), ("sr", 50303), ("su", 50357), ("sv", 50273), ("sw", 50318),
    ("ta", 50287), ("te", 50299), ("tg", 50331), ("th", 50289), ("tk", 50341), ("tl", 50348),
    ("tr", 50268), ("tt", 50351), ("uk", 50280), ("ur", 50290), ("uz", 50337), ("vi", 50278),
    ("yi", 50335), ("yo", 50325), ("zh", 50260),
];

fn language_token_id(lang: &str) -> Result<i64, String> {
    // `voice.asrLanguage` é texto livre: `pt-BR`/`pt_BR`/`PT` valem como `pt` (o Nemotron aceita os mesmos).
    let primary = lang.trim().split(['-', '_']).next().unwrap_or_default().to_lowercase();
    LANG_TO_ID
        .iter()
        .find(|(code, _)| *code == primary)
        .map(|(_, id)| *id)
        .ok_or_else(|| format!("idioma '{lang}' não é um código Whisper conhecido"))
}

// --- Inferência ONNX — encoder (1 chamada) + decoder (laço autoregressivo com cache KV) ---------
//
// Formato "merged decoder" do Optimum/transformers.js: `use_cache_branch=false` no primeiro passo
// (o grafo ignora o cache do decoder e computa o cross-attention K/V a partir de
// `encoder_hidden_states`, emitindo os dois em `present.*`); passos seguintes usam
// `use_cache_branch=true`, realimentam `present.*.decoder.*` como cache de self-attention E
// `present.*.encoder.*` do PRIMEIRO passo como cross-attention (que não muda — o encoder não roda
// de novo). Confirmado nomeando as entradas/saídas reais do `.onnx` baixado, não por suposição.
/// Forma do cache KV do decoder, lida das entradas do PRÓPRIO `.onnx`. Era `6 × 8 × 64` fixo — a
/// forma do whisper-base —, e tiny (4 camadas, 6 heads) e small (12, 12), oferecidos na UI, recebiam
/// tensores do tamanho errado e nunca transcreviam.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DecoderShape {
    layers: usize,
    heads: i64,
    head_dim: i64,
}

/// `inputs`: (nome, shape) de cada entrada do decoder. `past_key_values.N.decoder.key` tem forma
/// `[batch, heads, past_seq, head_dim]`, com `heads`/`head_dim` fixos no grafo (batch/seq dinâmicos, -1).
fn decoder_shape_from(inputs: &[(String, Vec<i64>)]) -> Result<DecoderShape, String> {
    let is_decoder_key = |name: &str| name.starts_with("past_key_values.") && name.ends_with(".decoder.key");
    let layers = inputs.iter().filter(|(name, _)| is_decoder_key(name)).count();
    let first = inputs.iter().find(|(name, _)| name == "past_key_values.0.decoder.key").map(|(_, shape)| shape);
    match first {
        Some(shape) if layers > 0 && shape.len() == 4 && shape[1] > 0 && shape[3] > 0 => {
            Ok(DecoderShape { layers, heads: shape[1], head_dim: shape[3] })
        }
        _ => Err(format!("decoder Whisper sem forma de cache KV legível ({layers} camadas encontradas)")),
    }
}

fn decoder_shape(decoder: &Session) -> Result<DecoderShape, String> {
    let inputs: Vec<(String, Vec<i64>)> =
        decoder.inputs().iter().map(|input| (input.name().to_string(), input.dtype().tensor_shape().map(|s| s.to_vec()).unwrap_or_default())).collect();
    decoder_shape_from(&inputs)
}

struct KvCache {
    key: Vec<f32>,
    value: Vec<f32>,
}

/// Um passo do decoder: `input_ids` novos + estado de cache, devolve os logits da última posição
/// e o cache atualizado (self-attention sempre; cross-attention só no primeiro passo, quando o
/// chamador ainda não tem um para reaproveitar).
#[allow(clippy::too_many_arguments)]
fn decoder_step(
    decoder: &mut Session,
    kv: DecoderShape,
    input_ids: &[i64],
    encoder_hidden_states: &[f32],
    enc_seq_len: i64,
    hidden: i64,
    use_cache: bool,
    past_decoder: &[KvCache],
    past_encoder: Option<&[KvCache]>,
) -> Result<(Vec<f32>, usize, Vec<KvCache>, Option<Vec<KvCache>>), String> {
    let cur_len = input_ids.len() as i64;
    let past_len = past_decoder[0].key.len() as i64 / (kv.heads * kv.head_dim);

    let mut inputs = ort::inputs![
        "input_ids" => Tensor::from_array(([1i64, cur_len], input_ids.to_vec())).map_err(|e| e.to_string())?,
        "encoder_hidden_states" => Tensor::from_array(([1i64, enc_seq_len, hidden], encoder_hidden_states.to_vec())).map_err(|e| e.to_string())?,
        "use_cache_branch" => Tensor::from_array(([1usize], vec![use_cache])).map_err(|e| e.to_string())?,
    ];

    for l in 0..kv.layers {
        let dec = &past_decoder[l];
        inputs.push((
            format!("past_key_values.{l}.decoder.key").into(),
            Tensor::from_array(([1i64, kv.heads, past_len, kv.head_dim], dec.key.clone())).map_err(|e| e.to_string())?.into(),
        ));
        inputs.push((
            format!("past_key_values.{l}.decoder.value").into(),
            Tensor::from_array(([1i64, kv.heads, past_len, kv.head_dim], dec.value.clone())).map_err(|e| e.to_string())?.into(),
        ));
        let (enc_key, enc_value, enc_len) = match past_encoder.map(|pe| &pe[l]) {
            Some(enc) => (enc.key.clone(), enc.value.clone(), enc_seq_len),
            None => (Vec::new(), Vec::new(), 0i64),
        };
        inputs.push((
            format!("past_key_values.{l}.encoder.key").into(),
            Tensor::from_array(([1i64, kv.heads, enc_len, kv.head_dim], enc_key)).map_err(|e| e.to_string())?.into(),
        ));
        inputs.push((
            format!("past_key_values.{l}.encoder.value").into(),
            Tensor::from_array(([1i64, kv.heads, enc_len, kv.head_dim], enc_value)).map_err(|e| e.to_string())?.into(),
        ));
    }

    let outputs = decoder.run(inputs).map_err(|e| format!("decoder Whisper falhou: {e}"))?;
    let (logits_shape, logits_data) = outputs["logits"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
    let vocab = logits_shape[2] as usize;
    let last_pos = (logits_shape[1] - 1) as usize;
    let last_logits = logits_data[last_pos * vocab..(last_pos + 1) * vocab].to_vec();

    let mut new_decoder_cache = Vec::with_capacity(kv.layers);
    let mut new_encoder_cache = if past_encoder.is_none() { Some(Vec::with_capacity(kv.layers)) } else { None };
    for l in 0..kv.layers {
        let (_, dk) = outputs[format!("present.{l}.decoder.key")].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        let (_, dv) = outputs[format!("present.{l}.decoder.value")].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        new_decoder_cache.push(KvCache { key: dk.to_vec(), value: dv.to_vec() });
        if let Some(acc) = new_encoder_cache.as_mut() {
            let (_, ek) = outputs[format!("present.{l}.encoder.key")].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            let (_, ev) = outputs[format!("present.{l}.encoder.value")].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            acc.push(KvCache { key: ek.to_vec(), value: ev.to_vec() });
        }
    }

    Ok((last_logits, vocab, new_decoder_cache, new_encoder_cache))
}

fn empty_decoder_cache(kv: DecoderShape) -> Vec<KvCache> {
    (0..kv.layers).map(|_| KvCache { key: Vec::new(), value: Vec::new() }).collect()
}

/// Melhor esforço de detecção de idioma: um passo do decoder com só o token inicial, argmax restrito
/// aos 99 tokens `<|lang|>` — mesma ideia do `detect_language` do Whisper original. Descarta o cache
/// desse passo; `greedy_generate` recomeça do zero com o prefixo forçado já sabendo o idioma.
fn detect_language(decoder: &mut Session, kv: DecoderShape, encoder_hidden_states: &[f32], enc_seq_len: i64, hidden: i64) -> Result<i64, String> {
    let (logits, _vocab, _dec_cache, _enc_cache) =
        decoder_step(decoder, kv, &[DECODER_START_TOKEN], encoder_hidden_states, enc_seq_len, hidden, false, &empty_decoder_cache(kv), None)?;

    LANG_TO_ID
        .iter()
        .max_by(|(_, a), (_, b)| logits[*a as usize].total_cmp(&logits[*b as usize]))
        .map(|(_, id)| *id)
        .ok_or_else(|| "tabela de idiomas vazia".to_string())
}

/// Decodificação gulosa (sem beam search — mesma simplificação de `whisper.runtime.ts`, que também
/// não faz beam search). Para para no EOS ou em `MAX_NEW_TOKENS`, o mesmo tipo de teto de
/// `generation_config.json.max_length` (448 tokens totais, incluindo o prefixo de 4).
fn greedy_generate(decoder: &mut Session, kv: DecoderShape, encoder_hidden_states: &[f32], enc_seq_len: i64, hidden: i64, lang_token: i64) -> Result<Vec<i64>, String> {
    let forced_prefix = [DECODER_START_TOKEN, lang_token, TASK_TRANSCRIBE_TOKEN, NO_TIMESTAMPS_TOKEN];
    let mut generated: Vec<i64> = forced_prefix.to_vec();
    let mut past_decoder = empty_decoder_cache(kv);
    let mut past_encoder: Option<Vec<KvCache>> = None;

    for step in 0..MAX_NEW_TOKENS {
        let use_cache = step > 0;
        let input_ids: Vec<i64> = if step == 0 { generated.clone() } else { vec![*generated.last().expect("generated nunca fica vazio")] };

        let (logits, vocab, new_decoder_cache, new_encoder_cache) = decoder_step(
            decoder,
            kv,
            &input_ids,
            encoder_hidden_states,
            enc_seq_len,
            hidden,
            use_cache,
            &past_decoder,
            past_encoder.as_deref(),
        )?;

        let (best_id, _) = logits.iter().enumerate().fold((0usize, f32::MIN), |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc });
        past_decoder = new_decoder_cache;
        if let Some(enc_cache) = new_encoder_cache {
            past_encoder = Some(enc_cache);
        }

        if best_id as i64 == EOS_TOKEN {
            break;
        }
        if best_id >= vocab {
            return Err(format!("token gerado ({best_id}) fora do vocabulário ({vocab})"));
        }
        generated.push(best_id as i64);
    }

    Ok(generated[forced_prefix.len()..].to_vec())
}

// --- Engine cacheada — evita recarregar ~76 MiB de pesos ONNX a cada transcrição, mesmo padrão do
// `pipelines: Map<string, Promise<Transcriber>>` de `whisper.runtime.ts`. -------------------------

struct WhisperEngine {
    data_dir: PathBuf,
    /// Chave de invalidação junto com `data_dir` — trocar `voice.whisperModel` (tiny/base/small)
    /// sem derrubar o cache faria a Mika continuar rodando o tamanho antigo até reiniciar.
    model_id: String,
    /// Dispositivo que o plano pediu para o encoder (`accel`); plano diferente recarrega.
    device: super::session::Device,
    encoder: Session,
    decoder: Session,
    kv: DecoderShape,
    tokenizer: PipelineTokenizer,
    mel_filters: Vec<Vec<f32>>,
}

fn engine_cache() -> &'static Mutex<Option<WhisperEngine>> {
    static CACHE: OnceLock<Mutex<Option<WhisperEngine>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

static LEASE: super::Lease = super::Lease::new();

/// Ver [`super::evict_idle`].
pub(super) fn evict_if_idle(max_idle: std::time::Duration, now_ms: u64) -> bool {
    super::evict_if_idle(engine_cache(), &LEASE, max_idle, now_ms)
}

fn load_engine(data_dir: &Path, spec: &WhisperModelSpec) -> Result<WhisperEngine, String> {
    let encoder_path = require_asset(data_dir, spec, ENCODER_FILE)?;
    let decoder_path = require_asset(data_dir, spec, DECODER_FILE)?;
    let tokenizer_path = require_asset(data_dir, spec, TOKENIZER_FILE)?;

    let threads = Some(crate::modules::system::governor::onnx_threads());
    // Encoder na GPU: 453 → 89 ms (p50, RTX 4060 Ti). Decoder fica na CPU: autoregressivo, uma
    // cópia CPU↔GPU por token e nenhum ganho medido (política em `accel`).
    let (encoder, plan) = super::session::load(Model::WhisperEncoder, &encoder_path, threads).map_err(|e| format!("ort não carregou o encoder Whisper ({encoder_path:?}): {e}"))?;
    let (decoder, _) = super::session::load(Model::WhisperDecoder, &decoder_path, threads).map_err(|e| format!("ort não carregou o decoder Whisper ({decoder_path:?}): {e}"))?;

    // `tokenizer.json` publicado pelo Hugging Face vem em "version": "1.0" (legado); o leitor deste
    // projeto (`tokenizers` 1.0.0-rc.2) só aceita "2.0" e recusa converter sozinho — precisa passar
    // por `canonicalize_file` primeiro. Descoberto rodando contra o arquivo real, não documentação.
    let canonical_json = tokenizers::canonicalize_file(&tokenizer_path).map_err(|e| format!("tokenizer.json não converteu para o formato canônico: {e}"))?;
    let tokenizer = tokenizers::from_json(&canonical_json).map_err(|e| format!("tokenizer.json canônico não carregou: {e}"))?;

    let mel_filters = build_mel_filterbank(N_FFT / 2 + 1, N_MELS, SAMPLE_RATE_F64);
    let kv = decoder_shape(&decoder)?;

    Ok(WhisperEngine { data_dir: data_dir.to_path_buf(), model_id: spec.id.to_string(), device: plan.device, encoder, decoder, kv, tokenizer, mel_filters })
}

/// O engine do cache, (re)carregado se ainda não existe ou é de outro modelo/diretório.
fn loaded_engine(data_dir: &Path, spec: &WhisperModelSpec) -> Result<std::sync::MutexGuard<'static, Option<WhisperEngine>>, String> {
    let mut guard = engine_cache().lock().map_err(|_| "cache do engine Whisper envenenado".to_string())?;
    LEASE.touch();
    let device = super::accel::plan(Model::WhisperEncoder).device;
    let stale = guard.as_ref().is_none_or(|e| e.data_dir != data_dir || e.model_id != spec.id || e.device != device);
    if stale {
        *guard = Some(load_engine(data_dir, spec)?);
    }
    Ok(guard)
}

/// Boot: carrega o modelo antes da primeira fala, que senão pagava a carga das sessões ONNX (e o
/// encoder na GPU) antes de decodificar uma palavra — mesmo papel de `nemotron::prewarm`. Modelo
/// ausente não é erro aqui: quem reporta é o card de download; `model_id` passa pelo allowlist.
pub fn prewarm(data_dir: &Path, model_id: &str) {
    let spec = whisper_model_spec(model_id);
    if [ENCODER_FILE, DECODER_FILE, TOKENIZER_FILE].iter().any(|file| require_asset(data_dir, spec, file).is_err()) {
        return;
    }
    let started = Instant::now();
    match loaded_engine(data_dir, spec) {
        Ok(_) => log::info!("Whisper STT carregado em {} ms", started.elapsed().as_millis()),
        Err(e) => log::warn!("Whisper STT não pré-carregou: {e}"),
    }
}

#[derive(Debug, serde::Serialize)]
pub struct WhisperResult {
    pub text: String,
    pub duration_ms: u64,
}

/// Quanto antes do fim da janela de 30 s o corte pode recuar para cair numa pausa.
const CUT_SEARCH_SAMPLES: usize = 3 * REQUIRED_SAMPLE_RATE as usize;
/// Passo da busca de pausa (20 ms): fino o bastante para achar um respiro entre palavras.
const CUT_FRAME_SAMPLES: usize = REQUIRED_SAMPLE_RATE as usize / 50;

/// Fatia o áudio em janelas de até 30 s (o encoder tem comprimento fixo — mais que isso era
/// descartado em silêncio). Cada corte recua até 3 s para o quadro de 20 ms de menor energia, para
/// não partir uma palavra ao meio; sem sobreposição, então nenhuma frase sai duplicada.
fn plan_windows(samples: &[f32]) -> Vec<std::ops::Range<usize>> {
    let mut windows = Vec::new();
    let mut start = 0;
    while samples.len() - start > N_SAMPLES_30S {
        let hard_end = start + N_SAMPLES_30S;
        let end = quietest_cut(samples, hard_end - CUT_SEARCH_SAMPLES, hard_end);
        windows.push(start..end);
        start = end;
    }
    windows.push(start..samples.len());
    windows
}

/// Início do quadro de menor energia em `[from, to)` — o primeiro em caso de empate.
fn quietest_cut(samples: &[f32], from: usize, to: usize) -> usize {
    let energy = |at: usize| samples[at..(at + CUT_FRAME_SAMPLES).min(to)].iter().map(|s| s * s).sum::<f32>();
    (from..to.saturating_sub(CUT_FRAME_SAMPLES))
        .step_by(CUT_FRAME_SAMPLES)
        .map(|at| (at, energy(at)))
        .fold((to, f32::MAX), |best, cur| if cur.1 < best.1 { cur } else { best })
        .0
}

/// Uma janela de até 30 s → texto + idioma usado. `lang_token` `None` detecta o idioma nesta janela.
fn transcribe_window(engine: &mut WhisperEngine, samples: &[f32], lang_token: Option<i64>) -> Result<(String, i64), String> {
    let mel = log_mel_spectrogram(samples, &engine.mel_filters);
    let input_features = Tensor::from_array(([1i64, N_MELS as i64, N_FRAMES_30S as i64], mel)).map_err(|e| e.to_string())?;
    let enc_outputs = engine.encoder.run(ort::inputs!["input_features" => input_features]).map_err(|e| format!("encoder Whisper falhou: {e}"))?;
    let (hs_shape, hs_data) = enc_outputs["last_hidden_state"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
    let enc_seq_len = hs_shape[1];
    let hidden = hs_shape[2];
    let hidden_states = hs_data.to_vec();

    let lang_token = match lang_token {
        Some(token) => token,
        None => detect_language(&mut engine.decoder, engine.kv, &hidden_states, enc_seq_len, hidden)?,
    };

    let token_ids = greedy_generate(&mut engine.decoder, engine.kv, &hidden_states, enc_seq_len, hidden, lang_token)?;
    let ids_u32: Vec<u32> = token_ids.iter().map(|&id| id as u32).collect();
    let text = engine.tokenizer.decode(&ids_u32, true).map_err(|e| format!("tokenizer não decodificou os ids gerados: {e}"))?.trim().to_string();
    Ok((text, lang_token))
}

/// Núcleo síncrono — roda numa `spawn_blocking` (ver `transcribe`), não na task async que também
/// resolve `mika_invoke`. Decisão já registrada no cabeçalho de `modules/inference/mod.rs` e
/// `modules/voice.rs`: `ort::Session::run` é síncrono, e travar só o worker thread é aceitável;
/// travar o processo inteiro não seria.
///
/// SEGURANÇA — `model_id` vem de `voice.whisperModel` (`resolve_whisper_model`), lido de volta de
/// `config.json` sem revalidação no boot (`ConfigStore::open`/`merge_over_defaults` só checa o
/// enum em `set()`, não ao carregar do disco). Por isso o PRIMEIRO passo aqui é resolver `model_id`
/// contra o allowlist fixo (`whisper_model_spec`) e só então construir qualquer path — nunca passar
/// o `&str` cru adiante para `load_engine`/`require_asset`/`whisper_cache_path`, que montariam o
/// caminho a partir dele (travessia de diretório com uma string arbitrária no `config.json`).
fn transcribe_blocking(data_dir: &Path, pcm: &[u8], sample_rate: u32, language: Option<&str>, model_id: &str) -> Result<Option<WhisperResult>, String> {
    let spec = whisper_model_spec(model_id);
    let samples = pcm16_to_f32(pcm, sample_rate)?;
    if samples.len() < MIN_SAMPLES {
        return Ok(None);
    }

    let started = Instant::now();
    let mut guard = loaded_engine(data_dir, spec)?;
    let engine = guard.as_mut().expect("loaded_engine só devolve o cache preenchido");

    // O idioma é detectado na primeira janela e reaproveitado nas demais: detectar por janela
    // trocaria de idioma no meio de uma fala só.
    let mut lang_token = language.map(language_token_id).transpose()?;
    let mut parts: Vec<String> = Vec::new();
    for window in plan_windows(&samples) {
        if window.len() < MIN_SAMPLES {
            continue;
        }
        let (text, used) = transcribe_window(engine, &samples[window], lang_token)?;
        lang_token = Some(used);
        if !text.is_empty() {
            parts.push(text);
        }
    }
    let text = parts.join(" ");

    Ok(Some(WhisperResult { text, duration_ms: started.elapsed().as_millis() as u64 }))
}

/// API pública deste módulo — `modules/voice/transcribe.rs` é quem liga `mika:voice-transcribe` a
/// esta função, resolvendo `model_id` a partir de `voice.whisperModel` (`resolve_whisper_model`).
pub async fn transcribe(data_dir: PathBuf, pcm: Vec<u8>, sample_rate: u32, language: Option<String>, model_id: String) -> Result<Option<WhisperResult>, String> {
    tokio::task::spawn_blocking(move || transcribe_blocking(&data_dir, &pcm, sample_rate, language.as_deref(), &model_id))
        .await
        .map_err(|e| format!("worker de transcrição não completou: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 75 s de fala contínua com uma pausa curta perto dos 28 s: nenhuma amostra fica de fora (o
    /// engine antes cortava tudo além de 30 s em silêncio) e o corte cai NA pausa, não no meio da fala.
    #[test]
    fn plan_windows_covers_75s_without_truncating_and_cuts_at_the_pause() {
        let rate = REQUIRED_SAMPLE_RATE as usize;
        let mut samples = vec![0.5f32; 75 * rate];
        let pause = 28 * rate..28 * rate + rate / 2;
        samples[pause.clone()].fill(0.0);

        let windows = plan_windows(&samples);

        assert!(windows.len() >= 3, "75 s precisam de ao menos 3 janelas de 30 s: {windows:?}");
        assert_eq!(windows.first().unwrap().start, 0);
        assert_eq!(windows.last().unwrap().end, samples.len());
        for pair in windows.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "janelas contíguas, sem buraco nem sobreposição");
        }
        assert!(windows.iter().all(|w| w.len() <= N_SAMPLES_30S), "janela maior que o encoder aceita: {windows:?}");
        assert!(pause.contains(&windows[0].end), "o corte tem de cair na pausa: {:?}", windows[0]);
    }

    #[test]
    fn plan_windows_keeps_short_audio_in_one_window() {
        let samples = vec![0.1f32; 10 * REQUIRED_SAMPLE_RATE as usize];
        assert_eq!(plan_windows(&samples), vec![0..samples.len()]);
    }

    // --- contrato de path -------------------------------------------------------------------

    #[test]
    fn whisper_cache_path_matches_desktop_rs_layout() {
        let dir = Path::new("/data");
        let spec = whisper_model_spec(DEFAULT_WHISPER_MODEL_ID);
        assert_eq!(
            whisper_cache_path(dir, spec, ENCODER_FILE),
            Path::new("/data/whisper-cache/whisper-base/encoder_model_quantized.onnx")
        );
        assert_eq!(
            whisper_cache_path(dir, spec, TOKENIZER_FILE),
            Path::new("/data/whisper-cache/whisper-base/tokenizer.json")
        );
    }

    /// Cada tamanho baixa para o próprio subdiretório — trocar de `whisper-base` para
    /// `whisper-small` não pode ler os pesos errados nem forçar novo download do que já está lá.
    #[test]
    fn whisper_cache_path_is_isolated_per_model_size() {
        let dir = Path::new("/data");
        let base = whisper_cache_path(dir, whisper_model_spec("onnx-community/whisper-base"), ENCODER_FILE);
        let small = whisper_cache_path(dir, whisper_model_spec("onnx-community/whisper-small"), ENCODER_FILE);
        assert_ne!(base, small);
    }

    #[test]
    fn prewarm_without_the_model_loads_nothing() {
        prewarm(&std::env::temp_dir().join("mika-whisper-never-downloaded"), DEFAULT_WHISPER_MODEL_ID);
        assert!(engine_cache().lock().unwrap().is_none(), "sem o modelo baixado o boot não carrega nada");
    }

    #[test]
    fn require_asset_fails_loud_when_missing() {
        let err = require_asset(Path::new("/nao/existe"), whisper_model_spec(DEFAULT_WHISPER_MODEL_ID), ENCODER_FILE).unwrap_err();
        assert!(err.contains("ausente"));
    }

    /// NEGATIVO — reforça a mesma garantia da regressão abaixo diretamente no par
    /// `whisper_cache_path`/`require_asset`: um `model_id` desconhecido nunca chega aqui como
    /// `&str` cru porque a assinatura só aceita `&WhisperModelSpec`, já resolvido pelo allowlist.
    #[test]
    fn whisper_cache_path_only_ever_builds_from_an_allowlisted_spec() {
        let spec = whisper_model_spec("..\\..\\..\\Windows\\System32\\evil");
        let path = whisper_cache_path(Path::new("/data"), spec, ENCODER_FILE);
        assert!(!path.to_string_lossy().contains(".."));
        assert_eq!(spec.id, DEFAULT_WHISPER_MODEL_ID, "id desconhecido cai no default, nunca passa cru");
    }

    /// SEGURANÇA — REGRESSÃO: `transcribe_blocking` passava `model_id` bruto direto para
    /// `load_engine`/`require_asset`/`whisper_cache_path`, que montam o path SEM passar pelo
    /// allowlist de `whisper_model_spec` primeiro. `voice.whisperModel` vem de
    /// `ConfigStore::get` (`resolve_whisper_model`), que só valida contra o enum em `set()` — um
    /// `config.json` editado fora da API (`ConfigStore::open`/`merge_over_defaults` não revalida no
    /// boot) podia carregar travessia de diretório e escapar de `whisper-cache/<slug>/`, fazendo
    /// `load_engine` executar/ler um arquivo colocado pelo atacante. `require_asset` falha antes de
    /// tocar em qualquer ONNX real (só checa `is_file`), então o path malicioso aparece na própria
    /// mensagem de erro sem precisar de pesos baixados.
    #[test]
    fn transcribe_blocking_never_builds_a_path_outside_the_allowlist_for_a_path_like_model_id() {
        let malicious = "..\\..\\..\\Windows\\System32\\evil";
        let pcm = vec![0u8; MIN_SAMPLES * 2]; // silêncio, mas longo o bastante para passar do corte de MIN_SAMPLES
        let err = transcribe_blocking(Path::new("/data"), &pcm, REQUIRED_SAMPLE_RATE, None, malicious).unwrap_err();

        assert!(!err.contains(".."), "path não pode carregar segmentos de travessia: {err}");
        assert!(
            WHISPER_MODELS.iter().any(|m| err.contains(&whisper_model_slug(m.id).to_string())),
            "path deveria estar sob um dos slugs permitidos ({:?}): {err}",
            WHISPER_MODELS.iter().map(|m| whisper_model_slug(m.id)).collect::<Vec<_>>()
        );
    }

    // --- tabela de modelos: tamanho -> (id, revisão, sha256) ---------------------------------

    fn kv_inputs(layers: usize, heads: i64) -> Vec<(String, Vec<i64>)> {
        let mut inputs = vec![("input_ids".to_string(), vec![-1, -1])];
        for l in 0..layers {
            for part in ["decoder.key", "decoder.value", "encoder.key", "encoder.value"] {
                inputs.push((format!("past_key_values.{l}.{part}"), vec![-1, heads, -1, 64]));
            }
        }
        inputs
    }

    /// As três formas reais (`config.json` de cada checkpoint): tiny 4×6, base 6×8, small 12×12.
    #[test]
    fn decoder_shape_comes_from_the_model_inputs_for_every_size() {
        for (layers, heads) in [(4, 6), (6, 8), (12, 12)] {
            assert_eq!(decoder_shape_from(&kv_inputs(layers, heads)), Ok(DecoderShape { layers, heads, head_dim: 64 }));
        }
    }

    #[test]
    fn a_decoder_without_a_readable_kv_shape_fails_loud() {
        assert!(decoder_shape_from(&[("input_ids".to_string(), vec![-1, -1])]).is_err());
        assert!(decoder_shape_from(&kv_inputs(4, -1)).is_err(), "heads dinâmico não pode virar tensor de forma negativa");
    }

    #[test]
    fn whisper_model_spec_resolves_all_three_sizes() {
        assert_eq!(whisper_model_spec("onnx-community/whisper-tiny").id, "onnx-community/whisper-tiny");
        assert_eq!(whisper_model_spec("onnx-community/whisper-base").id, "onnx-community/whisper-base");
        assert_eq!(whisper_model_spec("onnx-community/whisper-small").id, "onnx-community/whisper-small");
    }

    /// NEGATIVO — config corrompida ou um tamanho removido do enum não pode derrubar a
    /// transcrição inteira; cai no default em vez de entrar em pânico.
    #[test]
    fn whisper_model_spec_falls_back_to_default_for_unknown_id() {
        let spec = whisper_model_spec("onnx-community/whisper-large-v3");
        assert_eq!(spec.id, DEFAULT_WHISPER_MODEL_ID);
    }

    #[test]
    fn whisper_model_slug_is_the_last_path_segment() {
        assert_eq!(whisper_model_slug("onnx-community/whisper-tiny"), "whisper-tiny");
        assert_eq!(whisper_model_slug(DEFAULT_WHISPER_MODEL_ID), "whisper-base");
    }

    /// NEGATIVO — os três specs não podem apontar para os mesmos bytes: um erro de copy-paste na
    /// tabela (mesmo sha256 em dois tamanhos diferentes) baixaria o modelo errado em silêncio.
    #[test]
    fn whisper_models_have_distinct_weights_per_size() {
        let encoder_hashes: std::collections::HashSet<_> = WHISPER_MODELS.iter().map(|m| m.encoder_sha256).collect();
        let decoder_hashes: std::collections::HashSet<_> = WHISPER_MODELS.iter().map(|m| m.decoder_sha256).collect();
        assert_eq!(encoder_hashes.len(), WHISPER_MODELS.len(), "encoder sha256 duplicado entre tamanhos");
        assert_eq!(decoder_hashes.len(), WHISPER_MODELS.len(), "decoder sha256 duplicado entre tamanhos");
    }

    // --- contrato PCM16 mono 16 kHz ---------------------------------------------------------

    #[test]
    fn pcm16_rejects_wrong_sample_rate() {
        assert!(pcm16_to_f32(&[0, 0], 48_000).is_err());
    }

    #[test]
    fn pcm16_rejects_odd_byte_length() {
        assert!(pcm16_to_f32(&[0, 0, 1], REQUIRED_SAMPLE_RATE).is_err());
    }

    #[test]
    fn pcm16_converts_known_values() {
        // i16::MAX (32767) e i16::MIN (-32768) em little-endian.
        let pcm = [0xFF, 0x7F, 0x00, 0x80];
        let samples = pcm16_to_f32(&pcm, REQUIRED_SAMPLE_RATE).unwrap();
        assert!((samples[0] - 0.999_969_5).abs() < 1e-6);
        assert!((samples[1] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn pcm16_accepts_empty_input() {
        assert_eq!(pcm16_to_f32(&[], REQUIRED_SAMPLE_RATE).unwrap(), Vec::<f32>::new());
    }

    // --- janela e filtro mel: propriedades verificáveis sem áudio real -----------------------

    #[test]
    fn hann_window_starts_at_zero_and_is_symmetric() {
        let w = hann_window_periodic(N_FFT);
        assert!((w[0] - 0.0).abs() < 1e-6);
        for i in 1..N_FFT {
            assert!((w[i] - w[N_FFT - i]).abs() < 1e-5, "w[{i}] deveria espelhar w[{}]", N_FFT - i);
        }
    }

    #[test]
    fn mel_filterbank_has_expected_shape_and_is_non_negative() {
        let filters = build_mel_filterbank(N_FFT / 2 + 1, N_MELS, SAMPLE_RATE_F64);
        assert_eq!(filters.len(), N_MELS);
        for row in &filters {
            assert_eq!(row.len(), N_FFT / 2 + 1);
            assert!(row.iter().all(|&v| v >= 0.0));
        }
    }

    #[test]
    fn mel_filterbank_peaks_move_to_higher_bins_as_mel_index_grows() {
        // Cada filtro triangular cobre uma faixa de frequência mais alta que a anterior — o bin de
        // pico (`argmax`) tem de ser não-decrescente ao longo dos 80 filtros.
        let filters = build_mel_filterbank(N_FFT / 2 + 1, N_MELS, SAMPLE_RATE_F64);
        let mut last_peak = 0usize;
        for row in &filters {
            let peak = row.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i).unwrap();
            assert!(peak >= last_peak, "pico do filtro deveria avançar, ficou em {peak} depois de {last_peak}");
            last_peak = peak;
        }
    }

    // --- DFT: vetor conhecido de verdade, não achismo -----------------------------------------

    #[test]
    fn dft_of_dc_signal_has_all_energy_in_bin_zero() {
        // Sinal constante: soma de uma exponencial complexa sobre um período completo é
        // analiticamente zero para todo k != 0 — só o bin DC (k=0) deveria ter energia.
        let frame = vec![1.0f32; N_FFT];
        let power = dft_power_spectrum(&frame);
        let expected_dc = (N_FFT as f32).powi(2);
        assert!((power[0] - expected_dc).abs() < 1.0, "bin DC esperado ~{expected_dc}, veio {}", power[0]);
        for (k, &p) in power.iter().enumerate().skip(1) {
            assert!(p < 1e-3, "bin {k} deveria ser ~0 para um sinal DC, veio {p}");
        }
    }

    #[test]
    fn dft_of_pure_tone_concentrates_energy_at_its_bin() {
        let target_bin = 10usize;
        let frame: Vec<f32> = (0..N_FFT).map(|n| (2.0 * PI * target_bin as f64 * n as f64 / N_FFT as f64).cos() as f32).collect();
        let power = dft_power_spectrum(&frame);
        let peak_bin = power.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i).unwrap();
        assert_eq!(peak_bin, target_bin);
    }

    // --- log-mel: caso de silêncio tem valor exato e conhecido (a fórmula do módulo cai numa
    // constante fechada para entrada zero) --------------------------------------------------

    #[test]
    fn log_mel_of_silence_is_the_exact_clamp_floor() {
        // mel = 0 em todo lugar → log10(max(0, 1e-10)) = -10 em todo lugar → max já é -10 (nenhum
        // clamp de dinâmica dispara) → (-10 + 4) / 4 = -1.5 exatamente. Poucos frames para o teste
        // ficar barato (a produção sempre usa 3000; aqui bastam 5 para provar a fórmula).
        let filters = build_mel_filterbank(N_FFT / 2 + 1, N_MELS, SAMPLE_RATE_F64);
        let silence = vec![0.0f32; 2000];
        let mel = log_mel_spectrogram_with(&silence, &filters, 2000, 5);
        assert_eq!(mel.len(), N_MELS * 5);
        for v in mel {
            assert!((v - (-1.5)).abs() < 1e-4, "esperado -1.5 exato para silêncio, veio {v}");
        }
    }

    // --- tokens especiais / idioma -------------------------------------------------------------

    #[test]
    fn language_token_id_resolves_known_codes() {
        assert_eq!(language_token_id("pt").unwrap(), 50267);
        assert_eq!(language_token_id("en").unwrap(), 50259);
    }

    /// `voice.asrLanguage` é texto livre e o Nemotron aceita `pt-BR`/`pt_BR`; o Whisper recusava
    /// qualquer coisa fora do código de duas letras e cada fala falhava com "idioma desconhecido".
    #[test]
    fn language_token_id_accepts_a_regional_or_mixed_case_tag() {
        for tag in ["pt-BR", "pt_BR", "PT", " pt "] {
            assert_eq!(language_token_id(tag).unwrap(), 50267, "{tag:?}");
        }
    }

    #[test]
    fn language_token_id_rejects_unknown_code() {
        assert!(language_token_id("xx-not-a-language").is_err());
    }

    // --- verificação real de ponta a ponta — não roda no gate padrão (precisa dos pesos ONNX +
    // tokenizer.json baixados manualmente; ver o cabeçalho do módulo para o motivo de não haver
    // download automático aqui). Reproduzir:
    //   1. baixar para um diretório D: onnx/encoder_model_quantized.onnx,
    //      onnx/decoder_model_merged_quantized.onnx e tokenizer.json de
    //      https://huggingface.co/onnx-community/whisper-base/tree/1846881b6b3a3024392c1eea3ad983695bc23925
    //      renomeando para <D>/whisper-cache/whisper-base/{encoder_model_quantized.onnx,
    //      decoder_model_merged_quantized.onnx, tokenizer.json}
    //   2. MIKA_WHISPER_TEST_DATA_DIR=<D> cargo test --lib inference::whisper -- --ignored
    #[test]
    #[ignore = "precisa dos pesos ONNX + tokenizer.json baixados manualmente (rede) — ver comentário acima"]
    fn transcribe_matches_known_reference_wav() {
        let data_dir = std::env::var("MIKA_WHISPER_TEST_DATA_DIR").expect("defina MIKA_WHISPER_TEST_DATA_DIR");
        let wav_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/voice/fala-usuario-pt-br.wav");

        let mut reader = hound::WavReader::open(&wav_path).expect("fixture de voz não encontrada");
        let pcm: Vec<u8> = reader.samples::<i16>().flat_map(|s| s.unwrap().to_le_bytes()).collect();

        // `MIKA_WHISPER_TEST_MODEL` troca o tamanho (tiny/small têm outra forma de decoder que o base).
        let model_id = std::env::var("MIKA_WHISPER_TEST_MODEL").unwrap_or_else(|_| DEFAULT_WHISPER_MODEL_ID.to_string());
        let result = transcribe_blocking(Path::new(&data_dir), &pcm, REQUIRED_SAMPLE_RATE, Some("pt"), &model_id)
            .expect("transcrição não deveria falhar com os assets presentes")
            .expect("áudio real não deveria ser tratado como curto demais");

        // Referência documentada em e2e/fixtures/voice/README.md (faster-whisper, modelo base):
        // "Ah, ela voltou!" no início — a parte central é sabidamente corrompida pelo modelo base.
        // Tiny erra essa frase por qualidade, não por defeito: para ele e o small, provar que o
        // decoder roda com a própria forma de cache e devolve texto já é o contrato.
        if model_id == DEFAULT_WHISPER_MODEL_ID {
            assert!(result.text.to_lowercase().contains("ela voltou"), "transcrição inesperada: {:?}", result.text);
        } else {
            assert!(!result.text.trim().is_empty(), "{model_id} não transcreveu nada");
        }
        eprintln!("{model_id}: {:?}", result.text);
    }
}
