//! Tool `steam`, port de `createSteamTool` (`tools/index.ts`): `action=library` (list/open/recent),
//! lida do disco — combina `steam_library` + `steam_local_stats`, port de
//! `services/steam-top-games.ts` sem a etapa de resolução de nome via Steam Web API
//! (`buildNameResolver`'s `hasApi` branch) — e `action=achievements`, despachada de verdade em
//! `steam_achievements/` (arquivo à parte para caber no limite de 300 linhas, mesmo módulo TS
//! dividido entre `tools/index.ts` e `handlers/steam-achievements.ts`).
//!
//! **Remedido 2026-09-22b: passada de boot movida.** Até a rodada anterior, `SteamToolHandler` era
//! um unit struct cadastrado na PRIMEIRA passada (`gamer::register_tools`, config-livre) —
//! `achievements` degradava sempre, nomeado, porque nenhum `Arc<ConfigStore>` chegava ao handler.
//! Duas premissas foram medidas antes disso: "a criptografia de `steamApiKey` não estaria
//! portada" era FALSA (`config/crypto.rs` já é AES-256-GCM+scrypt byte-compatível com o Node,
//! `ConfigStore::get` decifra sufixo `key` de forma transparente); "`ToolHandler::call` não
//! recebe `AppHandle`" era verdadeira mas incompleta — `gamer::register_session_tools` (segunda
//! passada de boot) já resolvia isso para `RecommendMediaHandler`/`GameSessionHandler`. Agora
//! `SteamToolHandler` segue o mesmo padrão: guarda `Arc<ConfigStore>` e é cadastrado na SEGUNDA
//! passada (`gamer/mod.rs::register_session_tools`) — `steam` saiu da primeira. `library`
//! (list/open/recent) não precisa de config e continua igual.

use super::steam_art::read_game_art;
use super::steam_library::{find_steam_game, list_steam_games, open_steam_game, SteamGame};
use super::steam_local_stats::{read_local_playtime, resolve_steam_root, LocalPlaytime};
use crate::config::store::ConfigStore;
use crate::modules::tools::{ToolDefinition, ToolHandler};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

const CANDIDATE_POOL_MIN: usize = 12;
const CANDIDATE_POOL_FACTOR: usize = 3;

fn hours_played(minutes: i64) -> String {
    if minutes < 60 {
        return format!("{minutes}min");
    }
    let hours = minutes / 60;
    let rest = minutes % 60;
    if rest > 0 { format!("{hours}h{rest}min") } else { format!("{hours}h") }
}

struct TopGame {
    app_id: String,
    name: String,
    playtime_minutes: i64,
    installed: bool,
}

/// Port de `getSteamTopGames`, sem o ramo de resolução de nome via Steam Web API — sem config,
/// nomes só resolvem para jogos já instalados localmente (mesmo fallback que o TS usa quando
/// `hasApi` é falso). `include_env_defaults` espelha o parâmetro homônimo de `listSteamGames`
/// (`steam-library.ts`): produção sempre passa `true`; os testes passam `false` para não misturar
/// a Steam de verdade da máquina que roda `cargo test` com a fixture do teste.
fn top_games(root: &Path, limit: usize, recent_only: bool, include_env_defaults: bool) -> Vec<TopGame> {
    // A raiz resolvida (registro/HOME/config) é a biblioteca a listar: sem passá-la, um Steam fora
    // dos caminhos padrão tinha playtime lido mas NENHUM jogo instalado para casar com ele.
    let installed_games = list_steam_games(root.to_str(), include_env_defaults);
    let installed: HashMap<&str, &str> = installed_games.iter().map(|g| (g.app_id.as_str(), g.name.as_str())).collect();

    let all = read_local_playtime(root);
    let playtime: Vec<LocalPlaytime> = if recent_only {
        let mut recent: Vec<LocalPlaytime> = all.into_iter().filter(|p| p.minutes_2_weeks > 0).collect();
        recent.sort_by(|a, b| b.minutes_2_weeks.cmp(&a.minutes_2_weeks));
        recent
    } else {
        all
    };

    if recent_only && playtime.is_empty() {
        return Vec::new();
    }

    let candidate_pool = (limit * CANDIDATE_POOL_FACTOR).max(CANDIDATE_POOL_MIN);
    let ranked: Vec<(String, i64)> = if !playtime.is_empty() {
        playtime
            .into_iter()
            .take(candidate_pool)
            .map(|p| (p.app_id, if recent_only { p.minutes_2_weeks } else { p.minutes }))
            .collect()
    } else {
        installed_games.iter().take(candidate_pool).map(|g| (g.app_id.clone(), 0)).collect()
    };

    let mut games = Vec::new();
    for (app_id, minutes) in ranked {
        if games.len() >= limit {
            break;
        }
        let Some(&name) = installed.get(app_id.as_str()) else { continue };
        games.push(TopGame {
            installed: installed.contains_key(app_id.as_str()),
            app_id,
            name: name.to_string(),
            playtime_minutes: minutes,
        });
    }
    games
}

