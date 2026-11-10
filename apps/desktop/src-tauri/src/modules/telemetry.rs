//! Módulo `telemetry` em Rust — port do backend TypeScript correspondente.
//!
//! Cobre os 4 canais que hoje descem para o sidecar Node: `mika:telemetry-opt-in`,
//! `mika:telemetry-get-status`, `mika:token-usage-get`, `mika:token-usage-reset` (ver
//! `apps/desktop/src/main/ipc/telemetry.ts` e `token-usage.ts`). `track`/`flush`/`onError`/
//! `clearAll`/`exportAll` do `TelemetryService` original não são canais de IPC — são chamados
//! direto pelo processo principal — então ficam fora do escopo desta frente.
//!
//! ## Persistência
//! - `telemetry.enabled` / `telemetry.optInShown` / `telemetry.sessionId` já têm schema no
//!   `ConfigStore` (`config/schemas/mod.rs`), então usam o store canônico.
//! - Uso de tokens (era `stats.tokenUsage` no TypeScript; gravado por `modules/token_usage.rs`) NÃO tem schema Rust — o `SchemaRegistry`
//!   daqui recusa caminho não declarado de propósito (`config/registry.rs`), e schemas não são
//!   deste arquivo nesta frente. Guardo os contadores num SQLite próprio (`token_usage.db`, via
//!   `rusqlite`, já declarado em `Cargo.toml` com a feature `bundled`) em vez de só memória — só
//!   memória perderia o uso acumulado a cada reinício, uma mudança de comportamento visível para
//!   pior em relação ao original (que persiste no config).
//!
//! ## `ConfigStore` fora do `ShellState`
//! `ConfigStore` ainda não é montado no estado compartilhado do app (outra frente cuida disso).
//! Até lá, este módulo monta o seu próprio, uma vez, com `OnceLock`, a partir do mesmo
//! `app_data_dir` que o store canônico vai usar quando for centralizado.
//!
//! ## Desenho para teste sem subir o Tauri
//! A lógica de cada canal vive em funções `*_with`, que recebem `&ConfigStore`/`&Connection`
//! direto — nada de `AppHandle`. `cargo test --lib` cobre essas funções via `ConfigStore`/
//! `Connection` de teste; só a resolução do singleton a partir do `AppHandle` fica sem teste de
//! unidade (ela não tem lógica própria além de abrir o arquivo certo).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use async_trait::async_trait;
use rusqlite::Connection;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

const TELEMETRY_OPT_IN: &str = "mika:telemetry-opt-in";
const TELEMETRY_GET_STATUS: &str = "mika:telemetry-get-status";
const TOKEN_USAGE_GET: &str = "mika:token-usage-get";
const TOKEN_USAGE_RESET: &str = "mika:token-usage-reset";

const CHANNELS: &[&str] = &[
    TELEMETRY_OPT_IN,
    TELEMETRY_GET_STATUS,
    TOKEN_USAGE_GET,
    TOKEN_USAGE_RESET,
];

pub struct TelemetryModule;

#[async_trait]
impl NativeModule for TelemetryModule {
    fn name(&self) -> &'static str {
        "telemetry"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(
        &self,
        app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String> {
        ensure_owner_surface(ctx, channel)?;

        match channel {
            TELEMETRY_GET_STATUS => {
                let store = crate::state::config_store(app)?;
                telemetry_get_status_with(&store)
            }
            TELEMETRY_OPT_IN => {
                let store = crate::state::config_store(app)?;
                telemetry_opt_in_with(&store, payload)
            }
            TOKEN_USAGE_GET => with_token_usage_db(app, token_usage_get_with).await,
            TOKEN_USAGE_RESET => with_token_usage_db(app, token_usage_reset_with).await,
            // `channels()` é a fonte da verdade: um canal declarado e não tratado aqui vira erro
            // explícito, nunca some em silêncio rumo ao sidecar.
            other => Err(format!(
                "canal '{other}' declarado por telemetry mas não tratado"
            )),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(TelemetryModule)]
}

/// Abre (na primeira vez) e consulta `token_usage.db` fora do worker async.
async fn with_token_usage_db(app: &AppHandle, op: fn(&Connection) -> Result<Value, String>) -> Result<Value, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = token_usage_db(&app)?.lock().unwrap_or_else(|e| e.into_inner());
        op(&conn)
    })
    .await
    .map_err(|e| format!("token_usage.db: {e}"))?
}

/// Equivalente a `assertOwnerSurface`: só main/settings/wizard leem ou mudam telemetria e uso de
/// tokens. `plugin-view` e janela desconhecida são recusadas.
fn ensure_owner_surface(ctx: &InvokeContext, channel: &str) -> Result<(), String> {
    if is_owner_surface(ctx.window_type) {
        Ok(())
    } else {
        Err(format!(
            "acesso negado pela política de superfície ({channel})"
        ))
    }
}

