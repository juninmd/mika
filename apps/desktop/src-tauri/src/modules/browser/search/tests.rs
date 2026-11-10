use super::*;

// Fixtures FABRICADAS à mão — nunca uma página real capturada — mas com as classes/estrutura
// confirmadas contra uma captura real do endpoint nesta rodada (ver o relatório da tarefa).

const DDG_FIXTURE: &str = r#"<html><body>
<div class="results">
  <div class="result results_links results_links_deep result--ad ">
    <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fads.example.com%2F&amp;rut=x">Sponsored</a>
    <a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fads.example.com%2F&amp;rut=x">Buy now</a>
  </div>
  <div class="result results_links results_links_deep web-result ">
    <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F&amp;rut=abc">Rust Programming Language</a>
    <a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F&amp;rut=abc">A language empowering everyone.</a>
  </div>
</div>
</body></html>"#;

const YAHOO_FIXTURE: &str = r#"<html><body>
<div id="web">
<div class="dd fst algo algo-sr relsrch richAlgo">
  <div class="compTitle"><a href="https://r.search.yahoo.com/_ylt=x;_ylu=y/RV=2/RE=1/RU=https%3a%2f%2frust-lang.org%2f/RK=2/RS=zzz">
    <h3 class="title"><span>Rust Programming Language</span></h3>
  </a></div>
  <div class="compText"><p>Rust is a fast, reliable, and productive programming language.</p></div>
</div>
</div>
</body></html>"#;

// Payload real de `u=` é "a1" + base64(URL-safe, sem padding) da URL de destino — construído aqui
// com o mesmo `decode_base64_loosely` que a produção usa, não copiado de captura (ver cabeçalho).
fn bing_redirect_href(target: &str) -> String {
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(target);
    format!("https://www.bing.com/ck/a?ntb=1&u=a1{encoded}&p=abcd")
}

fn bing_fixture(target: &str) -> String {
    format!(
        r#"<html><body>
<ol id="b_results">
  <li class="b_ad">
    <h2><a href="https://www.bing.com/aclick?ad=1">Sponsored</a></h2>
    <div class="b_caption"><p>Buy now</p></div>
  </li>
  <li class="b_algo">
    <h2><a href="{}">Rust Programming Language</a></h2>
    <div class="b_caption"><p>A language empowering everyone.</p></div>
  </li>
</ol>
</body></html>"#,
        bing_redirect_href(target)
    )
}

/// Página real do desafio "anomaly" do DDG é fabricada (nunca uma captura), com as duas marcas que
/// `looks_like_duckduckgo_anti_bot` reconhece: HTTP 202 e o texto "anomaly" no corpo.
const DDG_ANTI_BOT_FIXTURE: &str = r#"<html><body>
<div id="anomaly-modal"><p>We detected unusual traffic (anomaly) from your computer network.</p></div>
</body></html>"#;

#[test]
fn search_request_url_picks_the_no_js_endpoint_per_engine() {
    assert_eq!(search_request_url("rust lang", "duckduckgo", "pt-BR"), "https://html.duckduckgo.com/html/?q=rust%20lang&kl=br-pt");
    assert_eq!(search_request_url("rust lang", "yahoo", "pt-BR"), "https://search.yahoo.com/search?p=rust%20lang");
    assert_eq!(search_request_url("rust lang", "bing", "pt-BR"), "https://www.bing.com/search?q=rust%20lang&setlang=pt-BR");
}

#[test]
fn search_request_url_follows_the_interface_language() {
    assert_eq!(search_request_url("x", "duckduckgo", "en-US"), "https://html.duckduckgo.com/html/?q=x&kl=us-en");
    assert_eq!(search_request_url("x", "bing", "ja-JP"), "https://www.bing.com/search?q=x&setlang=ja-JP");
}

/// Locale sem região própria caía em `br-pt`: a busca de um usuário coreano vinha em português.
#[test]
fn every_supported_locale_has_its_own_duckduckgo_region() {
    for (locale, region) in [
        ("en-US", "us-en"), ("pt-BR", "br-pt"), ("es-ES", "es-es"), ("fr-FR", "fr-fr"), ("it-IT", "it-it"),
        ("ja-JP", "jp-jp"), ("zh-CN", "cn-zh"), ("ko-KR", "kr-kr"), ("ru-RU", "ru-ru"), ("de-DE", "de-de"),
    ] {
        assert_eq!(duckduckgo_region(locale), region, "{locale}");
    }
}

