//! Tool `clear_history` (LLM, DESTRUTIVA) — port PARCIAL de
//! `apps/desktop/src/features/privacy/tools/index.ts` (`clear_history`/`wipeEveryStore`). Ver o
//! cabeçalho de `mod.rs` para o porquê de portar parcial em vez de não portar nada.
//!
//! ## O que esta ação apaga DE VERDADE (remedido em 2026-09-25 — o gap de 2026-09-22 ficou velho)
//!
//! | Store TS (`wipeEveryStore`) | Rust hoje | Como |
//! |---|---|---|
//! | `episodic memory` | **apagado** | `crate::modules::memory::MemoryStore::{keys,del}` sobre `mika:mem:*` — MESMO glob que `episodic::queries::clear` usa |
//! | `user profile` | **apagado** | cada `user.<campo>` de [`crate::modules::features::user::USER_PROFILE_KEYS`] volta ao default do `SchemaRegistry` |
//! | `reminders` | **apagado** | `SchedulerStore::clear_all` |
//! | `tts cache` | **apagado** | `crate::modules::voice::tts_cache().clear()` — o MESMO cache que as tools `speak`/`voice` usam |
//! | `person catalog`, `social memory`, `game sessions`, `screen captures`, `safe secrets`, `log files`, `exported data archives`, `usage habits`, `update backups`, `response reports` | **apagado** | ver [`super::clear_ext::ExtraStores`] — a tabela completa (função TS × mecanismo Rust) mora no cabeçalho de `clear_ext.rs` |
//! | `conversation history` | **apagado** (2026-09-25) | `crate::modules::chat::clear_all_history` — a sessão corrente (como `mika:chat-clear`) E as demais sessões de `chat.db` |
//! | `chat session memory` | **apagado** | `SessionMemoryAccumulator::clear` — sem isto o acumulador seguia no prompt e era regravado no episódico na ociosidade, ressuscitando o que foi apagado |
//! | `goals` (metade de `goals and mood`) | **apagado** (2026-09-25) | ver [`super::clear_more`] — `brain::reflection::goals::clear_goals` |
//! | `local telemetry` | **SKIPPED, nomeado** | `TelemetryService.clearAll` apaga `telemetry_events`; `modules/telemetry.rs` só tem `token_usage` (dado DIFERENTE) |
//! | `mood` (a outra metade de `goals and mood`) | **SKIPPED, nomeado** | `mood/persistence.rs::character.affinityScore` é um mecanismo ADITIVO separado do que `clearGoalsAndMood` de fato apagava (`this.emotional`, numa tabela `kv` própria do `PluginStore` de `brain` — ver o cabeçalho de `clear_more.rs`); nenhum Rust hoje tem essa tabela |
//! | `screen analysis cache`, `ocr cache` | **SKIPPED, nomeado** | caches em memória (DI) sem equivalente Rust — visão que depende de LLM ainda degrada honesto |
//! | `browser session` | **SKIPPED, nomeado** | o cookie jar existe (`guarded_fetch::guarded_http_client`), mas é um `OnceLock<reqwest::Client>` sem API de reset — exigiria reestruturar o cliente HTTP compartilhado |
//!
//! Isto NUNCA apaga mais do que a tabela acima: nenhum dos 6 skipped é tocado, mesmo de leve, e
//! nenhum dos 9 recém-cobertos apaga além da função TS correspondente (medido função a função, não
//! inferido do nome — ver o relatório da tarefa). `confirm: true` continua obrigatório e a tool é
//! registrada via `register_destructive` — toda chamada passa pelo `ApprovalGateService` antes de
//! despachar (ver `modules/tools/registry/mod.rs`, §"Gate de aprovação").

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::modules::features::scheduler::{db_path as scheduler_db_path, SchedulerStore};
use crate::modules::features::user::USER_PROFILE_KEYS;
use crate::modules::memory::MemoryStore;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::clear_ext::ExtraStores;
use super::clear_more;

/// Os 5 stores que este build não alcança ainda — nomeados um a um, nunca resumidos, para que
/// quem ler a resposta da tool saiba EXATAMENTE o que não foi tocado. `pub(super)`: `plugin_test.rs`
/// reusa a MESMA lista para `mika:plugin-test` em vez de duplicá-la (ver o cabeçalho de lá).
pub(super) const SKIPPED_STORES: [&str; 5] = [
    "local telemetry",
    "mood",
    "screen analysis cache",
    "ocr cache",
    "browser session",
];

