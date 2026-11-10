//! Observador de área de transferência — port funcional de `capture/clipboard-watch.ts`.
//!
//! **O CHOKEPOINT desta fronteira é DUPLO, de propósito**: `vision.enabled` + `vision.readClipboardImages`
//! (a feature tem de estar ligada) E `privacy.clipboardLogging` (consentimento explícito e
//! separado — o mesmo campo que `isClipboardReadAllowed()` lê no TS). Nenhum dos dois isolado
//! basta; os três são checados ANTES de qualquer chamada ao clipboard do SO, nunca depois.
//!
//! **Simplificações deliberadas (nomeadas, não escondidas):**
//! - **App sensível em foco bloqueia a leitura** (`isForegroundBlocked`): mesmo gate de
//!   `capture_gate::is_foreground_blocked` que a captura de tela usa, checado DEPOIS do
//!   consentimento e ANTES de tocar o clipboard do SO.
//! - **Análise por LLM mora em outro arquivo.** Este arquivo só publica a imagem deduplicada
//!   (`EventKey::VisionClipboardImage`); quem a analisa e decide se vira fala é
//!   [`super::clipboard_analysis`] (desde 2026-09-22, porte de
//!   `vision-proactive.ts::analyzeClipboardImage` + `ThoughtService::handleProactiveEvent`) —
//!   separação deliberada: este laço é só captura/dedup, o consumidor é quem toca o LLM.
//!
//! Erro de leitura do clipboard (nada copiado, formato != PNG, erro do SO) é **silencioso, no
//! máximo `debug`** — nunca `warn`/`error`: é o estado normal na maior parte do tempo (o usuário
//! raramente tem uma imagem nova a cada 1.5s), e logar isso em `warn` reproduziria a classe de
//! defeito já corrigida neste repositório (VAD logando a mesma ausência 50x/s).

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::image_hash::compute_image_hash;
use crate::config::store::ConfigStore;
use crate::events::{EventBus, EventKey};

