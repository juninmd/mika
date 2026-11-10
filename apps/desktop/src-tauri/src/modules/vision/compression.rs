//! Compressão adaptativa da imagem antes de mandar para o LLM de visão — specs/06-vision.md
//! ("Custo"): "existe teto de tamanho por imagem, e a qualidade se ajusta sozinha para respeitá-lo"
//! + "a compressão é adaptativa nas duas direções — sobe qualidade quando as capturas estão bem
//! abaixo do alvo".
//!
//! **Estado anterior a esta rodada:** as quatro chaves existiam no schema
//! (`config/schemas/vision.rs`) e na UI (`vision-settings-panel.tsx`), mas
//! `screen_capture.rs::capture_screen_frames` só usava `vision.captureQuality` — um valor fixo
//! pedido ao SO, sem ajuste nenhum pelo tamanho resultante. Toggle sem leitor no backend, mesma
//! classe de defeito que a seção "Controles de percepção" da spec proíbe.
//!
//! `vision.maxImageDimension` (teto de dimensão do lado LLM) and `vision.captureQuality` (the
//! ceiling the quality may rise to) apply in the same single encode — see [`encode_frame_for_llm`].
//!
//! `next_quality` é a decisão PURA (sem I/O de imagem), testável sem reencode de verdade;
//! [`encode_frame_for_llm`] é quem reencodifica em loop, bounded a [`MAX_ROUNDS`] idas — nunca
//! converge para sempre, e cada rodada reencodifica no máximo uma vez.

use image::DynamicImage;

use crate::config::store::ConfigStore;

const MIN_QUALITY: u8 = 10;
const MAX_QUALITY: u8 = 100;
const QUALITY_STEP: u8 = 10;
/// Teto de idas e vindas por imagem — cada rodada é um reencode JPEG de verdade (custo de CPU),
/// então isto é o piso de "converge ou desiste", nunca "tenta até acertar" sem limite.
const MAX_ROUNDS: u8 = 4;
/// "Bem abaixo do alvo" (spec) — metade do `compressionTargetSizeKB` configurado. Abaixo disso vale
/// gastar mais qualidade porque ainda sobra margem até o teto.
const WELL_BELOW_TARGET_RATIO: f64 = 0.5;

pub(super) struct CompressionConfig {
    pub adaptive: bool,
    pub max_size_kb: u64,
    pub target_size_kb: u64,
    pub llm_quality: u8,
    pub max_dimension: u32,
}

