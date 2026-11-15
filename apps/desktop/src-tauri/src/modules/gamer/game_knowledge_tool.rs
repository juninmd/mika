//! Tool `game_knowledge` — porte de `tools/index.ts::createGameKnowledgeTool`:
//! `memorize_location`/`get_location_guide` sobre [`super::game_locations`] (via `SessionMemory`),
//! e `ask` sobre `turn::run_turn_llm_raw`.
//!
//! **Remedido 2026-09-22c: `ask` também é portado — a mesma premissa refutada.** A rodada anterior
//! marcou `ask` como "sem porte" pela MESMA premissa que `recommend_media_tool.rs` já provou falsa
//! (ver o cabeçalho daquele arquivo e o de `game_session_tool.rs`): `turn::run_turn_llm_raw` é
//! `pub`, sem `AppHandle`, e `register_session_tools` já recebe `Arc<ConfigStore>`. Porte de
//! `handlers/game-knowledge.ts::askGameKnowledge`: mesmo template (`prompts/game-knowledge.md`,
//! `include_str!`), mesmos 6 placeholders, mesma checagem de guia vazio pós-geração, mesmo
//! `errorCode: 'internal'` no catch de geração.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::session_memory::SessionMemory;
use crate::config::store::ConfigStore;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortHandle};
use crate::modules::tools::{ToolDefinition, ToolHandler};

/// Cópia literal do `.md` que `loadPrompt('game-knowledge', ...)` carrega no TS.
const TEMPLATE: &str = include_str!("../../../../prompts/gamer-companion/game-knowledge.md");