/// Espelha `CLIPBOARD_POLL_MS` de `clipboard-watch.ts`.
const CLIPBOARD_POLL_MS: u64 = 1_500;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn get_bool(store: &ConfigStore, path: &str, default: bool) -> bool {
    store.get(path).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// `shouldWatchClipboard` — os TRÊS gates do cabeçalho, todos obrigatórios. Pura: o chamador já
/// resolveu os três booleanos do `ConfigStore`, para ficar testável sem abrir arquivo nenhum.
pub(super) fn should_watch_clipboard(enabled: bool, read_clipboard_images: bool, clipboard_logging_consent: bool) -> bool {
    enabled && read_clipboard_images && clipboard_logging_consent
}

/// `resizeToBitmap`/`readClipboardImage` combinados: RGBA cru do SO (via `tauri::image::Image`) ->
/// PNG em memória -> data URL base64. Mesmo encoder de `image_hash.rs`'s fixtures de teste, agora
/// em caminho de produção — não há decode/resize aqui, só o encode que faltava para a heurística de
/// hash (que já sabe ler qualquer data URL PNG) enxergar o clipboard.
pub(super) fn encode_rgba_as_png_data_url(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    let image = RgbaImage::from_raw(width, height, rgba.to_vec())?;
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
        .ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

/// Impressão digital barata do RGBA cru: dimensões + ~4096 pixels amostrados em passo fixo.
/// Antes, a MESMA imagem parada no clipboard era codificada em PNG (dezenas de MB de RGBA num
/// print 4K, centenas de ms de CPU) e decodificada de novo para o dHash a cada 1,5s, só para
/// descobrir que nada mudou.
pub(super) fn raw_fingerprint(rgba: &[u8], width: u32, height: u32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (width, height, rgba.len()).hash(&mut hasher);
    let pixels = rgba.len() / 4;
    let step = (pixels / 4096).max(1);
    for pixel in (0..pixels).step_by(step) {
        rgba[pixel * 4..pixel * 4 + 4].hash(&mut hasher);
    }
    hasher.finish()
}

/// Leitura bloqueante do clipboard do SO — chamada de dentro de `spawn_blocking` pelo laço.
/// `None` cobre "nada copiado", "não é imagem", "erro do SO" e "mesma imagem da última leitura"
/// ([`raw_fingerprint`] igual a `last_raw`), todos tratados como ausência — nunca um erro que
/// subiria a `warn`.
fn read_clipboard_png_data_url(app: &AppHandle, last_raw: &mut u64) -> Option<String> {
    let image = app.clipboard().read_image().ok()?;
    let fingerprint = raw_fingerprint(image.rgba(), image.width(), image.height());
    if std::mem::replace(last_raw, fingerprint) == fingerprint {
        return None;
    }
    encode_rgba_as_png_data_url(image.rgba(), image.width(), image.height())
}

async fn run_tick(store: &Arc<ConfigStore>, bus: &EventBus, app: &AppHandle, last_hash: &mut String, last_raw: &mut u64) {
    let enabled = get_bool(store, "vision.enabled", false);
    let read_clipboard_images = get_bool(store, "vision.readClipboardImages", false);
    let clipboard_logging_consent = get_bool(store, "privacy.clipboardLogging", false);
    if !should_watch_clipboard(enabled, read_clipboard_images, clipboard_logging_consent) {
        return;
    }
    if super::capture_gate::is_foreground_blocked(store) {
        return;
    }

    let app = app.clone();
    let mut raw = *last_raw;
    let read_result = tokio::task::spawn_blocking(move || (read_clipboard_png_data_url(&app, &mut raw), raw)).await;
    let data_url = match read_result {
        Ok((url, raw)) => {
            *last_raw = raw;
            let Some(url) = url else { return }; // nada de novo no clipboard — estado normal, sem log.
            url
        }
        Err(_) => {
            log::debug!("vision clipboard watch: leitura falhou (spawn_blocking)");
            return;
        }
    };

    let hash = compute_image_hash(&data_url);
    if hash.is_empty() || hash == *last_hash {
        return;
    }
    *last_hash = hash;

    // `.split(',')[1]` do TS: a data URL vira só o payload base64 no evento — o prefixo
    // `data:image/png;base64,` nunca sai daqui.
    let base64_payload = data_url.split_once(',').map(|(_, tail)| tail).unwrap_or(&data_url);
    bus.publish(EventKey::VisionClipboardImage, json!({ "imageBase64": base64_payload, "at": now_ms() }));
}

/// Laço de observação do clipboard do processo inteiro. Mesmo contrato de [`super::watcher::spawn_screen_watcher`]:
/// recebe `ConfigStore`/`EventBus` já resolvidos, nunca constrói o próprio store.
pub fn spawn_clipboard_watch(
    store: Arc<ConfigStore>,
    bus: &'static EventBus,
    app: AppHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut last_hash = String::new();
        let mut last_raw = 0u64;
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = tokio::time::sleep(crate::modules::system::governor::background(Duration::from_millis(CLIPBOARD_POLL_MS))) => {}
            }
            run_tick(&store, bus, &app, &mut last_hash, &mut last_raw).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- raw_fingerprint: a mesma imagem parada não é recodificada a cada tick ---------------------

    #[test]
    fn the_same_clipboard_image_has_the_same_fingerprint() {
        let rgba = vec![7u8; 64 * 64 * 4];
        assert_eq!(raw_fingerprint(&rgba, 64, 64), raw_fingerprint(&rgba.clone(), 64, 64));
    }

    #[test]
    fn a_new_copy_changes_the_fingerprint() {
        let before = vec![7u8; 64 * 64 * 4];
        let mut after = before.clone();
        after[0] = 8;
        assert_ne!(raw_fingerprint(&before, 64, 64), raw_fingerprint(&after, 64, 64));
        assert_ne!(raw_fingerprint(&before, 64, 64), raw_fingerprint(&before, 128, 32), "mesmos bytes, outra forma");
    }

    // -- should_watch_clipboard: os três gates, cada um sozinho não basta ---------------------------

    #[test]
    fn all_three_gates_true_watches() {
        assert!(should_watch_clipboard(true, true, true));
    }

    #[test]
    fn vision_disabled_refuses_even_with_the_other_two_on() {
        assert!(!should_watch_clipboard(false, true, true));
    }

    #[test]
    fn read_clipboard_images_off_refuses_even_with_consent_on() {
        assert!(!should_watch_clipboard(true, false, true));
    }

    /// NEGATIVO central deste arquivo — o gate que o cabeçalho chama de SecOps: sem o consentimento
    /// separado de `privacy.clipboardLogging`, nada lê o clipboard, mesmo com a feature ligada.
    #[test]
    fn missing_privacy_consent_refuses_even_with_the_feature_fully_enabled() {
        assert!(!should_watch_clipboard(true, true, false));
    }

    // -- encode_rgba_as_png_data_url + compute_image_hash: round trip sintético ---------------------

    #[test]
    fn encoding_a_synthetic_rgba_buffer_produces_a_decodable_png_data_url() {
        // 4x4 RGBA sólido — buffer fabricado em memória, nunca um clipboard real.
        let rgba = vec![10u8, 20, 30, 255].repeat(16);
        let data_url = encode_rgba_as_png_data_url(&rgba, 4, 4).expect("encode não deveria falhar");
        assert!(data_url.starts_with("data:image/png;base64,"));

        // A heurística de hash já sabe ler qualquer PNG data URL — prova que o encoder produziu
        // bytes válidos sem duplicar um decoder só para o teste.
        let hash = compute_image_hash(&data_url);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn encoding_with_a_raw_buffer_shorter_than_width_times_height_fails_closed() {
        // `RgbaImage::from_raw` recusa um buffer curto demais — `None`, nunca panic.
        assert!(encode_rgba_as_png_data_url(&[0u8; 4], 4, 4).is_none());
    }

    #[test]
    fn identical_rgba_buffers_hash_identically_two_ticks_in_a_row() {
        let rgba = vec![200u8, 5, 5, 255].repeat(64);
        let first = encode_rgba_as_png_data_url(&rgba, 8, 8).unwrap();
        let second = encode_rgba_as_png_data_url(&rgba, 8, 8).unwrap();
        assert_eq!(compute_image_hash(&first), compute_image_hash(&second));
    }
}
