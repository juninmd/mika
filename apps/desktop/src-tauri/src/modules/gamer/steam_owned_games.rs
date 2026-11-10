//! `IPlayerService/GetOwnedGames` — porte de `getOwnedGames` (`services/steam-api.ts`), só a
//! contagem que o "Validar" da Steam precisa. Mesmas regras de `steam_achievements`: URL fixa, gate
//! de modo offline antes do primeiro byte, e a chave (vai na query) nunca em log nem em erro.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use crate::config::store::ConfigStore;
use crate::modules::privacy;

const OWNED_GAMES_URL: &str = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v0001/";
const HTTP_TIMEOUT: Duration = Duration::from_secs(8);

#[async_trait]
pub(super) trait OwnedGamesFetcher: Send + Sync {
    async fn fetch(&self, api_key: &str, steam_id: &str) -> Result<(u16, Vec<u8>), String>;
}

pub(super) struct ReqwestFetcher;

fn http_client() -> &'static Client {
    crate::modules::privacy::fixed_endpoint_client()
}

#[async_trait]
impl OwnedGamesFetcher for ReqwestFetcher {
    async fn fetch(&self, api_key: &str, steam_id: &str) -> Result<(u16, Vec<u8>), String> {
        let query = [
            ("key", api_key),
            ("steamid", steam_id),
            ("include_appinfo", "1"),
            ("include_played_free_games", "1"),
            ("format", "json"),
        ];
        // Texto fixo, nunca o `Display` do reqwest: ele embute a URL com a chave na query.
        let response = http_client()
            .get(OWNED_GAMES_URL)
            .query(&query)
            .timeout(HTTP_TIMEOUT)
            .send()
            .await
            .map_err(|_| "Steam API offline ou inacessível.".to_string())?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(|_| "Steam API respondeu com dados inválidos.".to_string())?;
        Ok((status, body.to_vec()))
    }
}

/// Porte de `friendlyHttpError`.
fn friendly_http_error(status: u16) -> String {
    match status {
        401 => "Chave inválida (401): confira a Steam API Key nas settings.".into(),
        403 => "Acesso negado (403): a chave não pode acessar essa conta.".into(),
        429 => "Muitas requisições (429): aguarde alguns minutos e tente de novo.".into(),
        500.. => "Steam API indisponível no momento, tente novamente mais tarde.".into(),
        _ => format!("Steam API respondeu com erro ({status})."),
    }
}

pub(super) async fn owned_games_count(
    fetcher: &dyn OwnedGamesFetcher,
    store: &ConfigStore,
    api_key: &str,
    steam_id: &str,
) -> Result<usize, String> {
    privacy::assert_network_allowed(store, OWNED_GAMES_URL, "steam:owned-games")?;
    let (status, body) = fetcher.fetch(api_key, steam_id).await?;
    if !(200..300).contains(&status) {
        return Err(friendly_http_error(status));
    }
    let data: Value = serde_json::from_slice(&body).map_err(|_| "Steam API respondeu com dados inválidos.".to_string())?;
    Ok(data["response"]["games"].as_array().map_or(0, Vec::len))
}
