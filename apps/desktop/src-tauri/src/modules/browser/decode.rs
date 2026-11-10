//! Decodifica o corpo de uma página no charset que ela declara. `String::from_utf8_lossy` virava
//! `�` em toda página Shift-JIS/EUC-KR/GBK/Windows-1252 — o LLM lia lixo no lugar do texto.
//!
//! Ordem do HTML Living Standard: BOM, `charset` do `Content-Type`, `<meta charset>` /
//! `<meta http-equiv="Content-Type" content="…; charset=…">` nos primeiros bytes, UTF-8.

use std::sync::LazyLock;

use encoding_rs::Encoding;
use regex::Regex;

/// Bytes inspecionados atrás do `<meta>`: a spec manda 1024.
const META_SNIFF_BYTES: usize = 1024;

static CHARSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)charset\s*=\s*["']?\s*([A-Za-z0-9_:.\-]+)"#).expect("padrão de charset inválido"));

fn label_from(text: &str) -> Option<&'static Encoding> {
    let label = CHARSET.captures(text)?.get(1)?.as_str();
    Encoding::for_label(label.as_bytes())
}

/// `content_type` é o cabeçalho cru da resposta, se veio.
pub(super) fn decode_html(body: &[u8], content_type: Option<&str>) -> String {
    let declared = content_type.and_then(label_from).or_else(|| {
        // O trecho é só para achar o `<meta>`: ASCII basta, o resto vira `�` sem importar.
        let head = String::from_utf8_lossy(&body[..body.len().min(META_SNIFF_BYTES)]).to_ascii_lowercase();
        head.find("<meta").and_then(|at| label_from(&head[at..]))
    });
    // `decode` ainda dá precedência ao BOM sobre o charset declarado.
    declared.unwrap_or(encoding_rs::UTF_8).decode(body).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "日本語のページ";

    #[test]
    fn shift_jis_is_decoded_from_the_content_type_header() {
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(TEXT);
        let html = decode_html(&bytes, Some("text/html; charset=Shift_JIS"));
        assert_eq!(html, TEXT);
        assert!(String::from_utf8_lossy(&bytes).contains('\u{FFFD}'), "o caso só vale se UTF-8 lossy realmente quebra");
    }

    #[test]
    fn euc_kr_is_decoded_from_the_meta_tag_when_the_header_has_no_charset() {
        let page = "<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=euc-kr\"></head><body>한국어 페이지</body></html>";
        let (bytes, _, _) = encoding_rs::EUC_KR.encode(page);
        assert!(decode_html(&bytes, Some("text/html")).contains("한국어 페이지"));
        assert!(decode_html(&bytes, None).contains("한국어 페이지"));
    }

    #[test]
    fn meta_charset_shorthand_is_honoured() {
        let page = format!("<meta charset=\"gbk\"><p>{}</p>", "中文网页");
        let (bytes, _, _) = encoding_rs::GBK.encode(&page);
        assert!(decode_html(&bytes, None).contains("中文网页"));
    }

    #[test]
    fn the_header_wins_over_the_meta_tag() {
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode("<meta charset=\"utf-8\">日本語");
        assert!(decode_html(&bytes, Some("text/html; charset=shift_jis")).contains("日本語"));
    }

    #[test]
    fn utf8_and_unknown_labels_fall_back_to_utf8() {
        assert_eq!(decode_html("olá, 世界".as_bytes(), None), "olá, 世界");
        assert_eq!(decode_html("olá".as_bytes(), Some("text/html; charset=nao-existe")), "olá");
    }
}
