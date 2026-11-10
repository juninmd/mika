//! Tool `recommend_media` — porte de `createRecommendMediaTool`
//! (`apps/desktop/src/features/gamer-companion/tools/recommend-media.ts`).
//!
//! **A lacuna nomeada em `modules/gamer/mod.rs`/`modules/tools/mod.rs` fecha aqui.** As duas
//! rodadas anteriores marcaram esta tool como "sem porte" por presumir que não havia cliente de
//! LLM alcançável de `modules/gamer/`. Remedido: `brain::tools::content::ContentHandler` já prova
//! o caminho — `turn::run_turn_llm_raw(&ConfigStore, ...)` é uma função `pub`, sem `AppHandle`,
//! chamável de qualquer módulo que feche sobre `Arc<ConfigStore>`. `recommend_media` é 100% LLM no
//! TS (`generateText` puro, sem sub-ação não-LLM) — o MESMO formato de `craftSocialPost`
//! (`content.rs`), só com um prompt diferente. Porte real, não degradação nomeada.
//!
//! ## Tabela TS × Rust
//!
//! | TS | Rust | Nota |
//! |---|---|---|
//! | `buildCharacterContext`/`buildUserContextBlock`/`buildPreferencesBlock` | [`render_prompt`] | Mesmos 4 config paths, mesmo texto — `{{placeholder}}` trocado por `String::replace`, não um motor de template genérico (só este arquivo usa; mesmo raciocínio de `brain/tools/prompts.rs`). |
//! | `generateText({ model, messages, maxOutputTokens: 500, temperature: 0.9 })` | `turn::run_turn_llm_raw` | `run_turn_ollama_raw` já fixa `max_tokens`/`temperature` pela config do provedor (`resolve_ollama_config`), não por parâmetro por chamada — mesma limitação que `content.rs::craft_social_post` já aceitou; sem plumbing extra por chamador individual nesta rodada. |
//! | `parseRecommendations` (JSON.parse + fallback regex de objetos) | [`parse_recommendations`] | Porte literal dos dois caminhos. |
//! | `fail(..., 'internal')` no catch de `execute` | `errorCode: "internal"` | TS usa `'internal'` aqui — DIFERENTE do `'unavailable'` que `content.rs` usa para as próprias falhas de LLM; cada arquivo mantém o código do seu TS de origem. |

use std::sync::Arc;

use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortHandle};
use crate::modules::tools::{ToolDefinition, ToolHandler};

/// Cópia literal do `.md` que `loadPrompt('recommend-media', ...)` carrega no TS.
const TEMPLATE: &str = include_str!("../../../../prompts/gamer-companion/recommend-media.md");

/// Espelha `MEDIA_TYPE_LABELS` (`recommend-media.ts`).
fn media_label(media_type: &str) -> &str {
    match media_type {
        "game" => "jogo",
        "anime" => "anime",
        "movie" => "filme",
        "series" => "série",
        other => other, // TS cai no próprio `args.type` quando o rótulo não existe no map — mesmo aqui
    }
}

fn config_str(store: &ConfigStore, path: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Espelha `getConfig('user.nickname') || getConfig('user.name') || 'senpai'`.
fn user_display_name(store: &ConfigStore) -> String {
    let nickname = config_str(store, "user.nickname");
    if !nickname.is_empty() {
        return nickname;
    }
    let name = config_str(store, "user.name");
    if !name.is_empty() {
        return name;
    }
    "senpai".to_string()
}

/// Espelha `buildUserContextBlock`.
fn user_context_block(store: &ConfigStore, user_name: &str) -> String {
    let mut block = String::new();
    let favorite_games = config_str(store, "user.favoriteGames");
    if !favorite_games.is_empty() {
        block.push_str(&format!(" The user {user_name} likes: {favorite_games}."));
    }
    let favorite_anime = config_str(store, "user.favoriteAnime");
    if !favorite_anime.is_empty() {
        block.push_str(&format!(" The user's anime: {favorite_anime}."));
    }
    block
}

/// Espelha `buildPreferencesBlock`.
fn preferences_block(mood: Option<&str>, genre: Option<&str>, recently_consumed: Option<&str>) -> String {
    let mut block = String::new();
    if let Some(mood) = mood.filter(|s| !s.is_empty()) {
        block.push_str(&format!(" Current mood: {mood}."));
    }
    if let Some(genre) = genre.filter(|s| !s.is_empty()) {
        block.push_str(&format!(" Preferred genre: {genre}."));
    }
    if let Some(recently_consumed) = recently_consumed.filter(|s| !s.is_empty()) {
        block.push_str(&format!(" Recently consumed (avoid repeating): {recently_consumed}."));
    }
    block
}

/// Espelha `buildRecommendMediaPrompt` — os 7 placeholders de `recommend-media.md`.
fn render_prompt(store: &ConfigStore, args: &RecommendMediaArgs) -> String {
    let user_name = user_display_name(store);
    TEMPLATE
        .replace("{{characterName}}", &config_str(store, "character.name"))
        .replace("{{personality}}", &config_str(store, "character.personality"))
        .replace("{{favoriteGames}}", &config_str(store, "gamer-companion.favoriteGames"))
        .replace("{{favoriteAnime}}", &config_str(store, "gamer-companion.favoriteAnime"))
        .replace("{{userContextBlock}}", &user_context_block(store, &user_name))
        .replace("{{label}}", media_label(&args.media_type))
        .replace("{{userName}}", &user_name)
        .replace(
            "{{preferencesBlock}}",
            &preferences_block(args.mood.as_deref(), args.genre.as_deref(), args.recently_consumed.as_deref()),
        )
}

/// Espelha `parseRecommendations`: `JSON.parse` direto e, se falhar, extrai objetos `{...}` por
/// regex e faz `JSON.parse` em cada um, descartando os que não parseiam.
fn parse_recommendations(text: &str) -> Vec<Value> {
    let trimmed = text.trim();
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
        return items;
    }
    // `\{[^}]+\}` — mesmo padrão do `.match(/\{[^}]+\}/g)` do TS. Uma classe negada (`[^}]`)
    // cruza quebra de linha em ambas as engines (só o `.` fica preso ao modo dotall); porte
    // literal, mesmo alcance nas duas linguagens.
    let re = Regex::new(r"\{[^}]+\}").expect("regex literal válida");
    re.find_iter(trimmed)
        .filter_map(|m| serde_json::from_str::<Value>(m.as_str()).ok())
        .take(5)
        .collect()
}

