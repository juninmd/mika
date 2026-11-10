//! Tool `export_data` (LLM, leitura) — port PARCIAL de
//! `apps/desktop/src/features/privacy/{export-store.ts,export-ports.ts,tools/index.ts}`.
//!
//! Escreve os stores que este build alcança de verdade — `user-profile` (`ConfigStore`),
//! `reminders` (`SchedulerStore`) e, desde 2026-09-25, `episodic-memory`/`person-catalog`/
//! `social-memory`/`game-sessions`/`goals` (ver [`super::export_ext`]) — como um `.zip` REAL (`zip`
//! crate, já em `Cargo.toml`) sob `<app_data_dir>/exports/<timestamp>.zip`, com `manifest.json`.
//! Só `telemetry` continua `"not-covered"`: `modules/telemetry.rs` guarda `token_usage` (contadores
//! agregados), dado DIFERENTE do `telemetry_events` que o `TelemetryService` do TS exportava — mesma
//! distinção já documentada no cabeçalho de `clear.rs` para `clear_history`.

use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use tauri::AppHandle;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::config::store::ConfigStore;
use crate::modules::features::scheduler::{db_path as scheduler_db_path, SchedulerStore};
use crate::modules::features::user::USER_PROFILE_KEYS;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::export_ext;

/// O único store de `ExportRuntime` (`export-ports.ts`) sem caminho Rust alcançável nesta rodada —
/// mesmo nome de `exportUserData`'s `step()`, para o manifest continuar legível lado a lado com uma
/// exportação feita pelo sidecar.
const NOT_COVERED: [&str; 1] = ["telemetry"];

const DESCRIPTION: &str = "Export the user's data as one timestamped .zip in the app data folder: profile, reminders, episodic memory, person catalog, social memory, game sessions, goals, chat messages and saved response reports (one JSON per store plus manifest.json; local telemetry is marked not-covered). Use when the user asks to export, download or back up their data. Read-only: nothing is deleted or modified.";

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "export_data".into(),
        description: Some(DESCRIPTION.into()),
        input_schema: json!({ "type": "object", "properties": {}, "required": [] }),
        plugin_id: "mika.privacy".into(),
        requires_window: false,
    }
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Espelha `exportUserProfile`: só os campos não-vazios de `USER_PROFILE_KEYS`, mesma regra de
/// `user/tool.rs::config_profile`.
fn user_profile_json(config: &ConfigStore) -> Value {
    let mut profile = Map::new();
    for key in USER_PROFILE_KEYS {
        if let Some(value) = config.get(&format!("user.{key}")) {
            let is_empty_string = value.as_str().is_some_and(str::is_empty);
            if !value.is_null() && !is_empty_string {
                profile.insert((*key).to_string(), value);
            }
        }
    }
    Value::Object(profile)
}

/// Espelha `exportReminders`: a lista inteira, sem filtro.
fn reminders_json(scheduler: &SchedulerStore) -> Result<Value, String> {
    let reminders = scheduler.list()?;
    serde_json::to_value(reminders).map_err(|e| e.to_string())
}

pub(super) struct Outcome {
    pub(super) store: &'static str,
    pub(super) exported: bool,
    pub(super) error: Option<String>,
}

pub(super) fn write_json_entry(zip: &mut ZipWriter<File>, name: &str, value: &Value) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    zip.start_file(name, SimpleFileOptions::default()).map_err(|e| e.to_string())?;
    zip.write_all(&body).map_err(|e| e.to_string())?;
    Ok(())
}

fn manifest(stamp: i64, outcomes: &[Outcome]) -> Value {
    json!({
        "generatedAt": stamp,
        "stores": outcomes.iter().map(|o| json!({
            "store": o.store,
            "status": if o.error.is_some() { "error" } else if o.exported { "exported" } else { "not-covered" },
            "error": o.error,
        })).collect::<Vec<_>>(),
    })
}

struct ExportDataHandler {
    config: Arc<ConfigStore>,
    scheduler: SchedulerStore,
    /// Os 5 stores de 2026-09-25 (episodic-memory/person-catalog/social-memory/game-sessions/goals)
    /// — agrupados em `export_ext.rs` só pelo limite de linhas deste arquivo, mesmo motivo de
    /// `clear_ext.rs`/`clear_more.rs`.
    extra: export_ext::ExtraExports,
    exports_dir: PathBuf,
}

