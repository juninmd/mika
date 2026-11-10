use super::*;
use crate::modules::gamer::test_support::ScratchDir;
use std::sync::Mutex;

fn store_with(offline: bool, api_key: &str, steam_id: &str) -> (ConfigStore, ScratchDir) {
    let scratch = ScratchDir::new("steam-achievements");
    let store = ConfigStore::open(scratch.path(), crate::config::schemas::registry()).unwrap();
    store.set(privacy::OFFLINE_MODE_PATH, json!(offline)).unwrap();
    if !api_key.is_empty() {
        store.set(API_KEY_PATH, json!(api_key)).unwrap();
    }
    if !steam_id.is_empty() {
        store.set(STEAM_ID_PATH, json!(steam_id)).unwrap();
    }
    (store, scratch)
}

/// Fake transporte: devolve uma resposta programada e GRAVA se foi chamado — é essa gravação que
/// prova, positivamente, que um caminho bloqueado (sem config, offline) nunca despachou.
struct FakeFetcher {
    response: Result<FetcherResponse, String>,
    called: Mutex<bool>,
    language: Mutex<String>,
}

impl FakeFetcher {
    fn ok(body: Value) -> Self {
        Self { response: Ok(FetcherResponse { status: 200, body: serde_json::to_vec(&body).unwrap() }), called: Mutex::new(false), language: Mutex::default() }
    }

    fn status(status: u16) -> Self {
        Self { response: Ok(FetcherResponse { status, body: Vec::new() }), called: Mutex::new(false), language: Mutex::default() }
    }

    fn was_called(&self) -> bool {
        *self.called.lock().unwrap()
    }
}

#[async_trait]
impl AchievementsFetcher for FakeFetcher {
    async fn fetch(&self, _app_id: i64, _api_key: &str, _steam_id: &str, language: &str) -> Result<FetcherResponse, String> {
        *self.called.lock().unwrap() = true;
        *self.language.lock().unwrap() = language.to_string();
        match &self.response {
            Ok(r) => Ok(FetcherResponse { status: r.status, body: r.body.clone() }),
            Err(e) => Err(e.clone()),
        }
    }
}

/// A Steam só devolve nome/descrição da conquista quando recebe `l`; segue o idioma da interface.
#[tokio::test]
async fn the_request_asks_steam_for_names_in_the_interface_language() {
    let (store, _dir) = store_with(false, "key", "123");
    let fetcher = FakeFetcher::ok(json!({ "playerstats": { "achievements": [] } }));
    dispatch(&fetcher, &store, &json!({ "appId": 570 })).await;
    assert_eq!(*fetcher.language.lock().unwrap(), "brazilian");

    store.set("app.locale", json!("en-US")).unwrap();
    dispatch(&fetcher, &store, &json!({ "appId": 570 })).await;
    assert_eq!(*fetcher.language.lock().unwrap(), "english");
}

#[tokio::test]
async fn missing_app_id_is_invalid_args_and_never_touches_the_fetcher() {
    let (store, _dir) = store_with(false, "key", "123");
    let fetcher = FakeFetcher::status(200);
    let response = dispatch(&fetcher, &store, &json!({})).await;
    assert_eq!(response["errorCode"], "invalid_args");
    assert!(!fetcher.was_called());
}

#[tokio::test]
async fn missing_credentials_is_not_configured_and_never_touches_the_fetcher() {
    let (store, _dir) = store_with(false, "", "");
    let fetcher = FakeFetcher::status(200);
    let response = dispatch(&fetcher, &store, &json!({ "appId": 440 })).await;
    assert_eq!(response["errorCode"], "not_configured");
    assert!(!fetcher.was_called());
}

/// Teste negativo obrigatório: modo offline BARRA a chamada mesmo com credencial configurada e
/// válida — prova a ORDEM (gate de rede antes do transporte), não só que o gate existe.
#[tokio::test]
async fn offline_mode_blocks_the_call_even_with_valid_credentials() {
    let (store, _dir) = store_with(true, "key", "123");
    let fetcher = FakeFetcher::status(200);
    let response = dispatch(&fetcher, &store, &json!({ "appId": 440 })).await;
    assert_eq!(response["success"], false);
    assert!(response["error"].as_str().unwrap().contains("offline"), "erro não nomeia o modo offline: {response}");
    assert!(!fetcher.was_called(), "modo offline não pode chegar a despachar rede");
}