#[test]
fn extract_duckduckgo_skips_ads_and_decodes_the_redirect_wrapper() {
    let results = extract_results(DDG_FIXTURE, "duckduckgo");

    assert_eq!(results.len(), 1, "o anúncio (result--ad, sem web-result) não deveria aparecer");
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://rust-lang.org/");
    assert!(results[0].snippet.contains("empowering everyone"));
}

#[test]
fn extract_bing_skips_ads_and_decodes_the_ck_a_redirect() {
    let fixture = bing_fixture("https://rust-lang.org/");
    let results = extract_results(&fixture, "bing");

    assert_eq!(results.len(), 1, "o anúncio (b_ad, sem b_algo) não deveria aparecer");
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://rust-lang.org/");
    assert!(results[0].snippet.contains("empowering everyone"));
}

#[test]
fn unwrap_bing_redirect_passes_through_a_direct_organic_link() {
    assert_eq!(unwrap_bing_redirect("https://rust-lang.org/"), Some("https://rust-lang.org/".to_string()));
    assert_eq!(unwrap_bing_redirect("javascript:alert(1)"), None);
}

#[test]
fn unwrap_bing_redirect_rejects_a_ck_a_link_whose_payload_decodes_to_a_non_http_target() {
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("javascript:alert(1)");
    let href = format!("https://www.bing.com/ck/a?u=a1{encoded}");
    assert_eq!(unwrap_bing_redirect(&href), None);
}

#[test]
fn looks_like_duckduckgo_anti_bot_flags_status_202_or_the_anomaly_marker() {
    assert!(looks_like_duckduckgo_anti_bot(202, "<html></html>"), "202 sozinho já é o sinal");
    assert!(looks_like_duckduckgo_anti_bot(200, DDG_ANTI_BOT_FIXTURE), "marcador 'anomaly' no corpo, mesmo em 200");
    assert!(!looks_like_duckduckgo_anti_bot(200, DDG_FIXTURE), "página real de resultados não é anti-bot");
}

/// Buscar "anomaly detection" devolve uma página de resultados NORMAL que contém a palavra
/// "anomaly" (no título, no snippet e no campo de busca) — não é desafio anti-bot.
#[test]
fn looks_like_duckduckgo_anti_bot_ignores_the_word_anomaly_in_real_results() {
    let page = r#"<html><body><input name="q" value="anomaly detection">
<div class="web-result"><a class="result__a" href="https://example.com">Anomaly detection - Wikipedia</a></div></body></html>"#;
    assert!(!looks_like_duckduckgo_anti_bot(200, page));
}

/// A política em si (nunca `Ok(vec![])` quando a cadeia inteira falhou/veio vazia) — sem fetcher,
/// sem rede: alimenta `pick_first_non_empty` com resultados JÁ OBTIDOS, na ordem da cadeia.
mod fallback_chain_policy {
    use super::*;

    fn hit(title: &str) -> SearchResult {
        SearchResult { title: title.to_string(), url: format!("https://example.com/{title}"), snippet: String::new() }
    }

    #[test]
    fn returns_the_first_non_empty_attempt() {
        let result = pick_first_non_empty(vec![Ok(vec![]), Ok(vec![hit("bing")]), Ok(vec![hit("never reached")])]);
        assert_eq!(result.unwrap()[0].title, "bing");
    }

    #[test]
    fn skips_empty_and_errored_attempts_to_find_one_later_in_the_chain() {
        let result = pick_first_non_empty(vec![
            Err(SearchError::BlockedByEngine { engine: "bing" }),
            Ok(vec![]),
            Ok(vec![hit("yahoo")]),
        ]);
        assert_eq!(result.unwrap()[0].title, "yahoo");
    }

    #[test]
    fn surfaces_the_last_error_when_every_attempt_failed() {
        let result = pick_first_non_empty(vec![
            Err(SearchError::BlockedByEngine { engine: "duckduckgo" }),
            Err(SearchError::AllEnginesFailed),
        ]);
        assert_eq!(result, Err(SearchError::AllEnginesFailed));
    }

    /// O caso central da tarefa: nenhum motor deu erro, todos só vieram vazios — ainda assim tem
    /// de ser um erro explícito, nunca `Ok(vec![])` disfarçando "ninguém respondeu direito".
    #[test]
    fn is_an_explicit_error_when_every_engine_was_merely_empty_with_no_error_at_all() {
        let result: Result<Vec<SearchResult>, SearchError> = pick_first_non_empty(vec![Ok(vec![]), Ok(vec![]), Ok(vec![])]);
        assert_eq!(result, Err(SearchError::AllEnginesFailed));
    }
}