impl ExportDataHandler {
    fn export_to_zip(&self) -> Result<(PathBuf, Vec<Outcome>), String> {
        std::fs::create_dir_all(&self.exports_dir).map_err(|e| format!("não criou {}: {e}", self.exports_dir.display()))?;
        let stamp = now_millis();
        let archive_path = self.exports_dir.join(format!("{stamp}.zip"));

        let file = File::create(&archive_path).map_err(|e| format!("não criou {}: {e}", archive_path.display()))?;
        let mut zip = ZipWriter::new(file);
        let mut outcomes: Vec<Outcome> = Vec::new();

        match write_json_entry(&mut zip, "user-profile.json", &user_profile_json(&self.config)) {
            Ok(()) => outcomes.push(Outcome { store: "user-profile", exported: true, error: None }),
            Err(error) => outcomes.push(Outcome { store: "user-profile", exported: false, error: Some(error) }),
        }

        match reminders_json(&self.scheduler).and_then(|value| write_json_entry(&mut zip, "reminders.json", &value)) {
            Ok(()) => outcomes.push(Outcome { store: "reminders", exported: true, error: None }),
            Err(error) => outcomes.push(Outcome { store: "reminders", exported: false, error: Some(error) }),
        }

        self.extra.export(&mut zip, &mut outcomes);

        for store in NOT_COVERED {
            outcomes.push(Outcome { store, exported: false, error: None });
        }

        write_json_entry(&mut zip, "manifest.json", &manifest(stamp, &outcomes))?;
        zip.finish().map_err(|e| format!("não fechou o zip: {e}"))?;

        Ok((archive_path, outcomes))
    }
}

#[async_trait]
impl ToolHandler for ExportDataHandler {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        let (archive_path, outcomes) = match self.export_to_zip() {
            Ok(result) => result,
            Err(error) => return Ok(json!({ "success": false, "error": error, "errorCode": "internal" })),
        };
        let archive = archive_path.display().to_string();

        let exported: Vec<&str> = outcomes.iter().filter(|o| o.exported).map(|o| o.store).collect();
        let failed: Vec<&Outcome> = outcomes.iter().filter(|o| o.error.is_some()).collect();
        let not_covered: Vec<&str> = outcomes.iter().filter(|o| !o.exported && o.error.is_none()).map(|o| o.store).collect();

        if exported.is_empty() && failed.is_empty() {
            return Ok(json!({ "success": false, "error": "No local store was reachable, so nothing was exported.", "errorCode": "unavailable" }));
        }
        if !failed.is_empty() {
            let failed_text =
                failed.iter().map(|o| format!("{} ({})", o.store, o.error.clone().unwrap())).collect::<Vec<_>>().join(", ");
            return Ok(json!({
                "success": !exported.is_empty(),
                "error": format!("Failed: {failed_text}"),
                "errorCode": "internal",
                "data": { "archive": archive, "exported": exported, "skipped": not_covered },
            }));
        }
        Ok(json!({
            "success": true,
            "data": { "archive": archive, "exported": exported, "skipped": not_covered },
            "message": format!(
                "Exported to {archive}: {}. NOT covered (store unavailable): {}.",
                exported.join(", "),
                not_covered.join(", "),
            ),
        }))
    }
}

