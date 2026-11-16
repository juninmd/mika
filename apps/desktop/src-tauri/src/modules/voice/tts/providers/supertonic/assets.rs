//! Download/cache dos assets do Supertonic 3 — o export ONNX OFICIAL (`duration_predictor`,
//! `text_encoder`, `vector_estimator`, `vocoder` + `tts.json`, `unicode_indexer.json` e as vozes em
//! JSON), baixado sob demanda na primeira síntese.
//!
//! **Por que o espelho `supertone-oss-archive`.** A Supertone encerrou as atividades em 2026 e moveu
//! código e pesos para essa organização de arquivo (README de `supertone-inc/supertonic`, conferido
//! em 2026-11-15). Arquivo congelado é o que um download fixado quer: a revisão abaixo não muda. A
//! licença do modelo continua BigScience OpenRAIL-M (ver `LICENSE-ASSETS.md`).
//!
//! **Integridade.** Todo arquivo tem SHA-256 medido na revisão fixada e conferido no streaming: bytes
//! que não são os medidos nunca viram cache (o Supertonic 2 confiava só no HTTPS).
//!
//! **Por que `reqwest` direto aqui, e não `modules/desktop.rs::download_bytes`.** O download de TTS
//! local é preguiçoso, disparado na primeira síntese, não no fluxo do wizard; o chokepoint de
//! PRIVACIDADE vale igual: [`download_to_file`] chama `assert_network_allowed` antes de cada GET.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::store::ConfigStore;
use crate::modules::privacy::assert_network_allowed;

const MODEL_ID: &str = "supertone-oss-archive/supertonic-3";
const MODEL_REVISION: &str = "aafc6e32416a594460b32413efc49d7fe4ce6d46";
/// Subpasta do cache. O Supertonic 2 morava em `onnx-model/` — ver [`remove_legacy_cache`].
const MODEL_DIR: &str = "supertonic-3";
const LEGACY_DIR: &str = "onnx-model";

fn hf_base() -> String {
    format!("https://huggingface.co/{MODEL_ID}/resolve/{MODEL_REVISION}")
}

pub struct SupertonicPaths {
    pub duration_predictor: PathBuf,
    pub text_encoder: PathBuf,
    pub vector_estimator: PathBuf,
    pub vocoder: PathBuf,
    pub config: PathBuf,
    pub unicode_indexer: PathBuf,
}

/// `(caminho no repo, SHA-256)` medidos com `sha256sum` na revisão fixada (2026-11-15).
const MODEL_FILES: [(&str, &str); 6] = [
    (
        "onnx/duration_predictor.onnx",
        "c3eb91414d5ff8a7a239b7fe9e34e7e2bf8a8140d8375ffb14718b1c639325db",
    ),
    (
        "onnx/text_encoder.onnx",
        "c7befd5ea8c3119769e8a6c1486c4edc6a3bc8365c67621c881bbb774b9902ff",
    ),
    (
        "onnx/vector_estimator.onnx",
        "883ac868ea0275ef0e991524dc64f16b3c0376efd7c320af6b53f5b780d7c61c",
    ),
    (
        "onnx/vocoder.onnx",
        "085de76dd8e8d5836d6ca66826601f615939218f90e519f70ee8a36ed2a4c4ba",
    ),
    (
        "onnx/tts.json",
        "42078d3aef1cd43ab43021f3c54f47d2d75ceb4e75f627f118890128b06a0d09",
    ),
    (
        "onnx/unicode_indexer.json",
        "9bf7346e43883a81f8645c81224f786d43c5b57f3641f6e7671a7d6c493cb24f",
    ),
];

