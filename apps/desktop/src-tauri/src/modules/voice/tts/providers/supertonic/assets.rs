//! Download/cache dos assets do Supertonic — porte de
//! `apps/desktop/src/main/services/inference/voice-download.ts` (a voz, `.bin`) e da parte de
//! `supertonic.runtime.ts` que hoje delega para `@huggingface/transformers` (os 3 grafos ONNX +
//! `tokenizer.json`, baixados sob demanda pela própria lib no lado TS). Aqui não há uma lib de
//! pipeline para delegar — este arquivo é o download em si.
//!
//! **Por que `reqwest` direto aqui, e não `modules/desktop.rs::download_bytes`.** O download de
//! TTS local NUNCA passou pelo wizard no lado TS — `voice-download.ts` é seu próprio fetcher
//! (`createLoggedFetch('voice-download', ...)`), preguiçoso, disparado na primeira síntese, não no
//! fluxo `wizard-prepare-asset` que `desktop.rs`/`whisper.rs` usam. Reproduzir ESSA arquitetura é a
//! paridade certa aqui — mas o chokepoint de PRIVACIDADE de rede não é opcional só porque o
//! caminho é outro: [`ensure_model_files`]/[`ensure_voice`] chamam
//! `crate::modules::privacy::assert_network_allowed` antes de cada GET, exatamente como
//! `desktop.rs::download_bytes` faz e `whisper.rs` (que TAMBÉM não baixa pelo wizard, e documenta a
//! mesma escolha no próprio cabeçalho) teria de fazer se baixasse algo.
//!
//! Sem verificação de SHA-256: nem o lado TS verifica (a lib HF confia no HTTPS + cache por ETag,
//! não expõe um hash para o app checar) — paridade, não uma garantia nova inventada aqui.

use std::path::{Path, PathBuf};

use crate::config::store::ConfigStore;
use crate::modules::privacy::assert_network_allowed;

const MODEL_ID: &str = "onnx-community/Supertonic-TTS-2-ONNX";
const MODEL_REVISION: &str = "68d4d9420d0e0e51d14656e1ec5c9b091490b49e";
const VOICE_FLOATS: usize = 12_928;

fn hf_base() -> String {
    format!("https://huggingface.co/{MODEL_ID}/resolve/{MODEL_REVISION}")
}

/// Os 3 grafos + o tokenizer. Os `.onnx_data` (pesos externos) não aparecem aqui como campo
/// próprio — `ort` os resolve sozinho pelo nome ao lado do `.onnx` (mesma pasta), então só
/// precisam estar baixados, não referenciados por este struct.
pub struct SupertonicPaths {
    pub text_encoder: PathBuf,
    pub latent_denoiser: PathBuf,
    pub voice_decoder: PathBuf,
    pub tokenizer: PathBuf,
}

const MODEL_FILES: [&str; 7] = [
    "onnx/text_encoder.onnx",
    "onnx/text_encoder.onnx_data",
    "onnx/latent_denoiser.onnx",
    "onnx/latent_denoiser.onnx_data",
    "onnx/voice_decoder.onnx",
    "onnx/voice_decoder.onnx_data",
    "tokenizer.json",
];

fn is_valid_voice(voice: &str) -> bool {
    let mut chars = voice.trim().chars();
    let letter = chars.next();
    let digit = chars.next();
    matches!(letter, Some('F') | Some('M')) && matches!(digit, Some('1'..='5')) && chars.next().is_none()
}

/// `.part` ÚNICO por download: `with_extension("part")` fazia `text_encoder.onnx` e
/// `text_encoder.onnx_data` (e duas falas simultâneas no primeiro uso) escreverem no MESMO arquivo, e o
/// `rename` gravava um modelo intercalado — que então contava como baixado (`is_file`) para sempre.
fn part_path(target: &Path) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}-{}.part", std::process::id(), COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    target.with_file_name(name)
}

async fn download_to_file(store: &ConfigStore, url: &str, target: &Path) -> Result<(), String> {
    assert_network_allowed(store, url, "supertonic-tts")?;
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }

    let part = part_path(target);
    let result = stream_download(url, &part).await;
    match result {
        Ok(()) => tokio::fs::rename(&part, target).await.map_err(|e| format!("could not rename {}: {e}", part.display())),
        Err(err) => {
            let _ = tokio::fs::remove_file(&part).await;
            Err(err)
        }
    }
}

async fn stream_download(url: &str, part_path: &Path) -> Result<(), String> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    let response = crate::modules::privacy::fixed_endpoint_client().get(url).send().await.map_err(|e| format!("download failed ({url}): {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download failed ({url}): HTTP {}", response.status().as_u16()));
    }

    let mut file =
        tokio::fs::File::create(part_path).await.map_err(|e| format!("could not create {}: {e}", part_path.display()))?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download interrupted ({url}): {e}"))?;
        file.write_all(&chunk).await.map_err(|e| format!("could not write {}: {e}", part_path.display()))?;
    }
    Ok(())
}

