//! Captura de tela/janela via `xcap`. Sem este módulo `desktopCapturer` não tem equivalente
//! nativo no sidecar (ver `unsupported.ts` no lado TS) e a Mika perde a visão da tela — a
//! feature central de "companion que observa a tela" (AGENTS.md).
//!
//! Duas formas de uso reais moldam o formato da resposta:
//! - `screen-capture-consent.ts` pede só NOMES (`thumbnailWidth`/`thumbnailHeight` == 0) para
//!   checar a blocklist de privacidade — captura de pixel nenhum vale a pena nesse caminho,
//!   então `capture_image()` só roda quando um thumbnail de verdade foi pedido.
//! - `clipboard-screen-adapters.ts` pede thumbnail de verdade e o lado TS (`desktop-capturer.ts`)
//!   só empacota bytes já prontos: o encode acontece aqui, na captura, porque a API do
//!   Electron que os chamadores imitam é síncrona depois que `getSources()` resolve, e o Rust
//!   só responde por mensagem assíncrona — não há como reencodificar em outra qualidade depois.

use std::io::Cursor;

use base64::Engine as _;
use fast_image_resize::FilterType;
use image::{DynamicImage, RgbaImage, codecs::jpeg::JpegEncoder};
use serde_json::Value;
use xcap::{Monitor, Window, XCapResult};

/// Qualidade do JPEG quando o chamador não pede uma. Mesmo default de
/// `DEFAULT_CAPTURE_QUALITY` em `clipboard-screen-adapters.ts`.
const DEFAULT_JPEG_QUALITY: u8 = 80;

/// A qualidade vem com o PEDIDO, não depois dele.
///
/// `toJPEG(quality)` do Electron é síncrono e roda DEPOIS de `getSources()` resolver — de lá
/// não dá para pedir nada ao Rust. Mas quem chama já sabe a qualidade antes: a captura de
/// sondagem usa 25 e a análise completa 95. Fixar 80 aqui deixaria o probe ~3x mais pesado
/// que o necessário e tiraria fidelidade da análise, em toda captura, para sempre.
fn quality_of(payload: &Value) -> u8 {
    payload["thumbnailQuality"]
        .as_u64()
        .map(|q| q.clamp(1, 100) as u8)
        .unwrap_or(DEFAULT_JPEG_QUALITY)
}

/// `{ types: ('screen'|'window')[], thumbnailWidth, thumbnailHeight }` -> lista de fontes, no
/// formato que `desktop-capturer.ts` espera (`id`, `name`, `thumbnailBase64`, `thumbnailEmpty`).
pub fn get_sources(payload: &Value) -> Result<Value, String> {
    let types: Vec<&str> = payload["types"]
        .as_array()
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let want_screen = types.is_empty() || types.contains(&"screen");
    let want_window = types.contains(&"window");
    let thumb_w = payload["thumbnailWidth"].as_u64().unwrap_or(0) as u32;
    let thumb_h = payload["thumbnailHeight"].as_u64().unwrap_or(0) as u32;
    let want_thumbnail = thumb_w > 0 && thumb_h > 0;
    let quality = quality_of(payload);

    let mut sources = Vec::new();

    if want_screen {
        for monitor in Monitor::all().map_err(|e| e.to_string())? {
            let Ok(id) = monitor.id() else { continue };
            let name = monitor
                .friendly_name()
                .or_else(|_| monitor.name())
                .unwrap_or_else(|_| format!("Display {id}"));
            let (base64, empty) = if want_thumbnail {
                encode_thumbnail(monitor.capture_image(), thumb_w, thumb_h, quality)
            } else {
                (String::new(), true)
            };
            sources.push(source_json(format!("screen:{id}"), name, base64, empty));
        }
    }

    if want_window {
        for window in Window::all().map_err(|e| e.to_string())? {
            let Ok(id) = window.id() else { continue };
            let title = window.title().unwrap_or_default();
            // Sem título ou minimizada: nada legível para casar na blocklist, e capturar uma
            // janela minimizada devolve pixels vazios/pretos na maioria dos sistemas — mesmo
            // resultado de "não visível" que `isAnyVisibleWindowBlocked` já espera.
            if title.is_empty() || window.is_minimized().unwrap_or(false) {
                continue;
            }
            let (base64, empty) = if want_thumbnail {
                encode_thumbnail(window.capture_image(), thumb_w, thumb_h, quality)
            } else {
                (String::new(), true)
            };
            sources.push(source_json(format!("window:{id}"), title, base64, empty));
        }
    }

    Ok(Value::Array(sources))
}

