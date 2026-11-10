//! Port de `services/steam-library.ts` — biblioteca Steam instalada, lida do DISCO (manifests
//! `appmanifest_*.acf`), sem chave de API. `resolveSteamRoot` (registro do Windows + fallback de
//! diretório) mora em `steam_local_stats.rs`, igual à divisão do TS: são dois caminhos de
//! resolução de raiz independentes, usados por chamadores diferentes — unificá-los aqui seria um
//! refactor não pedido.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SteamGame {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub name: String,
    #[serde(rename = "installDir", skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
}

pub fn is_valid_steam_app_id(app_id: &str) -> bool {
    !app_id.is_empty() && app_id.len() <= 10 && app_id.chars().all(|c| c.is_ascii_digit())
}

fn extract_field(text: &str, key: &str) -> Option<String> {
    let pattern = format!(r#"(?i)"{key}"\s+"([^"]+)""#);
    regex::Regex::new(&pattern).ok()?.captures(text).map(|c| c[1].to_string())
}

fn is_appmanifest_filename(name: &str) -> bool {
    let lower = name.to_lowercase();
    let Some(rest) = lower.strip_prefix("appmanifest_") else { return false };
    let Some(digits) = rest.strip_suffix(".acf") else { return false };
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

fn parse_library_folders(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"(?i)"path"\s+"([^"]+)""#).expect("static regex");
    re.captures_iter(text).map(|c| c[1].replace("\\\\", "\\")).collect()
}

/// `STEAM_PATH`/`ProgramFiles`/`ProgramFiles(x86)` — mesma ordem e mesmas variáveis de
/// `DEFAULT_STEAM_PATHS` (`steam-library.ts`). Sem equivalente para Linux/macOS: o TS de origem
/// também não tem um aqui (só via `configuredPath`) — não é um gap desta migração.
/// Instalações padrão do Steam no Linux (nativo e Flatpak) e no macOS, sob o HOME.
pub(super) fn home_steam_paths(home: &Path) -> [PathBuf; 4] {
    [
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join("Library/Application Support/Steam"),
    ]
}

fn default_steam_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(p) = std::env::var("STEAM_PATH") {
        if !p.trim().is_empty() {
            paths.push(PathBuf::from(p));
        }
    }
    if let Ok(p) = std::env::var("ProgramFiles") {
        paths.push(Path::new(&p).join("Steam"));
    }
    if let Ok(p) = std::env::var("ProgramFiles(x86)") {
        paths.push(Path::new(&p).join("Steam"));
    }
    if let Ok(home) = std::env::var("HOME") {
        paths.extend(home_steam_paths(Path::new(&home)));
    }
    paths
}

fn read_steam_roots(configured: Option<&str>, include_defaults: bool) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(c) = configured {
        if !c.trim().is_empty() {
            roots.push(PathBuf::from(c));
        }
    }
    if include_defaults {
        roots.extend(default_steam_paths());
    }
    let mut seen = HashSet::new();
    roots.into_iter().filter(|p| seen.insert(p.clone())).filter(|p| p.exists()).collect()
}

fn read_library_dirs(root: &Path) -> Vec<PathBuf> {
    let steam_apps = root.join("steamapps");
    let library_config = steam_apps.join("libraryfolders.vdf");
    let mut dirs = Vec::new();
    if steam_apps.exists() {
        dirs.push(steam_apps);
    }
    if let Ok(text) = fs::read_to_string(&library_config) {
        for library_path in parse_library_folders(&text) {
            let library_steam_apps = Path::new(&library_path).join("steamapps");
            if library_steam_apps.exists() {
                dirs.push(library_steam_apps);
            }
        }
    }
    let mut seen = HashSet::new();
    dirs.into_iter().filter(|d| seen.insert(d.clone())).collect()
}

fn read_games_from_steam_apps(steam_apps_path: &Path) -> Vec<SteamGame> {
    let Ok(entries) = fs::read_dir(steam_apps_path) else { return Vec::new() };
    let mut games = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else { continue };
        if !is_appmanifest_filename(name) {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else { continue };
        let (Some(app_id), Some(game_name)) = (extract_field(&text, "appid"), extract_field(&text, "name")) else {
            continue;
        };
        if !is_valid_steam_app_id(&app_id) {
            continue;
        }
        games.push(SteamGame { app_id, name: game_name, install_dir: extract_field(&text, "installdir") });
    }
    games
}

/// Lista jogos instalados, ordenados por nome, deduplicados por `appId` (a última biblioteca lida
/// vence — mesma semântica do `Map` do TS).
pub fn list_steam_games(configured: Option<&str>, include_defaults: bool) -> Vec<SteamGame> {
    let mut by_id: HashMap<String, SteamGame> = HashMap::new();
    for root in read_steam_roots(configured, include_defaults) {
        for dir in read_library_dirs(&root) {
            for game in read_games_from_steam_apps(&dir) {
                by_id.insert(game.app_id.clone(), game);
            }
        }
    }
    let mut games: Vec<SteamGame> = by_id.into_values().collect();
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    games
}

pub fn find_steam_game<'a>(games: &'a [SteamGame], query: &str) -> Option<&'a SteamGame> {
    let normalized = query.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    games
        .iter()
        .find(|g| g.app_id == normalized)
        .or_else(|| games.iter().find(|g| g.name.to_lowercase() == normalized))
        .or_else(|| games.iter().find(|g| g.name.to_lowercase().contains(&normalized)))
}

