//! `action=achievements` do tool `steam` — despacho real contra
//! `ISteamUserStats/GetPlayerAchievements/v1/` da API Web da Steam. Port de
//! `handlers/steam-achievements.ts` + `services/steam-api.ts::getSteamCreds`. Diretório próprio
//! (não `steam_achievements.rs` solto) só para caber no limite de 300 linhas por arquivo com os
//! testes ao lado — mesma divisão que `browser::guarded_fetch` já usa (`mod.rs` + `tests.rs`).
//!
//! **Chokepoint de rede:** todo despacho passa por `privacy::assert_network_allowed` ANTES do
//! primeiro byte sair — modo offline bloqueia mesmo com credencial válida configurada (ver
//! `project_gate_de_rede_x_gate_de_ssrf`: um guard bom de uma categoria não cobre a outra; aqui a
//! URL é FIXA, sem risco de SSRF, mas isso não isenta do gate de modo offline).
//!
//! **SecOps: `steamApiKey`/`steamId64` nunca aparecem em log, erro ou teste.** A chave vai como
//! query string (a API da Steam não aceita header), então a URL completa NUNCA é logada nem
//! devolvida — `assert_network_allowed` já documenta só logar o `provider_tag`, e toda mensagem de
//! erro deste arquivo é um texto FIXO, nunca o `Display` de um erro de transporte (que embutiria a
//! URL com a query, chave incluída).
//!
//! **Transporte por trait** (`AchievementsFetcher`), mesmo padrão de `browser::guarded_fetch`
//! (`HopFetcher`): os testes injetam um fake que grava as chamadas, sem bater na Steam de verdade
//! nem abrir socket nenhum — gate dependente de rede é proibido nesta casa.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::privacy;

use super::iso_time;

const ACHIEVEMENTS_URL: &str = "https://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v1/";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const API_KEY_PATH: &str = "gamer-companion.steamApiKey";
const STEAM_ID_PATH: &str = "gamer-companion.steamId64";

/// Resposta crua de um transporte — status HTTP + corpo, sem opinião sobre o que significam.
pub(super) struct FetcherResponse {
    pub(super) status: u16,
    pub(super) body: Vec<u8>,
}

/// Abstrai o transporte para permitir testar [`dispatch`] sem rede real — [`ReqwestFetcher`] é a
/// única implementação que fala HTTP de verdade.
#[async_trait]
pub(super) trait AchievementsFetcher: Send + Sync {
    async fn fetch(&self, app_id: i64, api_key: &str, steam_id: &str, language: &str) -> Result<FetcherResponse, String>;
}

/// Código de idioma da API Web da Steam para o `app.locale` da interface. Sem `l`, a Steam omite
/// `name`/`description` e a Mika lia o `apiname` interno (`ACH_WIN_ONE_GAME`) em voz alta.
fn steam_language(locale: &str) -> &'static str {
    match locale {
        "en-US" => "english",
        "es-ES" => "spanish",
        "fr-FR" => "french",
        "it-IT" => "italian",
        "ja-JP" => "japanese",
        "zh-CN" => "schinese",
        "ko-KR" => "koreana",
        "ru-RU" => "russian",
        "de-DE" => "german",
        _ => "brazilian",
    }
}

fn http_client() -> &'static Client {
    crate::modules::privacy::fixed_endpoint_client()
}

struct ReqwestFetcher;

#[async_trait]
impl AchievementsFetcher for ReqwestFetcher {
    async fn fetch(&self, app_id: i64, api_key: &str, steam_id: &str, language: &str) -> Result<FetcherResponse, String> {
        let app_id_text = app_id.to_string();
        let response = http_client()
            .get(ACHIEVEMENTS_URL)
            .query(&[("appid", app_id_text.as_str()), ("key", api_key), ("steamid", steam_id), ("l", language)])
            .timeout(HTTP_TIMEOUT)
            .send()
            .await
            // NUNCA `error.to_string()`: o `Display` de `reqwest::Error` embute a URL da requisição
            // (com a query — chave incluída) quando o `send()` falha no transporte.
            .map_err(|_| "Steam API offline ou inacessível.".to_string())?;

        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(|_| "Steam API respondeu com corpo inválido.".to_string())?.to_vec();
        Ok(FetcherResponse { status, body })
    }
}

fn config_text(config: &ConfigStore, path: &str) -> String {
    config.get(path).and_then(|v| v.as_str().map(str::trim).map(str::to_string)).unwrap_or_default()
}

/// Ponto de entrada chamado por `steam_tool.rs::call`.
pub(super) async fn achievements_response(config: &ConfigStore, args: &Value) -> Value {
    dispatch(&ReqwestFetcher, config, args).await
}

