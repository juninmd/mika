//! Backup pré-update: antes de rodar o instalador, copia os dados que não se refazem (histórico de
//! chat, memória, agenda, sessões de jogo, uso de tokens, catálogo de pessoas, config) para `<app_data>/backups/pre-<versão>/`, mantendo só as duas últimas pastas.
//!
//! Um update que migra o schema (ver `sqlite_migrate`) é de mão única — um app mais antigo recusa
//! abrir um banco mais novo —, então o único caminho de volta é o backup feito ANTES.
//!
//! Os bancos são copiados com `VACUUM INTO` numa conexão própria, não com `fs::copy`: o banco vivo
//! está em WAL, e copiar só o `.db` perderia o que ainda está no `-wal`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rusqlite::Connection;

/// Pastas `pre-*` mantidas (a recém-criada conta).
const KEEP: usize = 2;

/// Arquivo de dados -> caminho relativo a `<app_data>`. SQLite usa `VACUUM INTO`; o resto, cópia.
const SQLITE_FILES: [&str; 5] = [
    "plugins/mika.chat/chat.db",
    "plugins/mika.brain/memory.db",
    "plugins/mika.scheduler/scheduler.db",
    "plugins/mika.gamer-companion/gamer-sessions.db",
    "token_usage.db",
];
const PLAIN_FILES: [&str; 2] = ["config.json", "plugins/mika.person-catalog/catalog.json"];

/// Faz o backup e roda a rotação. Devolve a pasta criada. Arquivo ausente (feature nunca usada) é
/// pulado, não é erro.
///
/// Copia para `pre-<versão>.tmp` e só ao final troca pela pasta definitiva: um backup que falha no
/// meio (disco cheio, banco ilegível) não destrói o anterior da mesma versão.
pub fn backup_before_update(data_dir: &Path, version: &str) -> Result<PathBuf, String> {
    let backups = data_dir.join("backups");
    let target = backups.join(format!("pre-{}", sanitize(version)));
    let staging = backups.join(format!("pre-{}.tmp", sanitize(version)));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| format!("não limpou {}: {e}", staging.display()))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("não criou {}: {e}", staging.display()))?;

    if let Err(error) = copy_all(data_dir, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|e| format!("não limpou {}: {e}", target.display()))?;
    }
    std::fs::rename(&staging, &target).map_err(|e| format!("não publicou {}: {e}", target.display()))?;

    rotate(&backups, KEEP)?;
    Ok(target)
}

fn copy_all(data_dir: &Path, destination: &Path) -> Result<(), String> {
    for relative in SQLITE_FILES {
        let source = data_dir.join(relative);
        if source.is_file() {
            copy_sqlite(&source, &destination.join(file_name(relative)))?;
        }
    }
    for relative in PLAIN_FILES {
        let source = data_dir.join(relative);
        if source.is_file() {
            std::fs::copy(&source, destination.join(file_name(relative))).map_err(|e| format!("copiar {relative}: {e}"))?;
        }
    }
    Ok(())
}

/// Versão vem do manifesto remoto: nada além de `[A-Za-z0-9._-]` vira nome de pasta.
fn sanitize(version: &str) -> String {
    version.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-') { c } else { '_' }).collect()
}

fn file_name(relative: &str) -> &str {
    relative.rsplit('/').next().unwrap_or(relative)
}

fn copy_sqlite(source: &Path, destination: &Path) -> Result<(), String> {
    let conn = Connection::open(source).map_err(|e| format!("abrir {}: {e}", source.display()))?;
    conn.busy_timeout(std::time::Duration::from_millis(5000)).map_err(|e| format!("busy_timeout: {e}"))?;
    conn.execute("VACUUM INTO ?1", [destination.to_string_lossy().as_ref()])
        .map(|_| ())
        .map_err(|e| format!("backup de {}: {e}", source.display()))
}

