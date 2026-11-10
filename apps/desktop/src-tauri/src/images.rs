//! Decodificação, redimensionamento e reencode de imagem para o pipeline de visão.
//!
//! Substitui `nativeImage` do Electron (síncrono) por ações de shell correlacionadas —
//! o backend Node manda `dataUrl` e espera de volta o bitmap redimensionado ou o JPEG
//! reencodado. Decode que falha (data URL corrompida, bytes que não são imagem) vira
//! `isEmpty: true`, nunca erro: é o mesmo contrato de `nativeImage.isEmpty()` no Electron,
//! que também nunca lança para entrada ruim — os três chamadores em `vision/` dependem
//! desse fail-soft (retornam hash/densidade vazios em vez de derrubar o pipeline).

use base64::Engine as _;
use image::codecs::jpeg::JpegEncoder;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ExtendedColorType, ImageEncoder};
use serde_json::Value;

/// Redimensiona `src` para exatamente `width`x`height` com SIMD (`fast_image_resize`: AVX2/SSE4.1
/// no x86, NEON no ARM, detectado em runtime) — o `resize_exact` do `image` é escalar e rodava de
/// 3 a 4 vezes por captura do laço de visão (thumbnail, JPEG do LLM, hash, densidade de texto).
/// `Bilinear` é o `Triangle` do `image`; `Box` é média por área (o `thumbnail` dele). Sem
/// pré-multiplicar alfa, como o `image` fazia — captura de tela é opaca. Formato de pixel que o
/// SIMD não cobre cai no caminho escalar de antes, nunca em erro.
pub fn resize(src: &DynamicImage, width: u32, height: u32, filter: FilterType) -> DynamicImage {
    let mut dst = DynamicImage::new(width, height, src.color());
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(filter)).use_alpha(false);
    match Resizer::new().resize(src, &mut dst, &options) {
        Ok(()) => dst,
        Err(_) => src.resize_exact(width, height, image::imageops::FilterType::Triangle),
    }
}

/// `None` cobre "bytes não decodificam como imagem" — o mesmo caso que o Electron modela como
/// `nativeImage.isEmpty() === true`. Aceita tanto uma data URL completa (`data:image/...;base64,`)
/// quanto base64 CRU sem prefixo: `screen_capture.rs::capture_screen_images` (captura real de tela)
/// devolve base64 cru — o mesmo formato que `llm_client::analyze` já manda direto para o modelo via
/// `UserContent::image_base64` — enquanto `clipboard_watch.rs` monta uma data URL completa. Antes
/// desta função exigir a vírgula, toda imagem de captura de tela real vinda sem prefixo falhava a
/// decodificar (`split_once(',')` devolvia `None`): o hash de mudança saía sempre vazio (dedup nunca
/// disparava, fail-open de `has_screen_changed`) e o gate de densidade de texto sempre bloqueava
/// (`estimateTextDensity` de imagem vazia é `0.0`, abaixo de qualquer limiar positivo) — os dois já
/// "portados" na letra do código, mas nunca exercitados de fato fora de teste sintético (que sempre
/// usava data URL completa).
pub(crate) fn decode_data_url(data_url: &str) -> Option<DynamicImage> {
    let base64_part = data_url.split_once(',').map_or(data_url, |(_, tail)| tail);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_part)
        .ok()?;
    image::load_from_memory(&bytes).ok()
}

/// Decodifica, redimensiona para EXATAMENTE `width`x`height` e devolve o bitmap RGBA8 cru
/// (base64). Usado por `image-hash.ts` (9x8) e `text-density-gate.ts` (64x64) — ambos leem
/// o buffer como RGBA na própria camada TS, então o formato aqui tem de ser RGBA, não BGRA.
pub fn resize_bitmap(payload: &Value) -> Result<Value, String> {
    let data_url = payload["dataUrl"].as_str().unwrap_or_default();
    let width = payload["width"].as_u64().unwrap_or(0) as u32;
    let height = payload["height"].as_u64().unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err("imageResizeBitmap exige width e height > 0".into());
    }

    let Some(img) = decode_data_url(data_url) else {
        return Ok(serde_json::json!({ "isEmpty": true, "bitmap": "" }));
    };

    let resized = resize(&img, width, height, FilterType::Bilinear).to_rgba8();
    let encoded = base64::engine::general_purpose::STANDARD.encode(resized.as_raw());
    Ok(serde_json::json!({ "isEmpty": false, "bitmap": encoded }))
}

/// Longest side clamped to `max_dimension`, aspect kept — `toJpegDataUrl`'s rule
/// (`jpeg-compression.ts`). `bool` = resized.
pub(crate) fn fit_longest_side(img: DynamicImage, max_dimension: f64) -> (DynamicImage, bool) {
    let (width, height) = (img.width(), img.height());
    let longest_side = width.max(height) as f64;
    if !(max_dimension > 0.0 && longest_side > max_dimension) {
        return (img, false);
    }
    let scale = max_dimension / longest_side;
    let new_width = ((width as f64) * scale).round().max(1.0) as u32;
    let new_height = ((height as f64) * scale).round().max(1.0) as u32;
    (resize(&img, new_width, new_height, FilterType::Bilinear), true)
}

/// RGB pixels -> `data:image/jpeg;base64,...` (JPEG has no alpha; Electron's `toJPEG()` drops it too).
pub(crate) fn jpeg_data_url(rgb: &image::RgbImage, quality: u8) -> Result<String, String> {
    let mut buffer = Vec::new();
    JpegEncoder::new_with_quality(&mut buffer, quality.clamp(1, 100))
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&buffer)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, RgbaImage};

    fn png_base64_raw(width: u32, height: u32) -> String {
        let image = RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
        let mut buffer = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buffer)
            .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
            .expect("encode fixture PNG");
        base64::engine::general_purpose::STANDARD.encode(&buffer)
    }

    /// REGRESSÃO — `screen_capture.rs::capture_screen_images` devolve base64 CRU (sem prefixo
    /// `data:image/...;base64,`), o mesmo formato que chega aqui em produção real para hash/gate de
    /// densidade. Antes do fix, `decode_data_url` exigia a vírgula e toda imagem de tela real virava
    /// `isEmpty: true` — este teste falha na versão antiga (`split_once(',')` -> `None`).
    #[test]
    fn resize_bitmap_decodes_raw_base64_without_a_data_url_prefix() {
        let raw = png_base64_raw(4, 4);
        let result = resize_bitmap(&serde_json::json!({ "dataUrl": raw, "width": 2, "height": 2 })).unwrap();
        assert_eq!(result["isEmpty"], serde_json::json!(false));
        assert!(!result["bitmap"].as_str().unwrap().is_empty());
    }

    /// A data URL completa (formato do clipboard) continua funcionando — a mudança só ADICIONA um
    /// caminho novo, não troca o existente.
    #[test]
    fn resize_bitmap_still_decodes_a_full_data_url() {
        let raw = png_base64_raw(4, 4);
        let data_url = format!("data:image/png;base64,{raw}");
        let result = resize_bitmap(&serde_json::json!({ "dataUrl": data_url, "width": 2, "height": 2 })).unwrap();
        assert_eq!(result["isEmpty"], serde_json::json!(false));
    }

    #[test]
    fn decode_failure_is_empty_not_an_error() {
        let result = resize_bitmap(&serde_json::json!({ "dataUrl": "not base64 at all!!", "width": 2, "height": 2 })).unwrap();
        assert_eq!(result["isEmpty"], serde_json::json!(true));
    }
}