/// Chamado por `super::register(store, app)`. Abre o `SchedulerStore` de PROCESSO (mesmo arquivo
/// que a tool `reminder` usa) uma vez, no boot — nunca por chamada.
pub(crate) fn register(store: Arc<ConfigStore>, app: &AppHandle) -> Result<(), String> {
    let scheduler = SchedulerStore::open(&scheduler_db_path(app)?)?;
    let extra = export_ext::ExtraExports::open(app)?;
    let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?;

    crate::modules::tools::registry().register(
        tool_definition(),
        Arc::new(ExportDataHandler { config: store, scheduler, extra, exports_dir: data_dir.join("exports") }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-privacy-export-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn handler() -> (ExportDataHandler, TempDir) {
        let dir = TempDir::new();
        let config = Arc::new(ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap());
        let scheduler = SchedulerStore::open(&dir.0.join("scheduler.db")).unwrap();
        let extra = export_ext::ExtraExports::open_for_test(&dir.0);
        (ExportDataHandler { config, scheduler, extra, exports_dir: dir.0.join("exports") }, dir)
    }

    #[tokio::test]
    async fn writes_a_real_zip_with_the_seven_wired_stores_and_names_the_rest_not_covered() {
        let (handler, _dir) = handler();
        handler.config.set("user.name", json!("Ana")).unwrap();
        handler.scheduler.add("r1".into(), "lembrete".into(), 1000, "once", 1, None).unwrap();

        let result = handler.call(json!({})).await.unwrap();
        assert_eq!(result["success"], json!(true));

        let archive = result["data"]["archive"].as_str().unwrap();
        assert!(std::path::Path::new(archive).exists(), "o .zip não foi gravado em disco");

        let exported = result["data"]["exported"].as_array().unwrap();
        for expected in [
            "user-profile",
            "reminders",
            "episodic-memory",
            "person-catalog",
            "social-memory",
            "game-sessions",
            "goals",
            "chat-messages",
            "response-reports",
        ] {
            assert!(exported.iter().any(|v| v == expected), "'{expected}' não está em exported: {exported:?}");
        }

        let skipped = result["data"]["skipped"].as_array().unwrap();
        assert_eq!(skipped.len(), NOT_COVERED.len());
        assert!(skipped.iter().any(|v| v == "telemetry"));
        assert!(!skipped.iter().any(|v| v == "episodic-memory"), "episodic-memory já tem caminho de leitura Rust");
    }

    /// O manifest DENTRO do zip espelha a resposta — não só o retorno da tool.
    /// Direito de acesso/portabilidade: a conversa de `chat.db` e os relatórios salvos vão no `.zip`,
    /// com o texto que o usuário de fato escreveu.
    #[tokio::test]
    async fn the_zip_carries_the_chat_messages_and_the_saved_reports() {
        let (handler, dir) = handler();
        crate::modules::chat::append_message(&dir.0.join("chat.db"), "user", "oi mika, tudo bem?").unwrap();
        std::fs::create_dir_all(dir.0.join("reports")).unwrap();
        std::fs::write(dir.0.join("reports").join("1700000000000.json"), r#"{"response":"resposta reportada"}"#).unwrap();

        let result = handler.call(json!({})).await.unwrap();
        let file = File::open(result["data"]["archive"].as_str().unwrap()).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();

        let mut chat = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("chat-messages.json").unwrap(), &mut chat).unwrap();
        let chat: Value = serde_json::from_str(&chat).unwrap();
        assert_eq!(chat[0]["content"], json!("oi mika, tudo bem?"));
        assert_eq!(chat[0]["role"], json!("user"));

        let mut reports = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("response-reports.json").unwrap(), &mut reports).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&reports).unwrap()[0]["response"], json!("resposta reportada"));
    }

    /// Nunca conversou: exportar não cria um `chat.db` vazio no disco só para ler dele.
    #[tokio::test]
    async fn exporting_without_a_chat_db_yields_an_empty_list_and_creates_nothing() {
        let (handler, dir) = handler();
        let result = handler.call(json!({})).await.unwrap();
        assert_eq!(result["success"], json!(true));
        assert!(!dir.0.join("chat.db").exists());
    }

    #[tokio::test]
    async fn the_manifest_inside_the_zip_marks_uncovered_stores_by_name() {
        let (handler, _dir) = handler();
        let result = handler.call(json!({})).await.unwrap();
        let archive = result["data"]["archive"].as_str().unwrap().to_string();

        let file = File::open(&archive).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let mut manifest_file = zip.by_name("manifest.json").unwrap();
        let mut body = String::new();
        std::io::Read::read_to_string(&mut manifest_file, &mut body).unwrap();
        let manifest: Value = serde_json::from_str(&body).unwrap();

        let stores = manifest["stores"].as_array().unwrap();
        let telemetry = stores.iter().find(|s| s["store"] == json!("telemetry")).unwrap();
        assert_eq!(telemetry["status"], json!("not-covered"));
        let episodic = stores.iter().find(|s| s["store"] == json!("episodic-memory")).unwrap();
        assert_eq!(episodic["status"], json!("exported"));
    }

    /// Prova, dado a dado, que o `.zip` carrega o que cada store recém-coberto de fato tinha —
    /// não só a chave "exported" na resposta.
    #[tokio::test]
    async fn the_zip_entries_carry_the_real_data_of_each_newly_wired_store() {
        let (handler, _dir) = handler();
        crate::modules::memory::episodic::EpisodicMemoryService::new(handler.extra.memory_for_test())
            .store(true, "fato episódico", None, None)
            .unwrap();
        crate::modules::brain::reflection::goals::add_goal(handler.extra.memory_for_test(), "g1", "aprender rust", 0.5, 1000).unwrap();

        let result = handler.call(json!({})).await.unwrap();
        let archive = result["data"]["archive"].as_str().unwrap().to_string();
        let file = File::open(&archive).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();

        let mut episodic_body = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("episodic-memory.json").unwrap(), &mut episodic_body).unwrap();
        let episodic: Value = serde_json::from_str(&episodic_body).unwrap();
        assert_eq!(episodic.as_array().unwrap()[0]["content"], json!("fato episódico"));

        let mut goals_body = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("goals.json").unwrap(), &mut goals_body).unwrap();
        let goals: Value = serde_json::from_str(&goals_body).unwrap();
        assert_eq!(goals.as_array().unwrap()[0]["description"], json!("aprender rust"));
    }

    #[test]
    fn register_adds_export_data_with_the_manifest_plugin_id() {
        let (handler, _dir) = handler();
        crate::modules::tools::registry().register(tool_definition(), Arc::new(handler));
        let definition = crate::modules::tools::registry().get("export_data").expect("export_data não registrada");
        assert_eq!(definition.plugin_id, "mika.privacy");
    }
}
