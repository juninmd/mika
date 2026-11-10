//! `resolveLaunchTarget`/`isBlockedExecutable` — port 1:1 de
//! `apps/desktop/src/main/infrastructure/os/app-launch-target.ts`. Puro: nenhuma das duas funções
//! toca disco, rede ou processo.
//!
//! **Chamador real desde esta rodada** — `super::launch::run_launch` usa as duas como as duas
//! primeiras barreiras de `action=launch` (formato + denylist de interpretador), antes do
//! chokepoint de modo offline (`crate::modules::privacy`, terceira barreira — ver `launch.rs`).

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LaunchTarget {
    App(String),
    Url(String),
}

const WINDOWS_ALIASES: &[(&str, &str)] = &[
    ("calculadora", "calc.exe"),
    ("calculador", "calc.exe"),
    ("calculator", "calc.exe"),
    ("calc", "calc.exe"),
    ("bloco de notas", "notepad.exe"),
    ("notepad", "notepad.exe"),
    ("editor de texto", "notepad.exe"),
    ("paint", "mspaint.exe"),
    ("explorador", "explorer.exe"),
    ("explorer", "explorer.exe"),
    ("gerenciador de arquivos", "explorer.exe"),
    ("cmd", "cmd.exe"),
    ("powershell", "powershell.exe"),
    ("chrome", "chrome.exe"),
    ("edge", "msedge.exe"),
    ("firefox", "firefox.exe"),
    ("spotify", "spotify.exe"),
    ("discord", "discord.exe"),
    ("steam", "steam.exe"),
    ("vs code", "code"),
    ("vscode", "code"),
    ("code", "code"),
];

const POSIX_ALIASES: &[(&str, &str)] = &[
    ("calculadora", "gnome-calculator"),
    ("calculator", "gnome-calculator"),
    ("bloco de notas", "gedit"),
    ("notepad", "gedit"),
    ("chrome", "google-chrome"),
    ("firefox", "firefox"),
    ("vs code", "code"),
    ("vscode", "code"),
    ("code", "code"),
];

const SITE_ALIASES: &[(&str, &str)] = &[
    ("youtube", "https://www.youtube.com"),
    ("you tube", "https://www.youtube.com"),
    ("google", "https://www.google.com"),
    ("github", "https://github.com"),
    ("duolingo", "https://www.duolingo.com"),
    ("gmail", "https://mail.google.com"),
    ("twitch", "https://www.twitch.tv"),
    ("netflix", "https://www.netflix.com"),
];

const BLOCKED_EXECUTABLES: &[&str] = &[
    "cmd", "cmd.exe", "powershell", "powershell.exe", "pwsh", "pwsh.exe", "powershell_ise",
    "powershell_ise.exe", "wt", "wt.exe", "regedit", "regedit.exe", "wsl", "wsl.exe", "certutil",
    "certutil.exe", "mshta", "mshta.exe", "rundll32", "rundll32.exe", "regsvr32", "regsvr32.exe",
    "bitsadmin", "bitsadmin.exe", "cscript", "cscript.exe", "wscript", "wscript.exe", "msiexec",
    "msiexec.exe", "forfiles", "forfiles.exe", "installutil", "installutil.exe", "regasm",
    "regasm.exe", "regsvcs", "regsvcs.exe", "msbuild", "msbuild.exe", "csc", "csc.exe",
];

pub(super) fn is_blocked_executable(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    BLOCKED_EXECUTABLES.contains(&lower.as_str())
}

/// Substituto aproximado de `stripDiacritics` (`packages/sdk/src/utils/text-normalize.ts`, que usa
/// `text.normalize('NFD')` + remoção de marcas diacríticas via Unicode) — sem `unicode-normalization`
/// disponível nesta frente, cobre os acentos latinos (pt-BR/es) que os alias acima realmente usam.
/// Chave de lookup para um mapa fixo de apelidos, não redação de segurança — aproximação aceitável.
fn strip_diacritics(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            other => other,
        })
        .collect()
}

fn normalize(value: &str) -> String {
    let stripped = strip_diacritics(value).to_lowercase();
    let mut normalized = String::with_capacity(stripped.len());
    let mut last_was_space = false;
    for ch in stripped.trim().chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                normalized.push(' ');
            }
            last_was_space = true;
        } else {
            normalized.push(ch);
            last_was_space = false;
        }
    }
    normalized.trim().to_string()
}