const VOICES: [(&str, &str); 10] = [
    (
        "F1",
        "bbdec6ee00231c2c742ad05483df5334cab3b52fda3ba38e6a07059c4563dbc2",
    ),
    (
        "F2",
        "7c722c6a72707b1a77f035d67f0d1351ba187738e06f7683e8c72b1df3477fc6",
    ),
    (
        "F3",
        "12f6ef2573baa2defa1128069cb59f203e3ab67c92af77b42df8a0e3a2f7c6ab",
    ),
    (
        "F4",
        "c2fa764c1225a76dfc3e2c73e8aa4f70d9ee48793860eb34c295fff01c2e032b",
    ),
    (
        "F5",
        "45966e73316415626cf41a7d1c6f3b4c70dbc1ba2bee5c1978ef0ce33244fc8d",
    ),
    (
        "M1",
        "e35604687f5d23694b8e91593a93eec0e4eca6c0b02bb8ed69139ab2ea6b0a5b",
    ),
    (
        "M2",
        "b76cbf62bac707c710cf0ae5aba5e31eea1a6339a9734bfae33ab98499534a50",
    ),
    (
        "M3",
        "ea1ac35ccb91b0d7ecad533a2fbd0eec10c91513d8951e3b25fbba99954e159b",
    ),
    (
        "M4",
        "ca8eefad4fcd989c9379032ff3e50738adc547eeb5e221b82593a6d7b3bac303",
    ),
    (
        "M5",
        "dd22b92740314321f8ae11c5e87f8dd60d060f15dd3a632b5adf77f471f77af2",
    ),
];

fn voice_sha256(voice: &str) -> Option<&'static str> {
    VOICES
        .iter()
        .find(|(name, _)| *name == voice.trim())
        .map(|(_, sha)| *sha)
}

/// Estilo de voz: dois tensores `[1, linhas, colunas]` achatados — `style_ttl` (texto→latente) e
/// `style_dp` (duração).
#[derive(Clone)]
pub struct VoiceStyle {
    pub ttl: Vec<f32>,
    pub ttl_dims: [i64; 3],
    pub dp: Vec<f32>,
    pub dp_dims: [i64; 3],
}

#[derive(serde::Deserialize)]
struct StyleComponent {
    data: Vec<Vec<Vec<f32>>>,
    dims: Vec<i64>,
}

#[derive(serde::Deserialize)]
struct StyleFile {
    style_ttl: StyleComponent,
    style_dp: StyleComponent,
}

fn flatten(component: StyleComponent, label: &str) -> Result<(Vec<f32>, [i64; 3]), String> {
    let dims: [i64; 3] = component
        .dims
        .try_into()
        .map_err(|_| format!("{label}: dims não tem 3 eixos"))?;
    let flat: Vec<f32> = component.data.into_iter().flatten().flatten().collect();
    if flat.len() as i64 != dims.iter().product::<i64>() {
        return Err(format!(
            "{label}: {} valores para dims {dims:?}",
            flat.len()
        ));
    }
    Ok((flat, dims))
}

pub(crate) fn parse_voice_style(bytes: &[u8]) -> Result<VoiceStyle, String> {
    let file: StyleFile =
        serde_json::from_slice(bytes).map_err(|e| format!("voz Supertonic ilegível: {e}"))?;
    let (ttl, ttl_dims) = flatten(file.style_ttl, "style_ttl")?;
    let (dp, dp_dims) = flatten(file.style_dp, "style_dp")?;
    Ok(VoiceStyle {
        ttl,
        ttl_dims,
        dp,
        dp_dims,
    })
}

/// `.part` ÚNICO por download: dois downloads do mesmo arquivo (duas falas simultâneas no primeiro
/// uso) escreviam no MESMO `.part`, e o `rename` gravava um modelo intercalado.
fn part_path(target: &Path) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(
        ".{}-{}.part",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    target.with_file_name(name)
}

async fn download_to_file(
    store: &ConfigStore,
    url: &str,
    target: &Path,
    sha256: &str,
) -> Result<(), String> {
    assert_network_allowed(store, url, "supertonic-tts")?;
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    let part = part_path(target);
    match stream_download(url, &part, sha256).await {
        Ok(()) => tokio::fs::rename(&part, target)
            .await
            .map_err(|e| format!("could not rename {}: {e}", part.display())),
        Err(err) => {
            let _ = tokio::fs::remove_file(&part).await;
            Err(err)
        }
    }
}