/// Núcleo testável: recebe o transporte por parâmetro em vez de abrir um de verdade, mesma
/// separação de `guarded_fetch::guarded_get`/`HopFetcher`.
async fn dispatch(fetcher: &dyn AchievementsFetcher, config: &ConfigStore, args: &Value) -> Value {
    let Some(app_id) = args.get("appId").and_then(Value::as_i64).filter(|&id| id > 0) else {
        return json!({ "success": false, "error": "appId é obrigatório para action=achievements", "errorCode": "invalid_args" });
    };
    let game_name = args.get("gameName").and_then(Value::as_str).map(str::to_string);

    let api_key = config_text(config, API_KEY_PATH);
    let steam_id = config_text(config, STEAM_ID_PATH);
    if api_key.is_empty() || steam_id.is_empty() {
        return json!({
            "success": false,
            "error": "Configure Steam API Key e Steam ID nas settings para ver achievements.",
            "errorCode": "not_configured",
        });
    }

    if let Err(error) = privacy::assert_network_allowed(config, ACHIEVEMENTS_URL, "steam:achievements") {
        return json!({ "success": false, "error": error, "errorCode": "internal" });
    }

    let language = steam_language(&crate::config::locale::app_locale(config));
    let response = match fetcher.fetch(app_id, &api_key, &steam_id, language).await {
        Ok(r) => r,
        Err(error) => return json!({ "success": false, "error": error, "errorCode": "internal" }),
    };

    if !(200..300).contains(&response.status) {
        return json!({ "success": false, "error": format!("Steam API error: {}", response.status), "errorCode": "unavailable" });
    }

    let Ok(body) = serde_json::from_slice::<Value>(&response.body) else {
        return json!({ "success": false, "error": "Steam API respondeu com JSON inválido.", "errorCode": "internal" });
    };

    summarize(&body, app_id, game_name)
}

/// Porte mínimo de `formatRelativeTime` (`packages/sdk/src/utils/time-format.ts`) — só o locale
/// pt-BR default que chamadores de main-process usam (`t()` sem `useI18n()`), sem puxar o sistema
/// de i18n inteiro pra uma tool que só precisa de 5 faixas de tempo.
fn relative_time_ago(unlocktime_seconds: i64) -> String {
    let diff_ms = iso_time::now_ms() as i64 - unlocktime_seconds * 1000;
    let minutes = diff_ms / 60_000;
    if minutes < 1 {
        return "agora".to_string();
    }
    if minutes < 60 {
        return format!("há {minutes}min");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("há {hours}h");
    }
    let days = hours / 24;
    if days < 30 {
        return format!("há {days}d");
    }
    format!("há {}m", days / 30)
}

/// Uma entrada de `recentlyUnlocked` (com `unlockTime`) ou de `latestUnlock` (sem — mesma forma de
/// `steam-achievements.ts`).
fn achievement_entry(entry: &Value, with_unlock_time: bool) -> Value {
    let apiname = entry.get("apiname").and_then(Value::as_str).unwrap_or_default();
    // `name ?? apiname` / `description ?? ''`: só cai no fallback se o campo estiver AUSENTE ou
    // nulo, não se vier como string vazia — mesma semântica do `??` do TS.
    let name = entry.get("name").and_then(Value::as_str).unwrap_or(apiname);
    let description = entry.get("description").and_then(Value::as_str).unwrap_or("");
    let unlocktime = entry.get("unlocktime").and_then(Value::as_i64).unwrap_or(0);
    let time_ago = if unlocktime > 0 { relative_time_ago(unlocktime) } else { "desconhecido".to_string() };

    let mut value = json!({ "name": name, "description": description, "timeAgo": time_ago });
    if with_unlock_time {
        value["unlockTime"] = json!(iso_time::format_iso8601(unlocktime.max(0) as u64 * 1000));
    }
    value
}

fn summarize(body: &Value, app_id: i64, game_name: Option<String>) -> Value {
    let achievements = body.get("playerstats").and_then(|p| p.get("achievements")).and_then(Value::as_array).cloned().unwrap_or_default();

    let total = achievements.len();
    let is_unlocked = |a: &&Value| a.get("achieved").and_then(Value::as_i64) == Some(1);
    let mut unlocked: Vec<&Value> = achievements.iter().filter(is_unlocked).collect();
    unlocked.sort_by_key(|a| std::cmp::Reverse(a.get("unlocktime").and_then(Value::as_i64).unwrap_or(0)));
    let locked = total - unlocked.len();
    let completion_pct = if total > 0 { ((unlocked.len() as f64 / total as f64) * 100.0).round() as i64 } else { 0 };

    let recently_unlocked: Vec<Value> = unlocked.iter().copied().take(5).map(|a| achievement_entry(a, true)).collect();
    let latest_unlock = unlocked.first().copied().map(|a| achievement_entry(a, false));

    json!({
        "success": true,
        "data": {
            "gameName": game_name.unwrap_or_else(|| format!("App {app_id}")),
            "total": total,
            "unlocked": unlocked.len(),
            "locked": locked,
            "completionPct": completion_pct,
            "recentlyUnlocked": recently_unlocked,
            "latestUnlock": latest_unlock,
        }
    })
}

#[cfg(test)]
mod tests;
