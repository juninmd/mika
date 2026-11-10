//! Tool `user_profile` (LLM) — dispatch e leitura config-only (`get`/`query`/`learn`) da tool
//! `user_profile`, porte de `apps/desktop/src/features/user/tools/index.ts`. As 3 escritas sociais
//! (`set`/`forget`/`set_personality`) moram em `social_actions.rs` — ver o cabeçalho de `mod.rs`
//! para a tabela de paridade completa e as divergências documentadas.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use crate::config::store::ConfigStore;
use crate::modules::memory::social::SocialMemoryService;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::social_actions;

/// Espelha `USER_PROFILE_KEYS` (`apps/desktop/src/shared/user-profile-fields.ts`) — ORDEM importa
/// para o pin de paridade no teste `profile_keys_match_the_typescript_contract`.
///
/// `pub(crate)`: `features::privacy::clear::reset_user_profile` (`clear_history`) reusa esta MESMA
/// lista em vez de duplicá-la — duas listas de campo divergiriam em silêncio no dia em que o
/// perfil ganhasse um campo novo aqui e não lá.
pub(crate) const USER_PROFILE_KEYS: &[&str] = &[
    "name",
    "nickname",
    "birthDate",
    "age",
    "gender",
    "country",
    "state",
    "city",
    "timezone",
    "profession",
    "company",
    "role",
    "skills",
    "hobbies",
    "favoriteGames",
    "favoriteAnime",
    "favoriteMusic",
    "favoriteFood",
    "partnerName",
    "partnerRelationship",
    "partnerLikes",
    "partnerNotes",
    "personalityTraits",
    "communicationStyle",
    "communicationNotes",
    "bio",
    "notes",
    "learnedByLLM",
];

const MAX_LEARNED_BY_LLM_CHARS: usize = 1900;

const NO_SOCIAL_MESSAGE: &str =
    "The stable profile store is unavailable right now (brain memory is not running).";

