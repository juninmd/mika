//! "Seus hábitos": o contador de uso de programas (a cada [`TICK`], o programa em primeiro plano
//! ganha o tempo do tick) e a tool `habits` que a aba Humor e o LLM leem — top de programas (7/30
//! dias) e de jogos (30 dias, das sessões que o `gamer` já grava; só leitura).
//!
//! **Privacidade.** Local (`<dados>/usage-habits.json`), só o nome do processo, nunca título.
//! Não conta quando `mood.habitsEnabled` está desligado, quando o watcher de janela está parado
//! (`brain.enabled` desligado — o último snapshot ficaria velho), quando o usuário está longe do
//! PC, nem quando a janela em foco bate a blocklist de captura (`vision.captureBlocklist`: banco,
//! gerenciador de senha) ou é a própria Mika. `action=clear` apaga tudo.
//!
//! **Plataforma.** A sonda de janela ativa (`system_signals::active_window`) cobre Windows, Linux
//! (X11) e macOS; sem ela (Wayland) a lista de programas fica vazia e a tool diz isso
//! (`platformSupported`) — os jogos vêm das sessões e funcionam em todo SO.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{json, Value};

use crate::config::atomic::write_atomic;
use crate::config::store::ConfigStore;
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::usage::UsageLog;

const TICK: Duration = Duration::from_secs(60);
/// Sem input há mais que isto = usuário longe; o programa aberto não ganha tempo.
const AWAY_SECS: u64 = 5 * 60;
const FILE_NAME: &str = "usage-habits.json";
pub const TOP_N: usize = 5;

struct Habits {
    path: PathBuf,
    log: Mutex<UsageLog>,
}

impl Habits {
    fn open(path: PathBuf) -> Self {
        let log = std::fs::read_to_string(&path).ok().and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or_default();
        Self { path, log: Mutex::new(log) }
    }

    fn change(&self, edit: impl FnOnce(&mut UsageLog)) {
        let mut log = self.log.lock().unwrap_or_else(|e| e.into_inner());
        edit(&mut log);
        if let Err(error) = write_atomic(&self.path, &serde_json::to_vec(&*log).unwrap_or_default()) {
            log::warn!("habits: {} não gravado: {error}", self.path.display());
        }
    }

    fn top(&self, today: NaiveDate, days: i64) -> Vec<(String, u64)> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).top(today, days, TOP_N)
    }
}

static HABITS: OnceLock<Habits> = OnceLock::new();

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn flag(store: &ConfigStore, path: &str) -> bool {
    store.get(path).and_then(|v| v.as_bool()) != Some(false)
}

/// Se o tick conta: tudo ligado, usuário presente e janela não sensível.
fn should_count(habits_enabled: bool, watcher_on: bool, idle_secs: Option<u64>, sensitive: bool) -> bool {
    habits_enabled && watcher_on && idle_secs.is_some_and(|idle| idle < AWAY_SECS) && !sensitive
}

async fn tick(store: &ConfigStore, habits: &Habits) {
    let Some(owner) = crate::modules::system_signals::active_window::last_known().and_then(|w| w.owner) else { return };
    let idle = tokio::task::spawn_blocking(crate::power::idle_seconds).await.ok().and_then(Result::ok);
    let sensitive = crate::modules::vision::is_foreground_blocked(store);
    if !should_count(flag(store, "mood.habitsEnabled"), flag(store, "brain.enabled"), idle, sensitive) {
        return;
    }
    let day = today();
    habits.change(|log| {
        log.record(day, &owner, TICK.as_secs());
        log.prune(day);
    });
}

/// Apaga os hábitos: o livro em memória (senão o próximo tick regravaria tudo) e o arquivo. Chamado
/// por `features::privacy` ao apagar os dados; `data_dir` é a pasta de dados do perfil.
pub(crate) fn clear_all(data_dir: &std::path::Path) -> Result<(), String> {
    if let Some(habits) = HABITS.get() {
        habits.change(|log| *log = UsageLog::default());
    }
    match std::fs::remove_file(data_dir.join(FILE_NAME)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("apagar {FILE_NAME}: {e}")),
    }
}

/// Top de programas nos últimos `days` dias — vazio com o contador desligado ou não montado.
pub(crate) fn top_programs(store: &ConfigStore, days: i64) -> Vec<(String, u64)> {
    if !flag(store, "mood.habitsEnabled") {
        return Vec::new();
    }
    HABITS.get().map(|h| h.top(today(), days)).unwrap_or_default()
}

/// Top de jogos (30 dias) das sessões do `gamer` — leitura, nada gravado aqui.
pub(crate) fn top_games(store: &ConfigStore) -> Vec<(String, u64)> {
    use crate::modules::gamer::session_stats::{day_key, playtime_by_game};
    if !flag(store, "mood.habitsEnabled") {
        return Vec::new();
    }
    let Some(memory) = crate::modules::gamer::session_memory() else { return Vec::new() };
    let history = memory.history(1_000).unwrap_or_default();
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let mut games = playtime_by_game(&history, day_key(now_ms, &chrono::Local), 29, &chrono::Local);
    games.retain(|(_, ms)| *ms > 0);
    games.truncate(TOP_N);
    games.into_iter().map(|(name, ms)| (name, ms / 1000)).collect()
}

