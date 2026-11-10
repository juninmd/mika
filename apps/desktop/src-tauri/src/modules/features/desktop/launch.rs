//! `action=launch` — port de `runLaunch` (`apps/desktop/src/features/desktop/tools/index.ts`) +
//! `createShellAdapter().launchApp`/`openExternal`
//! (`apps/desktop/src/main/infrastructure/adapters/shell-adapter.ts`).
//!
//! **Fronteira de confiança, não conveniência: o MODELO escolhe o alvo.** Três barreiras, na MESMA
//! ordem do TS:
//! 1. [`super::launch_target::resolve_launch_target`] — allowlist (URL http(s), ou apelido
//!    / executável de apelido conhecido da plataforma — nada de nome de comando arbitrário). Caminho absoluto, pipe,
//!    redirecionamento, argumento solto: nenhum vira alvo.
//! 2. [`super::launch_target::is_blocked_executable`] — denylist de interpretador/execução
//!    indireta (`cmd`, `powershell`, `wsl`, `mshta`, `rundll32`...), mesmo que o alvo já tenha
//!    passado pela allowlist de formato.
//! 3. O chokepoint de modo offline (`crate::modules::privacy`) — URL passa por
//!    [`crate::modules::privacy::assert_network_allowed`]; comando de app passa por
//!    [`assert_shell_launch_allowed`] (abaixo), o par que fecha a MESMA lacuna que
//!    `assertShellNetworkAllowed` fecha no TS: sem ela, pedir para "abrir" `curl`/`wget`/`npm`
//!    atravessaria o modo offline por fora do `fetch`/`createLoggedFetch`.
//!
//! **Nunca monta comando por concatenação de string nem passa por shell.** `std::process::Command`
//! recebe o executável como argv\[0\] sozinho — a tool nunca recebe `args` do modelo, só `target` —
//! e a URL abre via [`tauri_plugin_shell::open::open`] (função LIVRE, sem `AppHandle`), que por
//! baixo chama `open::that_detached`: `ShellExecuteW` no Windows, `open` no macOS, `xdg-open` no
//! Linux — nunca `cmd /c`/`sh -c`.
//!
//! ## Por que `assert_shell_launch_allowed` mora aqui, não em `crate::modules::privacy`
//! Conceitualmente é o par de `assert_network_allowed` (mesmo arquivo fonte TS,
//! `offline-mode.ts::assertShellNetworkAllowed`), mas meu escopo nesta rodada é só
//! `modules/features/desktop/` — mover para `modules/privacy` fica para quem for dono daquele
//! arquivo (relatado, não feito aqui).
//!
//! ## Divergência de FORMA medida, não corrigida: `errorCode` do bloqueio por modo offline
//! No TS, `assertNetworkAllowed` (ramo URL) é pego por um `try/catch` PRÓPRIO dentro de `runLaunch`
//! que devolve `errorCode: 'not_configured'`. `assertShellNetworkAllowed` (ramo app, chamado DE
//! DENTRO de `shellAdapter.launchApp`) não tem esse catch local — o `throw` escapa para o `catch`
//! EXTERNO de `runLaunch`, que devolve `errorCode: 'internal'`. Mesma causa (modo offline ligado),
//! dois códigos — uma assimetria do TS, não um bug introduzido aqui: reproduzida tal como o fonte
//! mede, porque trocar o código de erro é uma decisão de contrato que não é minha para tomar nesta
//! rodada.

use std::collections::HashSet;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{json, Value};

use crate::config::store::ConfigStore;

use super::launch_target::{is_blocked_executable, resolve_launch_target, LaunchTarget};

/// Espelha o `setTimeout(..., 200)` de `launchApp` (`shell-adapter.ts`): tempo suficiente para um
/// binário inexistente sair com erro antes de reportarmos sucesso.
const SPAWN_SETTLE_MS: u64 = 200;

/// Espelha `NETWORK_EXECUTABLES` (`packages/sdk/src/http/offline-mode.ts`) — clientes de rede de
/// linha de comando; mesma lista, mesma ordem irrelevante (é um `HashSet` dos dois lados).
fn network_executables() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        [
            "curl", "wget", "nc", "ncat", "netcat", "telnet", "ssh", "scp", "sftp", "rsync", "ftp", "git", "npm",
            "npx", "pnpm", "yarn", "pip", "pip3", "aria2c", "youtube-dl", "yt-dlp", "edge-tts",
        ]
        .into_iter()
        .collect()
    })
}

/// Espelha `executableName` (`offline-mode.ts`): tira diretório e extensão de shell conhecida.
fn executable_name(command: &str) -> String {
    let base = command.rsplit(['/', '\\']).next().unwrap_or(command);
    let lower = base.to_ascii_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
        if let Some(stripped) = lower.strip_suffix(ext) {
            return stripped.to_string();
        }
    }
    lower
}