fn no_social() -> Value {
    json!({ "success": false, "error": NO_SOCIAL_MESSAGE, "errorCode": "unavailable" })
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Carimbo `AAAA-MM-DD` do dia LOCAL (fuso do SO) — ver item 1 do cabeçalho de `mod.rs`.
fn date_stamp<Tz: chrono::TimeZone>(now: &chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    now.format("%Y-%m-%d").to_string()
}

/// Espelha `capLearnedByLLM` (`handlers/learned-entries.ts`): mantém só as ÚLTIMAS linhas que
/// cabem no teto, nunca corta uma linha no meio.
fn cap_learned_by_llm(text: &str) -> String {
    if text.len() <= MAX_LEARNED_BY_LLM_CHARS {
        return text.to_string();
    }
    let lines: Vec<&str> = text.split('\n').filter(|line| !line.is_empty()).collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut size = 0usize;
    for line in lines.iter().rev() {
        if size + line.len() + 1 > MAX_LEARNED_BY_LLM_CHARS {
            // A linha mais recente sozinha já estoura o teto: sem isto `kept` ficava vazio e o
            // resultado apagava TODO o aprendizado anterior (e o fato novo) calado, com `success: true`.
            if kept.is_empty() {
                let mut end = MAX_LEARNED_BY_LLM_CHARS;
                while !line.is_char_boundary(end) {
                    end -= 1;
                }
                kept.push(&line[..end]);
            }
            break;
        }
        kept.insert(0, line);
        size += line.len() + 1;
    }
    kept.join("\n")
}

fn config_profile(store: &ConfigStore) -> Value {
    let mut profile = Map::new();
    for key in USER_PROFILE_KEYS {
        let path = format!("user.{key}");
        if let Some(value) = store.get(&path) {
            let is_empty_string = value.as_str().is_some_and(str::is_empty);
            if !value.is_null() && !is_empty_string {
                profile.insert((*key).to_string(), value);
            }
        }
    }
    Value::Object(profile)
}

/// = `socialProfileOrNull()?.getUserProfile() ?? null` do TS. `get_graph()` é a única leitura
/// pública de `SocialMemoryService` que alcança a tabela `social_user_profile` (não existe um
/// `get_profile` dedicado) — custa ler pessoas/relações/fatos junto, aceito porque `user_profile`
/// já é uma tool de baixa frequência. Falha de leitura (banco corrompido) degrada para `null`, nunca
/// propaga erro para uma ação que só está tentando MOSTRAR o perfil.
fn stable_profile(social: Option<SocialMemoryService<'static>>) -> Value {
    social.and_then(|svc| svc.get_graph().ok()).and_then(|graph| graph.profile).unwrap_or(Value::Null)
}

fn run_get(store: &ConfigStore, social: Option<SocialMemoryService<'static>>, query: Option<&str>) -> Value {
    json!({
        "success": true,
        "data": {
            "query": query,
            "profile": config_profile(store),
            "stable": stable_profile(social),
        },
    })
}

fn run_learn(store: &ConfigStore, fact: &str, category: &str) -> Value {
    // "Aprender" é gravar memória: com `memory.enabled=false` nada é escrito (antes só o perfil
    // social era gateado; `user.learnedByLLM`, `user.name` e `user.profession` entravam mesmo assim).
    if !crate::modules::memory::memory_recording_allowed(store) {
        return json!({ "success": false, "error": "memory recording is disabled (memory.enabled=false); nothing was learned", "errorCode": "denied" });
    }
    let current = store.get("user.learnedByLLM").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    let bullet = format!("- [{}] ({category}): {fact}", date_stamp(&chrono::Local::now()));
    let updated = cap_learned_by_llm(&if current.is_empty() { bullet } else { format!("{}\n{bullet}", current.trim()) });

    if let Err(error) = store.set("user.learnedByLLM", json!(updated)) {
        return json!({ "success": false, "error": error, "errorCode": "unavailable" });
    }

    if category == "name" {
        let current_name = store.get("user.name").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        if current_name.is_empty() {
            let _ = store.set("user.name", json!(fact));
        }
    } else if category == "profession" {
        let current_profession = store.get("user.profession").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        if current_profession.is_empty() {
            let _ = store.set("user.profession", json!(fact));
        }
    }

    json!({
        "success": true,
        "data": { "fact": fact, "category": category, "message": format!("Successfully learned and persisted: {fact}") },
    })
}

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "user_profile".into(),
        description: Some(
            "The user's profile (essentials are already in your context — don't call it every message). action=learn: silently store one detail they just shared (`fact` + `category`). action=get / query: read what's known. action=set: stable profile (preferredName, pronouns, timezone, languages, facts). action=set_personality: how they like to be talked to. action=forget: remove `fields`/`facts`, or all=true for everything — only when they ask to be forgotten."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["get", "query", "learn", "set", "forget", "set_personality"] },
                "fact": { "type": "string", "description": "action=learn: the detail as a short sentence, e.g. \"works as a nurse\"." },
                "category": {
                    "type": "string",
                    "enum": ["name", "profession", "location", "hobby", "preference", "personality", "other"],
                },
                "query": { "type": "string" },
                "preferredName": { "type": "string" },
                "pronouns": { "type": "string" },
                "timezone": { "type": "string", "description": "action=set: IANA zone, e.g. \"America/Sao_Paulo\"." },
                "languages": { "type": "array", "items": { "type": "string" } },
                "facts": { "type": "array", "items": { "type": "string" } },
                "fields": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["preferredName", "pronouns", "timezone", "personality"] },
                },
                "all": { "type": "boolean" },
                "communicationStyle": { "type": "string", "enum": ["formal", "casual", "playful", "direct"] },
                "humorTolerance": { "type": "string", "enum": ["low", "medium", "high"], "description": "How much teasing they enjoy." },
                "energyLevel": { "type": "string", "enum": ["low", "medium", "high"] },
                "topInterests": { "type": "array", "items": { "type": "string" } },
            },
            "required": ["action"],
        }),
        plugin_id: "mika.user".into(),
        requires_window: false,
    }
}