const DESCRIPTION: &str = "Permanently delete the user's data this build can reach: conversation history, episodic memory, user profile, reminders, goals, TTS audio cache, person catalog (and photos), social memory, game sessions, stored screen captures, saved secrets, log files, previous export .zip files, usage habits (foreground-program counter), pre-update backups and saved response reports. Cannot be undone. Only when the user explicitly asks to wipe or clear their data/history. NOT covered yet: local telemetry, mood state, screen-analysis and OCR caches, embedded browser session.";

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "clear_history".into(),
        description: Some(DESCRIPTION.into()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "confirm": {
                    "type": "boolean",
                    "description": "Explicit confirmation parameter. Must be set to true to authorize clearing the history.",
                },
            },
            "required": ["confirm"],
        }),
        plugin_id: "mika.privacy".into(),
        requires_window: false,
    }
}

/// Espelha `episodic::queries::clear` (`DELETE` sobre `mika:mem:*`) — replicado aqui em vez de
/// chamado porque `episodic` é um submódulo PRIVADO de `crate::modules::memory` (ver `mod.rs`).
/// Só usa API pública de [`MemoryStore`], sem interpretar valor nenhum — nenhum risco de
/// divergência de parsing, ao contrário de `get_all`.
fn clear_episodic_memory(memory: &MemoryStore) -> Result<(), String> {
    // Também os caches de vetor em RAM, que guardam o texto do que foi embeddado.
    crate::modules::memory::clear_embedding_caches();
    for key in memory.keys("mika:mem:*")? {
        memory.del(&key)?;
    }
    Ok(())
}

/// Espelha `erase-ports.ts::clearUserProfile`: cada campo volta ao DEFAULT do schema (não a
/// `null`/`undefined` — `ConfigStore::set` recusa um valor que não valida contra o schema).
fn clear_user_profile(config: &ConfigStore) -> Result<(), String> {
    let defaults = config.schema().defaults();
    let user_defaults = defaults.get("user").cloned().unwrap_or(Value::Object(Map::new()));
    for key in USER_PROFILE_KEYS {
        let default = user_defaults.get(*key).cloned().unwrap_or(Value::Null);
        config.set(&format!("user.{key}"), default)?;
    }
    Ok(())
}

struct ClearHistoryHandler {
    config: Arc<ConfigStore>,
    memory: &'static MemoryStore,
    scheduler: SchedulerStore,
    /// Os 7 stores da rodada de 2026-09-22 (person catalog/social memory/game sessions/screen
    /// captures/safe secrets/log files/exported archives) — agrupados em `clear_ext.rs` só pelo
    /// limite de linhas deste arquivo, ver o cabeçalho de lá.
    extra: ExtraStores,
    /// Fecha sobre o `AppHandle` de produção (`crate::modules::chat::clear_all_history`: todas as sessões) — uma
    /// closure, não uma chamada direta, para este arquivo continuar testável sem forjar
    /// `AppHandle` (este crate não habilita a feature `test-support` do `tauri`, mesma nota de
    /// `chat/history_store.rs`). Ver [`super::clear_more`] para o porquê deste store deixar de
    /// estar em [`SKIPPED_STORES`].
    clear_conversation_history: Box<dyn Fn() -> Result<(), String> + Send + Sync>,
}

/// Espelha `wipeEveryStore`: tenta cada store alcançável, acumula `cleared`/`failed`, e devolve
/// `skipped` para o resto — MESMA forma de resposta do TS (`Removed: … . NOT covered: …`).
fn wipe_everything(handler: &ClearHistoryHandler) -> Value {
    let mut cleared: Vec<&str> = Vec::new();
    let mut failed: Vec<(&str, String)> = Vec::new();

    match clear_episodic_memory(handler.memory) {
        Ok(()) => cleared.push("episodic memory"),
        Err(error) => failed.push(("episodic memory", error)),
    }

    match clear_user_profile(&handler.config) {
        Ok(()) => cleared.push("user profile"),
        Err(error) => failed.push(("user profile", error)),
    }

    match handler.scheduler.clear_all() {
        Ok(_) => cleared.push("reminders"),
        Err(error) => failed.push(("reminders", error)),
    }

    match crate::modules::voice::tts_cache().lock() {
        Ok(mut cache) => {
            cache.clear();
            cleared.push("tts cache");
        }
        Err(_) => failed.push(("tts cache", "cache de TTS envenenado".into())),
    }

    // O acumulador de memória de SESSÃO vive só no processo: sem limpar, os fatos apagados seguiam
    // no prompt e eram regravados no episódico depois (ociosidade de 20 min ou saída do app).
    crate::modules::brain::post_turn::session_accumulator::accumulator().clear();
    cleared.push("chat session memory");

    match (handler.clear_conversation_history)() {
        Ok(()) => cleared.push("conversation history"),
        Err(error) => failed.push(("conversation history", error)),
    }

    clear_more::clear_goals(handler.memory, &mut cleared, &mut failed);

    handler.extra.wipe(handler.memory, &mut cleared, &mut failed);

    let skipped: Vec<&str> = SKIPPED_STORES.to_vec();

    if failed.is_empty() {
        return json!({
            "success": true,
            "data": { "cleared": cleared, "skipped": skipped },
            "message": format!(
                "Removed: {}. NOT covered (no Rust backend yet): {}.",
                cleared.join(", "),
                skipped.join(", "),
            ),
        });
    }

    json!({
        "success": false,
        "error": format!(
            "Partially cleared. Removed: {}. Failed: {}.",
            if cleared.is_empty() { "nothing".to_string() } else { cleared.join(", ") },
            failed.iter().map(|(store, error)| format!("{store} ({error})")).collect::<Vec<_>>().join(", "),
        ),
        "errorCode": "internal",
        "data": { "cleared": cleared, "failed": failed.iter().map(|(store, _)| *store).collect::<Vec<_>>(), "skipped": skipped },
    })
}

