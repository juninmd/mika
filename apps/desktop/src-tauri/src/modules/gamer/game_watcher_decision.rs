//! Núcleo puro de decisão de `game_watcher.rs` — separado dele pelo limite de 300 linhas do
//! AGENTS.md (mesmo corte que `system_signals/active_window.rs`/`foreground.rs` já fez). Cobre a
//! escolha de DETECTOR (janela em foco vs. processo, porte de `steam-game-watcher.ts::tick()`), a
//! graça de `handleNoGame()` (desde 2026-09-22) e a transição de sessão. Testável sem SO, SQLite
//! nem `AppHandle` — `game_watcher.rs` só chama, nunca reimplementa.

use super::game_detection::{self, ActiveGameInfo, CustomGame};
use super::steam_library;
use crate::modules::system_signals::active_window::ActiveWindowSnapshot;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Transition {
    None,
    Started { game_id: String, game_name: String, game_process: String },
    Stopped,
}

/// Porte de `source: 'foreground-window' | 'process-watcher'` (`steam-game-watcher.ts::tick()`) —
/// QUEM decidiu o `active` desta passada. [`decide_transition`] só concede a graça de
/// `handleNoGame` quando a fonte é a janela em foco, mesma condição do TS
/// (`source === 'foreground-window'`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Source {
    ForegroundWindow,
    ProcessWatcher,
}

/// `Some((title, owner))` quando a janela ativa TEM identidade — porte da condição
/// `foregroundWindow && (normalize(title) || normalize(processName))` de
/// `steam-game-watcher.ts::tick()`. `None` (sem sonda, ou plataforma sem sonda) é o sinal para
/// [`detect_active_game`] cair no fallback por processo.
pub(super) fn foreground_identity(window: Option<&ActiveWindowSnapshot>) -> Option<(&str, &str)> {
    let window = window?;
    let title = window.title.as_deref().unwrap_or("");
    let owner = window.owner.as_deref().unwrap_or("");
    if game_detection::normalize(title).is_empty() && game_detection::normalize(owner).is_empty() {
        return None;
    }
    Some((title, owner))
}

/// Núcleo da escolha de DETECTOR — porte do `if (foregroundWindow && ...) {...} else {...}` de
/// `steam-game-watcher.ts::tick()`. **Precedência é EXCLUSIVA, não um OR:** quando a janela ativa
/// tem QUALQUER identidade, só `find_foreground_game` decide — se ela não achar jogo nenhum, o tick
/// termina sem jogo ativo, NUNCA cai para o match por processo no mesmo tick (mesmo um jogo rodando
/// em segundo plano). O fallback por processo só roda quando a janela em foco está ausente.
///
/// Devolve `(jogo achado, fonte, chave de processo do jogo achado)` — a chave espelha
/// `activeGame.processKey` do TS (sempre presente junto do jogo, nas duas fontes) e alimenta
/// `last_game_process`, que [`decide_transition`] usa na graça da PRÓXIMA passada.
///
/// `process_keys` é uma closure, não um `&[String]` pronto: mesma preguiça do TS
/// (`listProcessNames` só roda dentro do `else`) — evita a varredura de processos
/// (`sysinfo::System::new_all`) num tick em que a janela em foco já decidiu.
///
/// `custom` (os "Jogos personalizados" da pessoa) é consultado ANTES de Steam/[`known_games`] nas
/// duas fontes, sempre por processo exato — ver [`game_detection::find_custom_game`].
pub(super) fn detect_active_game(
    window: Option<&ActiveWindowSnapshot>,
    games: &[steam_library::SteamGame],
    custom: &[CustomGame],
    process_keys: impl FnOnce() -> Vec<String>,
) -> (Option<ActiveGameInfo>, Source, Option<String>) {
    match foreground_identity(window) {
        Some((title, owner)) => {
            let process_key = game_detection::normalize(owner);
            let found = game_detection::find_custom_game([process_key.as_str()], custom)
                .map(|(info, _)| info)
                .or_else(|| game_detection::find_foreground_game(title, owner, games));
            let key = found.is_some().then_some(process_key);
            (found, Source::ForegroundWindow, key)
        }
        None => {
            let keys = process_keys();
            let found = game_detection::find_custom_game(keys.iter().map(String::as_str), custom)
                .or_else(|| game_detection::find_active_game(&keys, games));
            match found {
                Some((info, key)) => (Some(info), Source::ProcessWatcher, Some(key)),
                None => (None, Source::ProcessWatcher, None),
            }
        }
    }
}