fn config_str(store: &ConfigStore, path: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Espelha `getConfig('user.nickname') ?? getConfig('user.name') ?? 'usuario'` (`game-knowledge.ts`
/// — o default é `'usuario'` aqui, DIFERENTE do `'senpai'` de `commentary.ts`/`recommend-media.ts`,
/// fiel ao TS deste arquivo).
fn nickname(store: &ConfigStore) -> String {
    let value = config_str(store, "user.nickname");
    if !value.is_empty() {
        return value;
    }
    let name = config_str(store, "user.name");
    if !name.is_empty() {
        return name;
    }
    "usuario".to_string()
}

/// Espelha `getConfig('user.favoriteGames') || getConfig('gamer-companion.favoriteGames') || ''` —
/// ORDEM invertida da de `recommend_media_tool.rs` (que só lê `gamer-companion.favoriteGames`),
/// fiel ao TS deste arquivo.
fn favorite_games(store: &ConfigStore) -> String {
    let user_favorites = config_str(store, "user.favoriteGames");
    if !user_favorites.is_empty() {
        return user_favorites;
    }
    config_str(store, "gamer-companion.favoriteGames")
}

/// Espelha os 6 placeholders de `game-knowledge.md` (`loadPrompt('game-knowledge', {...})`).
fn render_ask_prompt(store: &ConfigStore, game: &str, topic: Option<&str>, spoiler_level: &str, help_level: &str) -> String {
    let favorites = favorite_games(store);
    TEMPLATE
        .replace("{{nickname}}", &nickname(store))
        .replace("{{favoriteGames}}", if favorites.is_empty() { "unknown" } else { favorites.as_str() })
        .replace("{{game}}", game)
        .replace("{{focus}}", topic.unwrap_or("general context useful for playing better"))
        .replace("{{spoilerLevel}}", spoiler_level)
        .replace("{{helpLevel}}", help_level)
        .replace("{{language}}", &crate::config::locale::app_language(store))
}

/// Porte de `askGameKnowledge`: gera o guia por LLM e recusa um guia vazio como sucesso (mesma
/// checagem do TS — `success: true` com string vazia faria a Mika "explicar" o jogo em silêncio).
async fn ask_response(store: &ConfigStore, game: &str, topic: Option<&str>, spoiler_level: &str, help_level: &str) -> Value {
    let prompt = render_ask_prompt(store, game, topic, spoiler_level, help_level);
    let (_handle, signal) = TurnAbortHandle::new();

    let text = match turn::run_turn_llm_raw(store, None, &prompt, signal, &NullObserver).await {
        Ok(result) => result.text,
        Err(error) => return json!({ "success": false, "error": format!("Failed to generate the guide: {error}"), "errorCode": "internal" }),
    };

    let guide = text.trim();
    if guide.is_empty() {
        return json!({
            "success": false,
            "error": format!("The model returned an empty guide about {game}."),
            "errorCode": "internal",
        });
    }

    json!({ "success": true, "data": { "game": game, "topic": topic.unwrap_or("geral"), "helpLevel": help_level, "guide": guide } })
}

/// Um travamento "esfria" depois disto: voltar ao mesmo chefe no dia seguinte recomeça na pista.
const HELP_LADDER_RESET_MS: u64 = 30 * 60_000;
const HELP_LEVELS: [&str; 3] = ["hint", "tip", "solution"];

/// Escada de ajuda sem spoiler (`gamer-companion.helpLadderEnabled`): o mesmo jogo+tópico pedido de
/// novo sobe um degrau — pista, depois dica, depois solução. Em memória de processo: é o
/// travamento de agora, não preferência para gravar.
#[derive(Default)]
pub struct HelpLadder(std::sync::Mutex<std::collections::HashMap<String, (usize, u64)>>);

impl HelpLadder {
    pub fn level(&self, store: &ConfigStore, game: &str, topic: Option<&str>, explicit: Option<&str>, now_ms: u64) -> &'static str {
        if let Some(level) = HELP_LEVELS.into_iter().find(|level| Some(*level) == explicit) {
            return level;
        }
        if !store.get("gamer-companion.helpLadderEnabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            return "solution";
        }
        let key = format!("{}|{}", game.trim().to_lowercase(), topic.unwrap_or("").trim().to_lowercase());
        let mut asked = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let (step, last) = asked.get(&key).copied().unwrap_or((0, now_ms));
        let step = if now_ms.saturating_sub(last) > HELP_LADDER_RESET_MS { 0 } else { step };
        asked.insert(key, (step + 1, now_ms));
        HELP_LEVELS[step.min(HELP_LEVELS.len() - 1)]
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn memorize_location(memory: &SessionMemory, args: &Value) -> Value {
    let game = args.get("game").and_then(Value::as_str).unwrap_or("").trim();
    let location_name = args.get("locationName").and_then(Value::as_str).unwrap_or("").trim();
    let description = args.get("description").and_then(Value::as_str).unwrap_or("").trim();

    if location_name.is_empty() {
        return json!({ "success": false, "error": "locationName is required for memorize_location", "errorCode": "invalid_args" });
    }
    if description.is_empty() {
        return json!({ "success": false, "error": "description is required for memorize_location", "errorCode": "invalid_args" });
    }

    let connections = string_array(args.get("connections"));
    let important_items = string_array(args.get("importantItems"));
    let notes = args.get("notes").and_then(Value::as_str);

    match memory.save_location(game, location_name, description, &connections, &important_items, notes) {
        Ok(location) => json!({
            "success": true,
            "data": {
                "message": format!("Location {location_name} memorized for {game}."),
                "location": location,
            }
        }),
        Err(error) => json!({ "success": false, "error": error, "errorCode": "internal" }),
    }
}

fn get_location_guide(memory: &SessionMemory, args: &Value) -> Value {
    let game = args.get("game").and_then(Value::as_str).unwrap_or("").trim();
    let query = args.get("query").and_then(Value::as_str).map(str::trim).filter(|q| !q.is_empty());

    match memory.find_locations(game, query) {
        Ok(locations) if locations.is_empty() => {
            let mut data = serde_json::Map::new();
            data.insert("gameName".into(), json!(game));
            if let Some(q) = query {
                data.insert("query".into(), json!(q));
            }
            data.insert("message".into(), json!("No saved location matches this search."));
            json!({ "success": true, "data": data })
        }
        Ok(locations) => {
            let mut data = serde_json::Map::new();
            data.insert("gameName".into(), json!(game));
            if let Some(q) = query {
                data.insert("query".into(), json!(q));
            }
            data.insert("locations".into(), json!(locations));
            json!({ "success": true, "data": data })
        }
        Err(error) => json!({ "success": false, "error": error, "errorCode": "internal" }),
    }
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "game_knowledge".into(),
        description: Some(
            "Game knowledge and map notes. action=ask: practical guide to a `game` or a `topic` in it (build, boss, mechanic). action=memorize_location: save a place they found (`locationName`, `description`, `connections`, `importantItems`, `notes`). action=get_location_guide: recall saved places, optional `query`. For \"how do I beat X\", \"remember the key is here\"."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["ask", "memorize_location", "get_location_guide"] },
                "game": { "type": "string" },
                "topic": { "type": "string" },
                "spoilerLevel": { "type": "string", "enum": ["none", "light", "full"], "description": "none = no plot reveals; light = early game only; full = anything." },
                "helpLevel": { "type": "string", "enum": ["hint", "tip", "solution"], "description": "Only when they say how much help they want (\"just the answer\" = solution). Omit it and the help climbs by itself: hint, then tip, then solution." },
                "query": { "type": "string" },
                "locationName": { "type": "string" },
                "description": { "type": "string" },
                "connections": { "type": "array", "items": { "type": "string" } },
                "importantItems": { "type": "array", "items": { "type": "string" } },
                "notes": { "type": "string" },
            },
            "required": ["action", "game"],
        }),
        plugin_id: super::feature::GAMER_COMPANION_PLUGIN_ID.into(),
        requires_window: false,
    }
}