/// Remove as pastas `pre-*` mais antigas (por data de modificação), deixando as `keep` mais novas.
/// `pre-*.tmp` é o resto de um backup que travou no meio (a atual já foi renomeada antes desta
/// chamada): não conta como backup e é apagada.
fn rotate(backups: &Path, keep: usize) -> Result<(), String> {
    let entries = std::fs::read_dir(backups).map_err(|e| format!("ler {}: {e}", backups.display()))?;
    let (leftovers, mut dirs): (Vec<_>, Vec<_>) = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("pre-") && e.path().is_dir())
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH), e.path()))
        .partition(|(_, path)| path.extension().is_some_and(|ext| ext == "tmp"));
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    let stale = leftovers.into_iter().chain(dirs.into_iter().skip(keep));
    for (_, stale) in stale {
        std::fs::remove_dir_all(&stale).map_err(|e| format!("remover {}: {e}", stale.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!("mika-backup-test-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn seed(data: &Path) {
        let chat = data.join("plugins/mika.chat");
        std::fs::create_dir_all(&chat).unwrap();
        let conn = Connection::open(chat.join("chat.db")).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.execute_batch("CREATE TABLE messages (content TEXT); INSERT INTO messages VALUES ('so no wal');").unwrap();
        // `conn` fica aberta: a escrita pode estar só no -wal, como com o app rodando.
        std::mem::forget(conn);
        std::fs::write(data.join("config.json"), r#"{"a":1}"#).unwrap();
    }

    #[test]
    fn backup_has_live_wal_data_and_config_and_skips_absent_files() {
        let dir = TempDir::new();
        seed(&dir.0);

        let target = backup_before_update(&dir.0, "1.2.3").unwrap();

        assert_eq!(target, dir.0.join("backups/pre-1.2.3"));
        let copy = Connection::open(target.join("chat.db")).unwrap();
        assert_eq!(copy.query_row("SELECT content FROM messages", [], |r| r.get::<_, String>(0)).unwrap(), "so no wal");
        assert_eq!(std::fs::read_to_string(target.join("config.json")).unwrap(), r#"{"a":1}"#);
        assert!(!target.join("memory.db").exists());
    }

    fn seed_sqlite(path: &Path, value: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(&format!("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('{value}');")).unwrap();
    }

    /// Todo dado que não se refaz entra no backup: agenda, sessões de jogo, uso de tokens e o
    /// catálogo de pessoas ficavam de fora e um update de schema de mão única os perdia.
    #[test]
    fn backup_includes_scheduler_game_sessions_token_usage_and_person_catalog() {
        let dir = TempDir::new();
        seed_sqlite(&dir.0.join("plugins/mika.scheduler/scheduler.db"), "agenda");
        seed_sqlite(&dir.0.join("plugins/mika.gamer-companion/gamer-sessions.db"), "sessao");
        seed_sqlite(&dir.0.join("token_usage.db"), "tokens");
        std::fs::create_dir_all(dir.0.join("plugins/mika.person-catalog")).unwrap();
        std::fs::write(dir.0.join("plugins/mika.person-catalog/catalog.json"), r#"{"people":[]}"#).unwrap();

        let target = backup_before_update(&dir.0, "2.0.0").unwrap();

        for (file, value) in [("scheduler.db", "agenda"), ("gamer-sessions.db", "sessao"), ("token_usage.db", "tokens")] {
            let copy = Connection::open(target.join(file)).unwrap();
            assert_eq!(copy.query_row("SELECT v FROM t", [], |r| r.get::<_, String>(0)).unwrap(), value, "{file}");
        }
        assert_eq!(std::fs::read_to_string(target.join("catalog.json")).unwrap(), r#"{"people":[]}"#);
    }

    /// Backup que falha no meio não apaga o anterior da mesma versão nem deixa pasta pela metade.
    #[test]
    fn a_failed_backup_keeps_the_previous_one_and_leaves_no_partial_folder() {
        let dir = TempDir::new();
        std::fs::write(dir.0.join("config.json"), r#"{"a":1}"#).unwrap();
        let first = backup_before_update(&dir.0, "3.0.0").unwrap();
        assert!(first.join("config.json").exists());

        std::fs::create_dir_all(dir.0.join("plugins/mika.scheduler")).unwrap();
        std::fs::write(dir.0.join("plugins/mika.scheduler/scheduler.db"), "isto não é um banco sqlite").unwrap();
        assert!(backup_before_update(&dir.0, "3.0.0").is_err());

        assert!(first.join("config.json").exists(), "o backup anterior sobreviveu");
        assert!(!dir.0.join("backups/pre-3.0.0.tmp").exists(), "pasta parcial ficou para trás");
    }

    #[test]
    fn hostile_version_cannot_escape_the_backups_folder() {
        let dir = TempDir::new();
        let target = backup_before_update(&dir.0, "../../evil").unwrap();
        assert_eq!(target.parent().unwrap(), dir.0.join("backups"));
    }

    fn set_directory_modified(path: &Path, modified: SystemTime) {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // CreateFile precisa de BACKUP_SEMANTICS para diretórios; SetFileTime requer
            // WRITE_ATTRIBUTES, sem necessidade de abrir os dados para escrita.
            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
            options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS).access_mode(FILE_WRITE_ATTRIBUTES);
        }
        options.open(path).unwrap().set_modified(modified).unwrap();
    }

    #[test]
    fn rotation_keeps_only_the_two_most_recent_backups() {
        let dir = TempDir::new();
        let backups = dir.0.join("backups");
        let now = SystemTime::now();
        for (name, age_secs) in [("pre-0.1.0", 300), ("pre-0.2.0", 200), ("pre-0.3.0", 100)] {
            let folder = backups.join(name);
            std::fs::create_dir_all(&folder).unwrap();
            set_directory_modified(&folder, now - Duration::from_secs(age_secs));
        }
        std::fs::create_dir_all(backups.join("outra-coisa")).unwrap();

        backup_before_update(&dir.0, "0.4.0").unwrap();

        let mut left: Vec<String> = std::fs::read_dir(&backups).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["outra-coisa", "pre-0.3.0", "pre-0.4.0"]);
    }

    /// Pasta `pre-X.tmp` de um backup que travou no meio (crash/queda de energia) não é backup: contá-la
    /// na rotação empurrava um backup bom para fora, e ela ficava para sempre ocupando disco.
    #[test]
    fn a_crashed_tmp_folder_is_swept_and_never_counts_as_a_backup() {
        let dir = TempDir::new();
        let backups = dir.0.join("backups");
        let now = SystemTime::now();
        for (name, age_secs) in [("pre-0.1.0", 300), ("pre-0.2.0", 200), ("pre-0.9.0.tmp", 10)] {
            let folder = backups.join(name);
            std::fs::create_dir_all(&folder).unwrap();
            set_directory_modified(&folder, now - Duration::from_secs(age_secs));
        }

        backup_before_update(&dir.0, "0.3.0").unwrap();

        let mut left: Vec<String> = std::fs::read_dir(&backups).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["pre-0.2.0", "pre-0.3.0"], "o .tmp sai e os dois backups de verdade ficam");
    }
}
