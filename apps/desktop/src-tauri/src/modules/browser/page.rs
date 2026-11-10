//! page.rs — leitura real de UMA página via `guarded_fetch`, sem motor JS: porta a parte
//! determinística de `fetchPage`/`cleanText`/`body.innerText()` (`browser.service.ts`).
//!
//! ## `innerText` não é `textContent` — e o gap nomeado que sobra
//!
//! `innerText` pula `script`/`style` e qualquer nó OCULTO POR CSS (`display:none`,
//! `visibility:hidden`), e colapsa espaço em branco visualmente. Sem motor de layout (a decisão
//! desta rodada é não ter Chromium — ver `mod.rs` e `docs/tauri-rust-migration.md` §5), a parte
//! "oculto por CSS" não dá pra replicar: calcular estilo computado pede o próprio motor que a
//! decisão descartou. [`inner_text`] cobre a parte barata e determinística — remove
//! `script`/`style`/`noscript` do DOM antes de extrair texto e colapsa espaço com
//! [`super::clean_text`] (a mesma função que porta `cleanText` do TS) — o suficiente pra não
//! devolver JS/CSS minificado pro LLM, que era o motivo citado na tarefa. Uma página cujo conteúdo
//! só existe atrás de `display:none` controlado por JS ainda vaza texto que um navegador
//! esconderia: gap nomeado, não escondido.

use scraper::{ElementRef, Html, Selector};

use crate::config::store::ConfigStore;

use super::guarded_fetch::{self, GuardedFetchError};
use super::{clean_text, label_untrusted_content};

/// = `text.slice(0, 20_000)` em `fetchPage` (`browser.service.ts`).
const MAX_TEXT_CHARS: usize = 20_000;

pub struct FetchedPage {
    pub title: String,
    pub text: String,
}

pub async fn fetch_page(store: &ConfigStore, url: &str) -> Result<FetchedPage, GuardedFetchError> {
    let client = guarded_fetch::guarded_http_client();
    let fetcher = guarded_fetch::ReqwestFetcher { client };
    let response = guarded_fetch::guarded_get(store, &fetcher, url).await?;

    Ok(extract_page(&response.text()))
}

fn extract_page(html: &str) -> FetchedPage {
    let document = Html::parse_document(html);

    let title_sel = Selector::parse("title").expect("seletor válido");
    let title = document.select(&title_sel).next().map(|el| clean_text(&el.text().collect::<String>())).unwrap_or_default();

    let text: String = inner_text(&document).chars().take(MAX_TEXT_CHARS).collect();
    FetchedPage { title, text: label_untrusted_content("PAGINA_WEB", &text) }
}

/// Aproximação de `page.locator('body').innerText()` sem motor de layout — ver o gap nomeado no
/// cabeçalho do arquivo.
fn inner_text(document: &Html) -> String {
    let body_sel = Selector::parse("body").expect("seletor válido");
    let skip_sel = Selector::parse("script, style, noscript").expect("seletor válido");

    let root: ElementRef = document.select(&body_sel).next().unwrap_or_else(|| document.root_element());
    let skipped: std::collections::HashSet<_> = root.select(&skip_sel).flat_map(|el| el.descendants().map(|node| node.id())).collect();

    let mut out = String::new();
    for node in root.descendants() {
        if skipped.contains(&node.id()) {
            continue;
        }
        if let Some(text) = node.value().as_text() {
            out.push_str(text);
            out.push(' ');
        }
    }
    clean_text(&out)
}

#[cfg(test)]
mod tests;
