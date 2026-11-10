//! json_fetch.rs — GET cru pelo pipeline guardado (SSRF + modo offline por hop), pra endpoints
//! REST que devolvem JSON em vez do HTML que `search.rs`/`page.rs` esperam. Único chamador hoje: o
//! resumo da Wikipedia em `person_catalog::enrichment` — o parse do corpo (struct `WikiSummary`)
//! mora lá, não aqui, mesma divisão que o TS original tinha (`fetchWikiSummary` usava
//! `createLoggedFetch` genérico, não um cliente dedicado à Wikipedia).

use crate::config::store::ConfigStore;

use super::guarded_fetch::{self, GuardedFetchError};

/// Bytes crus da resposta, já atravessando as duas guardas (SSRF + offline) e o loop de redirect
/// de `guarded_get` — nenhum `reqwest::Client` novo, reusa `guarded_http_client()`. Quem chama
/// decide o parse (JSON, aqui; texto em `page.rs`).
pub async fn fetch_bytes(store: &ConfigStore, url: &str) -> Result<Vec<u8>, GuardedFetchError> {
    let client = guarded_fetch::guarded_http_client();
    let fetcher = guarded_fetch::ReqwestFetcher { client };
    let response = guarded_fetch::guarded_get(store, &fetcher, url).await?;
    Ok(response.body)
}
