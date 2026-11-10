//! Port de `services/steam-local-stats.ts` (metade "tempo de jogo") — lido do DISCO, sem chave de
//! API (fato já verificado em rodadas anteriores da migração). `resolveSteamRoot` tenta o registro
//! do Windows primeiro (via `reg.exe`, sem crate nova — mesmo `Command` que o TS chama via
//! `child_process`), depois variáveis de ambiente. A arte da biblioteca mora em `steam_art.rs` —
//! separado só para caber no limite de 300 linhas por arquivo, mesmo módulo TS de origem.

use super::vdf::{parse_vdf, vdf_path, VdfNode, VdfObject};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocalPlaytime {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub minutes: i64,
    #[serde(rename = "minutes2Weeks")]
    pub minutes_2_weeks: i64,
    #[serde(rename = "lastPlayedAt")]
    pub last_played_at: i64,
}

/// Só a análise da saída de `reg query` — pura e testável sem depender do registro real da
/// máquina que roda o teste.
fn parse_registry_steam_path(output: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)SteamPath\s+REG_SZ\s+(.+)").expect("static regex");
    re.captures(output).map(|c| c[1].trim().to_string())
}

fn read_steam_path_from_registry() -> Option<String> {
    if std::env::consts::OS != "windows" {
        return None;
    }
    let output = Command::new("reg")
        .args(["query", "HKCU\\Software\\Valve\\Steam", "/v", "SteamPath"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let path = parse_registry_steam_path(&text)?;
    Path::new(&path).exists().then_some(path)
}

/// Núcleo puro: dado o caminho configurado e a lista de candidatos JÁ MONTADA (registro + env),
/// devolve o primeiro que tem `appcache`, senão o primeiro que existe.
fn resolve_steam_root_from_candidates(configured: Option<&str>, candidates: &[PathBuf]) -> Option<PathBuf> {
    if let Some(c) = configured {
        let trimmed = c.trim();
        if !trimmed.is_empty() {
            return Path::new(trimmed).exists().then(|| PathBuf::from(trimmed));
        }
    }
    candidates
        .iter()
        .find(|p| p.join("appcache").exists())
        .or_else(|| candidates.iter().find(|p| p.exists()))
        .cloned()
}

/// Lacuna conhecida: `configured` aqui não vem de `gamer-companion.steamPath` — `ToolHandler::call`
/// não recebe `AppHandle` na arquitetura atual (`modules/tools.rs`), então a tool `steam` chama
/// isto com `None` e cai direto para registro + variáveis de ambiente. Relatado, não escondido.
pub fn resolve_steam_root(configured: Option<&str>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(p) = read_steam_path_from_registry() {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("STEAM_PATH") {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("ProgramFiles(x86)") {
        candidates.push(Path::new(&p).join("Steam"));
    }
    if let Ok(p) = std::env::var("ProgramFiles") {
        candidates.push(Path::new(&p).join("Steam"));
    }
    // Linux/macOS: sem registro nem Program Files — as instalações padrão ficam sob o HOME.
    if let Ok(home) = std::env::var("HOME") {
        candidates.extend(super::steam_library::home_steam_paths(Path::new(&home)));
    }
    resolve_steam_root_from_candidates(configured, &candidates)
}

fn to_minutes(value: Option<&str>) -> i64 {
    value.and_then(|v| v.parse::<i64>().ok()).filter(|&n| n >= 0).unwrap_or(0)
}

fn field<'a>(obj: &'a VdfObject, key: &str) -> Option<&'a str> {
    match obj.get(key) {
        Some(VdfNode::Value(v)) => Some(v.as_str()),
        _ => None,
    }
}

fn read_user_playtime(local_config_path: &Path) -> Vec<LocalPlaytime> {
    let Ok(text) = fs::read_to_string(local_config_path) else { return Vec::new() };
    let parsed = parse_vdf(&text);
    let Some(apps) = vdf_path(Some(&parsed), &["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"]) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    for (app_id, node) in apps {
        if !app_id.chars().all(|c| c.is_ascii_digit()) || app_id.is_empty() {
            continue;
        }
        let VdfNode::Object(app) = node else { continue };
        let minutes = to_minutes(field(app, "Playtime").or_else(|| field(app, "playtime")));
        if minutes <= 0 {
            continue;
        }
        entries.push(LocalPlaytime {
            app_id: app_id.clone(),
            minutes,
            minutes_2_weeks: to_minutes(field(app, "Playtime2wks").or_else(|| field(app, "playtime2wks"))),
            last_played_at: to_minutes(field(app, "LastPlayed").or_else(|| field(app, "lastplayed"))),
        });
    }
    entries
}

/// Tempo de jogo local, mesclado entre TODAS as contas Steam já usadas nesta máquina (mantém a
/// maior contagem de minutos por jogo), ordenado do mais jogado para o menos jogado.
pub fn read_local_playtime(steam_root: &Path) -> Vec<LocalPlaytime> {
    let userdata = steam_root.join("userdata");
    let Ok(entries) = fs::read_dir(&userdata) else { return Vec::new() };

    let mut merged: std::collections::HashMap<String, LocalPlaytime> = std::collections::HashMap::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let local_config = entry.path().join("config").join("localconfig.vdf");
        if !local_config.exists() {
            continue;
        }
        for playtime in read_user_playtime(&local_config) {
            let better = merged.get(&playtime.app_id).is_none_or(|prev| playtime.minutes > prev.minutes);
            if better {
                merged.insert(playtime.app_id.clone(), playtime);
            }
        }
    }

    let mut all: Vec<LocalPlaytime> = merged.into_values().collect();
    all.sort_by(|a, b| b.minutes.cmp(&a.minutes));
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::test_support::ScratchDir;

    #[test]
    fn parses_steampath_out_of_a_reg_query_output() {
        let output = "\r\nHKEY_CURRENT_USER\\Software\\Valve\\Steam\r\n    SteamPath    REG_SZ    C:\\Steam\r\n\r\n";
        assert_eq!(parse_registry_steam_path(output), Some("C:\\Steam".to_string()));
    }

    #[test]
    fn parse_registry_steam_path_returns_none_on_unrelated_output() {
        assert!(parse_registry_steam_path("ERROR: The system was unable to find the specified registry key.").is_none());
    }

    #[test]
    fn resolve_steam_root_prefers_the_explicitly_configured_path_when_it_exists() {
        let scratch = ScratchDir::new("root-configured");
        let candidates = vec![PathBuf::from("Z:\\does-not-exist")];
        let resolved = resolve_steam_root_from_candidates(Some(scratch.path().to_str().unwrap()), &candidates);
        assert_eq!(resolved.as_deref(), Some(scratch.path()));
    }

    #[test]
    fn resolve_steam_root_configured_but_missing_is_none_not_a_silent_fallback() {
        let candidates = vec![PathBuf::from("Z:\\also-does-not-exist")];
        assert!(resolve_steam_root_from_candidates(Some("Z:\\configured-missing"), &candidates).is_none());
    }

    #[test]
    fn resolve_steam_root_prefers_a_candidate_that_has_an_appcache_folder() {
        let scratch = ScratchDir::new("root-appcache");
        let without_cache = scratch.path().join("no-cache");
        let with_cache = scratch.path().join("with-cache");
        fs::create_dir_all(&without_cache).unwrap();
        fs::create_dir_all(with_cache.join("appcache")).unwrap();

        let candidates = vec![without_cache, with_cache.clone()];
        assert_eq!(resolve_steam_root_from_candidates(None, &candidates).as_deref(), Some(with_cache.as_path()));
    }

    fn user_local_config(app_id: &str, minutes: &str, minutes_2w: &str) -> String {
        format!(
            "\"UserLocalConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"Valve\"\n\t\t{{\n\t\t\t\"Steam\"\n\t\t\t{{\n\t\t\t\t\"apps\"\n\t\t\t\t{{\n\t\t\t\t\t\"{app_id}\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"Playtime\"\t\t\"{minutes}\"\n\t\t\t\t\t\t\"Playtime2wks\"\t\t\"{minutes_2w}\"\n\t\t\t\t\t}}\n\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n"
        )
    }

    #[test]
    fn reads_and_merges_playtime_across_multiple_accounts_keeping_the_highest() {
        let scratch = ScratchDir::new("playtime");
        let root = scratch.path();

        let acc1 = root.join("userdata").join("1").join("config");
        let acc2 = root.join("userdata").join("2").join("config");
        fs::create_dir_all(&acc1).unwrap();
        fs::create_dir_all(&acc2).unwrap();
        fs::write(acc1.join("localconfig.vdf"), user_local_config("10", "120", "30")).unwrap();
        fs::write(acc2.join("localconfig.vdf"), user_local_config("10", "500", "0")).unwrap();

        let playtime = read_local_playtime(root);
        assert_eq!(playtime.len(), 1);
        assert_eq!(playtime[0].minutes, 500);
    }

    #[test]
    fn zero_minute_entries_are_skipped() {
        let scratch = ScratchDir::new("zero-minutes");
        let acc = scratch.path().join("userdata").join("1").join("config");
        fs::create_dir_all(&acc).unwrap();
        fs::write(acc.join("localconfig.vdf"), user_local_config("10", "0", "0")).unwrap();

        assert!(read_local_playtime(scratch.path()).is_empty());
    }

    #[test]
    fn missing_userdata_folder_returns_empty_instead_of_erroring() {
        let scratch = ScratchDir::new("no-userdata");
        assert!(read_local_playtime(scratch.path()).is_empty());
    }
}