#[derive(Debug)]
struct RecommendMediaArgs {
    media_type: String,
    mood: Option<String>,
    genre: Option<String>,
    recently_consumed: Option<String>,
}

fn extract_args(args: &Value) -> Result<RecommendMediaArgs, String> {
    let media_type = args["type"].as_str().ok_or("'type' é obrigatório")?.to_string();
    if !["game", "anime", "movie", "series"].contains(&media_type.as_str()) {
        return Err(format!("invalid 'type': '{media_type}'"));
    }
    Ok(RecommendMediaArgs {
        media_type,
        mood: args["mood"].as_str().map(str::to_string),
        genre: args["genre"].as_str().map(str::to_string),
        recently_consumed: args["recentlyConsumed"].as_str().map(str::to_string),
    })
}

fn fail(message: impl Into<String>, code: &str) -> Value {
    json!({ "success": false, "error": message.into(), "errorCode": code })
}

pub(super) struct RecommendMediaHandler {
    store: Arc<ConfigStore>,
}

impl RecommendMediaHandler {
    pub(super) fn new(store: Arc<ConfigStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ToolHandler for RecommendMediaHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        // Toggle mestre da feature (`gamer-companion.enabled`) — ver o cabeçalho equivalente em
        // `game_session_tool.rs`. Evita gastar uma chamada de LLM real com a feature desligada.
        if !super::feature::master_enabled(&self.store) {
            return Ok(super::feature::master_disabled_response());
        }
        let parsed = match extract_args(&args) {
            Ok(parsed) => parsed,
            Err(message) => return Ok(fail(message, "invalid_args")),
        };

        let prompt = render_prompt(&self.store, &parsed);
        let (_handle, signal) = TurnAbortHandle::new();

        // `task_prompt: None` — TS não passa `systemPrompt` nesta chamada (mesma escolha que
        // `content.rs::craft_social_post` já documenta): o prompt de sistema é só a persona.
        let text = match turn::run_turn_llm_raw(&self.store, None, &prompt, signal, &NullObserver).await {
            Ok(result) => result.text,
            // TS: `fail(toErrorMessage(error), 'internal')` no catch de `execute` — código
            // DIFERENTE de `content.rs` (`'unavailable'`), fiel ao TS deste arquivo.
            Err(error) => return Ok(fail(format!("Failed to generate recommendations: {error}"), "internal")),
        };

        let mut recommendations = parse_recommendations(&text);
        if recommendations.is_empty() {
            return Ok(fail(
                "Não consegui ler as recomendações da resposta do modelo — responda apenas com o array JSON pedido.",
                "transient",
            ));
        }
        recommendations.truncate(5);

        Ok(json!({ "success": true, "data": { "type": parsed.media_type, "recommendations": recommendations } }))
    }
}

