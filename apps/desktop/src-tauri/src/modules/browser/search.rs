//! search.rs — busca real, sem motor JS: Bing via `www.bing.com/search`, DuckDuckGo via
//! `html.duckduckgo.com/html/` (endpoint server-renderizado oficial do DDG) e Yahoo via
//! `search.yahoo.com/search`, os três atravessando o mesmo pipeline `guarded_fetch::guarded_get`
//! (SSRF + modo offline revalidados a cada hop).
//!
//! ## Motor "auto" — cadeia de fallback, nunca sucesso com `[]`
//!
//! DuckDuckGo devolve HTTP 202 com uma página anti-bot ("anomaly") pra este IP com frequência
//! suficiente para não ser confiável sozinho, e o Yahoo já respondeu 500 na mesma sessão de
//! diagnóstico (ver o relatório da tarefa). `engine == "auto"` (o default de
//! `browser.searchEngine`) tenta Bing, depois DuckDuckGo, depois Yahoo, nesta ordem, e devolve o
//! primeiro conjunto de resultados NÃO VAZIO. Se todos falharem ou vierem vazios,
//! [`SearchError::AllEnginesFailed`] — nunca `Ok(vec![])` disfarçando um motor bloqueado de "a
//! busca não achou nada". `pick_first_non_empty` (função pura, sem rede) é onde essa política vive
//! e onde ela é testada.
//!
//! ## `duckduckgo.com/?q=` (a URL de [`super::build_search_url`]) NÃO serve pra isto
//!
//! [`super::build_search_url`] já porta `buildSearchUrl` do TS byte a byte, mas aquela URL é a SPA
//! (`duckduckgo.com/?q=...&ia=web`) — só renderiza resultado via JS, e sem Chromium devolve uma
//! casca vazia. Confirmado com `curl` real nesta rodada (ver o relatório da tarefa):
//! `https://html.duckduckgo.com/html/?q=...` devolve os MESMOS resultados já renderizados em HTML
//! — é o endpoint sem-JS oficial do DDG, usado por praticamente todo scraper server-side da
//! comunidade. Por isso este arquivo NÃO reusa `build_search_url`: constrói sua própria URL de
//! requisição, pro endpoint irmão que existe pra ser lido por HTTP puro.
//!
//! ## Yahoo: bounce de verificação confirmado com captura real
//!
//! `search.yahoo.com/search?p=...` primeiro devolve um 307 same-origin pra `/_bv/v.gif?...` (seta
//! um cookie), que redireciona de volta pra URL original — só ENTÃO vem o HTML real. Confirmado com
//! `curl -c/-b` (cookie jar) nesta rodada, não suposição. `guarded_fetch::guarded_get` já segue
//! redirect hop a hop revalidando cada um, e `guarded_fetch::guarded_http_client` liga
//! `cookie_store(true)` — o MESMO pipeline atravessa o bounce sem nenhum código específico de
//! engine.

use scraper::{ElementRef, Html, Selector};

use crate::config::store::ConfigStore;

use super::guarded_fetch::{self, GuardedFetchError};
use super::{clean_text, encode_uri_component, label_untrusted_content};

/// Teto de EXTRAÇÃO = o máximo que a tool `web_search` aceita em `limit` (`tools/web.rs`); o
/// `limit` pedido (default 5) corta depois. Era 5, e a tool prometia até 10 sem nunca entregar.
const EXTRACT_LIMIT: usize = 10;

const BING_SEARCH_ENDPOINT: &str = "https://www.bing.com/search";
const DUCKDUCKGO_HTML_ENDPOINT: &str = "https://html.duckduckgo.com/html/";
const YAHOO_SEARCH_ENDPOINT: &str = "https://search.yahoo.com/search";

