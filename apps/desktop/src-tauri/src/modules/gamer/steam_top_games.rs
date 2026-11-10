//! `getSteamTopGames` — porte de `services/steam-top-games.ts`, SÓ o braço "sem Web API": o nome
//! do jogo vem exclusivamente da biblioteca INSTALADA (`steam_library::list_steam_games`), nunca de
//! `getOwnedGames`/`getSteamCreds` (`services/steam-api.ts`) — mesmo braço que o TS já usa quando
//! `hasApi` é falso (sem `steamApiKey`/`steamId64` configurados). Medido nesta rodada: playtime e
//! arte vêm do DISCO, sem chave de API — `steam_local_stats`/`steam_art` já fazem isso, reusados
//! aqui, não recriados. A Web API para enriquecer nomes de jogos NÃO instalados fica de fora —
//! nomeado, não escondido: sem ela, um jogo com playtime > 0 mas desinstalado é pulado (mesmo TS
//! quando `resolveName` não acha o nome, ver `pickNamedGames`).

use serde::Serialize;

use super::steam_art::{read_game_art, SteamArtKind};
use super::steam_library::list_steam_games;
use super::steam_local_stats::{read_local_playtime, resolve_steam_root};

const CANDIDATE_POOL_MIN: usize = 12;
const CANDIDATE_POOL_FACTOR: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SteamTopGame {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub name: String,
    #[serde(rename = "playtimeMinutes")]
    pub playtime_minutes: i64,
    #[serde(rename = "playtimeLabel")]
    pub playtime_label: String,
    pub installed: bool,
    pub art: Option<String>,
    #[serde(rename = "artKind")]
    pub art_kind: Option<SteamArtKind>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SteamTopGamesResult {
    #[serde(rename = "steamFound")]
    pub steam_found: bool,
    pub source: &'static str,
    pub games: Vec<SteamTopGame>,
}

fn empty() -> SteamTopGamesResult {
    SteamTopGamesResult { steam_found: false, source: "installed", games: Vec::new() }
}

/// Espelha `hoursPlayed` (`services/steam-api.ts`).
fn hours_played(minutes: i64) -> String {
    if minutes < 60 {
        return format!("{minutes}min");
    }
    let hours = minutes / 60;
    let rest = minutes % 60;
    if rest != 0 {
        format!("{hours}h{rest}min")
    } else {
        format!("{hours}h")
    }
}

/// Espelha `getSteamTopGames(getConfig, limit, by='total')` — `by='recent'` não é exercido pela
/// RPC `steam:top-games` (que nunca envia esse parâmetro), então não está portado.
pub fn get_steam_top_games(configured: Option<&str>, limit: usize) -> SteamTopGamesResult {
    let Some(root) = resolve_steam_root(configured) else { return empty() };

    // A raiz JÁ resolvida (registro/HOME quando `configured` é None) — não a configurada crua.
    let installed_games = list_steam_games(root.to_str(), true);
    let installed: std::collections::HashMap<&str, &str> =
        installed_games.iter().map(|g| (g.app_id.as_str(), g.name.as_str())).collect();

    let playtime = read_local_playtime(&root);
    let source = if playtime.is_empty() { "installed" } else { "local" };
    let candidate_pool = CANDIDATE_POOL_MIN.max(limit * CANDIDATE_POOL_FACTOR);

    let ranked: Vec<(String, i64)> = if !playtime.is_empty() {
        playtime.iter().take(candidate_pool).map(|entry| (entry.app_id.clone(), entry.minutes)).collect()
    } else {
        installed_games.iter().take(candidate_pool).map(|game| (game.app_id.clone(), 0)).collect()
    };

    // Jogo sem nome resolvível é pulado sem substituir a vaga — mesmo comentário de `pickNamedGames`.
    let mut games = Vec::new();
    for (app_id, minutes) in ranked {
        if games.len() >= limit {
            break;
        }
        let Some(&name) = installed.get(app_id.as_str()) else { continue };
        let art = read_game_art(&root, &app_id);
        games.push(SteamTopGame {
            app_id: app_id.clone(),
            name: name.to_string(),
            playtime_minutes: minutes,
            playtime_label: if minutes > 0 { hours_played(minutes) } else { String::new() },
            installed: installed.contains_key(app_id.as_str()),
            art: art.as_ref().map(|a| a.data_uri.clone()),
            art_kind: art.as_ref().map(|a| a.kind),
        });
    }

    SteamTopGamesResult { steam_found: true, source, games }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::test_support::ScratchDir;
    use std::fs;

    fn write_manifest(steam_apps: &std::path::Path, app_id: &str, name: &str) {
        fs::create_dir_all(steam_apps).unwrap();
        let text = format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{app_id}\"\n\t\"name\"\t\t\"{name}\"\n}}\n");
        fs::write(steam_apps.join(format!("appmanifest_{app_id}.acf")), text).unwrap();
    }

    /// Um `localconfig.vdf` com VÁRIOS apps — escrever um por chamada SOBRESCREVERIA o arquivo a
    /// cada vez (mesmo caminho, `userdata/1/config/localconfig.vdf`), perdendo os anteriores.
    fn write_playtime(root: &std::path::Path, entries: &[(&str, &str)]) {
        let config_dir = root.join("userdata").join("1").join("config");
        fs::create_dir_all(&config_dir).unwrap();
        let apps: String = entries
            .iter()
            .map(|(app_id, minutes)| {
                format!("\t\t\t\t\t\"{app_id}\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"Playtime\"\t\t\"{minutes}\"\n\t\t\t\t\t}}\n")
            })
            .collect();
        let text = format!(
            "\"UserLocalConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"Valve\"\n\t\t{{\n\t\t\t\"Steam\"\n\t\t\t{{\n\t\t\t\t\"apps\"\n\t\t\t\t{{\n{apps}\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n"
        );
        fs::write(config_dir.join("localconfig.vdf"), text).unwrap();
    }

    #[test]
    fn no_steam_root_returns_the_empty_result() {
        let result = get_steam_top_games(Some("Z:\\does-not-exist"), 8);
        assert_eq!(result, empty());
    }

    #[test]
    fn ranks_installed_games_by_local_playtime_and_skips_unresolvable_names() {
        let scratch = ScratchDir::new("top-games");
        let root = scratch.path();
        write_manifest(&root.join("steamapps"), "10", "Counter-Strike");
        write_manifest(&root.join("steamapps"), "570", "Dota 2");
        // "999" não tem manifest instalado — deve ser pulado sem tomar a vaga de um jogo com nome
        // resolvível. As três entram no MESMO `localconfig.vdf` (um por conta Steam).
        write_playtime(root, &[("10", "120"), ("570", "500"), ("999", "900")]);

        let result = get_steam_top_games(Some(root.to_str().unwrap()), 8);
        assert_eq!(result.steam_found, true);
        assert_eq!(result.source, "local");
        assert_eq!(result.games.len(), 2, "jogo 999 sem nome instalado não deveria contar contra o limite");
        assert_eq!(result.games[0].app_id, "570", "mais jogado vem primeiro");
        assert_eq!(result.games[0].playtime_label, "8h20min");
        assert_eq!(result.games[1].app_id, "10");
        assert_eq!(result.games[1].playtime_label, "2h");
    }

    /// `list_steam_games(configured, true)` também varre os caminhos PADRÃO do SO (mesma paridade
    /// de `listSteamGames(configuredPath)` no TS, `includeDefaults` default `true`) — então numa
    /// máquina com Steam de verdade instalado, `result.games` pode ter MAIS que o meu fixture. Um
    /// `limit` generoso garante que o meu apareça mesmo com uma biblioteca real grande ao lado;
    /// a asserção busca por `app_id`, nunca por tamanho total do vetor.
    #[test]
    fn falls_back_to_installed_games_with_zero_playtime_when_no_local_playtime_exists() {
        let scratch = ScratchDir::new("top-games-installed-only");
        let root = scratch.path();
        write_manifest(&root.join("steamapps"), "10", "Counter-Strike");

        let result = get_steam_top_games(Some(root.to_str().unwrap()), 1000);
        assert_eq!(result.source, "installed");
        let mine = result.games.iter().find(|g| g.app_id == "10").expect("meu fixture deveria estar no catálogo");
        assert_eq!(mine.playtime_minutes, 0);
        assert_eq!(mine.playtime_label, "", "sem playtime > 0 o rótulo fica vazio, não '0min'");
    }

    #[test]
    fn hours_played_matches_the_typescript_formatting() {
        assert_eq!(hours_played(30), "30min");
        assert_eq!(hours_played(60), "1h");
        assert_eq!(hours_played(90), "1h30min");
    }
}