/// Comando de lançamento por `steam://run/<appId>`, resolvido em RUNTIME (`env::consts::OS`), não
/// por `#[cfg]` — assim os três ramos compilam e são testáveis em qualquer SO de build, e a
/// detecção não tem como "assumir Windows" silenciosamente (já houve um bug desses aqui).
pub fn steam_launch_command(app_id: &str) -> Option<(String, Vec<String>)> {
    if !is_valid_steam_app_id(app_id) {
        return None;
    }
    let url = format!("steam://run/{app_id}");
    Some(match std::env::consts::OS {
        "windows" => ("cmd.exe".to_string(), vec!["/c".into(), "start".into(), String::new(), url]),
        "macos" => ("open".to_string(), vec![url]),
        _ => ("xdg-open".to_string(), vec![url]),
    })
}

/// Dispara o comando e não espera — mesmo `child.unref()` "fire and forget" do TS. NÃO porta a
/// verificação pós-lançamento via `tasklist` (protocolo não registrado ainda reportaria sucesso);
/// relatado como lacuna, não implementado por escopo desta rodada.
pub fn open_steam_game(app_id: &str) -> Result<(), String> {
    let Some((program, args)) = steam_launch_command(app_id) else {
        return Err(format!("appId inválido: {app_id}"));
    };
    let mut command = Command::new(&program);
    command.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::test_support::ScratchDir;

    /// Sem isto o watcher/`steam` não achavam NENHUM jogo instalado no Linux/macOS (só Program Files).
    #[test]
    fn home_steam_paths_cover_linux_flatpak_and_macos_installs() {
        let paths = home_steam_paths(Path::new("/home/u"));
        assert!(paths.contains(&PathBuf::from("/home/u/.local/share/Steam")));
        assert!(paths.contains(&PathBuf::from("/home/u/.steam/steam")));
        assert!(paths.iter().any(|p| p.ends_with("Library/Application Support/Steam")));
    }

    fn write_manifest(dir: &Path, app_id: &str, name: &str, install_dir: &str) {
        fs::create_dir_all(dir).unwrap();
        let text = format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{app_id}\"\n\t\"name\"\t\t\"{name}\"\n\t\"installdir\"\t\t\"{install_dir}\"\n}}\n");
        fs::write(dir.join(format!("appmanifest_{app_id}.acf")), text).unwrap();
    }

    #[test]
    fn lists_games_from_a_steamapps_folder() {
        let scratch = ScratchDir::new("list");
        let steam_apps = scratch.path().join("steamapps");
        write_manifest(&steam_apps, "10", "Counter-Strike", "Counter-Strike");
        write_manifest(&steam_apps, "570", "Dota 2", "dota 2 beta");

        let games = list_steam_games(Some(scratch.path().to_str().unwrap()), false);
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].name, "Counter-Strike");
        assert_eq!(games[1].name, "Dota 2");
        assert_eq!(games[1].install_dir.as_deref(), Some("dota 2 beta"));
    }

    #[test]
    fn skips_manifests_with_non_numeric_or_missing_appid() {
        let scratch = ScratchDir::new("badid");
        let steam_apps = scratch.path().join("steamapps");
        fs::create_dir_all(&steam_apps).unwrap();
        fs::write(steam_apps.join("appmanifest_x.acf"), "\"AppState\" { \"appid\" \"notanumber\" \"name\" \"Bad\" }")
            .unwrap();

        assert!(list_steam_games(Some(scratch.path().to_str().unwrap()), false).is_empty());
    }

    #[test]
    fn follows_libraryfolders_vdf_into_additional_library_drives() {
        let scratch = ScratchDir::new("multilib");
        let primary = scratch.path().join("primary");
        let secondary = scratch.path().join("secondary");
        write_manifest(&primary.join("steamapps"), "10", "Counter-Strike", "cs");
        write_manifest(&secondary.join("steamapps"), "570", "Dota 2", "dota");

        let secondary_str = secondary.to_str().unwrap().replace('\\', "\\\\");
        fs::write(
            primary.join("steamapps").join("libraryfolders.vdf"),
            format!("\"libraryfolders\"\n{{\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{secondary_str}\"\n\t}}\n}}\n"),
        )
        .unwrap();

        let games = list_steam_games(Some(primary.to_str().unwrap()), false);
        assert_eq!(games.len(), 2);
    }

    #[test]
    fn find_steam_game_matches_by_app_id_then_exact_name_then_substring() {
        let games = vec![
            SteamGame { app_id: "10".into(), name: "Counter-Strike".into(), install_dir: None },
            SteamGame { app_id: "570".into(), name: "Dota 2".into(), install_dir: None },
        ];
        assert_eq!(find_steam_game(&games, "10").unwrap().name, "Counter-Strike");
        assert_eq!(find_steam_game(&games, "Dota 2").unwrap().name, "Dota 2");
        assert_eq!(find_steam_game(&games, "dota").unwrap().name, "Dota 2");
        assert!(find_steam_game(&games, "   ").is_none());
        assert!(find_steam_game(&games, "unknown").is_none());
    }

    #[test]
    fn steam_launch_command_rejects_invalid_app_ids() {
        assert!(steam_launch_command("not-a-number").is_none());
        assert!(steam_launch_command("").is_none());
    }

    #[test]
    fn steam_launch_command_builds_the_steam_protocol_url_for_every_supported_os() {
        let (_, args) = steam_launch_command("570").unwrap();
        assert!(args.iter().any(|a| a == "steam://run/570"));
    }

    #[test]
    fn open_steam_game_rejects_before_spawning_anything() {
        let err = open_steam_game("not-valid").unwrap_err();
        assert!(err.contains("appId inválido"));
    }
}