/// Valor de `browser.searchEngine` que dispara [`search_fallback_chain`] em vez de um único motor.
pub const AUTO_ENGINE: &str = "auto";
/// Ordem da cadeia de fallback do motor "auto" — ver a nota no cabeçalho do módulo.
const ENGINE_FALLBACK_CHAIN: [&str; 3] = ["bing", "duckduckgo", "yahoo"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Erro honesto de `search()` — distinto de [`GuardedFetchError`] porque "motor bloqueou com uma
/// página anti-bot" e "a cadeia inteira falhou ou não achou nada" não são falhas de transporte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// Borbulhado do pipeline SSRF-guardado (SSRF, offline, status ruim, timeout, rede).
    Fetch(GuardedFetchError),
    /// O motor respondeu, mas com uma página de desafio anti-bot em vez de resultados.
    BlockedByEngine { engine: &'static str },
    /// Todo motor da cadeia "auto" falhou ou devolveu zero resultados — nunca vira `Ok(vec![])`.
    AllEnginesFailed,
}

impl From<GuardedFetchError> for SearchError {
    fn from(error: GuardedFetchError) -> Self {
        SearchError::Fetch(error)
    }
}

/// Região do DuckDuckGo (`kl`) para o `app.locale` da interface.
fn duckduckgo_region(locale: &str) -> &'static str {
    match locale {
        "en-US" => "us-en",
        "es-ES" => "es-es",
        "fr-FR" => "fr-fr",
        "it-IT" => "it-it",
        "ja-JP" => "jp-jp",
        "zh-CN" => "cn-zh",
        "ko-KR" => "kr-kr",
        "ru-RU" => "ru-ru",
        "de-DE" => "de-de",
        _ => "br-pt",
    }
}

/// Idioma dos resultados segue `app.locale`; antes era pt-BR fixo no Bing e nenhum no DuckDuckGo.
fn search_request_url(query: &str, engine: &str, locale: &str) -> String {
    let encoded = encode_uri_component(query);
    let region = duckduckgo_region(locale);
    match engine {
        "bing" => format!("{BING_SEARCH_ENDPOINT}?q={encoded}&setlang={}", encode_uri_component(locale)),
        "yahoo" => format!("{YAHOO_SEARCH_ENDPOINT}?p={encoded}"),
        _ => format!("{DUCKDUCKGO_HTML_ENDPOINT}?q={encoded}&kl={region}"),
    }
}

fn interface_locale(store: &ConfigStore) -> String {
    crate::config::locale::app_locale(store)
}

/// `search()` pública: `engine == "auto"` corre a cadeia de fallback; qualquer outro valor
/// consulta um único motor (comportamento antigo, preservado para quem escolhe um motor específico
/// em Configurações).
pub async fn search(store: &ConfigStore, query: &str, engine: &str) -> Result<Vec<SearchResult>, SearchError> {
    if engine == AUTO_ENGINE {
        return search_fallback_chain(store, query).await;
    }
    search_single_engine(store, query, engine).await
}

async fn search_fallback_chain(store: &ConfigStore, query: &str) -> Result<Vec<SearchResult>, SearchError> {
    let mut attempts = Vec::with_capacity(ENGINE_FALLBACK_CHAIN.len());
    for engine in ENGINE_FALLBACK_CHAIN {
        let attempt = search_single_engine(store, query, engine).await;
        let found_results = matches!(&attempt, Ok(results) if !results.is_empty());
        attempts.push(attempt);
        if found_results {
            break;
        }
    }
    pick_first_non_empty(attempts)
}