fn as_json(entries: &[(String, u64)]) -> Value {
    Value::Array(entries.iter().map(|(name, secs)| json!({ "name": name, "seconds": secs })).collect())
}

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "habits".into(),
        description: Some(
            "The user's local usage habits: most-used programs (last 7 and 30 days) and most-played \
             games (30 days). A compact top list is ALREADY in the system prompt; call action='get' \
             only when the user asks for details. action='clear' erases the history (only when the \
             user asks)."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": { "action": { "type": "string", "enum": ["get", "clear"] } },
            "required": ["action"],
        }),
        plugin_id: "mika.desktop".into(),
        requires_window: false,
    }
}

struct HabitsToolHandler {
    store: Arc<ConfigStore>,
}

#[async_trait]
impl ToolHandler for HabitsToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        match args.get("action").and_then(Value::as_str).unwrap_or_default() {
            "get" => Ok(json!({
                "success": true,
                "data": {
                    "enabled": flag(&self.store, "mood.habitsEnabled"),
                    "platformSupported": crate::modules::system_signals::active_window::is_supported(),
                    "programs7d": as_json(&top_programs(&self.store, 7)),
                    "programs30d": as_json(&top_programs(&self.store, 30)),
                    "games30d": as_json(&top_games(&self.store)),
                },
            })),
            "clear" => {
                let habits = HABITS.get().ok_or("habits: contador ainda não montado")?;
                habits.change(|log| *log = UsageLog::default());
                Ok(json!({ "success": true, "data": { "cleared": true } }))
            }
            other => Err(format!("habits: ação desconhecida \"{other}\"")),
        }
    }
}

/// Segunda passada de `desktop` (com `ConfigStore`): abre o livro, sobe o contador e registra a tool.
pub(crate) fn register(store: Arc<ConfigStore>) {
    let habits = HABITS.get_or_init(|| Habits::open(store.data_dir().join(FILE_NAME)));
    crate::modules::tools::registry().register(tool_definition(), Arc::new(HabitsToolHandler { store: Arc::clone(&store) }));
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(TICK);
        ticker.tick().await;
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = ticker.tick() => tick(&store, habits).await,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tick_counts_only_with_everything_on_and_the_user_present() {
        assert!(should_count(true, true, Some(10), false));
        assert!(!should_count(false, true, Some(10), false), "toggle de hábitos desligado");
        assert!(!should_count(true, false, Some(10), false), "watcher parado: snapshot velho");
        assert!(!should_count(true, true, Some(AWAY_SECS), false), "usuário longe do PC");
        assert!(!should_count(true, true, None, false), "ócio ilegível não conta como presente");
        assert!(!should_count(true, true, Some(10), true), "app sensível (blocklist) nunca é contado");
    }

    /// Tool `"habits"` com o contador desligado: nada sai, nem para a tela nem para o LLM.
    #[tokio::test]
    async fn the_habits_tool_reports_nothing_when_tracking_is_off() {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::boolean;
        let dir = std::env::temp_dir().join(format!("mika-habits-tool-{}", std::process::id()));
        let schema = SchemaRegistry::new(vec![Namespace { name: "mood", fields: vec![boolean("habitsEnabled", false)] }]);
        let handler = HabitsToolHandler { store: Arc::new(ConfigStore::open(&dir, schema).unwrap()) };
        assert_eq!(tool_definition().name, "habits");

        let result = handler.call(json!({ "action": "get" })).await.unwrap();
        assert_eq!(result["data"]["enabled"], json!(false));
        assert_eq!(result["data"]["programs7d"], json!([]));
        assert_eq!(result["data"]["games30d"], json!([]));
        assert!(handler.call(json!({ "action": "fly" })).await.unwrap_err().contains("desconhecida"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// "Apagar meus dados" tem de levar o arquivo de hábitos junto (nome do programa em foco, dia a dia).
    #[test]
    fn clear_all_removes_the_habits_file_and_is_quiet_when_there_is_none() {
        let dir = std::env::temp_dir().join(format!("mika-habits-clear-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap();
        Habits::open(dir.join(FILE_NAME)).change(|log| log.record(day, "Code", 120));
        assert!(dir.join(FILE_NAME).exists());

        clear_all(&dir).unwrap();
        assert!(!dir.join(FILE_NAME).exists(), "o arquivo de hábitos sobreviveu");
        clear_all(&dir).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_log_survives_a_restart_and_clear_empties_it() {
        let path = std::env::temp_dir().join(format!("mika-habits-{}.json", std::process::id()));
        let day = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap();
        Habits::open(path.clone()).change(|log| log.record(day, "Code", 120));
        let reopened = Habits::open(path.clone());
        assert_eq!(reopened.top(day, 7), vec![("Code".to_string(), 120)]);
        reopened.change(|log| *log = UsageLog::default());
        assert!(Habits::open(path.clone()).top(day, 7).is_empty());
        let _ = std::fs::remove_file(path);
    }
}
