//! Bitmap compartilhado por [`super::image_hash`] e [`super::text_density`]: as duas heurísticas
//! pedem o MESMO redimensionamento de uma data URL (`resizeToBitmap`, `shared/image-ops.ts`), só
//! com dimensões-alvo diferentes — reusa `images::resize_bitmap`, já portado, em vez de decodificar
//! a imagem duas vezes.

use base64::Engine as _;
use serde_json::json;

/// `None` cobre tanto "não é uma data URL" quanto "bytes não decodificam como imagem" — mesmo
/// fail-soft que `images.rs` já documenta (`isEmpty: true`, nunca erro); um payload com
/// `width`/`height` fixos e > 0 nunca cai no único branch de erro de `resize_bitmap`, mas o
/// `.ok()?` cobre esse caso também, por segurança.
/// Raw RGBA of a captured frame resized to `width`x`height` — the watcher hashes and gates pixels
/// before any JPEG exists, so an unchanged screen is never encoded.
pub(super) fn frame_bitmap(frame: &image::DynamicImage, width: u32, height: u32) -> Vec<u8> {
    crate::images::resize(frame, width, height, fast_image_resize::FilterType::Bilinear).to_rgba8().into_raw()
}

pub(super) fn resize_to_bitmap(data_url: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    let payload = json!({ "dataUrl": data_url, "width": width, "height": height });
    let result = crate::images::resize_bitmap(&payload).ok()?;
    if result["isEmpty"].as_bool().unwrap_or(true) {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(result["bitmap"].as_str()?)
        .ok()
}