struct UserProfileToolHandler {
    store: Arc<ConfigStore>,
    /// Closure, não uma referência guardada: `SocialMemoryService` empresta de `MEMORY_STORE`
    /// (`OnceLock` de processo em `modules::memory`), que pode não estar aberto ainda no instante
    /// do `register()` (`register(store)` não recebe `AppHandle`, ao contrário de
    /// `person_catalog::register_tools`) — cada chamada resolve de novo. Em produção é
    /// `memory::social_reader` (o MESMO getter que `brain::tools::memory_console` já usa para o
    /// caso "sem `AppHandle` no ponto de registro"); em teste, uma closure que fecha sobre um
    /// `MemoryStore` isolado por teste (`Box::leak`, mesmo truque de `person_catalog::tool.rs`) —
    /// nunca um dublê do serviço, só a fonte do `MemoryStore` muda.
    social: Box<dyn Fn() -> Option<SocialMemoryService<'static>> + Send + Sync>,
}

#[async_trait]
impl ToolHandler for UserProfileToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        match action {
            "get" | "query" => {
                Ok(run_get(&self.store, (self.social)(), args.get("query").and_then(Value::as_str)))
            }
            "learn" => {
                let fact = args.get("fact").and_then(Value::as_str);
                let category = args.get("category").and_then(Value::as_str);
                match (fact, category) {
                    (Some(fact), Some(category)) => {
                        let result = run_learn(&self.store, fact, category);
                        // = `fillPreferredNameIfEmpty` — melhor esforço, só quando `learn` gravou
                        // de verdade e a categoria é `name` (mesmo gate do TS).
                        if category == "name" && result["success"] == json!(true) {
                            if let Some(social) = (self.social)() {
                                social_actions::fill_preferred_name_if_empty(&social, self.recording_allowed(), fact, now_ms());
                            }
                        }
                        Ok(result)
                    }
                    (None, _) => Ok(json!({ "success": false, "error": "fact is required for action=learn.", "errorCode": "invalid_args" })),
                    (_, None) => Ok(json!({ "success": false, "error": "category is required for action=learn.", "errorCode": "invalid_args" })),
                }
            }
            "set" => Ok(self.with_social(|social| social_actions::handle_set(social, self.recording_allowed(), &args, now_ms()))),
            "forget" => Ok(self.with_social(|social| social_actions::handle_forget(&self.store, social, self.recording_allowed(), &args, now_ms()))),
            "set_personality" => Ok(self.with_social(|social| social_actions::handle_set_personality(social, self.recording_allowed(), &args, now_ms()))),
            other => Err(format!("user_profile: unknown action \"{other}\"")),
        }
    }
}

impl UserProfileToolHandler {
    /// = `memory_recording_allowed(&self.store)`, lido FRESCO a cada chamada — mesma escolha de
    /// `person_catalog::tool.rs::recording_allowed` (a tool tem `ConfigStore` disponível, ao
    /// contrário de `memory_console`, que usa o cache de processo).
    fn recording_allowed(&self) -> bool {
        crate::modules::memory::memory_recording_allowed(&self.store)
    }

