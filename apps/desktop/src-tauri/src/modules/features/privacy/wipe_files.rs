//! Varreduras de arquivo puras para `clear_history` — sem `AppHandle`: cada função recebe o
//! diretório/arquivo JÁ RESOLVIDO (mesmo padrão de `chat_db.rs`/`history_store.rs::attach`, que
//! separam "resolver o caminho a partir do `AppHandle`" — só em produção, sem teste de unidade —
//! de "operar sobre um caminho conhecido" — testável com um `TempDir` comum). `clear.rs::register`
//! resolve os caminhos uma única vez, no boot, e guarda os `PathBuf` prontos no handler.
//!
//! Cada função aqui espelha, arquivo a arquivo, uma varredura que o TypeScript já faz hoje:
//! `deleteAllCaptures` (`vision/memory/visual-memory.ts`), `clearLogFiles`
//! (`main/infrastructure/log-file-sink.ts`), `clearExportedArchives` (`privacy/export-store.ts`) e
//! `clearAllSafeSecrets` (`main/infrastructure/adapters/safe-secret-store.ts`). Diretório ausente é
//! SEMPRE `0` removido, nunca erro — mesma política de todas as quatro origens.

use std::path::Path;

/// Apaga todo arquivo direto de `dir` cujo nome passa em `should_delete` — melhor esforço: uma
/// falha (arquivo em uso, permissão) não aborta as demais, só não conta. Subdiretórios são
/// ignorados (`remove_file` recusa diretório), nunca descidos — nenhuma das quatro origens TS
/// espera achar subpastas nesses diretórios.
///
/// **O predicado diz o que APAGAR, não o que preservar.** Ele já se chamou `keep`, e o nome dizia o
/// oposto do que o corpo faz: `clear_log_files` passa "começa com `mika.log`", que sob a leitura de
/// `keep` seria "apague todo arquivo que NÃO é log da Mika".
///
/// A inversão em si não passaria: `clear_log_files_only_matches_the_three_known_prefixes` e
/// `clear_exported_archives_only_matches_zip_files` provam que o arquivo alheio SOBREVIVE, e
/// quebrariam na hora. O risco é o outro: esta é a função mais destrutiva do crate, e um leitor que
/// confie no nome lê o corpo como bug e "conserta" — aí o teste vermelho vira, para ele, evidência
/// de que o teste é que estava errado. Nome que contradiz o corpo custa caro exatamente onde o erro
/// é irreversível.
fn wipe_dir_entries(dir: &Path, should_delete: impl Fn(&str) -> bool) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        if !should_delete(&name) {
            continue;
        }
        if std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// = `PersonImageCache.clearAll` (metade de disco — a memória não existe deste lado, ver
/// `features::person_catalog::store.rs::clear_all`) e `deleteAllCaptures`: os dois apagam TODO
/// arquivo do diretório, sem filtro de nome.
pub(super) fn clear_dir_all(dir: &Path) -> usize {
    wipe_dir_entries(dir, |_| true)
}

/// Apaga as SUBPASTAS de `dir` cujo nome começa com `prefix` (`backups/pre-*` do backup pré-update:
/// cada uma é uma cópia do chat, da memória e do perfil). Melhor esforço, como as irmãs; arquivos
/// soltos e pastas de outro nome ficam.
pub(super) fn clear_prefixed_dirs(dir: &Path, prefix: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix) && entry.path().is_dir())
        .filter(|entry| std::fs::remove_dir_all(entry.path()).is_ok())
        .count()
}

/// Cópias `<catálogo>.json.corrupt-<ms>.bak` que o catálogo de pessoas deixa ao descartar um JSON
/// corrompido: carregam os mesmos nomes em claro, então morrem junto com o catálogo.
pub(super) fn clear_catalog_backups(catalog_dir: &Path) -> usize {
    wipe_dir_entries(catalog_dir, |name| name.contains(".json.corrupt-") && name.ends_with(".bak"))
}

/// = `clearLogFiles`: só os 3 prefixos que `log-file-sink.ts` também varre (`purgeExpired` usa a
/// MESMA lista) — nunca um arquivo de log de outro processo que por acaso more na mesma pasta.
pub(super) fn clear_log_files(logs_dir: &Path) -> usize {
    wipe_dir_entries(logs_dir, |name| {
        name.starts_with("mika-human.log") || name.starts_with("mika-ai.ndjson") || name.starts_with("mika.log")
    })
}

/// = `clearExportedArchives`: só `.zip` — a pasta é exclusiva de `export_data`
/// (`features::privacy::export::register` grava no MESMO `exports_dir`), mas o filtro por
/// extensão é o que o TS também aplica, não uma suposição de exclusividade.
pub(super) fn clear_exported_archives(exports_dir: &Path) -> usize {
    wipe_dir_entries(exports_dir, |name| name.ends_with(".zip"))
}