/// Every monitor's raw frame, no encode — the vision pipeline resizes and encodes it once for the
/// LLM (`vision::compression`). A monitor that fails to capture (protected content) is skipped.
pub fn capture_monitor_frames() -> Result<Vec<DynamicImage>, String> {
    let frames = Monitor::all()
        .map_err(|e| e.to_string())?
        .iter()
        .filter_map(|monitor| monitor.capture_image().ok())
        .filter(|image| image.width() > 0 && image.height() > 0)
        .map(DynamicImage::ImageRgba8)
        .collect();
    Ok(frames)
}

fn source_json(id: String, name: String, thumbnail_base64: String, thumbnail_empty: bool) -> Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "thumbnailBase64": thumbnail_base64,
        "thumbnailEmpty": thumbnail_empty,
    })
}

/// Maior tamanho com a proporção de `width`x`height` que cabe em `max_w`x`max_h` (amplia ou
/// reduz) — a regra de `DynamicImage::resize`/`thumbnail` do `image`, que este caminho usava.
fn fit(width: u32, height: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let ratio = f64::min(max_w as f64 / width as f64, max_h as f64 / height as f64);
    let scale = |side: u32| ((side as f64 * ratio).round() as u32).max(1);
    (scale(width), scale(height))
}

/// RGBA capturado -> JPEG em base64, redimensionado para caber em `max_w`x`max_h` preservando
/// proporção (mesma semântica do `thumbnailSize` do Electron). Falha de captura vira thumbnail
/// vazio, não erro: uma janela protegida (DRM, elevada) não pode derrubar a lista inteira de
/// fontes — o chamador já trata `isEmpty()` e cai para a próxima fonte.
fn encode_thumbnail(captured: XCapResult<RgbaImage>, max_w: u32, max_h: u32, quality: u8) -> (String, bool) {
    let Ok(image) = captured else { return (String::new(), true) };
    if image.width() == 0 || image.height() == 0 {
        return (String::new(), true);
    }

    // Reduzir (o caso comum: monitor 1440p/4K -> 1920x1080) é média por área (`Box`); ampliar
    // (janela pequena) é bilinear. Os dois com SIMD (`images::resize`).
    let image = DynamicImage::ImageRgba8(image);
    let (width, height) = fit(image.width(), image.height(), max_w, max_h);
    let filter = if width < image.width() { FilterType::Box } else { FilterType::Bilinear };
    let resized = crate::images::resize(&image, width, height, filter);

    let mut buffer = Vec::new();
    let encoded = {
        let mut cursor = Cursor::new(&mut buffer);
        let encoder = JpegEncoder::new_with_quality(&mut cursor, quality);
        resized.write_with_encoder(encoder)
    };
    if encoded.is_err() || buffer.is_empty() {
        return (String::new(), true);
    }

    (base64::engine::general_purpose::STANDARD.encode(&buffer), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A regra do `thumbnailSize` do Electron: cabe na caixa, proporção preservada, nos dois sentidos.
    #[test]
    fn thumbnails_fit_the_box_keeping_the_aspect_ratio() {
        assert_eq!(fit(3840, 2160, 1920, 1080), (1920, 1080), "4K reduz");
        assert_eq!(fit(5120, 1440, 1920, 1080), (1920, 540), "ultrawide limita pela largura");
        assert_eq!(fit(800, 600, 1920, 1080), (1440, 1080), "janela pequena amplia");
        assert_eq!(fit(10_000, 1, 1920, 1080), (1920, 1), "nunca zero");
    }

    /// Captura real de monitor é RGBA; o thumbnail tem de sair JPEG decodificável no tamanho
    /// ajustado, com a cor preservada pelo redimensionamento SIMD.
    #[test]
    fn a_4k_capture_becomes_a_1080p_jpeg_with_the_same_colour() {
        let capture = RgbaImage::from_pixel(3840, 2160, image::Rgba([200, 30, 30, 255]));
        let (b64, empty) = encode_thumbnail(Ok(capture), 1920, 1080, 90);
        assert!(!empty);
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (1920, 1080));
        let [r, g, b] = decoded.get_pixel(960, 540).0;
        assert!(r > 180 && g < 60 && b < 60, "cor perdida: {r},{g},{b}");
    }
}
