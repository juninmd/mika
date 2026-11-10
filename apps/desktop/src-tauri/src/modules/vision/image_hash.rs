//! `image-hash.ts` — hash de mudança (dHash). Port 1:1, decisão por decisão:
//!
//! - `HASH_SIZE = 8` (igual). O bitmap pedido é `(HASH_SIZE+1) x HASH_SIZE` = 9x8: a coluna extra
//!   dá o vizinho `x+1` para cada uma das 64 comparações — é um dHash (difference hash) clássico,
//!   não um aHash de média.
//! - `gray(x, y)` no TS soma R+G+B sem dividir por 3 nem ponderar — não é luminância perceptual
//!   aqui (essa conta separada é só do gate de densidade, em `text_density.rs`). Reproduzido bit a
//!   bit: `gray_sum` soma os três canais em `u32` (cabe até 765, nunca estoura).
//! - Cada bit do hash é `gray(x,y) > gray(x+1,y) ? '1' : '0'` — comparação estrita, empate vira
//!   '0'. Uma imagem sólida (sem gradiente nenhum) produz hash TODO ZERO, não hash vazio.
//! - `HASH_CHANGE_THRESHOLD = 0.1` (igual). "Mudou" quando `similarity < 1 - 0.1 = 0.9`, ou seja,
//!   menos de 90% dos 64 bits batem.
//! - Unidade de distância: `hamming_distance` conta POSIÇÕES DE CARACTERE que diferem na string de
//!   bits (um '0'/'1' por bit do hash) — não é uma distância normalizada; a normalização para
//!   `similarity` (dividir por `max_len`) é o próprio TS quem faz, reproduzido igual aqui.
//! - `!previousHash` no TS é `true` tanto para `undefined` quanto para string vazia (JS trata
//!   string vazia como falsy) — os dois casos aqui viram "sem hash anterior" → sempre "mudou"
//!   (primeira observação nunca é comparada com nada).
//! - O dedup de log (`lastHashFailed`, uma flag estática só para não repetir o mesmo warn a cada
//!   tick) foi deixado de fora: é uma preocupação operacional de quem vai possuir o laço de polling
//!   (fase 6), não do algoritmo de comparação em si — misturar log com estado mutável global numa
//!   função pura tornaria este código dependente de ordem de chamada para testar.

use super::bitmap::resize_to_bitmap;

const HASH_SIZE: u32 = 8;
const HASH_WIDTH: u32 = HASH_SIZE + 1;
const HASH_CHANGE_THRESHOLD: f64 = 0.1;

fn gray_sum(bitmap: &[u8], width: u32, x: u32, y: u32) -> u32 {
    let index = ((y * width + x) * 4) as usize;
    let channel = |offset: usize| *bitmap.get(index + offset).unwrap_or(&0) as u32;
    channel(0) + channel(1) + channel(2)
}

/// `computeImageHash` — string de 64 '0'/'1', ou `""` quando a imagem falha ao decodificar.
pub(super) fn compute_image_hash(data_url: &str) -> String {
    let Some(bitmap) = resize_to_bitmap(data_url, HASH_WIDTH, HASH_SIZE) else {
        return String::new();
    };

    let mut bits = String::with_capacity((HASH_SIZE * HASH_SIZE) as usize);
    for y in 0..HASH_SIZE {
        for x in 0..HASH_SIZE {
            let bit = gray_sum(&bitmap, HASH_WIDTH, x, y) > gray_sum(&bitmap, HASH_WIDTH, x + 1, y);
            bits.push(if bit { '1' } else { '0' });
        }
    }
    bits
}

/// `computeImagesHash` — um hash por imagem (multi-monitor), hashes vazios descartados, o resto
/// junto por `:`. Preserva a peculiaridade do TS: o `:` vira só mais um caractere para
/// `hamming_distance` comparar depois, não um separador estrutural.
pub(super) fn compute_images_hash(data_urls: &[&str]) -> String {
    data_urls
        .iter()
        .map(|url| compute_image_hash(url))
        .filter(|hash| !hash.is_empty())
        .collect::<Vec<_>>()
        .join(":")
}

/// Distância de Hamming em POSIÇÕES DE CARACTERE (mesma unidade do laço `for` do TS), mais a
/// diferença de comprimento — cobre o caso "número de monitores mudou" sem tratamento especial.
fn hamming_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let min_len = a.len().min(b.len());
    let differing = (0..min_len).filter(|&i| a[i] != b[i]).count();
    differing + a.len().abs_diff(b.len())
}