async fn stream_download(url: &str, part_path: &Path, sha256: &str) -> Result<(), String> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    let response = crate::modules::privacy::fixed_endpoint_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download failed ({url}): {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "download failed ({url}): HTTP {}",
            response.status().as_u16()
        ));
    }
    let mut file = tokio::fs::File::create(part_path)
        .await
        .map_err(|e| format!("could not create {}: {e}", part_path.display()))?;
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download interrupted ({url}): {e}"))?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("could not write {}: {e}", part_path.display()))?;
    }
    file.flush()
        .await
        .map_err(|e| format!("could not write {}: {e}", part_path.display()))?;
    let digest = hex::encode(hasher.finalize());
    if digest != sha256 {
        return Err(format!("digest mismatch ({url}): got {digest}"));
    }
    Ok(())
}

fn model_root(cache_dir: &Path) -> PathBuf {
    cache_dir.join(MODEL_DIR)
}

/// O Supertonic 2 (`onnx-model/`, ~380 MB) não é lido por mais nada depois que o 3 está completo.
fn remove_legacy_cache(cache_dir: &Path) {
    let legacy = cache_dir.join(LEGACY_DIR);
    if legacy.is_dir() {
        match std::fs::remove_dir_all(&legacy) {
            Ok(()) => log::info!("cache do Supertonic 2 removido ({})", legacy.display()),
            Err(e) => log::warn!("cache do Supertonic 2 não saiu ({}): {e}", legacy.display()),
        }
    }
}

/// Baixa (se ausente) os 6 arquivos do modelo. Idempotente: arquivo presente não é rebaixado.
pub async fn ensure_model_files(
    cache_dir: &Path,
    store: &ConfigStore,
) -> Result<SupertonicPaths, String> {
    let base = hf_base();
    for (file, sha256) in MODEL_FILES {
        let target = model_root(cache_dir).join(file);
        if !target.is_file() {
            download_to_file(store, &format!("{base}/{file}"), &target, sha256).await?;
        }
    }
    remove_legacy_cache(cache_dir);
    Ok(model_paths(cache_dir))
}

fn model_paths(cache_dir: &Path) -> SupertonicPaths {
    let onnx = model_root(cache_dir).join("onnx");
    SupertonicPaths {
        duration_predictor: onnx.join("duration_predictor.onnx"),
        text_encoder: onnx.join("text_encoder.onnx"),
        vector_estimator: onnx.join("vector_estimator.onnx"),
        vocoder: onnx.join("vocoder.onnx"),
        config: onnx.join("tts.json"),
        unicode_indexer: onnx.join("unicode_indexer.json"),
    }
}

/// O modelo SÓ se os 6 arquivos já estão no disco — nunca baixa (é o que o pré-carregamento do boot
/// usa; o download continua sendo da primeira síntese, com o chokepoint de rede).
pub fn installed_model_files(cache_dir: &Path) -> Option<SupertonicPaths> {
    MODEL_FILES
        .iter()
        .all(|(file, _)| model_root(cache_dir).join(file).is_file())
        .then(|| model_paths(cache_dir))
}

fn voice_path(cache_dir: &Path, voice: &str) -> PathBuf {
    model_root(cache_dir)
        .join("voice_styles")
        .join(format!("{}.json", voice.trim()))
}

