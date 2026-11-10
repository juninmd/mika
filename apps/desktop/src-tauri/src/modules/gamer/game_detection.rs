//! Port de `services/game-detection.ts`. `find_active_game` (correspondência por processo em
//! execução) e, desde 2026-09-22, [`find_foreground_game`] (o irmão do `features/gamer-companion`,
//! usado por `steam-game-watcher.ts` para casar a janela em foco com título/`installDir` por
//! SUBSTRING, além do match por processo) — a lacuna de DEPENDÊNCIA que bloqueava isto fechou junto
//! com `modules/system_signals/active_window.rs` (Windows, Linux/X11 e macOS; ver `probe.rs`). Quem chama com o
//! snapshot é `gamer::game_watcher::tick`, na MESMA precedência de `steam-game-watcher.ts::tick()`
//! (janela em foco disponível → só ela decide, nunca cai para processo no mesmo tick).
//!
//! Listagem de processos usa `sysinfo` (já declarada em `Cargo.toml` para este propósito) em vez
//! do `tasklist`/`ps` que o TS chama via `child_process` — mesmo dado, sem shell-out, e já
//! multiplataforma pela própria crate.

use super::known_games::{self, KnownGame};
use super::steam_library::SteamGame;
use std::collections::HashSet;

pub struct ActiveGameInfo {
    pub name: String,
    pub app_id: Option<String>,
}

/// Um jogo que a pessoa ensinou em Configurações → Gamer Companion ("Jogos personalizados"):
/// processo → título. Vem de `gamer-companion.customGames` (`{ "processo": "Título" }`) e tem
/// PRECEDÊNCIA sobre Steam e [`known_games`] — quem cadastrou sabe melhor que a tabela estática.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomGame {
    pub process_key: String,
    pub name: String,
}

/// Chaves normalizadas com [`normalize`] (mesma regra do processo em execução), título aparado.
/// Entrada sem chave útil (< 3 chars normalizados) ou sem título é descartada, não adivinhada.
pub fn custom_games(value: Option<&serde_json::Value>) -> Vec<CustomGame> {
    let Some(map) = value.and_then(serde_json::Value::as_object) else { return Vec::new() };
    map.iter()
        .filter_map(|(process, title)| {
            let process_key = normalize(process);
            let name = title.as_str()?.trim().to_string();
            (process_key.len() >= 3 && !name.is_empty()).then_some(CustomGame { process_key, name })
        })
        .collect()
}

/// Primeiro processo (na ordem dada) que bate EXATO com um jogo personalizado.
pub fn find_custom_game<'a>(process_keys: impl IntoIterator<Item = &'a str>, custom: &[CustomGame]) -> Option<(ActiveGameInfo, String)> {
    process_keys.into_iter().find_map(|key| {
        custom.iter().find(|game| game.process_key == key).map(|game| {
            (ActiveGameInfo { name: game.name.clone(), app_id: None }, key.to_string())
        })
    })
}

pub fn normalize(value: &str) -> String {
    let lower = value.to_lowercase();
    let without_exe = lower.strip_suffix(".exe").unwrap_or(&lower);
    without_exe.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn game_keys(game: &SteamGame) -> Vec<String> {
    [Some(game.name.as_str()), game.install_dir.as_deref()]
        .into_iter()
        .flatten()
        .map(normalize)
        .filter(|k| k.len() >= 4)
        .collect()
}

/// Nomes de processo em execução, já normalizados e deduplicados mantendo a ordem da PRIMEIRA
/// ocorrência — mesma semântica de `new Set((await listProcessNames()).map(normalize))` no TS.
///
/// Só a lista de processos, sem CPU/memória/disco de cada um (`ProcessRefreshKind::nothing()`, nome
/// vem de graça): `System::new_all()` enumerava também discos, rede e sensores a cada 30s.
pub fn list_process_names() -> Vec<String> {
    let mut seen = HashSet::new();
    running_processes().into_iter().map(|(_, key)| key).filter(|key| seen.insert(key.clone())).collect()
}

/// `(pid, nome normalizado)` de cada processo em execução, na ordem do SO. `list_process_names` é
/// este retrato sem o pid e sem repetição; o pid existe para o teste com processos reais olhar SÓ os
/// filhos que ele próprio criou (`game_detection_live_tests`) — o retrato inteiro inclui o que
/// outras execuções de `cargo test` na mesma máquina estão rodando agora.
pub(super) fn running_processes() -> Vec<(u32, String)> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, sysinfo::ProcessRefreshKind::nothing());
    system.processes().iter().map(|(pid, p)| (pid.as_u32(), normalize(&p.name().to_string_lossy()))).collect()
}