/// `hasScreenChanged`. Sem hash anterior (`None`, equivalente a `undefined`/`""` no TS) é sempre
/// "mudou" — não há o que comparar na primeira observação. Hash atual vazio (captura/decode falhou)
/// também é sempre "mudou": fail-open, para nunca travar o laço proativo silenciosamente por causa
/// de uma captura ruim.
pub(super) fn has_screen_changed(current_hash: &str, previous_hash: Option<&str>) -> bool {
    let previous = match previous_hash {
        None => return true,
        Some(hash) if hash.is_empty() => return true,
        Some(hash) => hash,
    };
    if current_hash.is_empty() {
        return true;
    }

    let distance = hamming_distance(current_hash, previous);
    let max_len = current_hash.len().max(previous.len());
    let similarity = if max_len > 0 { 1.0 - (distance as f64 / max_len as f64) } else { 0.0 };

    similarity < 1.0 - HASH_CHANGE_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use image::{ExtendedColorType, ImageEncoder, RgbaImage};

    /// Fixture sintética — nunca captura tela de verdade. Gera um PNG em memória a partir de uma
    /// função de pixel e devolve a data URL que `resize_to_bitmap` (via `images::resize_bitmap`)
    /// consome, do mesmo jeito que uma captura real chegaria.
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

    // -- compute_image_hash ---------------------------------------------------------------------

    #[test]
    fn hash_of_empty_data_url_is_empty_string() {
        assert_eq!(compute_image_hash(""), "");
    }

    #[test]
    fn hash_of_malformed_data_url_is_empty_string_not_a_panic() {
        assert_eq!(compute_image_hash("not a data url"), "");
        assert_eq!(compute_image_hash("data:image/png;base64,"), "");
    }

    #[test]
    fn hash_of_solid_black_image_is_all_zero_bits() {
        // Sem gradiente nenhum: gray(x,y) == gray(x+1,y) sempre, e '>' estrito nunca é verdadeiro.
        let hash = compute_image_hash(&solid(32, 32, [0, 0, 0, 255]));
        assert_eq!(hash, "0".repeat(64));
    }

    #[test]
    fn hash_of_solid_white_image_is_also_all_zero_bits() {
        // Mesma razão do preto: "sólida" importa, não a cor — dHash mede GRADIENTE, não brilho.
        let hash = compute_image_hash(&solid(32, 32, [255, 255, 255, 255]));
        assert_eq!(hash, "0".repeat(64));
    }

    #[test]
    fn hash_has_64_bits_for_a_valid_image() {
        assert_eq!(compute_image_hash(&solid(16, 16, [10, 20, 30, 255])).len(), 64);
    }

    #[test]
    fn hash_is_deterministic_for_identical_input() {
        let data_url = solid(20, 20, [90, 40, 200, 255]);
        assert_eq!(compute_image_hash(&data_url), compute_image_hash(&data_url));
    }

    /// "Dimensão zero": a fonte mais degenerada que ainda é uma imagem válida (1x1) tem de
    /// redimensionar para 9x8 sem panic nem divisão por zero, e produzir um hash bem definido
    /// (uniforme, já que um único pixel esticado não tem gradiente).
    fn tiny_one_by_one_image_upscales_without_panicking() -> String {
        solid(1, 1, [128, 64, 32, 255])
    }

    #[test]
    fn hash_of_one_by_one_source_image_is_well_defined() {
        let hash = compute_image_hash(&tiny_one_by_one_image_upscales_without_panicking());
        assert_eq!(hash, "0".repeat(64));
    }

    #[test]
    fn images_hash_joins_non_empty_hashes_and_drops_empty_ones() {
        let a = solid(16, 16, [1, 1, 1, 255]);
        let joined = compute_images_hash(&[&a, "", "not a data url"]);
        assert_eq!(joined, compute_image_hash(&a));
    }

    #[test]
    fn images_hash_of_two_screens_joins_with_colon() {
        let a = solid(16, 16, [1, 1, 1, 255]);
        let b = solid(16, 16, [2, 2, 2, 255]);
        let joined = compute_images_hash(&[&a, &b]);
        assert_eq!(joined, format!("{}:{}", compute_image_hash(&a), compute_image_hash(&b)));
    }

    // -- hamming_distance -------------------------------------------------------------------------

    #[test]
    fn hamming_distance_of_identical_strings_is_zero() {
        assert_eq!(hamming_distance("00110011", "00110011"), 0);
    }

    #[test]
    fn hamming_distance_counts_differing_positions() {
        assert_eq!(hamming_distance("0000", "0001"), 1);
        assert_eq!(hamming_distance("0000", "1111"), 4);
    }

    #[test]
    fn hamming_distance_adds_length_difference() {
        // "monitor a mais" muda o comprimento do hash combinado — cada caractere extra soma 1,
        // mesmo sem nenhuma posição comum divergir.
        assert_eq!(hamming_distance("0000", "0000:1111"), 5);
    }

    // -- has_screen_changed -------------------------------------------------------------------------

    #[test]
    fn first_observation_with_no_previous_hash_always_counts_as_changed() {
        assert!(has_screen_changed("0".repeat(64).as_str(), None));
    }

    #[test]
    fn empty_previous_hash_is_treated_like_no_previous_hash() {
        assert!(has_screen_changed("0".repeat(64).as_str(), Some("")));
    }

    #[test]
    fn empty_current_hash_fails_open_as_changed() {
        assert!(has_screen_changed("", Some("0".repeat(64).as_str())));
    }

    #[test]
    fn identical_hash_is_not_a_change() {
        let hash = "1010".repeat(16);
        assert!(!has_screen_changed(&hash, Some(&hash)));
    }

    /// Limiar exato do TS: `similarity < 0.9`. Com hash de 64 bits, 6/64 bits diferentes ainda dá
    /// `similarity = 58/64 = 0.90625 >= 0.9` — abaixo do gatilho, considerado "não mudou".
    #[test]
    fn six_of_sixty_four_bits_differing_stays_under_the_change_threshold() {
        let previous = "0".repeat(64);
        let mut current: Vec<char> = previous.chars().collect();
        for bit in current.iter_mut().take(6) {
            *bit = '1';
        }
        let current: String = current.into_iter().collect();
        assert!(!has_screen_changed(&current, Some(&previous)));
    }

    /// Um bit a mais (7/64) já cruza o limiar: `similarity = 57/64 = 0.890625 < 0.9`.
    #[test]
    fn seven_of_sixty_four_bits_differing_crosses_the_change_threshold() {
        let previous = "0".repeat(64);
        let mut current: Vec<char> = previous.chars().collect();
        for bit in current.iter_mut().take(7) {
            *bit = '1';
        }
        let current: String = current.into_iter().collect();
        assert!(has_screen_changed(&current, Some(&previous)));
    }
}
