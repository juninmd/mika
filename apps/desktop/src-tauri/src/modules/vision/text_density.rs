//! `text-density-gate.ts` — gate de densidade de texto/UI. Port 1:1, decisão por decisão:
//!
//! - `GRID_SIZE = 64`, `BLOCK_SIZE = 8`, `BLOCKS_PER_SIDE = 8` (todos iguais). O bitmap pedido é
//!   64x64 — diferente do hash em `image_hash.rs` (9x8): densidade de texto precisa de resolução
//!   para enxergar blocos, o dHash só precisa de 64 comparações de vizinho.
//! - `luminance` é BT.601 (`0.299R + 0.587G + 0.114B`), igual ao TS — luminância PERCEPTUAL, ao
//!   contrário da soma crua do hash (as duas heurísticas medem coisas diferentes de propósito: uma
//!   é "mudou a imagem", a outra é "isto parece texto").
//! - `BLOCK_VARIANCE_THRESHOLD = 18` (igual, escala 0-255). Um bloco "tem borda" quando
//!   `max - min >= 18` dentro dele — comparação inclusiva, reproduzida com `>=`.
//! - `estimateTextDensity` retorna `edgeBlocks / (BLOCKS_PER_SIDE * BLOCKS_PER_SIDE)` — fração
//!   0.0-1.0, `0.0` para imagem vazia (fail-soft, nunca erro).
//! - `hasEnoughTextDensity([], threshold)` é `true` para lista vazia (vácuo: sem imagem nenhuma
//!   para desqualificar, o gate não barra). Com imagens, é `true` se QUALQUER UMA bate o limiar —
//!   aqui via `.any()`, que já para na primeira que bate; o TS calcula todas via `Promise.all`
//!   antes de checar `.some()`, mas o resultado booleano é idêntico — só a ordem de computação
//!   muda, e nenhuma das duas tem efeito colateral observável.

use super::bitmap::resize_to_bitmap;

const TEXT_DENSITY_GRID_SIZE: u32 = 64;
const TEXT_DENSITY_BLOCK_SIZE: u32 = 8;
const TEXT_DENSITY_BLOCKS_PER_SIDE: u32 = TEXT_DENSITY_GRID_SIZE / TEXT_DENSITY_BLOCK_SIZE;
const TEXT_DENSITY_BLOCK_VARIANCE_THRESHOLD: f64 = 18.0;

fn luminance(bitmap: &[u8], index: usize) -> f64 {
    let channel = |offset: usize| *bitmap.get(index + offset).unwrap_or(&0) as f64;
    0.299 * channel(0) + 0.587 * channel(1) + 0.114 * channel(2)
}

fn block_has_edge(bitmap: &[u8], width: u32, block_x: u32, block_y: u32) -> bool {
    let mut min = 255.0_f64;
    let mut max = 0.0_f64;
    for y in 0..TEXT_DENSITY_BLOCK_SIZE {
        for x in 0..TEXT_DENSITY_BLOCK_SIZE {
            let px = block_x * TEXT_DENSITY_BLOCK_SIZE + x;
            let py = block_y * TEXT_DENSITY_BLOCK_SIZE + y;
            let value = luminance(bitmap, ((py * width + px) * 4) as usize);
            min = min.min(value);
            max = max.max(value);
        }
    }
    max - min >= TEXT_DENSITY_BLOCK_VARIANCE_THRESHOLD
}

/// `estimateTextDensity` — 0.0 (tela lisa/sem UI perceptível) a 1.0 (tela densa de texto/UI).
pub(super) fn estimate_text_density(data_url: &str) -> f64 {
    let Some(bitmap) = resize_to_bitmap(data_url, TEXT_DENSITY_GRID_SIZE, TEXT_DENSITY_GRID_SIZE) else {
        return 0.0;
    };

    let mut edge_blocks = 0u32;
    for by in 0..TEXT_DENSITY_BLOCKS_PER_SIDE {
        for bx in 0..TEXT_DENSITY_BLOCKS_PER_SIDE {
            if block_has_edge(&bitmap, TEXT_DENSITY_GRID_SIZE, bx, by) {
                edge_blocks += 1;
            }
        }
    }
    edge_blocks as f64 / (TEXT_DENSITY_BLOCKS_PER_SIDE * TEXT_DENSITY_BLOCKS_PER_SIDE) as f64
}