// --- telemetry-get-status / telemetry-opt-in ----------------------------------------------------

fn telemetry_get_status_with(store: &ConfigStore) -> Result<Value, String> {
    let enabled = store
        .get("telemetry.enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let opt_in_shown = store
        .get("telemetry.optInShown")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let session_id = ensure_session_id(store)?;

    Ok(json!({
        "enabled": enabled,
        "optInShown": opt_in_shown,
        "sessionId": session_id,
    }))
}

fn telemetry_opt_in_with(store: &ConfigStore, payload: Value) -> Result<Value, String> {
    let enabled = payload
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or("payload sem 'enabled' booleano")?;
    let opt_in_shown = payload
        .get("optInShown")
        .and_then(Value::as_bool)
        .ok_or("payload sem 'optInShown' booleano")?;

    store.set("telemetry.enabled", Value::Bool(enabled))?;
    store.set("telemetry.optInShown", Value::Bool(opt_in_shown))?;

    Ok(json!({ "success": true }))
}

/// Espelha `ensureSessionId` do serviço TypeScript: gera na primeira leitura e persiste, para o
/// mesmo id sobreviver ao reinício.
fn ensure_session_id(store: &ConfigStore) -> Result<String, String> {
    if let Some(existing) = store
        .get("telemetry.sessionId")
        .and_then(|v| v.as_str().map(str::to_owned))
    {
        if !existing.is_empty() {
            return Ok(existing);
        }
    }

    let generated = generate_uuid_v4();
    store.set("telemetry.sessionId", Value::String(generated.clone()))?;
    Ok(generated)
}

/// UUID v4 sem dependência extra: `Cargo.toml` já traz `rand`, e não este arquivo, que não pode
/// tocar nele. `rand::rng().fill_bytes` é o mesmo par usado em `config/crypto.rs`.
fn generate_uuid_v4() -> String {
    use rand::Rng;

    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // versão 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variante RFC 4122

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

// --- token-usage-get / token-usage-reset --------------------------------------------------------

const TOKEN_USAGE_COLUMNS: &str = "total_tokens, total_prompt_tokens, total_completion_tokens, \
    total_cost_usd, unknown_cost_turns, total_interactions, first_used_at, last_used_at";

fn default_token_usage() -> Value {
    json!({
        "totalTokens": 0,
        "totalPromptTokens": 0,
        "totalCompletionTokens": 0,
        "totalCostUsd": 0.0,
        "unknownCostTurns": 0,
        "totalInteractions": 0,
        "firstUsedAt": 0,
        "lastUsedAt": 0,
    })
}

pub(super) fn token_usage_get_with(conn: &Connection) -> Result<Value, String> {
    let row = conn.query_row(
        &format!("SELECT {TOKEN_USAGE_COLUMNS} FROM token_usage WHERE id = 1"),
        [],
        |row| {
            Ok(json!({
                "totalTokens": row.get::<_, i64>(0)?,
                "totalPromptTokens": row.get::<_, i64>(1)?,
                "totalCompletionTokens": row.get::<_, i64>(2)?,
                "totalCostUsd": row.get::<_, f64>(3)?,
                "unknownCostTurns": row.get::<_, i64>(4)?,
                "totalInteractions": row.get::<_, i64>(5)?,
                "firstUsedAt": row.get::<_, i64>(6)?,
                "lastUsedAt": row.get::<_, i64>(7)?,
            }))
        },
    );

    match row {
        Ok(stats) => Ok(stats),
        // Sem linha ainda = app novo ou nunca usou tokens: mesmos defaults de
        // `getDefaultStats()` no TypeScript, sem gravar nada.
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_token_usage()),
        Err(e) => Err(format!("falha ao ler token_usage: {e}")),
    }
}

fn token_usage_reset_with(conn: &Connection) -> Result<Value, String> {
    conn.execute(
        "INSERT INTO token_usage (id, total_tokens, total_prompt_tokens, total_completion_tokens, \
            total_cost_usd, unknown_cost_turns, total_interactions, first_used_at, last_used_at) \
         VALUES (1, 0, 0, 0, 0, 0, 0, 0, 0) \
         ON CONFLICT(id) DO UPDATE SET \
            total_tokens = 0, total_prompt_tokens = 0, total_completion_tokens = 0, \
            total_cost_usd = 0, unknown_cost_turns = 0, total_interactions = 0, \
            first_used_at = 0, last_used_at = 0",
        [],
    )
    .map_err(|e| format!("falha ao resetar token_usage: {e}"))?;

    Ok(json!({ "success": true }))
}

/// Migração aditiva: cria `token_usage` (total agregado, inalterada desde sempre) e
/// `token_usage_by_provider` (uma linha por `surface`+`provider`+`model`, ver `token_usage.rs`) — a
/// segunda nunca lê nem reescreve a primeira, então bancos já existentes mantêm o total acumulado
/// intacto e a migração é reversível com um `DROP TABLE token_usage_by_provider` sem perda alguma
/// do total já gravado.
pub(super) fn create_token_usage_table(conn: &Connection) -> Result<(), String> {
    super::sqlite_migrate::migrate(conn, &[TOKEN_USAGE_MIGRATION_1]).map_err(|e| format!("token_usage.db: {e}"))
}

const TOKEN_USAGE_MIGRATION_1: &str = "
    CREATE TABLE IF NOT EXISTS token_usage (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        total_tokens INTEGER NOT NULL DEFAULT 0,
        total_prompt_tokens INTEGER NOT NULL DEFAULT 0,
        total_completion_tokens INTEGER NOT NULL DEFAULT 0,
        total_cost_usd REAL NOT NULL DEFAULT 0,
        unknown_cost_turns INTEGER NOT NULL DEFAULT 0,
        total_interactions INTEGER NOT NULL DEFAULT 0,
        first_used_at INTEGER NOT NULL DEFAULT 0,
        last_used_at INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS token_usage_by_provider (
        surface TEXT NOT NULL,
        provider TEXT NOT NULL,
        model TEXT NOT NULL,
        total_tokens INTEGER NOT NULL DEFAULT 0,
        total_prompt_tokens INTEGER NOT NULL DEFAULT 0,
        total_completion_tokens INTEGER NOT NULL DEFAULT 0,
        total_cost_usd REAL NOT NULL DEFAULT 0,
        unknown_cost_turns INTEGER NOT NULL DEFAULT 0,
        total_interactions INTEGER NOT NULL DEFAULT 0,
        first_used_at INTEGER NOT NULL DEFAULT 0,
        last_used_at INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (surface, provider, model)
    )
";

static TOKEN_USAGE_DB: OnceLock<Mutex<Connection>> = OnceLock::new();

pub(super) fn token_usage_db(app: &AppHandle) -> Result<&'static Mutex<Connection>, String> {
    if let Some(db) = TOKEN_USAGE_DB.get() {
        return Ok(db);
    }

    let data_dir = crate::state::app_data_dir(app)
        .map_err(|e| format!("sem app_data_dir: {e}"))?;
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("não criou {}: {e}", data_dir.display()))?;

    let path: PathBuf = data_dir.join("token_usage.db");
    let conn = Connection::open(&path).map_err(|e| format!("não abriu token_usage.db: {e}"))?;
    create_token_usage_table(&conn)?;

    let _ = TOKEN_USAGE_DB.set(Mutex::new(conn));
    Ok(TOKEN_USAGE_DB.get().expect("acabou de inicializar"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema_with_telemetry() -> crate::config::registry::SchemaRegistry {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::{boolean, text};

        SchemaRegistry::new(vec![Namespace {
            name: "telemetry",
            fields: vec![
                boolean("enabled", false),
                boolean("optInShown", false),
                text("sessionId", ""),
            ],
        }])
    }

    /// Diretório temporário mínimo, mesmo padrão do usado em `config/store.rs`: não vale trazer
    /// dependência só para isolar os testes.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);

            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mika-telemetry-test-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_store() -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, schema_with_telemetry()).unwrap();
        (store, dir)
    }

    #[test]
    fn channels_match_the_ipc_contract() {
        let module = TelemetryModule;
        assert_eq!(
            module.channels(),
            &[
                "mika:telemetry-opt-in",
                "mika:telemetry-get-status",
                "mika:token-usage-get",
                "mika:token-usage-reset",
            ]
        );
    }

    // --- guarda de superfície ----------------------------------------------------------------

    #[test]
    fn owner_windows_pass_the_surface_guard() {
        use crate::protocol::WindowType;

        for window_type in [WindowType::Main, WindowType::Settings, WindowType::Wizard] {
            let ctx = InvokeContext {
                window_type,
                window_label: "w".into(),
                plugin_id: None,
            };
            assert!(ensure_owner_surface(&ctx, "mika:telemetry-get-status").is_ok());
        }
    }

    #[test]
    fn plugin_and_unknown_windows_are_denied() {
        use crate::protocol::WindowType;

        for window_type in [WindowType::PluginView, WindowType::Unknown] {
            let ctx = InvokeContext {
                window_type,
                window_label: "w".into(),
                plugin_id: Some("some-plugin".into()),
            };
            assert!(ensure_owner_surface(&ctx, "mika:telemetry-get-status").is_err());
        }
    }

    // --- telemetry-get-status / telemetry-opt-in ----------------------------------------------

    #[test]
    fn status_defaults_to_disabled_with_a_generated_session_id() {
        let (store, _dir) = test_store();

        let status = telemetry_get_status_with(&store).unwrap();
        assert_eq!(status["enabled"], json!(false));
        assert_eq!(status["optInShown"], json!(false));
        assert!(status["sessionId"]
            .as_str()
            .is_some_and(|id| !id.is_empty()));
    }

    /// O id gerado na primeira leitura sobrevive à segunda — é o que `ensureSessionId` garante no
    /// TypeScript, e o que faz eventos de sessões diferentes do mesmo usuário se agruparem.
    #[test]
    fn session_id_is_stable_across_reads() {
        let (store, _dir) = test_store();

        let first = telemetry_get_status_with(&store).unwrap();
        let second = telemetry_get_status_with(&store).unwrap();
        assert_eq!(first["sessionId"], second["sessionId"]);
    }

    #[test]
    fn opt_in_persists_both_fields() {
        let (store, _dir) = test_store();

        let result =
            telemetry_opt_in_with(&store, json!({ "enabled": true, "optInShown": true })).unwrap();
        assert_eq!(result["success"], json!(true));

        let status = telemetry_get_status_with(&store).unwrap();
        assert_eq!(status["enabled"], json!(true));
        assert_eq!(status["optInShown"], json!(true));
    }

    /// Payload malformado é ERRO explícito, não um `false` silencioso — telemetria desligada sem o
    /// usuário ter pedido seria tão ruim quanto ligada sem consentimento.
    #[test]
    fn opt_in_rejects_a_malformed_payload() {
        let (store, _dir) = test_store();

        assert!(telemetry_opt_in_with(&store, json!({ "enabled": true })).is_err());
        assert!(telemetry_opt_in_with(&store, json!({})).is_err());
    }

    // --- token-usage-get / token-usage-reset --------------------------------------------------

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_token_usage_table(&conn).unwrap();
        conn
    }

    #[test]
    fn token_usage_defaults_to_zero_without_writing_a_row() {
        let conn = test_conn();

        let stats = token_usage_get_with(&conn).unwrap();
        assert_eq!(stats["totalTokens"], json!(0));
        assert_eq!(stats["totalCostUsd"], json!(0.0));

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM token_usage", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "a leitura não pode gravar linha nenhuma");
    }

    #[test]
    fn reset_zeroes_out_accumulated_usage() {
        let conn = test_conn();
        // Simula uso acumulado por outro caminho (o `set` de uso real fica fora do escopo dos 4
        // canais desta frente).
        conn.execute(
            "INSERT INTO token_usage (id, total_tokens, total_prompt_tokens, total_completion_tokens, \
                total_cost_usd, unknown_cost_turns, total_interactions, first_used_at, last_used_at) \
             VALUES (1, 5000, 3000, 2000, 1.25, 1, 10, 100, 200)",
            [],
        )
        .unwrap();

        let result = token_usage_reset_with(&conn).unwrap();
        assert_eq!(result["success"], json!(true));

        let stats = token_usage_get_with(&conn).unwrap();
        assert_eq!(stats, default_token_usage());
    }

    /// Banco legado (só `token_usage`, `user_version = 0`): migra sem perder o total acumulado.
    #[test]
    fn legacy_unversioned_database_keeps_its_accumulated_total() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE token_usage (id INTEGER PRIMARY KEY CHECK (id = 1), total_tokens INTEGER NOT NULL DEFAULT 0,
                total_prompt_tokens INTEGER NOT NULL DEFAULT 0, total_completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_cost_usd REAL NOT NULL DEFAULT 0, unknown_cost_turns INTEGER NOT NULL DEFAULT 0,
                total_interactions INTEGER NOT NULL DEFAULT 0, first_used_at INTEGER NOT NULL DEFAULT 0,
                last_used_at INTEGER NOT NULL DEFAULT 0);
             INSERT INTO token_usage VALUES (1, 5000, 3000, 2000, 1.25, 1, 10, 100, 200);",
        )
        .unwrap();

        create_token_usage_table(&conn).unwrap();

        assert_eq!(token_usage_get_with(&conn).unwrap()["totalTokens"], json!(5000));
        conn.query_row("SELECT COUNT(*) FROM token_usage_by_provider", [], |r| r.get::<_, i64>(0)).unwrap();
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, 1);
    }
}