#[async_trait]
impl ToolHandler for ClearHistoryHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let confirm = args.get("confirm").and_then(Value::as_bool).unwrap_or(false);
        if !confirm {
            return Ok(json!({
                "success": false,
                "error": "Confirmation is required to clear the chat history.",
                "errorCode": "invalid_args",
            }));
        }
        Ok(wipe_everything(self))
    }
}

/// Chamado por `super::register(store, app)`. Abre os stores de PROCESSO (mesmos arquivos que a
/// produção usa) uma única vez, no boot — nunca por chamada.
pub(crate) fn register(store: Arc<ConfigStore>, app: &AppHandle) -> Result<(), String> {
    let memory = crate::modules::memory::memory_store(app)?;
    let scheduler = SchedulerStore::open(&scheduler_db_path(app)?)?;
    let extra = ExtraStores::open(app)?;
    let app_handle = app.clone();
    let clear_conversation_history: Box<dyn Fn() -> Result<(), String> + Send + Sync> =
        Box::new(move || crate::modules::chat::clear_all_history(&app_handle));

    crate::modules::tools::registry().register_destructive(
        tool_definition(),
        Arc::new(ClearHistoryHandler { config: store, memory, scheduler, extra, clear_conversation_history }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-privacy-clear-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Schema REAL de produção (`config::schemas::registry()`, não um recorte à mão): `USER_PROFILE_KEYS`
    /// tem 28 campos e `clear_user_profile` os grava TODOS — um schema de teste que só declarasse
    /// 2 deles quebraria no 3º campo com "caminho não declarado", mascarando o comportamento real.
    fn config_store() -> (Arc<ConfigStore>, TempDir) {
        let dir = TempDir::new();
        (Arc::new(ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap()), dir)
    }

    /// Teste: a limpeza do histórico de conversa é injetada como closure `Ok(())`, nunca um
    /// `AppHandle` forjado (este crate não habilita `test-support` do `tauri` — mesma nota de
    /// `chat/history_store.rs`). O que ela de fato apaga (`HistoryStore::clear` + `chat.db`) já tem
    /// prova própria em `chat/history.rs`/`chat/mod.rs`; aqui só a fiação de `wipe_everything`
    /// (`Ok` vira `cleared`, `Err` vira `failed`) importa.
    fn handler(config: Arc<ConfigStore>, dir: &TempDir) -> ClearHistoryHandler {
        handler_with_history_result(config, dir, Ok(()))
    }

    fn handler_with_history_result(config: Arc<ConfigStore>, dir: &TempDir, result: Result<(), String>) -> ClearHistoryHandler {
        // `Box::leak` para satisfazer `&'static MemoryStore` — mesmo truque de teste que
        // `episodic/*.rs` não precisa (eles recebem `&MemoryStore` com lifetime livre); aqui o
        // handler de produção fecha sobre o `'static` que `memory_store(app)` devolve.
        let memory: &'static MemoryStore = Box::leak(Box::new(MemoryStore::open_in_memory().unwrap()));
        let scheduler = SchedulerStore::open(&dir.0.join("scheduler.db")).unwrap();
        let extra = ExtraStores::open_for_test(&dir.0);
        let clear_conversation_history: Box<dyn Fn() -> Result<(), String> + Send + Sync> = Box::new(move || result.clone());
        ClearHistoryHandler { config, memory, scheduler, extra, clear_conversation_history }
    }

    #[tokio::test]
    async fn without_confirm_refuses_and_touches_nothing() {
        let (config, dir) = config_store();
        config.set("user.name", json!("Ana")).unwrap();
        let h = handler(config.clone(), &dir);

        let result = h.call(json!({})).await.unwrap();
        assert_eq!(result["errorCode"], json!("invalid_args"));
        assert_eq!(config.get("user.name"), Some(json!("Ana")), "sem confirm, nada pode mudar");
    }

    #[tokio::test]
    async fn with_confirm_clears_the_wired_stores_and_names_the_skipped_ones() {
        let (config, dir) = config_store();
        config.set("user.name", json!("Ana")).unwrap();
        let h = handler(config.clone(), &dir);
        h.memory.set("mika:mem:short:1", "{}", None).unwrap();

        let result = h.call(json!({ "confirm": true })).await.unwrap();
        assert_eq!(result["success"], json!(true));
        assert_eq!(config.get("user.name"), Some(json!("")), "perfil não voltou ao default");
        assert!(h.memory.keys("mika:mem:*").unwrap().is_empty(), "memória episódica não foi apagada");

        let cleared = result["data"]["cleared"].as_array().unwrap();
        for expected in [
            "conversation history",
            "episodic memory",
            "user profile",
            "reminders",
            "goals",
            "tts cache",
            "person catalog",
            "social memory",
            "game sessions",
            "screen captures",
            "safe secrets",
            "log files",
            "exported data archives",
            "usage habits",
            "update backups",
            "response reports",
        ] {
            assert!(cleared.iter().any(|v| v == expected), "'{expected}' não está em cleared: {cleared:?}");
        }
        let skipped = result["data"]["skipped"].as_array().unwrap();
        assert_eq!(skipped.len(), SKIPPED_STORES.len());
        assert!(skipped.iter().any(|v| v == "mood"));
        assert!(cleared.iter().any(|v| v == "chat session memory"), "a memória de sessão tem backing Rust e é apagada");
        assert!(!skipped.iter().any(|v| v == "chat session memory"));
        assert!(!skipped.iter().any(|v| v == "conversation history"), "conversation history já tem backing Rust — não pode mais estar skipped");
    }

    #[tokio::test]
    async fn the_session_accumulator_cannot_resurrect_erased_facts() {
        use crate::modules::brain::post_turn::session_accumulator::accumulator;
        accumulator().add_facts(&["gosta de café".to_string()], &[], &[]);
        assert!(!accumulator().lines().is_empty(), "pré-condição: há fato de sessão acumulado");
        let (config, dir) = config_store();
        let h = handler(config, &dir);

        h.call(json!({ "confirm": true })).await.unwrap();

        assert!(accumulator().lines().is_empty(), "o acumulador seguia no prompt depois de 'apagar tudo'");
    }

    /// A tool devolve `failed`, nunca `success: true` com a conversa intacta, quando o chokepoint
    /// de `chat` (a closure injetada) falha — mesma regra que `handler.scheduler`/`handler.memory`
    /// já seguem para os outros stores.
    #[tokio::test]
    async fn a_failed_history_clear_is_reported_as_failed_not_swallowed() {
        let (config, dir) = config_store();
        let h = handler_with_history_result(config, &dir, Err("chat.db bloqueado".into()));

        let result = h.call(json!({ "confirm": true })).await.unwrap();
        assert_eq!(result["success"], json!(false));
        let failed = result["data"]["failed"].as_array().unwrap();
        assert!(failed.iter().any(|v| v == "conversation history"), "falha do histórico não apareceu em failed: {failed:?}");
        let cleared = result["data"]["cleared"].as_array().unwrap();
        assert!(!cleared.iter().any(|v| v == "conversation history"), "histórico marcado cleared apesar da closure ter falhado");
    }

    #[tokio::test]
    async fn clears_goals_for_real() {
        let (config, dir) = config_store();
        let h = handler(config, &dir);
        crate::modules::brain::reflection::goals::add_goal(h.memory, "g1", "aprender rust", 0.5, 1000).unwrap();

        h.call(json!({ "confirm": true })).await.unwrap();
        assert!(crate::modules::brain::reflection::goals::get_active_goals(h.memory).unwrap().is_empty());
    }

    /// NEGATIVO — nenhum dos 5 stores nomeados como skipped aparece em `cleared`.
    #[tokio::test]
    async fn skipped_stores_never_leak_into_cleared() {
        let (config, dir) = config_store();
        let h = handler(config, &dir);
        let result = h.call(json!({ "confirm": true })).await.unwrap();
        let cleared: Vec<&str> = result["data"]["cleared"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        for skipped in SKIPPED_STORES {
            assert!(!cleared.contains(&skipped), "'{skipped}' deveria estar skipped, não cleared");
        }
    }

    #[tokio::test]
    async fn clears_reminders_for_real() {
        let (config, dir) = config_store();
        let h = handler(config, &dir);
        h.scheduler.add("r1".into(), "lembrete".into(), 1000, "once", 1, None).unwrap();

        h.call(json!({ "confirm": true })).await.unwrap();
        assert!(h.scheduler.list().unwrap().is_empty());
    }
}