fn list_response(root: &Path, limit: usize, recent: bool, include_env_defaults: bool) -> Value {
    let games = top_games(root, limit, recent, include_env_defaults);
    if games.is_empty() {
        let note = if recent { "No games played in the last 2 weeks." } else { "Library is empty." };
        return json!({ "success": true, "data": { "count": 0, "games": [], "note": note } });
    }
    let games_json: Vec<Value> = games
        .iter()
        .map(|g| {
            let mut entry = json!({
                "appId": g.app_id,
                "name": g.name,
                "installed": g.installed,
                "playtimeMinutes": g.playtime_minutes,
            });
            let label = if g.playtime_minutes > 0 { hours_played(g.playtime_minutes) } else { String::new() };
            entry[if recent { "playtime2weeks" } else { "playtime" }] = json!(label);
            let art = read_game_art(root, &g.app_id);
            entry["art"] = json!(art.as_ref().map(|a| a.data_uri.clone()));
            entry
        })
        .collect();
    json!({ "success": true, "data": { "count": games_json.len(), "games": games_json } })
}

fn open_response(root: &Path, query: &str, include_env_defaults: bool) -> Value {
    let query = query.trim();
    if query.is_empty() {
        return json!({ "success": false, "error": "query is required for mode=open", "errorCode": "invalid_args" });
    }
    let games: Vec<SteamGame> = list_steam_games(root.to_str(), include_env_defaults);
    let Some(found) = find_steam_game(&games, query) else {
        return json!({ "success": false, "error": format!("Game not found on Steam: {query}"), "errorCode": "not_found" });
    };
    let app_id = found.app_id.clone();
    let name = found.name.clone();
    match open_steam_game(&app_id) {
        Ok(()) => json!({ "success": true, "data": { "launched": name, "appId": app_id } }),
        Err(error) => json!({ "success": false, "error": error, "errorCode": "internal" }),
    }
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "steam".into(),
        description: Some(
            "The user's Steam games. action=library: mode=list (most played), recent, or open (launch the game matching `query`). action=achievements: for `appId`/`gameName` (needs Steam API key + Steam ID in settings). Use instead of guessing hours or achievements."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["library", "achievements"] },
                "mode": { "type": "string", "enum": ["list", "open", "recent"] },
                "query": { "type": "string" },
                "limit": { "type": "integer" },
                "appId": { "type": "integer" },
                "gameName": { "type": "string" },
            },
            "required": ["action"],
        }),
        plugin_id: super::feature::GAMER_COMPANION_PLUGIN_ID.into(),
        requires_window: false,
    }
}

/// Guarda `Arc<ConfigStore>` desde 2026-09-22b — `action=achievements` precisa dele para ler
/// `steamApiKey`/`steamId64` (`steam_achievements::achievements_response`). `library` ignora o
/// campo; nada nesta struct decide isso, é `call` que ramifica por `action`.
pub struct SteamToolHandler {
    config: Arc<ConfigStore>,
}

impl SteamToolHandler {
    pub fn new(config: Arc<ConfigStore>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for SteamToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        // Toggle mestre da feature (`gamer-companion.enabled`) — ver o cabeçalho equivalente em
        // `game_session_tool.rs`.
        if !super::feature::master_enabled(&self.config) {
            return Ok(super::feature::master_disabled_response());
        }
        let action = args.get("action").and_then(Value::as_str).unwrap_or("library");
        if action == "achievements" {
            return Ok(super::steam_achievements::achievements_response(&self.config, &args).await);
        }

        let mode = args.get("mode").and_then(Value::as_str).unwrap_or("list");
        let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20).clamp(1, 50) as usize;

        // `gamer-companion.steamPath` (Settings) tem precedência; sem ela, registro do Windows + HOME + env.
        let configured = self.config.get("gamer-companion.steamPath").and_then(|v| v.as_str().map(str::to_string));
        let Some(root) = resolve_steam_root(configured.as_deref()) else {
            return Ok(
                json!({ "success": false, "error": "Steam was not found on this machine. Set its folder in Settings.", "errorCode": "not_configured" }),
            );
        };