/// Espelha `assertShellNetworkAllowed`. Sem parâmetro `args`: a tool `desktop` nunca passa
/// argumento nenhum para `launch` (só `target`), então o filtro por `args` do TS (regex sobre
/// `invoke-webrequest`/`curl`/`wget` inline) nunca teria como disparar aqui — omitido em vez de
/// portado morto.
fn assert_shell_launch_allowed(config: &ConfigStore, command: &str) -> Result<(), String> {
    if !crate::modules::privacy::is_offline_mode_enabled(config) {
        return Ok(());
    }
    let executable = executable_name(command);
    if !network_executables().contains(executable.as_str()) {
        return Ok(());
    }
    log::warn!("modo offline: comando de rede bloqueado (command={executable})");
    Err(format!(
        "Offline mode is on: request to 'shell:{executable}' blocked ({})",
        crate::modules::privacy::OFFLINE_MODE_PATH
    ))
}

fn invalid_args(message: String) -> Value {
    json!({ "success": false, "error": message, "errorCode": "invalid_args" })
}

/// Porte de `runLaunch`. `config` vem do handler — só existe na segunda passada de registro (ver
/// `tool.rs`/`mod.rs`).
pub(super) async fn run_launch(target: Option<&str>, config: &ConfigStore) -> Value {
    let Some(target) = target.map(str::trim).filter(|t| !t.is_empty()) else {
        return invalid_args("target is required for action=launch.".into());
    };

    let is_windows = cfg!(target_os = "windows");
    let Some(resolved) = resolve_launch_target(target, is_windows) else {
        return invalid_args(format!(
            "Cannot safely open \"{target}\" — give a program name (e.g. calc.exe) or a URL."
        ));
    };

    match resolved {
        LaunchTarget::App(command) => {
            if is_blocked_executable(&command) {
                return json!({
                    "success": false,
                    "error": format!("\"{command}\" is a shell/system tool and is always blocked for safety."),
                    "errorCode": "not_configured",
                });
            }
            if let Err(error) = assert_shell_launch_allowed(config, &command) {
                // Mesma assimetria de errorCode do TS — ver o cabeçalho do arquivo.
                return json!({ "success": false, "error": error, "errorCode": "internal" });
            }
            spawn_app(&command).await
        }
        LaunchTarget::Url(url) => {
            if let Err(error) = crate::modules::privacy::assert_network_allowed(config, &url, "desktop") {
                return json!({ "success": false, "error": error, "errorCode": "not_configured" });
            }
            open_url(&url)
        }
    }
}

/// `std::process::Command` com o executável como argv\[0\] e SEM argumentos. Nunca passa por
/// shell: `resolve_launch_target` já reduziu `target` a um executável de apelido conhecido (sem espaço,
/// pipe ou redirecionamento — `known_app` em `launch_target.rs`), e `Command::spawn` invoca o
/// executável diretamente (`CreateProcess`/`execvp`, nunca `cmd /c`/`sh -c`).
async fn spawn_app(command: &str) -> Value {
    let spawned = std::process::Command::new(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => return json!({ "success": false, "error": format!("Failed to spawn: {error}"), "errorCode": "internal" }),
    };

    // Espelha o `setTimeout(200)` de `launchApp`: dá tempo de um binário que sai na hora (erro,
    // arg inválido) reportar isso, antes de assumirmos sucesso. `try_wait` é não bloqueante — não
    // precisa da feature `process` do tokio (fora do que posso declarar em Cargo.toml nesta frente).
    tokio::time::sleep(Duration::from_millis(SPAWN_SETTLE_MS)).await;

    match child.try_wait() {
        Ok(Some(status)) if status.success() => json!({ "success": true, "data": { "opened": command, "kind": "app" } }),
        Ok(Some(status)) => json!({
            "success": false,
            "error": format!("Exited immediately with code {}.", status.code().unwrap_or(-1)),
            "errorCode": "internal",
        }),
        Ok(None) => json!({ "success": true, "data": { "opened": command, "kind": "app", "pid": child.id() } }),
        Err(error) => json!({ "success": false, "error": error.to_string(), "errorCode": "internal" }),
    }
}

/// Abre a URL no navegador padrão do SO sem shell nenhum — `tauri_plugin_shell::open::open` é
/// função LIVRE (sem `AppHandle`) que chama `open::that_detached` por baixo: `ShellExecuteW` no
/// Windows, `open` no macOS, `xdg-open` no Linux. Depreciada desde tauri-plugin-shell 2.1 em favor
/// de `tauri-plugin-opener` — que NÃO está em `Cargo.toml` (fora do que posso adicionar nesta
/// frente) — então uso o que já está disponível em vez de um shell-out manual.
fn open_url(url: &str) -> Value {
    #[allow(deprecated)]
    let result = tauri_plugin_shell::open::open(None, url, None);
    match result {
        Ok(()) => json!({ "success": true, "data": { "opened": url, "kind": "url" } }),
        Err(error) => json!({ "success": false, "error": error.to_string(), "errorCode": "internal" }),
    }
}

// Suíte em arquivo separado só pela regra de 300 linhas do AGENTS.md — mesmo padrão de
// `privacy/tests.rs`. Continua sendo o mesmo módulo `launch::tests`.
#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