#[test]
fn extract_yahoo_decodes_the_path_wrapped_redirect() {
    let results = extract_results(YAHOO_FIXTURE, "yahoo");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://rust-lang.org/");
    assert!(results[0].snippet.contains("fast, reliable"));
}

#[test]
fn extract_results_is_empty_not_panicking_when_markup_does_not_match() {
    assert!(extract_results("<html><body><p>nothing here</p></body></html>", "duckduckgo").is_empty());
    assert!(extract_results("<html><body><p>nothing here</p></body></html>", "yahoo").is_empty());
}

#[test]
fn extract_results_never_exceeds_the_extract_limit() {
    let mut html = String::from("<html><body>");
    for i in 0..(EXTRACT_LIMIT + 2) {
        html.push_str(&format!(
            r#"<div class="result web-result">
                 <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F{i}">Result {i}</a>
                 <a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F{i}">snippet {i}</a>
               </div>"#
        ));
    }
    html.push_str("</body></html>");

    assert_eq!(extract_results(&html, "duckduckgo").len(), EXTRACT_LIMIT);
}

#[test]
fn unwrap_duckduckgo_redirect_rejects_non_http_targets() {
    assert_eq!(unwrap_duckduckgo_redirect("//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F"), Some("https://rust-lang.org/".to_string()));
    assert_eq!(unwrap_duckduckgo_redirect("//duckduckgo.com/l/?uddg=javascript%3Aalert(1)"), None);
    assert_eq!(unwrap_duckduckgo_redirect("//duckduckgo.com/l/?rut=abc"), None, "sem uddg não tem alvo");
}

#[test]
fn unwrap_yahoo_redirect_rejects_non_http_targets() {
    assert_eq!(
        unwrap_yahoo_redirect("https://r.search.yahoo.com/RU=https%3a%2f%2frust-lang.org%2f/RK=2/RS=zzz"),
        Some("https://rust-lang.org/".to_string())
    );
    assert_eq!(unwrap_yahoo_redirect("https://r.search.yahoo.com/RU=javascript%3aalert(1)/RK=2"), None);
    assert_eq!(unwrap_yahoo_redirect("https://r.search.yahoo.com/RK=2/RS=zzz"), None, "sem segmento RU= não tem alvo");
}

#[test]
fn percent_decode_matches_percent_encoded_bytes() {
    assert_eq!(percent_decode("https%3a%2f%2frust-lang.org%2f"), "https://rust-lang.org/");
    assert_eq!(percent_decode("no-escapes"), "no-escapes");
    assert_eq!(percent_decode("trailing%"), "trailing%", "% truncado no fim não deve estourar índice");
}

/// O href vem da página do buscador: `%` seguido de multibyte caía no meio de um char e panicava.
#[test]
fn percent_decode_survives_multibyte_text_after_a_percent_sign() {
    assert_eq!(percent_decode("100%é"), "100%é");
    assert_eq!(percent_decode("%aé"), "%aé");
    assert_eq!(percent_decode("%é%41"), "%éA");
    assert_eq!(percent_decode("%+4"), "%+4", "sinal não é dígito hexadecimal");
}

#[test]
fn pick_snippet_falls_back_to_title_and_caps_at_280_chars() {
    assert_eq!(pick_snippet("", "Title"), "Title");
    assert_eq!(pick_snippet("Title", "Title"), "Title", "sobra vazio depois de remover o título -> cai pro título");

    let long = "a".repeat(400);
    assert_eq!(pick_snippet(&long, "Title").chars().count(), 280);
}

#[test]
fn snippet_is_labeled_as_untrusted_web_content() {
    // `search()` em si não é testável sem rede real (proibido); pinamos que o `snippet` é rotulado
    // com o mesmo marcador de `browser.service.ts` lendo o próprio fonte de produção.
    let source = include_str!("../search.rs");
    let production = source.split("#[cfg(test)]").next().unwrap_or_default();
    assert!(production.contains(r#"label_untrusted_content("BUSCA_WEB", &item.snippet)"#));
}

/// A extração tem de alcançar o `maximum` que o schema da tool `web_search` anuncia (10): com 5,
/// um `limit: 10` pedido pelo modelo recebia 5 em silêncio.
#[test]
fn extract_limit_reaches_the_maximum_the_search_tool_advertises() {
    assert_eq!(EXTRACT_LIMIT, 10);
}