/// = `clearAllSafeSecrets`: conta as chaves do JSON (sem decifrar nenhuma — `clear` nunca precisa
/// ler o segredo, só contar e apagar o arquivo inteiro) e só apaga se havia pelo menos uma — um
/// arquivo ausente OU vazio (`{}`) devolve `0` sem tocar o disco, mesma política do TS
/// (`count === 0` nunca chama `unlink`).
pub(super) fn clear_safe_secrets(path: &Path) -> Result<usize, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(format!("ler {}: {e}", path.display())),
    };

    // JSON corrompido conta como "sem segredo legível" — mesmo efeito de `readSecretStore` (TS),
    // que devolve `{}` num `catch(SyntaxError)` em vez de propagar o erro de parse.
    let count = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&raw).map(|m| m.len()).unwrap_or(0);
    if count == 0 {
        return Ok(0);
    }
    std::fs::remove_file(path).map_err(|e| format!("apagar {}: {e}", path.display()))?;
    Ok(count)
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
            let path = std::env::temp_dir().join(format!("mika-privacy-wipe-files-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"conteudo fabricado de teste").unwrap();
    }

    #[test]
    fn clear_dir_all_removes_every_file_regardless_of_name() {
        let dir = TempDir::new();
        touch(&dir.0, "foto1.png");
        touch(&dir.0, "captura-2026-09-20.jpg");
        assert_eq!(clear_dir_all(&dir.0), 2);
        assert!(std::fs::read_dir(&dir.0).unwrap().next().is_none());
    }

    #[test]
    fn clear_dir_all_on_a_missing_directory_is_zero_not_an_error() {
        let dir = TempDir::new();
        assert_eq!(clear_dir_all(&dir.0.join("nao-existe")), 0);
    }

    #[test]
    fn clear_prefixed_dirs_removes_the_pre_update_backups_and_nothing_else() {
        let dir = TempDir::new();
        for backup in ["pre-1.2.3", "pre-1.2.4.tmp"] {
            std::fs::create_dir_all(dir.0.join(backup)).unwrap();
            touch(&dir.0.join(backup), "chat.db");
        }
        std::fs::create_dir_all(dir.0.join("outra-pasta")).unwrap();
        touch(&dir.0, "pre-solto.txt");

        assert_eq!(clear_prefixed_dirs(&dir.0, "pre-"), 2);
        assert!(!dir.0.join("pre-1.2.3").exists() && !dir.0.join("pre-1.2.4.tmp").exists());
        assert!(dir.0.join("outra-pasta").exists() && dir.0.join("pre-solto.txt").exists());
        assert_eq!(clear_prefixed_dirs(&dir.0.join("nao-existe"), "pre-"), 0);
    }

    #[test]
    fn clear_log_files_only_matches_the_three_known_prefixes() {
        let dir = TempDir::new();
        touch(&dir.0, "mika-human.log");
        touch(&dir.0, "mika-human.log.1");
        touch(&dir.0, "mika-ai.ndjson");
        touch(&dir.0, "mika.log");
        touch(&dir.0, "some-other-app.log");

        assert_eq!(clear_log_files(&dir.0), 4);
        let remaining: Vec<_> = std::fs::read_dir(&dir.0).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].to_str().unwrap(), "some-other-app.log");
    }

    #[test]
    fn clear_exported_archives_only_matches_zip_files() {
        let dir = TempDir::new();
        touch(&dir.0, "1700000000000.zip");
        touch(&dir.0, "manifest.json");

        assert_eq!(clear_exported_archives(&dir.0), 1);
        let remaining: Vec<_> = std::fs::read_dir(&dir.0).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].to_str().unwrap(), "manifest.json");
    }

    #[test]
    fn clear_safe_secrets_counts_keys_and_deletes_the_whole_file() {
        let dir = TempDir::new();
        let path = dir.0.join("mika-safe-secrets.json");
        std::fs::write(&path, r#"{"a:b":"cifrado1","c:d":"cifrado2"}"#).unwrap();

        assert_eq!(clear_safe_secrets(&path).unwrap(), 2);
        assert!(!path.exists());
    }

    #[test]
    fn clear_safe_secrets_on_a_missing_file_is_zero_not_an_error() {
        let dir = TempDir::new();
        assert_eq!(clear_safe_secrets(&dir.0.join("nao-existe.json")).unwrap(), 0);
    }

    /// NEGATIVO — arquivo vazio (`{}`) devolve 0 e o arquivo continua no disco, mesma política do
    /// TS (`count === 0` nunca chama `unlink`).
    #[test]
    fn clear_safe_secrets_leaves_an_empty_object_file_untouched() {
        let dir = TempDir::new();
        let path = dir.0.join("mika-safe-secrets.json");
        std::fs::write(&path, "{}").unwrap();

        assert_eq!(clear_safe_secrets(&path).unwrap(), 0);
        assert!(path.exists(), "arquivo vazio não devia ter sido apagado");
    }

    #[test]
    fn clear_safe_secrets_on_a_corrupted_file_reports_zero_without_erroring() {
        let dir = TempDir::new();
        let path = dir.0.join("mika-safe-secrets.json");
        std::fs::write(&path, "isto nao e json").unwrap();

        assert_eq!(clear_safe_secrets(&path).unwrap(), 0);
        assert!(path.exists());
    }
}