fn as_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return None;
    }
    let parsed = url::Url::parse(trimmed).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

/// ALLOWLIST: só um executável que algum apelido conhecido da plataforma já resolve (`calc.exe`,
/// `notepad.exe`, `code`...) é aceito como nome "cru". Antes, qualquer `[A-Za-z0-9._-]{1,64}` passava
/// e só uma denylist curta barrava — `bash`, `sh`, `python`, `node`, `perl`, `osascript` e
/// `x-terminal-emulator` abriam. Devolve a forma canônica da tabela, nunca a string do modelo.
fn known_app(candidate: &str, is_windows: bool) -> Option<&'static str> {
    let aliases = if is_windows { WINDOWS_ALIASES } else { POSIX_ALIASES };
    aliases.iter().map(|(_, command)| *command).find(|command| command.eq_ignore_ascii_case(candidate))
}

/// Port de `resolveLaunchTarget`. `is_windows` espelha o parâmetro `platform` do TS.
pub(super) fn resolve_launch_target(raw: &str, is_windows: bool) -> Option<LaunchTarget> {
    if let Some(url) = as_url(raw) {
        return Some(LaunchTarget::Url(url));
    }

    let key = normalize(raw);
    if key.is_empty() {
        return None;
    }

    if let Some((_, site)) = SITE_ALIASES.iter().find(|(alias, _)| *alias == key) {
        return Some(LaunchTarget::Url((*site).to_string()));
    }

    let aliases = if is_windows { WINDOWS_ALIASES } else { POSIX_ALIASES };
    if let Some((_, command)) = aliases.iter().find(|(alias, _)| *alias == key) {
        return Some(LaunchTarget::App((*command).to_string()));
    }

    known_app(raw.trim(), is_windows).map(|command| LaunchTarget::App(command.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_http_and_https_as_url_targets() {
        assert_eq!(
            resolve_launch_target("https://example.com/path", true),
            Some(LaunchTarget::Url("https://example.com/path".into()))
        );
        assert_eq!(resolve_launch_target("HTTP://EXAMPLE.COM", true), Some(LaunchTarget::Url("http://example.com/".into())));
    }

    #[test]
    fn resolves_site_aliases_regardless_of_platform() {
        assert_eq!(resolve_launch_target("youtube", true), Some(LaunchTarget::Url("https://www.youtube.com".into())));
        assert_eq!(resolve_launch_target("YouTube", false), Some(LaunchTarget::Url("https://www.youtube.com".into())));
    }

    #[test]
    fn resolves_platform_specific_app_aliases() {
        assert_eq!(resolve_launch_target("calculadora", true), Some(LaunchTarget::App("calc.exe".into())));
        assert_eq!(resolve_launch_target("calculadora", false), Some(LaunchTarget::App("gnome-calculator".into())));
    }

    #[test]
    fn accepts_the_executable_of_a_known_alias_as_an_app_target() {
        assert_eq!(resolve_launch_target("notepad.exe", true), Some(LaunchTarget::App("notepad.exe".into())));
        assert_eq!(resolve_launch_target("NOTEPAD.EXE", true), Some(LaunchTarget::App("notepad.exe".into())));
        assert_eq!(resolve_launch_target("gnome-calculator", false), Some(LaunchTarget::App("gnome-calculator".into())));
    }

    /// NEGATIVO — a denylist deixava interpretadores e utilitários passarem; a allowlist não
    /// reconhece nada que não seja o executável de um apelido conhecido DA PLATAFORMA.
    #[test]
    fn rejects_interpreters_and_unknown_binaries_that_the_old_denylist_let_through() {
        for command in [
            "bash", "sh", "zsh", "python", "python3", "node", "perl", "ruby", "osascript", "x-terminal-emulator",
            "terminal", "xterm", "curl", "npm", "explorer.exe.bat", "malware.exe", "open", "xdg-open",
        ] {
            for is_windows in [true, false] {
                assert_eq!(resolve_launch_target(command, is_windows), None, "{command} (windows={is_windows}) não pode virar alvo");
            }
        }
        // Executável de OUTRA plataforma não vale: `notepad.exe` não existe no alias POSIX.
        assert_eq!(resolve_launch_target("notepad.exe", false), None);
        assert_eq!(resolve_launch_target("gnome-calculator", true), None);
    }

    /// NEGATIVO — comando com metacaracteres de shell não vira alvo nenhum.
    #[test]
    fn rejects_unsafe_command_candidates() {
        assert_eq!(resolve_launch_target("calc.exe & del *", true), None);
        assert_eq!(resolve_launch_target("../../etc/passwd", true), None);
        assert_eq!(resolve_launch_target("", true), None);
        assert_eq!(resolve_launch_target("   ", true), None);
    }

    #[test]
    fn blocks_the_documented_shell_tools_case_insensitively() {
        for blocked in ["cmd.exe", "PowerShell.EXE", "regedit", "wsl"] {
            assert!(is_blocked_executable(blocked), "{blocked} deveria estar bloqueado");
        }
    }

    #[test]
    fn does_not_block_ordinary_applications() {
        for allowed in ["notepad.exe", "code", "spotify.exe"] {
            assert!(!is_blocked_executable(allowed), "{allowed} não deveria estar bloqueado");
        }
    }

    #[test]
    fn normalize_strips_accents_case_and_collapses_whitespace() {
        assert_eq!(normalize("  Calculadora  "), "calculadora");
        assert_eq!(normalize("é    um    teste"), "e um teste");
    }

    // --- pin de segurança (`app-launch-target.ts` foi removido — backend Node deletado na
    // migração para Tauri) ---------------------------------------------------------------------
    //
    // O invariante que importava era o SENTIDO "não falta": nenhum interpretador que o TS
    // bloqueava podia faltar aqui, porque cada um que faltasse era um shell que o modelo
    // conseguia abrir pelo backend Rust e não conseguia no Electron. `BLOCKED_EXECUTABLES` e
    // `SITE_ALIASES` já são a única fonte de verdade em produção; o snapshot abaixo é a lista que
    // `app-launch-target.ts` tinha no momento da deleção, congelada em Rust — um item removido de
    // `BLOCKED_EXECUTABLES` sem querer ainda reprova aqui, só que contra um literal em vez de um
    // `include_str!`.
    const FROZEN_BLOCKED_EXECUTABLES: &[&str] = &[
        "cmd", "cmd.exe", "powershell", "powershell.exe", "pwsh", "pwsh.exe", "powershell_ise",
        "powershell_ise.exe", "wt", "wt.exe", "regedit", "regedit.exe", "wsl", "wsl.exe", "certutil",
        "certutil.exe", "mshta", "mshta.exe", "rundll32", "rundll32.exe", "regsvr32", "regsvr32.exe",
        "bitsadmin", "bitsadmin.exe", "cscript", "cscript.exe", "wscript", "wscript.exe", "msiexec",
        "msiexec.exe", "forfiles", "forfiles.exe", "installutil", "installutil.exe", "regasm",
        "regasm.exe", "regsvcs", "regsvcs.exe", "msbuild", "msbuild.exe", "csc", "csc.exe",
    ];

    const FROZEN_SITE_ALIASES: &[(&str, &str)] = &[
        ("youtube", "https://www.youtube.com"),
        ("you tube", "https://www.youtube.com"),
        ("google", "https://www.google.com"),
        ("github", "https://github.com"),
        ("duolingo", "https://www.duolingo.com"),
        ("gmail", "https://mail.google.com"),
        ("twitch", "https://www.twitch.tv"),
        ("netflix", "https://www.netflix.com"),
    ];

    #[test]
    fn no_frozen_blocked_executable_is_missing_from_production() {
        for command in FROZEN_BLOCKED_EXECUTABLES {
            assert!(
                is_blocked_executable(command),
                "'{command}' estava bloqueado antes da deleção do TS e NÃO está mais aqui — o modelo abriria esse interpretador pelo backend Rust"
            );
        }
        assert_eq!(BLOCKED_EXECUTABLES.len(), FROZEN_BLOCKED_EXECUTABLES.len(), "BLOCKED_EXECUTABLES mudou de tamanho — releia antes de ajustar o pin");
    }

    #[test]
    fn no_frozen_site_alias_is_missing_from_production() {
        for (alias, url) in FROZEN_SITE_ALIASES {
            assert!(
                SITE_ALIASES.iter().any(|(a, u)| a == alias && u == url),
                "alias de site '{alias}' estava presente antes da deleção do TS e sumiu daqui"
            );
        }
        assert_eq!(SITE_ALIASES.len(), FROZEN_SITE_ALIASES.len(), "SITE_ALIASES mudou de tamanho — releia antes de ajustar o pin");
    }
}