/// Espelha `inputSchema` de `createRecommendMediaTool` (`z.object({...})`).
pub(super) fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": {
                "type": "string",
                "enum": ["game", "anime", "movie", "series"],
                "description": "Type of media to recommend.",
            },
            "mood": {
                "type": "string",
                "maxLength": 120,
                "description": "User's current mood, e.g. relaxed, pumped, sad, bored.",
            },
            "genre": {
                "type": "string",
                "maxLength": 80,
                "description": "Preferred genre, e.g. RPG, shonen, horror, comedy.",
            },
            "recentlyConsumed": {
                "type": "string",
                "maxLength": 240,
                "description": "Titles they recently played/watched, to avoid repeats.",
            },
        },
        "required": ["type"],
    })
}

/// Cópia literal de `createRecommendMediaTool().description`.
pub(super) const DESCRIPTION: &str = "Recommend 5 games, anime, movies or series based on the user's mood, preferred genre or what they recently played/watched. Use when they ask what to play or watch next — not for facts about a specific title (use web or game_knowledge).";

pub(super) fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "recommend_media".into(),
        description: Some(DESCRIPTION.into()),
        input_schema: schema(),
        plugin_id: super::feature::GAMER_COMPANION_PLUGIN_ID.into(),
        requires_window: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-gamer-recommend-media-tool-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn handler() -> (RecommendMediaHandler, TempDir) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        super::super::llm_dead_port::point_the_llm_at_a_dead_port(&store);
        (RecommendMediaHandler::new(Arc::new(store)), dir)
    }

    /// Toggle mestre da feature (`gamer-companion.enabled=false`) — `recommend_media` não gasta
    /// uma chamada de LLM com a feature desligada (varredura de controles de UI de 2026-09-27).
    #[tokio::test]
    async fn call_is_denied_when_the_master_toggle_is_off() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        super::super::llm_dead_port::point_the_llm_at_a_dead_port(&store);
        store.set("gamer-companion.enabled", json!(false)).unwrap();
        let handler = RecommendMediaHandler::new(Arc::new(store));
        let result = handler.call(json!({ "mediaType": "game" })).await.unwrap();
        assert_eq!(result["success"], false);
        assert_eq!(result["errorCode"], "denied");
    }

    #[test]
    fn media_label_mirrors_the_ts_lookup_table() {
        assert_eq!(media_label("game"), "jogo");
        assert_eq!(media_label("anime"), "anime");
        assert_eq!(media_label("movie"), "filme");
        assert_eq!(media_label("series"), "série");
    }

    #[test]
    fn parse_recommendations_reads_a_clean_json_array() {
        let text = r#"[{"title":"Hades","reason":"roguelike divino","rating":9}]"#;
        let parsed = parse_recommendations(text);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["title"], json!("Hades"));
    }

    #[test]
    fn parse_recommendations_falls_back_to_extracting_objects_from_prose() {
        let text = "Claro! {\"title\":\"Hades\",\"reason\":\"top\",\"rating\":9} e também {\"title\":\"Frieren\",\"reason\":\"lindo\",\"rating\":10}";
        let parsed = parse_recommendations(text);
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn parse_recommendations_returns_empty_for_unparseable_text() {
        assert!(parse_recommendations("não sei recomendar nada hoje").is_empty());
    }

    #[test]
    fn extract_args_rejects_missing_type() {
        assert!(extract_args(&json!({})).is_err());
    }

    #[test]
    fn extract_args_rejects_an_invalid_type() {
        assert!(extract_args(&json!({ "type": "board-game" })).is_err());
    }

    #[test]
    fn extract_args_accepts_the_four_declared_types_with_optional_fields_absent() {
        for media_type in ["game", "anime", "movie", "series"] {
            let parsed = extract_args(&json!({ "type": media_type })).unwrap();
            assert_eq!(parsed.media_type, media_type);
            assert!(parsed.mood.is_none());
        }
    }

    #[tokio::test]
    async fn call_with_missing_type_is_a_named_invalid_args_error_not_a_panic() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({})).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }

    /// Sem servidor Ollama real no ambiente de teste, o caminho degrada honestamente a `internal`
    /// (código do TS deste arquivo) — prova que a validação passou e a geração foi alcançada de
    /// verdade, não a resposta antiga "sem cliente de LLM alcançável".
    #[tokio::test]
    async fn call_with_a_valid_type_reaches_generation_and_degrades_internal_without_a_live_provider() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({ "type": "game" })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("internal"));
        let message = result["error"].as_str().unwrap();
        assert!(message.contains("recommendations"), "mensagem deveria nomear a falha de geração: {message}");
    }

    #[test]
    fn render_prompt_fills_every_placeholder_the_template_declares() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        let args = RecommendMediaArgs {
            media_type: "anime".to_string(),
            mood: Some("empolgado".to_string()),
            genre: None,
            recently_consumed: None,
        };
        let rendered = render_prompt(&store, &args);
        assert!(!rendered.contains("{{"), "placeholder não preenchido: {rendered}");
        assert!(rendered.contains("Mika"), "default de character.name deveria aparecer");
        assert!(rendered.contains("empolgado"));
    }
}