    /// `None` de `(self.social)()` é a ÚNICA causa real hoje: `SocialMemoryService` existe e roda
    /// em produção (`modules::memory::social`), só falta o `MemoryStore` de processo ter aberto.
    fn with_social(&self, run: impl FnOnce(&SocialMemoryService<'static>) -> Value) -> Value {
        match (self.social)() {
            Some(social) => run(&social),
            None => no_social(),
        }
    }
}

/// Chamado por `modules::register_config_bound_tools(app)`. Registra a tool `user_profile` no
/// `ToolRegistry` nativo e a declaração de settings da aba `user` (7 grupos, ver `super::feature`)
/// em `crate::feature_registry::shared()` (canal `mika:settings-declarations`). Um registro só por
/// `tabId`: isto SUBSTITUI o `declare_namespace` de cartão único que vivia aqui — mesma razão do
/// `character/tool.rs`.
pub fn register(store: Arc<ConfigStore>) {
    crate::modules::tools::registry().register(
        tool_definition(),
        Arc::new(UserProfileToolHandler { store: Arc::clone(&store), social: Box::new(crate::modules::memory::social_reader) }),
    );
    super::plugin_test::register();

    if let Some(settings) = super::feature::declaration(store.schema()) {
        crate::feature_registry::shared().register(
            "mika.user",
            crate::feature_registry::FeatureRegistration { enabled: true, settings: vec![settings], ..Default::default() },
        );
    } else {
        log::warn!("user: namespace 'user' ausente do SchemaRegistry — settings não declaradas");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::text;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-user-tool-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (Arc<ConfigStore>, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![
            Namespace {
                name: "user",
                fields: vec![
                    text("name", ""),
                    text("profession", ""),
                    text("learnedByLLM", ""),
                    text("nickname", ""),
                    text("birthDate", ""),
                ],
            },
            Namespace { name: "memory", fields: vec![crate::config::schema::boolean("enabled", true)] },
        ]);
        (Arc::new(ConfigStore::open(&dir.0, schema).unwrap()), dir)
    }

    fn social_unavailable() -> Box<dyn Fn() -> Option<SocialMemoryService<'static>> + Send + Sync> {
        Box::new(|| None)
    }

