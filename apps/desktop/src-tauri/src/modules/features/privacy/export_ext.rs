//! Os 5 stores que `export_data` ganha nesta rodada (2026-09-25) — extraído de `export.rs` só pelo
//! limite de 300 linhas do arquivo, mesmo motivo de `clear_ext.rs`/`clear_more.rs`. Cada leitura usa
//! a MESMA porta que a tool de leitura correspondente já usa, sem duplicar lógica de
//! ordenação/TTL/poda:
//!
//! | Store | Leitura |
//! |---|---|
//! | `episodic-memory` | `EpisodicMemoryService::get_all` — mesma classe que `list_memories.rs` usa |
//! | `person-catalog` | `CatalogStore::list` — o SINGLETON de processo (`person_catalog::catalog_store`), nunca uma abertura própria (mesmo risco de desatualização que o cabeçalho de `store_singleton.rs` documenta) |
//! | `social-memory` | `SocialMemoryService::get_graph` |
//! | `game-sessions` | `SessionDb::history` — conexão própria (SQLite WAL), mesmo raciocínio de `clear_ext.rs` |
//! | `goals` | `brain::reflection::goals::get_active_goals` |
//!
//! Diferente de `clear_ext.rs` (que só precisa saber SE apagou), aqui o dado em si vai para o
//! `.zip` — cada `Outcome` carrega `exported: true` só depois do `write_json_entry` ter dado certo.

use tauri::AppHandle;

use crate::modules::features::person_catalog::{catalog_store, CatalogStore};
use crate::modules::gamer::session_db::{db_path as game_sessions_db_path, SessionDb};
use crate::modules::memory::episodic::EpisodicMemoryService;
use crate::modules::memory::social::SocialMemoryService;
use crate::modules::memory::MemoryStore;

use super::export::{write_json_entry, Outcome};

pub(super) struct ExtraExports {
    memory: &'static MemoryStore,
    catalog: &'static CatalogStore,
    game_sessions: SessionDb,
    chat_db_path: std::path::PathBuf,
    reports_dir: std::path::PathBuf,
}

impl ExtraExports {
    /// Produção: `catalog` vem do SINGLETON de processo — ver a tabela do cabeçalho do arquivo.
    pub(super) fn open(app: &AppHandle) -> Result<Self, String> {
        let memory = crate::modules::memory::memory_store(app)?;
        let catalog = catalog_store(app)?;
        let game_sessions = SessionDb::open(&game_sessions_db_path(app)?)?;
        let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?;
        Ok(Self {
            memory,
            catalog,
            game_sessions,
            chat_db_path: crate::modules::chat::chat_db_path(app)?,
            reports_dir: data_dir.join(crate::modules::chat::report::REPORTS_DIR),
        })
    }

    /// Teste: `catalog` PRÓPRIO (`Box::leak`, isolado por `TempDir`), nunca o singleton de
    /// processo — mesmo truque que `clear_ext.rs::ExtraStores::open_for_test` já usa.
    #[cfg(test)]
    pub(super) fn open_for_test(dir: &std::path::Path) -> Self {
        let memory: &'static MemoryStore = Box::leak(Box::new(MemoryStore::open_in_memory().unwrap()));
        let catalog_path = dir.join("plugins").join("mika.person-catalog").join("catalog.json");
        let catalog: &'static CatalogStore = Box::leak(Box::new(CatalogStore::open(&catalog_path).unwrap()));
        Self {
            memory,
            catalog,
            game_sessions: SessionDb::open(&dir.join("gamer-sessions.db")).unwrap(),
            chat_db_path: dir.join("chat.db"),
            reports_dir: dir.join(crate::modules::chat::report::REPORTS_DIR),
        }
    }

    #[cfg(test)]
    pub(super) fn memory_for_test(&self) -> &'static MemoryStore {
        self.memory
    }

    /// Tenta os 5 stores, escrevendo cada um no `.zip` e empurrando o `Outcome` correspondente —
    /// MESMO contrato de `write_json_entry`/`Outcome` que `export.rs::export_to_zip` já usa para
    /// `user-profile`/`reminders`.
    pub(super) fn export(&self, zip: &mut zip::ZipWriter<std::fs::File>, outcomes: &mut Vec<Outcome>) {
        write_outcome(zip, outcomes, "episodic-memory", episodic_json(self.memory));
        write_outcome(zip, outcomes, "person-catalog", person_catalog_json(self.catalog));
        write_outcome(zip, outcomes, "social-memory", social_memory_json(self.memory));
        write_outcome(zip, outcomes, "game-sessions", game_sessions_json(&self.game_sessions));
        write_outcome(zip, outcomes, "goals", goals_json(self.memory));
        write_outcome(zip, outcomes, "chat-messages", chat_messages_json(&self.chat_db_path));
        write_outcome(zip, outcomes, "response-reports", reports_json(&self.reports_dir));
    }
}

/// Grava `value` como `<store>.json` e empurra o `Outcome` de acordo — laço comum aos 5 stores
/// deste arquivo, sem repetir o `match` 5 vezes.
fn write_outcome(
    zip: &mut zip::ZipWriter<std::fs::File>,
    outcomes: &mut Vec<Outcome>,
    store: &'static str,
    data: Result<serde_json::Value, String>,
) {
    match data.and_then(|value| write_json_entry(zip, &format!("{store}.json"), &value)) {
        Ok(()) => outcomes.push(Outcome { store, exported: true, error: None }),
        Err(error) => outcomes.push(Outcome { store, exported: false, error: Some(error) }),
    }
}