pub struct GameKnowledgeHandler {
    memory: Arc<SessionMemory>,
    store: Arc<ConfigStore>,
    ladder: HelpLadder,
}

impl GameKnowledgeHandler {
    pub fn new(memory: Arc<SessionMemory>, store: Arc<ConfigStore>) -> Self {
        Self { memory, store, ladder: HelpLadder::default() }
    }
}

#[async_trait]
impl ToolHandler for GameKnowledgeHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        // Toggle mestre da feature (`gamer-companion.enabled`) — ver o cabeçalho equivalente em
        // `game_session_tool.rs`.
        if !super::feature::master_enabled(&self.store) {
            return Ok(super::feature::master_disabled_response());
        }
        let action = args.get("action").and_then(Value::as_str).unwrap_or("");
        Ok(match action {
            "memorize_location" => memorize_location(&self.memory, &args),
            "get_location_guide" => get_location_guide(&self.memory, &args),
            "ask" => {
                let game = args.get("game").and_then(Value::as_str).map(str::trim).unwrap_or("");
                let topic = args.get("topic").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
                let spoiler_level = args.get("spoilerLevel").and_then(Value::as_str).unwrap_or("none");
                let help_level = self.ladder.level(&self.store, game, topic, args.get("helpLevel").and_then(Value::as_str), super::iso_time::now_ms());
                ask_response(&self.store, game, topic, spoiler_level, help_level).await
            }
            other => json!({ "success": false, "error": format!("unknown action: {other}"), "errorCode": "invalid_args" }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::session_db::SessionDb;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-gamer-game-knowledge-tool-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn handler() -> (GameKnowledgeHandler, TempDir) {
        let dir = TempDir::new();
        let store = Arc::new(ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap());
        super::super::llm_dead_port::point_the_llm_at_a_dead_port(&store);
        (GameKnowledgeHandler::new(Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap())), store), dir)
    }

    /// Toggle mestre da feature (`gamer-companion.enabled=false`) — nenhuma ação de
    /// `game_knowledge` dispacha (varredura de controles de UI de 2026-09-27).
    #[tokio::test]
    async fn call_is_denied_when_the_master_toggle_is_off() {
        let dir = TempDir::new();
        let store = Arc::new(ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap());
        super::super::llm_dead_port::point_the_llm_at_a_dead_port(&store);
        store.set("gamer-companion.enabled", json!(false)).unwrap();
        let handler = GameKnowledgeHandler::new(Arc::new(SessionMemory::new(SessionDb::open_in_memory().unwrap())), store);
        let result = handler.call(json!({ "action": "memorize_location", "game": "Hades", "locationName": "x", "description": "y" })).await.unwrap();
        assert_eq!(result["success"], false);
        assert_eq!(result["errorCode"], "denied");
    }

    #[tokio::test]
    async fn memorize_location_requires_a_location_name() {
        let (handler, _dir) = handler();
        let result = handler
            .call(json!({ "action": "memorize_location", "game": "Hades", "description": "desc" }))
            .await
            .unwrap();
        assert_eq!(result["errorCode"], "invalid_args");
    }

    #[tokio::test]
    async fn memorize_location_requires_a_description() {
        let (handler, _dir) = handler();
        let result = handler
            .call(json!({ "action": "memorize_location", "game": "Hades", "locationName": "Tartarus" }))
            .await
            .unwrap();
        assert_eq!(result["errorCode"], "invalid_args");
    }

    #[tokio::test]
    async fn memorized_locations_are_findable_afterwards() {
        let (handler, _dir) = handler();
        let saved = handler
            .call(json!({
                "action": "memorize_location",
                "game": "Hades",
                "locationName": "Tartarus",
                "description": "primeira zona",
                "connections": ["Asphodel"],
                "importantItems": ["Stygian Blade"],
            }))
            .await
            .unwrap();
        assert_eq!(saved["success"], true);
        assert!(saved["data"]["message"].as_str().unwrap().contains("Tartarus"));

        let found = handler.call(json!({ "action": "get_location_guide", "game": "Hades" })).await.unwrap();
        assert_eq!(found["data"]["locations"][0]["locationName"], "Tartarus");
    }

    #[tokio::test]
    async fn get_location_guide_reports_an_empty_note_instead_of_a_bare_empty_array() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "get_location_guide", "game": "Hades" })).await.unwrap();
        assert_eq!(result["data"]["message"], "No saved location matches this search.");
        assert!(result["data"].get("locations").is_none());
    }

    /// Sem servidor Ollama real no ambiente de teste, `ask` degrada honestamente a `internal`
    /// (código do TS de origem) — prova que o caminho de geração foi alcançado de verdade, não a
    /// resposta antiga "sem porte nesta rodada".
    #[tokio::test]
    async fn ask_reaches_generation_and_degrades_internal_without_a_live_provider() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "action": "ask", "game": "Hades" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("internal"));
        let message = result["error"].as_str().unwrap();
        assert!(message.contains("guide"), "mensagem deveria nomear a falha de geração: {message}");
    }

    #[test]
    fn render_ask_prompt_fills_every_placeholder_the_template_declares() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        let rendered = render_ask_prompt(&store, "Hollow Knight", Some("City of Tears"), "light", "tip");
        assert!(!rendered.contains("{{"), "placeholder não preenchido: {rendered}");
        assert!(rendered.contains("Hollow Knight"));
        assert!(rendered.contains("City of Tears"));
        assert!(rendered.contains("light"));
        assert!(rendered.contains("Help level: tip"));
    }

    /// Escada sem spoiler: o mesmo travamento pedido de novo sobe um degrau (pista → dica → solução),
    /// um pedido explícito pula direto, e o toggle desligado devolve o guia completo de antes.
    #[test]
    fn the_help_ladder_climbs_per_game_and_topic_and_respects_an_explicit_level() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        let ladder = HelpLadder::default();
        let climb = |topic: &str, explicit: Option<&str>| ladder.level(&store, "Hollow Knight", Some(topic), explicit, 0);

        assert_eq!(climb("Hornet", None), "hint");
        assert_eq!(climb("hornet ", None), "tip", "mesmo tópico, caixa/espaço diferentes");
        assert_eq!(climb("Hornet", None), "solution");
        assert_eq!(climb("Hornet", None), "solution");
        assert_eq!(climb("Mantis Lords", None), "hint", "outro tópico começa do zero");
        assert_eq!(climb("Soul Master", Some("solution")), "solution");
        assert_eq!(ladder.level(&store, "Hollow Knight", Some("Mantis Lords"), None, HELP_LADDER_RESET_MS + 1), "hint", "esfria depois de um tempo");

        store.set("gamer-companion.helpLadderEnabled", json!(false)).unwrap();
        assert_eq!(climb("Grimm", None), "solution");
    }

    #[test]
    fn favorite_games_prefers_user_favorites_over_the_module_default() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        store.set("user.favoriteGames", json!("Hades, Celeste")).unwrap();
        assert_eq!(favorite_games(&store), "Hades, Celeste");
    }
}