/// Política pura da cadeia de fallback, separada da rede para ser testável sem fetcher: o primeiro
/// resultado NÃO VAZIO ganha; se nenhum vier, o último erro é o que sobe — e se todo motor apenas
/// respondeu vazio (sem nenhum `Err`), a cadeia inteira ainda é um erro explícito, nunca `Ok(vec![])`.
fn pick_first_non_empty(attempts: Vec<Result<Vec<SearchResult>, SearchError>>) -> Result<Vec<SearchResult>, SearchError> {
    let mut last_error = None;
    for attempt in attempts {
        match attempt {
            Ok(results) if !results.is_empty() => return Ok(results),
            Ok(_) => {}
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or(SearchError::AllEnginesFailed))
}

async fn search_single_engine(store: &ConfigStore, query: &str, engine: &str) -> Result<Vec<SearchResult>, SearchError> {
    let client = guarded_fetch::guarded_http_client();
    let fetcher = guarded_fetch::ReqwestFetcher { client };
    let response = guarded_fetch::guarded_get(store, &fetcher, &search_request_url(query, engine, &interface_locale(store))).await?;

    let html = response.text();
    if engine == "duckduckgo" && looks_like_duckduckgo_anti_bot(response.status, &html) {
        return Err(SearchError::BlockedByEngine { engine: "duckduckgo" });
    }

    Ok(extract_results(&html, engine)
        .into_iter()
        .map(|item| SearchResult { snippet: label_untrusted_content("BUSCA_WEB", &item.snippet), ..item })
        .collect())
}

/// DDG serve uma página de desafio anti-bot ("anomaly") com HTTP 202 — ainda dentro do range 2xx
/// que `guarded_get` já aceita como sucesso, então isto tem de olhar o CONTEÚDO também: sem
/// nenhum `div.web-result`/`div.result`, essa página teria virado silenciosamente "0 resultados".
/// Confirmado contra uma captura real do endpoint nesta rodada (ver o relatório da tarefa).
fn looks_like_duckduckgo_anti_bot(status: u16, html: &str) -> bool {
    status == 202 || html.to_ascii_lowercase().contains("anomaly-modal")
}

fn extract_results(html: &str, engine: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let results = match engine {
        "bing" => extract_bing(&document),
        "yahoo" => extract_yahoo(&document),
        _ => extract_duckduckgo(&document),
    };
    results.into_iter().take(EXTRACT_LIMIT).collect()
}

fn text_of(el: &ElementRef) -> String {
    clean_text(&el.text().collect::<String>())
}

/// Espelha `pickSnippet` de `search-results.ts`: se o trecho de texto contiver o título embutido,
/// remove-o; cai pro título quando sobra vazio.
fn pick_snippet(text: &str, title: &str) -> String {
    if text.is_empty() {
        return title.to_string();
    }
    let trimmed = clean_text(&text.replace(title, ""));
    if trimmed.is_empty() {
        title.to_string()
    } else {
        trimmed.chars().take(280).collect()
    }
}

fn extract_duckduckgo(document: &Html) -> Vec<SearchResult> {
    let container_sel = Selector::parse("div.web-result").expect("seletor válido");
    let title_sel = Selector::parse("a.result__a").expect("seletor válido");
    let snippet_sel = Selector::parse("a.result__snippet").expect("seletor válido");

    document
        .select(&container_sel)
        .filter_map(|node| {
            let title_el = node.select(&title_sel).next()?;
            let url = unwrap_duckduckgo_redirect(title_el.attr("href")?)?;
            let title = text_of(&title_el);
            if title.is_empty() {
                return None;
            }
            let snippet_text = node.select(&snippet_sel).next().map(|el| text_of(&el)).unwrap_or_default();
            Some(SearchResult { snippet: pick_snippet(&snippet_text, &title), title, url })
        })
        .collect()
}

/// DuckDuckGo embrulha o destino real em `//duckduckgo.com/l/?uddg=<url percent-encoded>&rut=...`
/// — nunca devolve o link cru. Confirmado com captura real (ver o relatório da tarefa).
fn unwrap_duckduckgo_redirect(href: &str) -> Option<String> {
    let absolute = if let Some(rest) = href.strip_prefix("//") { format!("https://{rest}") } else { href.to_string() };
    let parsed = url::Url::parse(&absolute).ok()?;
    let target = parsed.query_pairs().find(|(key, _)| key == "uddg")?.1.into_owned();
    (target.starts_with("http://") || target.starts_with("https://")).then_some(target)
}

fn extract_bing(document: &Html) -> Vec<SearchResult> {
    let container_sel = Selector::parse("li.b_algo").expect("seletor válido");
    let title_sel = Selector::parse("h2 a").expect("seletor válido");
    let snippet_sel = Selector::parse(".b_caption p").expect("seletor válido");

    document
        .select(&container_sel)
        .filter_map(|node| {
            let title_el = node.select(&title_sel).next()?;
            let url = unwrap_bing_redirect(title_el.attr("href")?)?;
            let title = text_of(&title_el);
            if title.is_empty() {
                return None;
            }
            let snippet_text = node.select(&snippet_sel).next().map(|el| text_of(&el)).unwrap_or_default();
            Some(SearchResult { snippet: pick_snippet(&snippet_text, &title), title, url })
        })
        .collect()
}

/// Bing embrulha o destino real em `bing.com/ck/a?...&u=a1<base64>&...` — o `u=` sempre começa com
/// o prefixo literal `a1` antes do base64 da URL de verdade (confirmado com captura real, ver o
/// relatório da tarefa). Um `<a>` sem o wrapper `ck/a` (link orgânico direto) passa como está.
fn unwrap_bing_redirect(href: &str) -> Option<String> {
    if !href.contains("bing.com/ck/a") {
        return (href.starts_with("http://") || href.starts_with("https://")).then(|| href.to_string());
    }
    let parsed = url::Url::parse(href).ok()?;
    let wrapped = parsed.query_pairs().find(|(key, _)| key == "u")?.1.into_owned();
    let payload = wrapped.strip_prefix("a1")?;
    let decoded = decode_base64_loosely(payload)?;
    let target = String::from_utf8(decoded).ok()?;
    (target.starts_with("http://") || target.starts_with("https://")).then_some(target)
}

/// Bing não documenta publicamente qual variante de base64 usa nesse parâmetro; tenta as três
/// plausíveis (URL-safe sem padding é a mais comum em query strings) antes de desistir.
fn decode_base64_loosely(input: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(input))
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(input))
        .ok()
}