/// Núcleo puro da decisão — porte de `tick()` + `handleNoGame()`. A graça
/// (`source === 'foreground-window' && lastGameProcess && isProcessRunning(lastGameProcess)`)
/// mantém a sessão aberta quando a janela em foco momentaneamente não bate com nenhum jogo mas o
/// processo do último jogo detectado continua rodando (ex.: alt-tab rápido para o launcher/chat).
/// `is_last_process_running` é injetada — mesma preguiça do TS (`isProcessRunning` só é `await`ado
/// quando as duas condições anteriores já são verdadeiras) e o que torna esta função testável sem
/// tocar `sysinfo`/o SO real.
pub(super) fn decide_transition(
    active: Option<&ActiveGameInfo>,
    active_process_key: Option<&str>,
    source: Source,
    last_game_name: Option<&str>,
    last_game_process: Option<&str>,
    is_last_process_running: impl FnOnce() -> bool,
) -> Transition {
    match (active, last_game_name) {
        (Some(game), Some(last)) if game.name == last => Transition::None,
        (Some(game), _) => Transition::Started {
            game_id: game.app_id.clone().unwrap_or_else(|| game.name.clone()),
            game_name: game.name.clone(),
            game_process: active_process_key.unwrap_or_default().to_string(),
        },
        (None, Some(_)) => {
            let has_grace = source == Source::ForegroundWindow
                && last_game_process.is_some_and(|p| !p.is_empty())
                && is_last_process_running();
            if has_grace { Transition::None } else { Transition::Stopped }
        }
        (None, None) => Transition::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(name: &str, app_id: Option<&str>) -> ActiveGameInfo {
        ActiveGameInfo { name: name.to_string(), app_id: app_id.map(str::to_string) }
    }

    fn window(title: Option<&str>, owner: Option<&str>) -> ActiveWindowSnapshot {
        ActiveWindowSnapshot { title: title.map(String::from), owner: owner.map(String::from) }
    }

    fn steam_game(app_id: &str, name: &str, install_dir: Option<&str>) -> steam_library::SteamGame {
        steam_library::SteamGame { app_id: app_id.into(), name: name.into(), install_dir: install_dir.map(String::from) }
    }

    // -- foreground_identity -------------------------------------------------------------------------

    #[test]
    fn foreground_identity_is_none_without_a_window() {
        assert!(foreground_identity(None).is_none());
    }

    #[test]
    fn foreground_identity_is_none_when_title_and_owner_normalize_to_empty() {
        let w = window(Some("---"), Some(""));
        assert!(foreground_identity(Some(&w)).is_none());
    }

    #[test]
    fn foreground_identity_is_some_when_only_the_owner_has_identity() {
        let w = window(None, Some("code.exe"));
        assert_eq!(foreground_identity(Some(&w)), Some(("", "code.exe")));
    }

    // -- detect_active_game ----------------------------------------------------------------------------

    #[test]
    fn detect_active_game_prefers_the_foreground_window_and_never_calls_the_process_closure() {
        let w = window(Some("Hades - Steam"), Some("hades.exe"));
        let games = vec![steam_game("1145360", "Hades", None)];
        let (active, source, process_key) =
            detect_active_game(Some(&w), &games, &[], || panic!("não deveria varrer processos com janela em foco"));
        assert_eq!(active.unwrap().name, "Hades");
        assert_eq!(source, Source::ForegroundWindow);
        assert_eq!(process_key.as_deref(), Some("hades"));
    }

    /// A precedência é EXCLUSIVA (ver o cabeçalho): a janela em foco não achando jogo NÃO cai para
    /// o fallback por processo, mesmo que ele achasse um jogo real.
    #[test]
    fn detect_active_game_does_not_fall_back_to_process_matching_when_the_foreground_window_finds_nothing() {
        let w = window(Some("random.txt - Notepad"), Some("notepad.exe"));
        let games: Vec<steam_library::SteamGame> = vec![];
        let (active, source, process_key) = detect_active_game(Some(&w), &games, &[], || vec!["valorant".to_string()]);
        assert!(active.is_none(), "janela em foco sem match não deveria cair para o processo");
        assert_eq!(source, Source::ForegroundWindow);
        assert_eq!(process_key, None);
    }

    #[test]
    fn detect_active_game_falls_back_to_process_matching_without_a_foreground_window() {
        let games: Vec<steam_library::SteamGame> = vec![];
        let (active, source, process_key) = detect_active_game(None, &games, &[], || vec!["valorant".to_string()]);
        assert_eq!(active.unwrap().name, "Valorant");
        assert_eq!(source, Source::ProcessWatcher);
        assert_eq!(process_key.as_deref(), Some("valorant"));
    }

    /// "Jogos personalizados": o processo cadastrado vence Steam/known_games nas duas fontes.
    #[test]
    fn detect_active_game_prefers_a_custom_game_in_both_sources() {
        let custom = vec![CustomGame { process_key: "valorant".into(), name: "Meu Valorant".into() }];
        let w = window(Some("VALORANT"), Some("VALORANT.exe"));
        assert_eq!(detect_active_game(Some(&w), &[], &custom, Vec::new).0.unwrap().name, "Meu Valorant");
        let (active, _, key) = detect_active_game(None, &[], &custom, || vec!["valorant".to_string()]);
        assert_eq!((active.unwrap().name, key.as_deref()), ("Meu Valorant".to_string(), Some("valorant")));
    }

    // -- decide_transition --------------------------------------------------------------------------

    #[test]
    fn decide_transition_starts_when_a_new_game_is_detected() {
        assert_eq!(
            decide_transition(Some(&game("Hades", Some("1145360"))), Some("hades"), Source::ForegroundWindow, None, None, || false),
            Transition::Started { game_id: "1145360".into(), game_name: "Hades".into(), game_process: "hades".into() }
        );
    }

    #[test]
    fn decide_transition_falls_back_to_the_name_as_id_when_there_is_no_app_id() {
        assert_eq!(
            decide_transition(Some(&game("Valorant", None)), Some("valorant"), Source::ProcessWatcher, None, None, || false),
            Transition::Started { game_id: "Valorant".into(), game_name: "Valorant".into(), game_process: "valorant".into() }
        );
    }

    #[test]
    fn decide_transition_is_a_no_op_while_the_same_game_keeps_running() {
        let t = decide_transition(
            Some(&game("Hades", Some("1"))),
            Some("hades"),
            Source::ForegroundWindow,
            Some("Hades"),
            Some("hades"),
            || panic!("jogo igual ao anterior não deveria checar processo nenhum"),
        );
        assert_eq!(t, Transition::None);
    }

    #[test]
    fn decide_transition_stops_when_the_game_disappears_from_the_process_watcher_source() {
        // Fonte é process-watcher: a graça de handleNoGame só existe para foreground-window.
        let t = decide_transition(None, None, Source::ProcessWatcher, Some("Hades"), Some("hades"), || true);
        assert_eq!(t, Transition::Stopped);
    }

    /// A GRAÇA — porte de `handleNoGame`: a janela em foco não achou jogo, mas o processo do
    /// último jogo detectado continua rodando, então a sessão fica aberta (sem stop, sem start).
    #[test]
    fn decide_transition_keeps_the_session_when_the_foreground_window_misses_but_the_last_game_process_still_runs() {
        let t = decide_transition(None, None, Source::ForegroundWindow, Some("Hades"), Some("hades"), || true);
        assert_eq!(t, Transition::None);
    }

    /// NEGATIVO — mesma fonte e mesmo processo lembrado, mas ele NÃO está mais rodando: a graça
    /// não se aplica, a sessão encerra normalmente.
    #[test]
    fn decide_transition_stops_when_the_foreground_window_misses_and_the_last_game_process_is_gone() {
        let t = decide_transition(None, None, Source::ForegroundWindow, Some("Hades"), Some("hades"), || false);
        assert_eq!(t, Transition::Stopped);
    }

    /// NEGATIVO — sem `last_game_process` (sessão antiga, de antes desta mudança, ou processo
    /// nunca guardado) a graça não tem o que checar: mesmo comportamento de `lastGameProcess`
    /// falsy no TS.
    #[test]
    fn decide_transition_stops_without_a_remembered_process_even_from_the_foreground_window_source() {
        let t = decide_transition(
            None,
            None,
            Source::ForegroundWindow,
            Some("Hades"),
            None,
            || panic!("sem processo lembrado não deveria nem checar se está rodando"),
        );
        assert_eq!(t, Transition::Stopped);
    }

    #[test]
    fn decide_transition_is_a_no_op_when_nothing_was_running_and_still_is_not() {
        assert_eq!(decide_transition(None, None, Source::ProcessWatcher, None, None, || false), Transition::None);
    }
}