/// `process_keys_ordered` precisa preservar a ordem de descoberta: o fallback para jogos
/// conhecidos usa a PRIMEIRA chave que bater, e um `HashSet` sem ordem tornaria esse resultado
/// não-determinístico quando dois jogos conhecidos estão rodando ao mesmo tempo.
pub fn find_active_game(process_keys_ordered: &[String], steam_games: &[SteamGame]) -> Option<(ActiveGameInfo, String)> {
    let process_set: HashSet<&str> = process_keys_ordered.iter().map(String::as_str).collect();

    if let Some(steam) = steam_games.iter().find(|g| game_keys(g).iter().any(|k| process_set.contains(k.as_str()))) {
        let process_key = game_keys(steam).into_iter().find(|k| process_set.contains(k.as_str())).unwrap_or_default();
        return Some((
            ActiveGameInfo { name: steam.name.clone(), app_id: Some(steam.app_id.clone()) },
            process_key,
        ));
    }

    for key in process_keys_ordered {
        if let Some(known) = known_games::find_known_game(key) {
            let KnownGame { display_name, .. } = known;
            return Some((
                ActiveGameInfo { name: (*display_name).to_string(), app_id: None },
                key.clone(),
            ));
        }
    }
    None
}

/// Porte de `isNonGameForeground` (`game-detection.ts`, `features/gamer-companion`) — nome de
/// processo em foco que NUNCA é considerado jogo mesmo com o título batendo por substring (IDE,
/// browser, launcher de OS). Substring (`contains`), não fronteira de palavra — cópia literal do
/// `.includes()` do TS, propositalmente mais permissivo que `matches_pattern` de
/// `vision::capture_gate` (arquivos diferentes, contratos diferentes).
///
/// Estendida (smoke-baseline.md item 8, P0): "Stray" foi detectado com o Windows Terminal em foco
/// no boot — a lista não cobria terminais/consoles/IDEs/chat suficientes. Ampliada com o console
/// host do Windows (que muitas vezes é quem está em foco de verdade atrás de um terminal), IDEs,
/// mensageiros e os terminais mais comuns de Linux/macOS (multiplataforma por padrão, AGENTS.md).
const NON_GAME_FOREGROUND_PROCESSES: &[&str] = &[
    "code",
    "visualstudio",
    "devenv",
    "cursor",
    "idea64",
    "chrome",
    "msedge",
    "edge",
    "firefox",
    "brave",
    "opera",
    "vivaldi",
    "figma",
    "notion",
    "slack",
    "discord",
    "telegram",
    "explorer",
    "notepad",
    "obsidian",
    "powershell",
    "pwsh",
    "terminal",
    "windowsterminal",
    "conhost",
    "cmd",
    "gnometerminal",
    "konsole",
    "iterm2",
];

fn is_non_game_foreground(process_key: &str) -> bool {
    NON_GAME_FOREGROUND_PROCESSES.iter().any(|name| process_key.contains(name))
}