fn extract_yahoo(document: &Html) -> Vec<SearchResult> {
    let container_sel = Selector::parse("div.algo-sr").expect("seletor válido");
    let title_sel = Selector::parse("h3.title").expect("seletor válido");
    let link_sel = Selector::parse("a[href]").expect("seletor válido");
    let snippet_sel = Selector::parse(".compText p").expect("seletor válido");

    document
        .select(&container_sel)
        .filter_map(|node| {
            let title_el = node.select(&title_sel).next()?;
            let title = text_of(&title_el);
            if title.is_empty() {
                return None;
            }
            let href = node.select(&link_sel).find_map(|a| a.attr("href").filter(|h| h.contains("r.search.yahoo.com")))?;
            let url = unwrap_yahoo_redirect(href)?;
            let snippet_text = node.select(&snippet_sel).next().map(|el| text_of(&el)).unwrap_or_default();
            Some(SearchResult { snippet: pick_snippet(&snippet_text, &title), title, url })
        })
        .collect()
}

/// Yahoo embrulha o destino real num segmento de PATH
/// (`https://r.search.yahoo.com/.../RU=<url percent-encoded>/RK=.../RS=...`), não numa query
/// string — por isso não dá pra reusar `url::Url::query_pairs` aqui como no DuckDuckGo. Confirmado
/// com captura real (ver o relatório da tarefa).
fn unwrap_yahoo_redirect(href: &str) -> Option<String> {
    let segment = href.split('/').find(|segment| segment.starts_with("RU="))?;
    let decoded = percent_decode(segment.strip_prefix("RU=")?);
    (decoded.starts_with("http://") || decoded.starts_with("https://")).then_some(decoded)
}

/// Percent-decode manual (não form-decode: `%2f` é `/` literal aqui, não espaço-por-`+`). Contraparte
/// de [`super::encode_uri_component`] — mesma razão pra ser escrito à mão: sem depender de mais uma
/// crate pra uma operação de poucas linhas.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        // `get`, não índice: o texto depois do `%` pode ser multibyte e `i + 3` cair no meio de um char.
        if let (b'%', Some([hi, lo])) = (bytes[i], bytes.get(i + 1..i + 3).map(|pair| [pair[0], pair[1]])) {
            if let (Some(hi), Some(lo)) = ((hi as char).to_digit(16), (lo as char).to_digit(16)) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests;