/// Baixa (se ausente) os 7 arquivos do modelo. Idempotente: um arquivo já presente não é
/// rebaixado — mesmo contrato de `downloadVoiceBinIfMissing`.
pub async fn ensure_model_files(cache_dir: &Path, store: &ConfigStore) -> Result<SupertonicPaths, String> {
    let base = hf_base();
    for file in MODEL_FILES {
        let target = cache_dir.join("onnx-model").join(file);
        if target.is_file() {
            continue;
        }
        download_to_file(store, &format!("{base}/{file}"), &target).await?;
    }

    Ok(model_paths(cache_dir))
}

fn model_paths(cache_dir: &Path) -> SupertonicPaths {
    let root = cache_dir.join("onnx-model");
    SupertonicPaths {
        text_encoder: root.join("onnx/text_encoder.onnx"),
        latent_denoiser: root.join("onnx/latent_denoiser.onnx"),
        voice_decoder: root.join("onnx/voice_decoder.onnx"),
        tokenizer: root.join("tokenizer.json"),
    }
}

/// O modelo SÓ se os 7 arquivos já estão no disco — nunca baixa (é o que o pré-carregamento do boot
/// usa; o download continua sendo da primeira síntese, com o chokepoint de rede).
pub fn installed_model_files(cache_dir: &Path) -> Option<SupertonicPaths> {
    MODEL_FILES.iter().all(|file| cache_dir.join("onnx-model").join(file).is_file()).then(|| model_paths(cache_dir))
}

/// Porte de `ensureSupertonicVoice`. `None` (não `Err`) para voz inválida/indisponível — mesmo
/// contrato do TS, que loga e devolve nulo para o chamador decidir (aqui, `model.rs` transforma em
/// `Err` nomeado, porque sem voz não há síntese possível).
pub async fn ensure_voice(cache_dir: &Path, voice: &str, store: &ConfigStore) -> Result<Vec<f32>, String> {
    if !is_valid_voice(voice) {
        return Err(format!("invalid Supertonic voice: '{voice}' (expected [F|M][1-5])"));
    }

    let target = cache_dir.join("onnx-model").join("voices").join(format!("{voice}.bin"));
    if !target.is_file() {
        let url = format!("{}/voices/{voice}.bin", hf_base());
        download_to_file(store, &url, &target).await?;
    }

    let bytes = tokio::fs::read(&target).await.map_err(|e| format!("could not read {}: {e}", target.display()))?;
    if bytes.len() != VOICE_FLOATS * 4 {
        let _ = tokio::fs::remove_file(&target).await;
        return Err(format!(
            "truncated voice embedding for '{voice}': {} bytes, expected {}",
            bytes.len(),
            VOICE_FLOATS * 4
        ));
    }

    let mut floats = Vec::with_capacity(VOICE_FLOATS);
    for chunk in bytes.chunks_exact(4) {
        floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(floats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O pré-carregamento do boot nunca baixa nada: só enxerga o modelo com os 7 arquivos já no disco.
    #[test]
    fn installed_model_files_needs_every_file_already_on_disk() {
        let dir = std::env::temp_dir().join(format!("mika-supertonic-installed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for file in &MODEL_FILES[..MODEL_FILES.len() - 1] {
            let path = dir.join("onnx-model").join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"x").unwrap();
        }
        assert!(installed_model_files(&dir).is_none(), "falta o tokenizer: nada para carregar");

        std::fs::write(dir.join("onnx-model").join(MODEL_FILES[MODEL_FILES.len() - 1]), b"x").unwrap();
        let paths = installed_model_files(&dir).expect("os 7 arquivos presentes");
        assert_eq!(paths.tokenizer, dir.join("onnx-model").join("tokenizer.json"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_download_gets_its_own_part_file() {
        let model = Path::new("/c/onnx-model/onnx/text_encoder.onnx");
        let weights = Path::new("/c/onnx-model/onnx/text_encoder.onnx_data");
        let (a, b, c) = (part_path(model), part_path(model), part_path(weights));
        assert!(a != b && a != c && b != c, "{a:?} {b:?} {c:?}");
        assert_eq!(a.parent(), model.parent(), "o rename tem de ficar no mesmo diretório");
    }

    #[test]
    fn is_valid_voice_accepts_the_ten_documented_voices() {
        for letter in ['F', 'M'] {
            for digit in '1'..='5' {
                assert!(is_valid_voice(&format!("{letter}{digit}")));
            }
        }
    }

    #[test]
    fn is_valid_voice_rejects_garbage() {
        assert!(!is_valid_voice("F6"));
        assert!(!is_valid_voice("X1"));
        assert!(!is_valid_voice("F1x"));
        assert!(!is_valid_voice(""));
    }

    #[test]
    fn model_files_cover_the_three_graphs_and_the_tokenizer() {
        assert!(MODEL_FILES.contains(&"onnx/text_encoder.onnx"));
        assert!(MODEL_FILES.contains(&"onnx/latent_denoiser.onnx"));
        assert!(MODEL_FILES.contains(&"onnx/voice_decoder.onnx"));
        assert!(MODEL_FILES.contains(&"tokenizer.json"));
    }
}