/// `hasEnoughTextDensity`.
pub(super) fn has_enough_text_density(data_urls: &[&str], threshold: f64) -> bool {
    if data_urls.is_empty() {
        return true;
    }
    data_urls.iter().any(|url| estimate_text_density(url) >= threshold)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use image::{ExtendedColorType, ImageEncoder, RgbaImage};

    /// Mesma fixture sintética de `image_hash.rs` — duplicada de propósito (é curta e o teste que
    /// a usa é o único dono do formato que precisa), em vez de um módulo de fixtures compartilhado
    /// entre arquivos só para 15 linhas.
    fn png_data_url(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> String {
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(x, y, image::Rgba(pixel(x, y)));
            }
        }
        let mut buffer = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buffer)
            .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
            .expect("encode fixture PNG");
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&buffer)
        )
    }

    fn solid(width: u32, height: u32, rgba: [u8; 4]) -> String {
        png_data_url(width, height, |_, _| rgba)
    }

    fn tiny_one_by_one_image_upscales_without_panicking() -> String {
        solid(1, 1, [128, 64, 32, 255])
    }

    // -- luminance / block_has_edge (bitmap sintético direto, sem passar por PNG/resize) --------

    fn flat_block(value: u8) -> Vec<u8> {
        // 8x8 RGBA uniforme, cinza puro (R=G=B) — luminância BT.601 de um pixel assim é
        // exatamente `value`, já que os pesos somam 1.0. `width` == 8 nestes testes, um bloco
        // isolado. `repeat().take()` em vez de `repeat_n` (estabilizada só em 1.82) para não furar
        // o MSRV 1.77 declarado no Cargo.toml.
        std::iter::repeat([value, value, value, 255]).take(64).flatten().collect()
    }

    /// Ajusta os três canais de cor (R,G,B) do pixel `index` para `value`, mantendo cinza puro —
    /// assim a diferença de luminância contra o resto do bloco é EXATAMENTE `value - base`, sem
    /// precisar recalcular a ponderação BT.601 a mão em cada teste de limiar.
    fn set_gray_pixel(bitmap: &mut [u8], index: usize, value: u8) {
        let offset = index * 4;
        bitmap[offset] = value;
        bitmap[offset + 1] = value;
        bitmap[offset + 2] = value;
    }

    #[test]
    fn luminance_matches_bt601_weights() {
        // R puro: 0.299 * 255 ≈ 76.245.
        let bitmap = [255u8, 0, 0, 255];
        assert!((luminance(&bitmap, 0) - 76.245).abs() < 1e-9);
    }

    #[test]
    fn flat_block_has_no_edge() {
        assert!(!block_has_edge(&flat_block(120), 8, 0, 0));
    }

    #[test]
    fn block_variance_exactly_at_threshold_counts_as_edge() {
        // Base preta (luminância 0.0 exata) em vez de 100: `0.299*V + 0.587*V + 0.114*V` só cai
        // EXATAMENTE em V para V ∈ {0, 17, 18, 19} em f64 — testado com `python3 -c` antes de
        // escrever isto. Uma base de 100 com delta de 18 vira 17.999999999999986 por erro de
        // arredondamento (`0.299+0.587+0.114 != 1.0` bit a bit) e reprovaria por um triliardo, não
        // pela lógica — mesma fragilidade que já existe no `luminance()` original do TS.
        let mut bitmap = flat_block(0);
        set_gray_pixel(&mut bitmap, 0, 18);
        assert!(block_has_edge(&bitmap, 8, 0, 0));
    }

    #[test]
    fn block_variance_just_under_threshold_is_not_an_edge() {
        let mut bitmap = flat_block(0);
        set_gray_pixel(&mut bitmap, 0, 17);
        assert!(!block_has_edge(&bitmap, 8, 0, 0));
    }

    // -- estimate_text_density / has_enough_text_density (integração via PNG sintético) ---------

    #[test]
    fn text_density_of_empty_data_url_is_zero() {
        assert_eq!(estimate_text_density(""), 0.0);
    }

    #[test]
    fn text_density_of_flat_image_is_zero() {
        assert_eq!(estimate_text_density(&solid(64, 64, [50, 50, 50, 255])), 0.0);
    }

    #[test]
    fn text_density_of_one_by_one_source_image_is_zero_not_a_panic() {
        // Mesmo espírito do teste de hash: fonte degenerada não pode travar o gate.
        assert_eq!(estimate_text_density(&tiny_one_by_one_image_upscales_without_panicking()), 0.0);
    }

    #[test]
    fn text_density_of_fine_stripes_is_maximal() {
        // Listras de 1px alternando preto/branco: toda coluna de todo bloco 8x8 tem
        // max-min = 255 >= 18 — os 64 blocos viram borda, densidade = 1.0.
        let stripes = png_data_url(64, 64, |x, _| if x % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
        assert_eq!(estimate_text_density(&stripes), 1.0);
    }

    #[test]
    fn has_enough_text_density_of_empty_list_is_vacuously_true() {
        assert!(has_enough_text_density(&[], 0.5));
    }

    #[test]
    fn has_enough_text_density_is_true_when_any_image_meets_the_threshold() {
        let flat = solid(64, 64, [10, 10, 10, 255]);
        let stripes = png_data_url(64, 64, |x, _| if x % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
        assert!(has_enough_text_density(&[&flat, &stripes], 0.5));
    }

    #[test]
    fn has_enough_text_density_is_false_when_no_image_meets_the_threshold() {
        let flat = solid(64, 64, [10, 10, 10, 255]);
        assert!(!has_enough_text_density(&[&flat], 0.5));
    }
}