fn get_bool(store: &ConfigStore, path: &str, default: bool) -> bool {
    store.get(path).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn get_u64(store: &ConfigStore, path: &str, default: u64) -> u64 {
    store.get(path).and_then(|v| v.as_u64()).unwrap_or(default)
}

pub(super) fn read_compression_config(store: &ConfigStore) -> CompressionConfig {
    CompressionConfig {
        adaptive: get_bool(store, "vision.adaptiveCompression", true),
        max_size_kb: get_u64(store, "vision.maxImageSizeKB", 500),
        target_size_kb: get_u64(store, "vision.compressionTargetSizeKB", 300),
        llm_quality: get_u64(store, "vision.llmQuality", 60).clamp(1, 100) as u8,
        max_dimension: get_u64(store, "vision.maxImageDimension", 1280).clamp(320, 4096) as u32,
    }
}

/// Decisão PURA: dado o tamanho atual (KB) e os limites configurados, qual a próxima qualidade a
/// tentar — `None` quando o tamanho já está aceitável (nada a fazer) ou quando o passo não mudaria
/// a qualidade (já no piso/teto, evita loop infinito de "tentar de novo o mesmo valor").
pub(super) fn next_quality(current_quality: u8, size_kb: u64, cfg: &CompressionConfig) -> Option<u8> {
    if !cfg.adaptive {
        return None;
    }
    if size_kb > cfg.max_size_kb {
        let lowered = current_quality.saturating_sub(QUALITY_STEP).max(MIN_QUALITY);
        return (lowered != current_quality).then_some(lowered);
    }
    let well_below = ((cfg.target_size_kb as f64) * WELL_BELOW_TARGET_RATIO) as u64;
    if size_kb > 0 && size_kb < well_below {
        let raised = current_quality.saturating_add(QUALITY_STEP).min(MAX_QUALITY);
        return (raised != current_quality).then_some(raised);
    }
    None
}

/// One frame -> the JPEG data URL the LLM gets, in a single pass from pixels: resized ONCE to
/// `maxImageDimension`, then only the JPEG quality moves — adaptive rounds re-encode the same RGB
/// buffer, never decode. Starts at `llmQuality` and never rises past `ceiling` (the capture quality
/// the caller asked for: `vision.captureQuality`, a tool's `quality`). The frame used to become a
/// 1920x1080 JPEG first, be decoded here and re-encoded (and re-decoded + re-resized every round).
fn encode_frame_for_llm(frame: DynamicImage, cfg: &CompressionConfig, ceiling: u8) -> Option<String> {
    let rgb = crate::images::fit_longest_side(frame, f64::from(cfg.max_dimension)).0.to_rgb8();
    // Size of what travels (the base64 string that goes out and counts as tokens), not the raw bytes.
    let encode = |quality: u8| crate::images::jpeg_data_url(&rgb, quality).ok().map(|out| (out.len() as u64 / 1024, out));
    let mut quality = cfg.llm_quality.min(ceiling);
    let (mut size_kb, mut out) = encode(quality)?;
    for _ in 0..MAX_ROUNDS {
        let Some(next) = next_quality(quality, size_kb, cfg).map(|q| q.min(ceiling)).filter(|q| *q != quality) else { break };
        quality = next;
        let Some((next_size, next_out)) = encode(quality) else { break };
        (size_kb, out) = (next_size, next_out);
    }
    Some(out)
}

/// Screen frames straight from the capture ([`encode_frame_for_llm`] each). Blocking CPU work.
pub(super) fn encode_frames_for_llm(frames: Vec<DynamicImage>, cfg: &CompressionConfig, ceiling: u8) -> Vec<String> {
    frames.into_iter().filter_map(|frame| encode_frame_for_llm(frame, cfg, ceiling)).collect()
}

/// Recomprime cada imagem que já chega codificada (clipboard, `vision_read describe`): decodifica
/// UMA vez e segue o mesmo caminho dos frames. Imagem que falha ao decodificar é descartada do
/// resultado — mesma política de fail-soft do resto do pipeline (nunca derruba o tick por uma
/// imagem ruim).
pub(super) fn compress_images_for_llm(images: &[String], cfg: &CompressionConfig) -> Vec<String> {
    images.iter().filter_map(|data_url| encode_frame_for_llm(crate::images::decode_data_url(data_url)?, cfg, MAX_QUALITY)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(adaptive: bool, max_size_kb: u64, target_size_kb: u64) -> CompressionConfig {
        CompressionConfig { adaptive, max_size_kb, target_size_kb, llm_quality: 60, max_dimension: 1280 }
    }

    // -- next_quality --------------------------------------------------------------------------

    #[test]
    fn adaptive_off_never_suggests_a_change() {
        assert_eq!(next_quality(60, 10_000, &cfg(false, 500, 300)), None);
    }

    #[test]
    fn over_the_size_cap_lowers_quality() {
        assert_eq!(next_quality(60, 600, &cfg(true, 500, 300)), Some(50));
    }

    #[test]
    fn well_below_the_target_raises_quality() {
        // Alvo 300KB, "bem abaixo" é < 150KB (metade) — 80KB qualifica.
        assert_eq!(next_quality(60, 80, &cfg(true, 500, 300)), Some(70));
    }

    #[test]
    fn between_target_and_cap_keeps_the_quality_as_is() {
        assert_eq!(next_quality(60, 400, &cfg(true, 500, 300)), None);
    }

    #[test]
    fn quality_never_drops_below_the_floor() {
        assert_eq!(next_quality(MIN_QUALITY, 10_000, &cfg(true, 500, 300)), None);
    }

    #[test]
    fn quality_never_rises_above_the_ceiling() {
        assert_eq!(next_quality(MAX_QUALITY, 1, &cfg(true, 500, 300)), None);
    }

    #[test]
    fn a_zero_size_image_never_triggers_a_raise() {
        // Tamanho 0 só acontece quando o reencode falhou — não é "bem abaixo do alvo" de verdade.
        assert_eq!(next_quality(60, 0, &cfg(true, 500, 300)), None);
    }

    // -- read_compression_config: defaults quando a chave está ausente ---------------------------

    #[test]
    fn read_compression_config_uses_the_same_defaults_as_the_ts_schema() {
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir: PathBuf = std::env::temp_dir().join(format!("mika-vision-compression-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();

        let read = read_compression_config(&store);
        assert!(read.adaptive);
        assert_eq!(read.max_size_kb, 500);
        assert_eq!(read.target_size_kb, 300);
        assert_eq!(read.llm_quality, 60);
        assert_eq!(read.max_dimension, 1280);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- compress_images_for_llm: caminho real de encode ------------------------------------------

    fn png_base64(width: u32, height: u32) -> String {
        use base64::Engine as _;
        use image::{ExtendedColorType, ImageEncoder};
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
        let mut buffer = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buffer)
            .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(&buffer)
    }

    #[test]
    fn compress_images_for_llm_clamps_dimension_even_with_adaptive_off() {
        let raw = png_base64(2000, 10);
        let cfg = CompressionConfig { adaptive: false, max_size_kb: 500, target_size_kb: 300, llm_quality: 80, max_dimension: 100 };
        let out = compress_images_for_llm(&[raw], &cfg);
        assert_eq!(out.len(), 1);
        // Reencode aplicado (JPEG) mesmo sem o loop adaptativo — o teto de dimensão não é opcional.
        assert!(out[0].starts_with("data:image/jpeg;base64,"));
    }

    fn decode(data_url: &str) -> image::DynamicImage {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD.decode(data_url.split_once(',').unwrap().1).unwrap();
        image::load_from_memory(&bytes).unwrap()
    }

    /// The captured frame used to become a 1920x1080 JPEG, be decoded again and re-encoded for the
    /// LLM — two lossy passes and a decode per frame. It now goes from pixels to the LLM's JPEG in
    /// one encode: resized once to `maxImageDimension`, one generation of JPEG loss.
    #[test]
    fn a_captured_frame_reaches_the_llm_resized_once_in_a_single_encode() {
        let frame = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(2560, 1440, image::Rgba([200, 30, 30, 255])));
        let out = encode_frames_for_llm(vec![frame], &cfg(true, 500, 300), 100);
        assert_eq!(out.len(), 1);
        let decoded = decode(&out[0]).to_rgb8();
        assert_eq!(decoded.dimensions(), (1280, 720));
        let [r, g, b] = decoded.get_pixel(640, 360).0;
        assert!(r.abs_diff(200) <= 4 && g.abs_diff(30) <= 4 && b.abs_diff(30) <= 4, "one lossy pass: {r},{g},{b}");
    }

    /// `vision.captureQuality` (and the tools' capture quality) still means something: adaptive
    /// raising never goes past it — the intermediate JPEG used to cap fidelity the same way.
    #[test]
    fn the_capture_quality_caps_the_adaptive_raise() {
        let noisy = image::RgbaImage::from_fn(640, 360, |x, y| image::Rgba([(x * 7 % 251) as u8, (y * 13 % 241) as u8, ((x ^ y) % 256) as u8, 255]));
        let frame = image::DynamicImage::ImageRgba8(noisy);
        let generous = cfg(true, 100_000, 100_000);
        let capped = encode_frames_for_llm(vec![frame.clone()], &generous, 40);
        let uncapped = encode_frames_for_llm(vec![frame], &generous, 100);
        assert!(capped[0].len() < uncapped[0].len(), "quality 40 ceiling must yield a smaller JPEG than 100");
    }

    #[test]
    fn compress_images_for_llm_drops_an_image_that_fails_to_decode() {
        let cfg = CompressionConfig { adaptive: true, max_size_kb: 500, target_size_kb: 300, llm_quality: 60, max_dimension: 1280 };
        let out = compress_images_for_llm(&["not an image".to_string()], &cfg);
        assert!(out.is_empty());
    }
}