/// Espelha `exportEpisodicMemory` (TS): a memória episódica inteira, na MESMA ordenação de
/// `EpisodicMemoryService::get_all` — sem o recorte `id`/`content`/`type`/`timestamp` que
/// `list_memories.rs` aplica (aqui é exportação de dado do usuário, não resposta de tool de LLM).
fn episodic_json(memory: &MemoryStore) -> Result<serde_json::Value, String> {
    let entries = EpisodicMemoryService::new(memory).get_all()?;
    serde_json::to_value(entries).map_err(|e| e.to_string())
}

/// `catalog.list` com um limite alto o bastante para nunca cortar em uso normal (o índice inteiro
/// cabe em memória — é o mesmo `CatalogStore` que já mantém tudo carregado, ver `store.rs`).
fn person_catalog_json(catalog: &CatalogStore) -> Result<serde_json::Value, String> {
    let people = catalog.list(None, None, usize::MAX);
    serde_json::to_value(people).map_err(|e| e.to_string())
}

fn social_memory_json(memory: &MemoryStore) -> Result<serde_json::Value, String> {
    let graph = SocialMemoryService::new(memory).get_graph()?;
    serde_json::to_value(graph).map_err(|e| e.to_string())
}

fn game_sessions_json(game_sessions: &SessionDb) -> Result<serde_json::Value, String> {
    let history = game_sessions.history(i64::MAX)?;
    serde_json::to_value(history).map_err(|e| e.to_string())
}

/// A conversa guardada em `chat.db` (a sessão atual): é dado do usuário, e a portabilidade sem ela
/// deixava de fora justamente o que ele mais escreveu. Sem `chat.db` ainda (nunca conversou) a lista
/// é vazia — e o arquivo não é criado só por exportar.
fn chat_messages_json(db_path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !db_path.is_file() {
        return Ok(serde_json::json!([]));
    }
    let messages = crate::modules::chat::all_messages(db_path)?;
    Ok(serde_json::Value::Array(messages.iter().map(|m| m.to_json()).collect()))
}

/// Os relatórios "reportar resposta" (`reports/*.json`), por nome de arquivo (= por data).
fn reports_json(dir: &std::path::Path) -> Result<serde_json::Value, String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(serde_json::json!([])) };
    let mut files: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    let reports = files.iter().filter_map(|p| std::fs::read_to_string(p).ok()).filter_map(|raw| serde_json::from_str(&raw).ok());
    Ok(serde_json::Value::Array(reports.collect()))
}

fn goals_json(memory: &MemoryStore) -> Result<serde_json::Value, String> {
    let goals = crate::modules::brain::reflection::goals::get_active_goals(memory)?;
    serde_json::to_value(goals).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::features::person_catalog::types::UpsertInput;
    use crate::modules::gamer::session_types::GameSession;
    use crate::modules::memory::social::AddFactInput;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-privacy-export-ext-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn game_session(id: &str) -> GameSession {
        GameSession {
            id: id.into(),
            game_id: "g1".into(),
            game_name: "Hades".into(),
            start_time_ms: 1000,
            end_time_ms: None,
            events: vec![],
            summary: None,
        }
    }

    /// Prova, dado a dado, que os 5 stores exportam o que de fato tinham — não só que `exported`
    /// aparece na lista (essa parte já é coberta em `export.rs::the_zip_entries_carry...`).
    #[test]
    fn all_five_stores_serialize_what_was_actually_written() {
        let dir = TempDir::new();
        let extra = ExtraExports::open_for_test(&dir.0);

        crate::modules::memory::episodic::EpisodicMemoryService::new(extra.memory).store(true, "fato", None, None).unwrap();
        extra.catalog.upsert(UpsertInput { name: "Ada".into(), ..Default::default() }, 1000, 100).unwrap();
        SocialMemoryService::new(extra.memory)
            .add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1)
            .unwrap();
        extra.game_sessions.insert(&game_session("s1")).unwrap();
        crate::modules::brain::reflection::goals::add_goal(extra.memory, "g1", "aprender rust", 0.5, 1000).unwrap();

        assert_eq!(episodic_json(extra.memory).unwrap().as_array().unwrap().len(), 1);
        assert_eq!(person_catalog_json(extra.catalog).unwrap().as_array().unwrap().len(), 1);
        assert_eq!(social_memory_json(extra.memory).unwrap()["facts"].as_array().unwrap().len(), 1);
        assert_eq!(game_sessions_json(&extra.game_sessions).unwrap().as_array().unwrap().len(), 1);
        assert_eq!(goals_json(extra.memory).unwrap().as_array().unwrap().len(), 1);
    }

    /// NEGATIVO — nenhum store vazio vira erro: primeira instalação exporta 7 listas vazias, não 7
    /// falhas.
    #[test]
    fn export_on_a_fresh_install_fails_nothing() {
        let dir = TempDir::new();
        let extra = ExtraExports::open_for_test(&dir.0);
        let file = std::fs::File::create(dir.0.join("out.zip")).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let mut outcomes = Vec::new();

        extra.export(&mut zip, &mut outcomes);

        assert_eq!(outcomes.len(), 7);
        assert!(outcomes.iter().all(|o| o.error.is_none()), "instalação nova não pode falhar: {:?}", outcomes.iter().map(|o| &o.error).collect::<Vec<_>>());
        assert!(outcomes.iter().all(|o| o.exported), "instalação nova ainda exporta (listas vazias), não vira not-covered");
    }
}