        Ok(match mode {
            "open" => open_response(&root, args.get("query").and_then(Value::as_str).unwrap_or(""), true),
            "recent" => list_response(&root, limit, true, true),
            _ => list_response(&root, limit, false, true),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::test_support::ScratchDir;
    use std::fs;

    /// Toggle mestre da feature (`gamer-companion.enabled=false`) — nenhuma ação de `steam`
    /// dispacha, `achievements` incluso (varredura de controles de UI de 2026-09-27).
    #[tokio::test]
    async fn call_is_denied_when_the_master_toggle_is_off() {
        let scratch = ScratchDir::new("master-toggle-off");
        let store = ConfigStore::open(scratch.path(), crate::config::schemas::registry()).unwrap();
        store.set("gamer-companion.enabled", serde_json::json!(false)).unwrap();
        let handler = SteamToolHandler::new(Arc::new(store));
        let result = handler.call(json!({ "action": "library" })).await.unwrap();
        assert_eq!(result["success"], false);
        assert_eq!(result["errorCode"], "denied");
    }

    #[test]
    fn hours_played_formats_minutes_hours_and_mixed() {
        assert_eq!(hours_played(45), "45min");
        assert_eq!(hours_played(60), "1h");
        assert_eq!(hours_played(125), "2h5min");
    }

    /// O Steam resolvido (aqui, fora de qualquer caminho padrão do SO) é a biblioteca a listar:
    /// antes `list_steam_games(None, ..)` ignorava a raiz e o jogo com playtime ficava sem nome.
    #[test]
    fn top_games_lists_the_library_of_the_resolved_root_not_only_the_os_defaults() {
        let scratch = ScratchDir::new("top-games-resolved-root");
        let root = scratch.path();
        let steam_apps = root.join("steamapps");
        fs::create_dir_all(&steam_apps).unwrap();
        fs::write(steam_apps.join("appmanifest_10.acf"), "\"AppState\"\n{\n\t\"appid\"\t\"10\"\n\t\"name\"\t\"Counter-Strike\"\n}\n").unwrap();
        let config_dir = root.join("userdata").join("1").join("config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("localconfig.vdf"),
            "\"UserLocalConfigStore\" { \"Software\" { \"Valve\" { \"Steam\" { \"apps\" { \"10\" { \"Playtime\" \"90\" } } } } } }",
        )
        .unwrap();

        let games = top_games(root, 5, false, false);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Counter-Strike");
    }

    #[test]
    fn open_without_a_query_is_a_named_invalid_args_error() {
        let response = open_response(Path::new("Z:\\unused"), "   ", false);
        assert_eq!(response["errorCode"], "invalid_args");
    }

    #[test]
    fn open_reports_not_found_instead_of_silently_launching_nothing() {
        let scratch = ScratchDir::new("open-not-found");
        // `include_env_defaults=false` isola de qualquer Steam de verdade instalada na máquina que
        // roda o teste — só a fixture (vazia) do scratch dir conta.
        let response = open_response(scratch.path(), "jogo-que-nao-existe", false);
        assert_eq!(response["success"], false);
        assert_eq!(response["errorCode"], "not_found");
    }

    #[test]
    fn list_response_reports_an_empty_library_with_a_note_instead_of_a_bare_empty_array() {
        let scratch = ScratchDir::new("list-empty");
        fs::create_dir_all(scratch.path().join("userdata")).unwrap();

        let response = list_response(scratch.path(), 20, false, false);
        assert_eq!(response["success"], true);
        assert_eq!(response["data"]["count"], 0);
        assert_eq!(response["data"]["note"], "Library is empty.");
    }

    #[test]
    fn list_response_recent_mode_uses_the_recent_specific_empty_note() {
        let scratch = ScratchDir::new("list-recent-empty");
        fs::create_dir_all(scratch.path().join("userdata")).unwrap();

        let response = list_response(scratch.path(), 20, true, false);
        assert_eq!(response["data"]["note"], "No games played in the last 2 weeks.");
    }

    // --- action=achievements / divergência de forma (action vs mode) --------------------------

    /// Trava a forma do discriminador contra `tools/index.ts`: `action` é o campo de topo
    /// (`library`/`achievements`), obrigatório, com `mode` só fazendo sentido dentro de
    /// `action=library`. Um catálogo com `mode` como discriminador único (a forma antiga deste
    /// arquivo) ensina o LLM a chamar a tool com um shape que o TS nunca aceitou.
    #[test]
    fn schema_discriminator_matches_the_typescript_tool_shape() {
        let schema = definition().input_schema;
        assert_eq!(schema["properties"]["action"]["enum"], json!(["library", "achievements"]));
        assert_eq!(schema["required"], json!(["action"]));
        assert!(schema["properties"]["appId"].is_object(), "appId (action=achievements) sumiu do schema");
    }

    fn handler_with_store() -> (SteamToolHandler, ScratchDir) {
        let scratch = ScratchDir::new("steam-tool-dispatch");
        let store = crate::config::store::ConfigStore::open(scratch.path(), crate::config::schemas::registry()).unwrap();
        (SteamToolHandler::new(std::sync::Arc::new(store)), scratch)
    }

    /// Prova que `call()` de fato RAMIFICA por `action` até `steam_achievements::dispatch` — o
    /// comportamento detalhado de `achievements` (not_configured/offline/fixture) já tem cobertura
    /// própria em `steam_achievements/tests.rs`; aqui só a roteamento importa, via despacho real
    /// (`ToolHandler::call`), não uma função pura chamada direto.
    #[tokio::test]
    async fn achievements_action_routes_to_the_achievements_module_through_real_dispatch() {
        let (handler, _scratch) = handler_with_store();
        let response = handler.call(json!({ "action": "achievements" })).await.unwrap();
        assert_eq!(response["errorCode"], "invalid_args", "call() não chegou ao branch de achievements");
    }
}