/// Baixa (se ausente) a voz — também o que o wizard chama para deixá-la pronta antes da primeira fala.
pub async fn download_voice(
    cache_dir: &Path,
    voice: &str,
    store: &ConfigStore,
) -> Result<PathBuf, String> {
    let sha256 = voice_sha256(voice)
        .ok_or_else(|| format!("invalid Supertonic voice: '{voice}' (expected [F|M][1-5])"))?;
    let target = voice_path(cache_dir, voice);
    if !target.is_file() {
        let url = format!("{}/voice_styles/{}.json", hf_base(), voice.trim());
        download_to_file(store, &url, &target, sha256).await?;
    }
    Ok(target)
}

pub async fn ensure_voice(
    cache_dir: &Path,
    voice: &str,
    store: &ConfigStore,
) -> Result<VoiceStyle, String> {
    let target = download_voice(cache_dir, voice, store).await?;
    let bytes = tokio::fs::read(&target)
        .await
        .map_err(|e| format!("could not read {}: {e}", target.display()))?;
    parse_voice_style(&bytes).inspect_err(|_| {
        let _ = std::fs::remove_file(&target);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O pré-carregamento do boot nunca baixa nada: só enxerga o modelo com os 6 arquivos já no disco.
    #[test]
    fn installed_model_files_needs_every_file_already_on_disk() {
        let dir =
            std::env::temp_dir().join(format!("mika-supertonic-installed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (file, _) in &MODEL_FILES[..MODEL_FILES.len() - 1] {
            let path = model_root(&dir).join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"x").unwrap();
        }
        assert!(
            installed_model_files(&dir).is_none(),
            "falta o unicode_indexer: nada para carregar"
        );

        std::fs::write(
            model_root(&dir).join(MODEL_FILES[MODEL_FILES.len() - 1].0),
            b"x",
        )
        .unwrap();
        let paths = installed_model_files(&dir).expect("os 6 arquivos presentes");
        assert_eq!(
            paths.unicode_indexer,
            dir.join("supertonic-3/onnx/unicode_indexer.json")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Só o cache do Supertonic 2 sai; o do 3 (e o resto da pasta) fica.
    #[test]
    fn legacy_cache_removal_touches_only_the_supertonic_2_folder() {
        let dir =
            std::env::temp_dir().join(format!("mika-supertonic-legacy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(LEGACY_DIR).join("onnx")).unwrap();
        std::fs::create_dir_all(model_root(&dir)).unwrap();
        remove_legacy_cache(&dir);
        assert!(!dir.join(LEGACY_DIR).exists());
        assert!(model_root(&dir).is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_download_gets_its_own_part_file() {
        let model = Path::new("/c/supertonic-3/onnx/vocoder.onnx");
        let (a, b) = (part_path(model), part_path(model));
        assert_ne!(a, b);
        assert_eq!(
            a.parent(),
            model.parent(),
            "o rename tem de ficar no mesmo diretório"
        );
    }

    #[test]
    fn only_the_ten_documented_voices_have_a_pinned_digest() {
        for letter in ['F', 'M'] {
            for digit in '1'..='5' {
                assert!(voice_sha256(&format!("{letter}{digit}")).is_some());
            }
        }
        for bad in ["F6", "X1", "F1x", "", "../F1"] {
            assert!(voice_sha256(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn a_voice_style_flattens_both_tensors_and_rejects_a_dims_mismatch() {
        let ok = br#"{"style_ttl":{"data":[[[1,2],[3,4]]],"dims":[1,2,2],"type":"float32"},"style_dp":{"data":[[[5,6]]],"dims":[1,1,2],"type":"float32"}}"#;
        let style = parse_voice_style(ok).unwrap();
        assert_eq!(
            (style.ttl, style.ttl_dims),
            (vec![1.0, 2.0, 3.0, 4.0], [1, 2, 2])
        );
        assert_eq!((style.dp, style.dp_dims), (vec![5.0, 6.0], [1, 1, 2]));
        let bad = br#"{"style_ttl":{"data":[[[1,2]]],"dims":[1,2,2]},"style_dp":{"data":[[[5,6]]],"dims":[1,1,2]}}"#;
        assert!(parse_voice_style(bad).is_err());
    }
}