/// Chave mínima para um match de TÍTULO por substring "cru" (contra o blob inteiro, sem respeitar
/// separador de palavra) — abaixo disto, só conta um token exato ou o título inteiro. Existe porque
/// `game_keys`/`known_games` normalizam removendo TODO caractere não-alfanumérico, então uma chave
/// curta e comum ("stray", "prey", "inside", "control", "portal" — todas nomes reais de jogo)
/// aparece como substring de palavras do dia a dia sem ligação nenhuma com o jogo ("**astray**",
/// "**o**s**prey**", "**control**e de volume", "tele**portal**"). Foi assim que a Mika comentou
/// "parkour de gato" (Stray) com o Windows Terminal em foco (smoke-baseline.md item 8): o processo
/// já estava coberto, mas o TÍTULO da janela também precisa da mesma disciplina de fronteira, não
/// só o processo. `8` cobre folgado as 5 chaves do relatório (4-7 chars) e ainda deixa nomes
/// compostos longos (`counterstrikeglobaloffensive`, 29 chars) baterem por substring como antes.
const MIN_TITLE_BLOB_KEY_LEN: usize = 8;

/// Tokeniza o título ORIGINAL (não o blob já normalizado) em palavras minúsculas, cada uma cortada
/// nas mesmas fronteiras não-alfanuméricas que [`normalize`] já usa para o resto — mas SEM juntar
/// tudo numa string só, preservando onde uma palavra termina e a próxima começa.
fn title_tokens(title: &str) -> HashSet<String> {
    title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

/// Só true quando a chave é uma PALAVRA inteira do título (fronteira real) ou longa o bastante para
/// que uma coincidência de substring seja implausível (ver [`MIN_TITLE_BLOB_KEY_LEN`]) — nunca mais
/// "stray" batendo dentro de "astray".
fn title_matches_key(tokens: &HashSet<String>, title_blob: &str, key: &str) -> bool {
    tokens.contains(key) || (key.len() >= MIN_TITLE_BLOB_KEY_LEN && title_blob.contains(key))
}

/// Porte de `findForegroundGame(window, steamGames)` — casa a janela em primeiro plano com a
/// biblioteca Steam (processo OU, se o processo não é um app "claramente não-jogo", TÍTULO por
/// palavra/substring — ver [`title_matches_key`]) e, sem match, com [`known_games`] pela mesma
/// regra. `title`/`process_name` chegam CRUS (não normalizados) — a função normaliza por conta
/// própria, mesmo contrato de `find_active_game` recebendo chaves já prontas vs. este recebendo
/// texto cru, espelhando a assinatura exata do TS (`ForegroundWindow` inteiro, não só chaves).
pub fn find_foreground_game(title: &str, process_name: &str, steam_games: &[SteamGame]) -> Option<ActiveGameInfo> {
    let process_key = normalize(process_name);
    let title_key = normalize(title);
    let title_eligible = !title_key.is_empty() && !is_non_game_foreground(&process_key);
    let tokens = title_tokens(title);

    if let Some(steam) = steam_games.iter().find(|g| {
        game_keys(g).iter().any(|key| process_key == *key || (title_eligible && title_matches_key(&tokens, &title_key, key)))
    }) {
        return Some(ActiveGameInfo { name: steam.name.clone(), app_id: Some(steam.app_id.clone()) });
    }

    let known = known_games::find_known_game(&process_key)
        .or_else(|| if title_eligible { known_games::find_known_game_by_title(&tokens, &title_key) } else { None })?;
    let KnownGame { display_name, .. } = known;
    Some(ActiveGameInfo { name: (*display_name).to_string(), app_id: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(app_id: &str, name: &str, install_dir: Option<&str>) -> SteamGame {
        SteamGame { app_id: app_id.into(), name: name.into(), install_dir: install_dir.map(String::from) }
    }

    #[test]
    fn custom_games_normalize_the_process_and_drop_unusable_entries() {
        let raw = serde_json::json!({ "MyIndie.exe": " Meu Indie ", "ab": "Curto", "outro.exe": "", "num": 3 });
        assert_eq!(custom_games(Some(&raw)), vec![CustomGame { process_key: "myindie".into(), name: "Meu Indie".into() }]);
        assert!(custom_games(None).is_empty());
    }

    #[test]
    fn a_custom_game_matches_its_process_exactly() {
        let custom = vec![CustomGame { process_key: "myindie".into(), name: "Meu Indie".into() }];
        let (info, key) = find_custom_game(["notepad", "myindie"], &custom).unwrap();
        assert_eq!((info.name.as_str(), key.as_str()), ("Meu Indie", "myindie"));
        assert!(find_custom_game(["myindie2"], &custom).is_none(), "substring não conta, só o processo exato");
    }

    #[test]
    fn normalize_strips_trailing_exe_and_non_alphanumeric_characters() {
        assert_eq!(normalize("Counter-Strike2.EXE"), "counterstrike2");
        assert_eq!(normalize("steam.exe"), "steam");
    }

    #[test]
    fn finds_an_active_steam_game_by_normalized_name() {
        let games = vec![game("730", "Counter-Strike 2", Some("Counter-Strike Global Offensive"))];
        let processes = vec!["counterstrike2".to_string()];

        let (info, process_key) = find_active_game(&processes, &games).unwrap();
        assert_eq!(info.name, "Counter-Strike 2");
        assert_eq!(info.app_id.as_deref(), Some("730"));
        assert_eq!(process_key, "counterstrike2");
    }

    #[test]
    fn falls_back_to_a_known_non_steam_game_when_no_steam_title_matches() {
        let processes = vec!["valorant".to_string()];
        let (info, _) = find_active_game(&processes, &[]).unwrap();
        assert_eq!(info.name, "Valorant");
    }

    #[test]
    fn no_match_returns_none() {
        let processes = vec!["notepad".to_string()];
        assert!(find_active_game(&processes, &[]).is_none());
    }

    #[test]
    fn short_install_dir_keys_are_ignored_to_avoid_false_positives() {
        // installDir com menos de 4 caracteres normalizados nunca deveria casar sozinho.
        let games = vec![game("1", "Some Game", Some("abc"))];
        let processes = vec!["abc".to_string()];
        assert!(find_active_game(&processes, &games).is_none());
    }

    #[test]
    fn find_foreground_game_matches_steam_by_process_name() {
        let games = vec![game("730", "Counter-Strike 2", Some("Counter-Strike Global Offensive"))];
        let found = find_foreground_game("some window", "CounterStrike2.exe", &games);
        assert_eq!(found.unwrap().name, "Counter-Strike 2");
    }

    /// NEGATIVO — processo sem nenhuma chave Steam (nem `>= 4` chars) nem entrada em
    /// `known_games` não deveria bater com nada.
    #[test]
    fn find_foreground_game_returns_none_when_neither_steam_nor_known_games_match() {
        let games = vec![game("730", "Counter-Strike 2", Some("Counter-Strike Global Offensive"))];
        let found = find_foreground_game("some window", "unknownlauncher.exe", &games);
        assert!(found.is_none());
    }

    #[test]
    fn find_foreground_game_matches_steam_by_title_substring_when_the_process_is_eligible() {
        let games = vec![game("1145360", "Hades", None)];
        let found = find_foreground_game("Hades - Steam", "hades.exe", &games);
        assert_eq!(found.unwrap().name, "Hades");
    }

    /// Mesmo cenário acima, mas com o PROCESSO deliberadamente diferente da chave do jogo (o teste
    /// irmão passava mesmo sem a correção, via `process_key == key`, sem exercitar o caminho de
    /// título de verdade) — prova que um título de várias palavras ainda casa por PALAVRA inteira
    /// mesmo depois da disciplina de fronteira.
    #[test]
    fn find_foreground_game_matches_steam_by_a_whole_title_word_when_the_process_key_differs() {
        let games = vec![game("1145360", "Hades", None)];
        let found = find_foreground_game("Hades - Steam", "steam.exe", &games);
        assert_eq!(found.unwrap().name, "Hades");
    }

    /// NEGATIVO — reprodução direta do defeito real (smoke-baseline.md item 8): "Stray" é um jogo
    /// conhecido de chave curta ("stray"); um título que contém essa chave só como substring de uma
    /// palavra MAIOR (não a palavra inteira) não deveria bater, mesmo com um processo elegível
    /// (nunca visto antes, fora de `NON_GAME_FOREGROUND_PROCESSES`).
    #[test]
    fn find_foreground_game_ignores_a_short_known_game_key_embedded_inside_a_longer_title_word() {
        let found = find_foreground_game("Astray Thoughts Journal", "notepad2.exe", &[]);
        assert!(found.is_none(), "'stray' dentro de 'Astray' não deveria virar o jogo Stray");
    }

    /// NEGATIVO — mesmo defeito, agora com o cenário exato relatado: Windows Terminal em foco, sem
    /// nenhum jogo rodando, título genérico de shell.
    #[test]
    fn find_foreground_game_returns_none_for_a_generic_shell_title_with_windows_terminal_in_foreground() {
        let found = find_foreground_game("Windows PowerShell", "WindowsTerminal.exe", &[]);
        assert!(found.is_none());
    }

    /// NEGATIVO — a própria janela da Mika (configurações/about) nunca deveria virar detecção de
    /// jogo, mesmo com um processo nunca catalogado.
    #[test]
    fn find_foreground_game_returns_none_for_mikas_own_window_title() {
        let found = find_foreground_game("Mika conversational AI assistant", "mika.exe", &[]);
        assert!(found.is_none());
    }

    /// POSITIVO — ainda reconhece o jogo de verdade quando a chave curta aparece como PALAVRA
    /// inteira e isolada do título (não overcorrigido a ponto de nunca mais reconhecer "Stray").
    #[test]
    fn find_foreground_game_still_matches_a_short_known_game_key_as_a_whole_title_word() {
        let found = find_foreground_game("Stray - Steam", "unknownlauncher.exe", &[]);
        assert_eq!(found.unwrap().name, "Stray");
    }

    /// A extensão de `NON_GAME_FOREGROUND_PROCESSES` (terminais/IDEs/chat cross-platform) cobre um
    /// processo elegível ANTES mesmo de chegar na disciplina de palavra do título.
    #[test]
    fn find_foreground_game_excludes_the_newly_added_non_game_processes_even_with_a_matching_title() {
        for process in [
            "conhost.exe",
            "idea64.exe",
            "Telegram.exe",
            "notepad.exe",
            "Obsidian.exe",
            "gnome-terminal",
            "konsole",
            "iTerm2",
        ] {
            let found = find_foreground_game("Stray - playing now", process, &[]);
            assert!(found.is_none(), "{process} deveria estar em NON_GAME_FOREGROUND_PROCESSES");
        }
    }

    /// NEGATIVO — porte de `isNonGameForeground`: um título que contém o nome de um jogo por
    /// coincidência (aba de navegador, documento) não conta enquanto o PROCESSO em foco é
    /// claramente não-jogo (browser, IDE, launcher de OS).
    #[test]
    fn find_foreground_game_ignores_a_title_match_when_the_process_is_a_known_non_game_app() {
        let games = vec![game("1145360", "Hades", None)];
        let found = find_foreground_game("wiki: Hades walkthrough - Chrome", "chrome.exe", &games);
        assert!(found.is_none(), "chrome é NON_GAME_FOREGROUND_PROCESSES — título não deveria bastar");
    }

    #[test]
    fn find_foreground_game_falls_back_to_a_known_non_steam_game_by_process() {
        let found = find_foreground_game("", "valorant.exe", &[]);
        assert_eq!(found.unwrap().name, "Valorant");
    }

    #[test]
    fn find_foreground_game_returns_none_without_any_match() {
        assert!(find_foreground_game("index.ts - Visual Studio Code", "code.exe", &[]).is_none());
    }
}
