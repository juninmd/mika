//! `person_catalog_image` — foto de pessoa por URL para a tela de Pessoas (`use-person-image.ts`).
//!
//! A rede passa por `browser::fetch_bytes` (SSRF e modo offline revalidados a cada hop de redirect),
//! o mesmo pipeline do resumo da Wikipedia em `enrichment.rs`. O tipo vem dos BYTES, nunca do
//! header nem da extensão: só PNG/JPEG/GIF/WebP viram `data:` — SVG (pode carregar script) e HTML
//! de página de erro ficam de fora. Sem cache aqui: o renderer já guarda as últimas 60 por URL.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::browser;
use crate::modules::plugins::rpc::RpcHandler;

const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn to_data_url(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(format!("Image exceeds {} MiB.", MAX_IMAGE_BYTES / 1024 / 1024));
    }
    let mime = sniff_mime(bytes).ok_or("Not a PNG, JPEG, GIF or WebP image.")?;
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

async fn fetch_data_url(store: &ConfigStore, url: &str) -> Result<String, String> {
    let bytes = tokio::time::timeout(FETCH_TIMEOUT, browser::fetch_bytes(store, url))
        .await
        .map_err(|_| "Image fetch timed out.".to_string())?
        .map_err(|error| format!("Image fetch refused or failed: {error:?}"))?;
    to_data_url(&bytes)
}

pub(super) struct ImageHandler {
    pub(super) store: Arc<ConfigStore>,
}

#[async_trait]
impl RpcHandler for ImageHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let Some(url) = args.get("url").and_then(Value::as_str).filter(|u| !u.is_empty()) else {
            return Ok(json!({ "success": false, "error": "url is required." }));
        };
        Ok(match fetch_data_url(&self.store, url).await {
            Ok(data_url) => json!({ "success": true, "data": { "dataUrl": data_url } }),
            Err(error) => json!({ "success": false, "error": error }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_raster_images_become_a_data_url() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        assert!(to_data_url(png).unwrap().starts_with("data:image/png;base64,"));
        assert!(to_data_url(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap().starts_with("data:image/jpeg;"));
        assert!(to_data_url(b"RIFF\0\0\0\0WEBPVP8 ").unwrap().starts_with("data:image/webp;"));
        assert!(to_data_url(b"<svg onload=\"alert(1)\"/>").is_err(), "SVG pode carregar script");
        assert!(to_data_url(b"<!doctype html><title>404</title>").is_err(), "página de erro não é foto");
    }

    #[test]
    fn an_oversized_image_is_refused_even_with_a_valid_signature() {
        let mut huge = b"\x89PNG\r\n\x1a\n".to_vec();
        huge.resize(MAX_IMAGE_BYTES + 1, 0);
        assert!(to_data_url(&huge).is_err());
    }

    #[tokio::test]
    async fn a_private_address_is_refused_before_any_network() {
        let dir = std::env::temp_dir().join(format!("mika-person-image-{}", std::process::id()));
        let store = Arc::new(ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap());
        let handler = ImageHandler { store };
        let response = handler.call(json!({ "url": "http://127.0.0.1/avatar.png" })).await.unwrap();
        assert_eq!(response["success"], json!(false), "{response}");
        let missing = handler.call(json!({})).await.unwrap();
        assert_eq!(missing, json!({ "success": false, "error": "url is required." }));
        let _ = std::fs::remove_dir_all(dir);
    }
}