#[tokio::test]
async fn bad_status_becomes_a_named_unavailable_error() {
    let (store, _dir) = store_with(false, "key", "123");
    let fetcher = FakeFetcher::status(503);
    let response = dispatch(&fetcher, &store, &json!({ "appId": 440 })).await;
    assert_eq!(response["errorCode"], "unavailable");
    assert_eq!(response["error"], "Steam API error: 503");
}

#[tokio::test]
async fn dispatches_and_summarizes_a_fixture_response_for_real() {
    let (store, _dir) = store_with(false, "key", "123");
    let achievements = json!([
        { "apiname": "a1", "achieved": 1, "unlocktime": 1_700_000_000i64, "name": "First", "description": "d1" },
        { "apiname": "a2", "achieved": 0, "unlocktime": 0 },
        { "apiname": "a3", "achieved": 1, "unlocktime": 1_700_000_500i64 },
    ]);
    let fetcher = FakeFetcher::ok(json!({ "playerstats": { "achievements": achievements } }));

    let response = dispatch(&fetcher, &store, &json!({ "appId": 440, "gameName": "Half-Life" })).await;

    assert!(fetcher.was_called());
    assert_eq!(response["success"], true);
    assert_eq!(response["data"]["gameName"], "Half-Life");
    assert_eq!(response["data"]["total"], 3);
    assert_eq!(response["data"]["unlocked"], 2);
    assert_eq!(response["data"]["locked"], 1);
    assert_eq!(response["data"]["completionPct"], 67);
    // Ordenado por unlocktime desc: "a3" (500) antes de "First"/a1 (0 offset).
    assert_eq!(response["data"]["recentlyUnlocked"][0]["name"], "a3");
    assert_eq!(response["data"]["latestUnlock"]["name"], "a3");
}

#[test]
fn credential_paths_are_never_embedded_in_any_fixed_error_string() {
    // Sonda estática: nenhuma mensagem fixa deste módulo concatena `api_key`/`steam_id`, e o único
    // ponto que fala com a rede nunca usa `error.to_string()` do `reqwest::Error` (que embutiria a
    // URL com a chave na query). Gate de fonte porque nenhum teste de execução veria a REGRESSÃO
    // até alguém logar o erro de verdade em produção.
    //
    // Corte no `//`: a linha só acima deste teste MENCIONA `error.to_string()` num comentário que
    // avisa contra ela — sem cortar, o gate se auto-acusa (mesma classe de defeito documentada em
    // `state.rs::single_config_store_per_process`).
    fn calls_error_to_string(line: &str) -> bool {
        line.split("//").next().unwrap_or_default().contains("error.to_string()")
    }
    assert!(calls_error_to_string("    .map_err(|error| error.to_string())"));
    assert!(!calls_error_to_string("// NUNCA `error.to_string()`: vaza a URL"));

    let source = include_str!("mod.rs");
    let production = source.split("#[cfg(test)]").next().unwrap_or_default();
    let offenders: Vec<&str> = production.lines().filter(|line| calls_error_to_string(line)).collect();
    assert!(offenders.is_empty(), "erro de transporte cru vazaria a URL com a chave na query: {offenders:?}");
}

/// Locale sem idioma da Steam caía em `brazilian`: conquistas de um usuário coreano vinham em português.
#[test]
fn every_supported_locale_asks_steam_for_its_own_language() {
    for (locale, language) in [
        ("en-US", "english"), ("pt-BR", "brazilian"), ("es-ES", "spanish"), ("fr-FR", "french"), ("it-IT", "italian"),
        ("ja-JP", "japanese"), ("zh-CN", "schinese"), ("ko-KR", "koreana"), ("ru-RU", "russian"), ("de-DE", "german"),
    ] {
        assert_eq!(steam_language(locale), language, "{locale}");
    }
}