    /// `SocialMemoryService` REAL sobre um `MemoryStore` ISOLADO por teste (`Box::leak`, mesmo
    /// truque que `person_catalog::tool.rs` usa para `&'static MemoryStore`) — nunca um dublê do
    /// serviço, e nunca compartilhado entre testes: dois testes de despacho escrevendo no MESMO
    /// `MemoryStore` (um `OnceLock` reusado) se pisariam sob `cargo test` paralelo, já que o
    /// perfil social é UM registro por processo (`id: "user"`).
    fn fresh_social() -> Box<dyn Fn() -> Option<SocialMemoryService<'static>> + Send + Sync> {
        use crate::modules::memory::MemoryStore;
        let mem: &'static MemoryStore = Box::leak(Box::new(MemoryStore::open_in_memory().unwrap()));
        Box::new(move || Some(SocialMemoryService::new(mem)))
    }

    /// Mesmo `MemoryStore` isolado de [`fresh_social`], mas com o perfil pré-semeado — a chave crua
    /// é a MESMA que `memory::social::mod.rs::tests` já usa (`store::record_key` é privado do
    /// módulo social).
    fn social_with_seeded_profile() -> Box<dyn Fn() -> Option<SocialMemoryService<'static>> + Send + Sync> {
        use crate::modules::memory::MemoryStore;
        let mem: &'static MemoryStore = Box::leak(Box::new(MemoryStore::open_in_memory().unwrap()));
        mem.set("mika:social:social_user_profile:user", r#"{"id":"user","preferredName":"Ana","languages":[],"facts":[],"updatedAt":1}"#, None).unwrap();
        Box::new(move || Some(SocialMemoryService::new(mem)))
    }

    #[test]
    fn profile_keys_match_the_typescript_contract() {
        let source = include_str!("../../../../../src/shared/user-profile-fields.ts");
        for key in USER_PROFILE_KEYS {
            assert!(source.contains(&format!("'{key}'")), "chave '{key}' não bate com user-profile-fields.ts");
        }
    }

    #[test]
    fn config_profile_omits_empty_and_absent_fields() {
        let (store, _dir) = store();
        store.set("user.name", json!("Ana")).unwrap();

        let profile = config_profile(&store);
        assert_eq!(profile["name"], json!("Ana"));
        assert!(profile.get("profession").is_none(), "campo vazio vazou no perfil");
    }

    #[tokio::test]
    async fn tool_learn_persists_the_fact_and_fills_empty_name() {
        let (store, _dir) = store();
        let handler = UserProfileToolHandler { store: Arc::clone(&store), social: social_unavailable() };

        let result = handler
            .call(json!({ "action": "learn", "fact": "Ana", "category": "name" }))
            .await
            .unwrap();
        assert_eq!(result["success"], json!(true));
        assert_eq!(store.get("user.name"), Some(json!("Ana")));
        let learned = store.get("user.learnedByLLM").unwrap();
        assert!(learned.as_str().unwrap().contains("(name): Ana"), "got: {learned}");
    }

    /// NEGATIVO — nome já preenchido não é sobrescrito por um `learn` subsequente.
    #[tokio::test]
    async fn tool_learn_does_not_overwrite_an_already_set_name() {
        let (store, _dir) = store();
        store.set("user.name", json!("Original")).unwrap();
        let handler = UserProfileToolHandler { store: Arc::clone(&store), social: social_unavailable() };

        handler.call(json!({ "action": "learn", "fact": "Outro", "category": "name" })).await.unwrap();
        assert_eq!(store.get("user.name"), Some(json!("Original")));
    }

    #[tokio::test]
    async fn tool_learn_without_fact_or_category_is_a_named_invalid_args_error() {
        let (store, _dir) = store();
        let handler = UserProfileToolHandler { store, social: social_unavailable() };

        let missing_fact = handler.call(json!({ "action": "learn", "category": "name" })).await.unwrap();
        assert_eq!(missing_fact["errorCode"], json!("invalid_args"));

        let missing_category = handler.call(json!({ "action": "learn", "fact": "x" })).await.unwrap();
        assert_eq!(missing_category["errorCode"], json!("invalid_args"));
    }

    /// = `fillPreferredNameIfEmpty` disparado por `learn(category=name)`: com memória social de pé,
    /// o `preferredName` do perfil social também é preenchido — não só a cópia em `ConfigStore`.
    #[tokio::test]
    async fn tool_learn_of_a_name_also_fills_the_social_profiles_preferred_name() {
        let (store, _dir) = store();
        let social = fresh_social();
        let handler = UserProfileToolHandler { store, social };

        handler.call(json!({ "action": "learn", "fact": "Ada", "category": "name" })).await.unwrap();

        let stable = handler.call(json!({ "action": "get" })).await.unwrap();
        assert_eq!(stable["data"]["stable"]["preferredName"], json!("Ada"));
    }

    /// NEGATIVO — sem `MemoryStore` de processo, as 3 ações degradam nomeando a causa real (o
    /// serviço não está de pé), nunca fingindo sucesso.
    #[tokio::test]
    async fn set_forget_and_set_personality_degrade_honestly_when_social_memory_is_down() {
        let (store, _dir) = store();
        let handler = UserProfileToolHandler { store, social: social_unavailable() };

        for action in ["set", "forget", "set_personality"] {
            let result = handler.call(json!({ "action": action })).await.unwrap();
            assert_eq!(result["success"], json!(false), "{action} devia degradar");
            assert_eq!(result["errorCode"], json!("unavailable"));
            assert_eq!(result["error"], json!(NO_SOCIAL_MESSAGE));
        }
    }

    /// As 3 escritas sociais agora dispacham DE VERDADE pela tool — não só pelo
    /// `SocialMemoryService` direto (já coberto em `social_actions.rs`/`memory::social::tests`).
    #[tokio::test]
    async fn set_forget_and_set_personality_dispatch_for_real_through_the_tool() {
        let (store, _dir) = store();
        let handler = UserProfileToolHandler { store, social: fresh_social() };

        let set = handler.call(json!({ "action": "set", "preferredName": "Ada" })).await.unwrap();
        assert_eq!(set["success"], json!(true));
        assert_eq!(set["data"]["profile"]["preferredName"], json!("Ada"));

        let personality = handler.call(json!({ "action": "set_personality", "communicationStyle": "playful" })).await.unwrap();
        assert_eq!(personality["data"]["profile"]["personality"]["communicationStyle"], json!("playful"));

        let forgotten = handler.call(json!({ "action": "forget", "fields": ["preferredName"] })).await.unwrap();
        assert_eq!(forgotten["success"], json!(true));
        assert_eq!(forgotten["data"]["profile"]["preferredName"], Value::Null);
    }

    /// `action=get`/`query` já lia `stable` sempre `null`, mesmo com dado social real disponível —
    /// prova que agora reflete o `SocialMemoryService` de verdade em vez do valor fixo.
    #[tokio::test]
    async fn get_reflects_the_real_stable_profile_when_social_memory_is_up() {
        let (store, _dir) = store();
        let handler = UserProfileToolHandler { store, social: social_with_seeded_profile() };

        let result = handler.call(json!({ "action": "get" })).await.unwrap();
        assert_eq!(result["data"]["stable"]["preferredName"], json!("Ana"));
    }

    /// NEGATIVO — sem memória social de pé, `stable` continua `null`, nunca inventado.
    #[tokio::test]
    async fn get_stable_is_null_when_social_memory_is_down() {
        let (store, _dir) = store();
        let handler = UserProfileToolHandler { store, social: social_unavailable() };

        let result = handler.call(json!({ "action": "get" })).await.unwrap();
        assert_eq!(result["data"]["stable"], json!(null));
    }

    #[test]
    fn cap_learned_by_llm_keeps_the_newest_lines_whole() {
        let long_line = "x".repeat(100);
        let text = (0..30).map(|i| format!("{long_line}-{i}")).collect::<Vec<_>>().join("\n");
        let capped = cap_learned_by_llm(&text);
        assert!(capped.len() <= MAX_LEARNED_BY_LLM_CHARS);
        assert!(capped.ends_with("-29"), "perdeu a linha mais recente: {capped}");
    }

    /// Um fato maior que o teto sozinho: antes a lista inteira virava "" (aprendizado apagado).
    #[test]
    fn an_oversized_newest_line_does_not_wipe_the_learned_notes() {
        let text = format!("- [2026-10-01] (hobby): joga xadrez\n- [2026-10-02] (other): {}", "ã".repeat(2000));
        let capped = cap_learned_by_llm(&text);
        assert!(!capped.is_empty(), "o aprendizado foi apagado");
        assert!(capped.len() <= MAX_LEARNED_BY_LLM_CHARS, "{}", capped.len());
        assert!(capped.starts_with("- [2026-10-02] (other): ã"), "{capped}");
    }

    #[test]
    fn learning_is_denied_and_writes_nothing_while_memory_recording_is_off() {
        let (store, _dir) = store();
        store.set("memory.enabled", json!(false)).unwrap();

        let result = run_learn(&store, "se chama Ana", "name");

        assert_eq!(result["errorCode"], json!("denied"));
        assert_eq!(store.get("user.learnedByLLM").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(), "");
        assert_eq!(store.get("user.name").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(), "");
    }

    /// 2024-01-01T00:00Z ainda é 31/12 em BRT: o fato aprendido às 21h de réveillon é de 2023.
    #[test]
    fn learn_stamp_is_the_local_calendar_day() {
        use chrono::TimeZone;
        let utc = chrono::Utc.timestamp_millis_opt(1_704_067_200_000).unwrap();
        let brt = utc.with_timezone(&chrono::FixedOffset::west_opt(3 * 3600).unwrap());
        assert_eq!(date_stamp(&utc), "2024-01-01");
        assert_eq!(date_stamp(&brt), "2023-12-31");
    }

    #[tokio::test]
    async fn register_adds_the_tool_and_a_settings_declaration() {
        let (store, _dir) = store();
        register(store);

        let definition = crate::modules::tools::registry().get("user_profile").expect("user_profile não registrada");
        assert_eq!(definition.plugin_id, "mika.user");

        let feature = crate::feature_registry::shared().get(&crate::feature_registry::FeatureId::from("mika.user"));
        let feature = feature.expect("settings de 'user' não registradas em feature_registry::shared()");
        // Prova que o registro usa `super::feature::declaration` (7 grupos), não mais o cartão
        // único de `declare_namespace` (1 grupo).
        assert_eq!(feature.settings[0]["groups"].as_array().unwrap().len(), 7);
    }
}
