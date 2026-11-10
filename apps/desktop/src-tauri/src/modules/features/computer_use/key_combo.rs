//! Parses a key-combo string (`"ctrl+c"`, `"enter"`, `"ctrl+shift+t"`) into the [`enigo`] keys to
//! press. Pure — no `enigo::Enigo` instance touched here, so this is testable without a display
//! server/session, and it is the piece [`super::tool`] validates BEFORE asking for approval.
//!
//! Only key names that `enigo::Key` exposes on every platform are accepted (see the crate's own
//! `#[cfg(target_os = "windows")]` gates on `Key::A`..`Key::Z`/`Key::Num0`..`Key::Num9` — those
//! variants don't exist on Linux/macOS builds at all). A single printable character falls back to
//! `Key::Unicode`, which every platform backend supports; that covers `"ctrl+c"`,
//! `"ctrl+shift+t"` and similar shortcuts without a platform-gated variant per letter/digit.

use enigo::Key;

#[derive(Debug, Clone, PartialEq)]
pub struct KeyCombo {
    pub modifiers: Vec<Key>,
    pub main: Key,
}

fn modifier_key(name: &str) -> Option<Key> {
    match name {
        "ctrl" | "control" => Some(Key::Control),
        "alt" | "option" => Some(Key::Alt),
        "shift" => Some(Key::Shift),
        "cmd" | "meta" | "super" | "win" | "windows" | "command" => Some(Key::Meta),
        _ => None,
    }
}

fn named_key(name: &str) -> Option<Key> {
    match name {
        "enter" | "return" => Some(Key::Return),
        "esc" | "escape" => Some(Key::Escape),
        "tab" => Some(Key::Tab),
        "space" | "spacebar" => Some(Key::Space),
        "backspace" => Some(Key::Backspace),
        "delete" | "del" => Some(Key::Delete),
        "up" | "uparrow" => Some(Key::UpArrow),
        "down" | "downarrow" => Some(Key::DownArrow),
        "left" | "leftarrow" => Some(Key::LeftArrow),
        "right" | "rightarrow" => Some(Key::RightArrow),
        "home" => Some(Key::Home),
        "end" => Some(Key::End),
        "pageup" => Some(Key::PageUp),
        "pagedown" => Some(Key::PageDown),
        _ => None,
    }
}

/// The trailing token of a combo: a named key, a modifier used standalone (`"ctrl"` alone), or a
/// single character (`Key::Unicode`) — never a whole word, which would silently type the first
/// letter of a typo instead of failing it.
fn main_key(name: &str) -> Option<Key> {
    named_key(name).or_else(|| modifier_key(name)).or_else(|| {
        let mut chars = name.chars();
        let only = chars.next()?;
        if chars.next().is_none() {
            Some(Key::Unicode(only))
        } else {
            None
        }
    })
}

/// Com a tecla Meta (Windows/Super/Cmd) só passa o que não abre nada que aceite texto: alternar
/// janelas/apps (`tab`), mover/encaixar janela (setas) e mostrar a área de trabalho (`d`) — e, onde
/// Meta é o Cmd, copiar/colar/desfazer/selecionar tudo. Allowlist e não denylist porque a lista de
/// "Meta+letra que abre algo" não fecha: `win+s` (busca), `win+e` (Explorer, barra de endereço),
/// `win+v`, `win+a`, `win+1..9` (lança app da barra), `cmd+shift+g` (Ir para a pasta), `cmd+backspace`
/// (lixo)... — e com `type` depois de qualquer um deles o modelo executa comando arbitrário.
fn meta_allows(main: Key) -> bool {
    match main {
        Key::Tab | Key::UpArrow | Key::DownArrow | Key::LeftArrow | Key::RightArrow => true,
        Key::Unicode(ch) => {
            let ch = ch.to_ascii_lowercase();
            ch == 'd' || (cfg!(target_os = "macos") && matches!(ch, 'c' | 'v' | 'z' | 'a'))
        }
        _ => false,
    }
}

/// Atalho que abre lançador/Executar/menu do sistema, trava a sessão ou chama o gerenciador de
/// tarefas (ver [`meta_allows`] para tudo o que envolve Meta). `alt+f4` não está aqui porque `F4`
/// nem parseia (só teclas nomeadas e caractere único) — se alguém acrescentar teclas de função,
/// entra aqui.
fn is_blocked_combo(combo: &KeyCombo) -> bool {
    let has = |key: Key| combo.modifiers.contains(&key);
    let (ctrl, alt, shift, meta) = (has(Key::Control), has(Key::Alt), has(Key::Shift), has(Key::Meta));
    let letter = match combo.main {
        Key::Unicode(ch) => Some(ch.to_ascii_lowercase()),
        _ => None,
    };
    let meta_blocked = combo.main == Key::Meta || (meta && !meta_allows(combo.main));
    let session = ctrl && alt && (matches!(combo.main, Key::Delete | Key::Backspace) || matches!(letter, Some('t' | 'l')));
    let task_manager_or_start = ctrl && combo.main == Key::Escape && (shift || !alt);
    meta_blocked || session || task_manager_or_start
}

/// `"ctrl+shift+t"` -> modifiers `[Control, Shift]`, main `Unicode('t')`. Empty segments (`"ctrl+"`,
/// `"+c"`, `""`) are refused rather than silently dropped — a combo that always sent SOMETHING,
/// even from bad input, is how a click meant for one key ends up pressing another.
pub fn parse_key_combo(combo: &str) -> Result<KeyCombo, String> {
    let trimmed = combo.trim();
    if trimmed.is_empty() {
        return Err("empty key combo".into());
    }

    let parts: Vec<&str> = trimmed.split('+').map(str::trim).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("malformed key combo '{combo}'"));
    }

    let (last, heads) = parts.split_last().expect("parts is non-empty (checked above)");
    let mut modifiers = Vec::with_capacity(heads.len());
    for head in heads {
        let lower = head.to_ascii_lowercase();
        modifiers.push(modifier_key(&lower).ok_or_else(|| format!("unknown modifier '{head}' in combo '{combo}'"))?);
    }

    let lower_last = last.to_ascii_lowercase();
    let main = main_key(&lower_last).ok_or_else(|| format!("unknown key '{last}' in combo '{combo}'"))?;

    let parsed = KeyCombo { modifiers, main };
    if is_blocked_combo(&parsed) {
        return Err(format!("key combo '{combo}' is blocked: it opens a system launcher, the Start/Activities menu or session controls"));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_single_named_key() {
        let combo = parse_key_combo("enter").unwrap();
        assert!(combo.modifiers.is_empty());
        assert_eq!(combo.main, Key::Return);
    }

    #[test]
    fn parses_a_modifier_plus_character() {
        let combo = parse_key_combo("ctrl+c").unwrap();
        assert_eq!(combo.modifiers, vec![Key::Control]);
        assert_eq!(combo.main, Key::Unicode('c'));
    }

    #[test]
    fn parses_multiple_modifiers_in_order() {
        let combo = parse_key_combo("ctrl+shift+t").unwrap();
        assert_eq!(combo.modifiers, vec![Key::Control, Key::Shift]);
        assert_eq!(combo.main, Key::Unicode('t'));
    }

    #[test]
    fn is_case_insensitive() {
        let combo = parse_key_combo("Ctrl+ALT+Left").unwrap();
        assert_eq!(combo.modifiers, vec![Key::Control, Key::Alt]);
        assert_eq!(combo.main, Key::LeftArrow);
    }

    #[test]
    fn accepts_common_modifier_aliases() {
        assert_eq!(parse_key_combo("cmd+tab").unwrap().modifiers, vec![Key::Meta]);
        assert_eq!(parse_key_combo("win+d").unwrap().modifiers, vec![Key::Meta]);
        assert_eq!(parse_key_combo("option+tab").unwrap().modifiers, vec![Key::Alt]);
    }

    /// Combo que abre um lançador/sessão do SO deixa de ser "tecla": com ela o modelo abre o Executar
    /// e DIGITA um comando (a ação `type` existe). Negado já no parse — antes de pedir aprovação.
    #[test]
    fn blocks_combos_that_open_system_launchers_or_session_controls() {
        for combo in [
            "win+r", "Win+R", "windows+x", "meta+l", "super+i", "cmd+q", "cmd+space", "win+space", "ctrl+cmd+q",
            "ctrl+alt+delete", "ctrl+alt+del", "CTRL+ALT+DELETE", "ctrl+alt+backspace", "ctrl+shift+esc", "ctrl+esc",
            "ctrl+alt+t", "ctrl+alt+l", "cmd+alt+esc", "win", "super", "cmd", "ctrl+win",
            // Win+S/Win+E abrem busca/Explorer (o `type` seguinte digita `cmd`); Cmd+Shift+G é "Ir para a
            // pasta" do Finder; Win+1..9 lança o app da barra; Cmd+Backspace manda o arquivo ao lixo.
            "win+s", "win+e", "win+shift+s", "win+v", "win+a", "win+1", "cmd+shift+g", "cmd+backspace", "meta+enter", "win+ctrl+enter",
        ] {
            let error = parse_key_combo(combo).unwrap_err();
            assert!(error.contains("blocked"), "{combo}: {error}");
        }
    }

    #[test]
    fn keeps_ordinary_shortcuts_working_including_the_ones_next_to_blocked_ones() {
        for combo in [
            "ctrl+c", "ctrl+v", "ctrl+shift+t", "alt+tab", "win+d", "cmd+tab", "win+left", "cmd+up", "ctrl+alt+left",
            "ctrl+alt+d", "alt+left", "esc", "ctrl+z",
        ] {
            assert!(parse_key_combo(combo).is_ok(), "{combo} não deveria ser bloqueado");
        }
    }

    /// No macOS a tecla Meta é o Cmd: copiar/colar/desfazer/selecionar tudo passam. Nos outros SOs ela é a
    /// tecla Windows/Super, e `win+c` abre o assistente — a mesma combinação tem de ser negada.
    #[test]
    fn meta_plus_an_editing_letter_is_allowed_only_where_meta_is_cmd() {
        for combo in ["cmd+c", "cmd+v", "cmd+z", "cmd+a"] {
            assert_eq!(parse_key_combo(combo).is_ok(), cfg!(target_os = "macos"), "{combo}");
        }
    }

    #[test]
    fn rejects_an_empty_combo() {
        assert!(parse_key_combo("").is_err());
        assert!(parse_key_combo("   ").is_err());
    }

    /// Empty segments (a stray `+`) are a malformed request, never "ignore the gap and press
    /// whatever is left" — that would be a different key than the one asked for.
    #[test]
    fn rejects_malformed_combos_with_empty_segments() {
        assert!(parse_key_combo("ctrl+").is_err());
        assert!(parse_key_combo("+c").is_err());
        assert!(parse_key_combo("ctrl++c").is_err());
    }

    #[test]
    fn rejects_an_unknown_modifier() {
        let error = parse_key_combo("hyper+c").unwrap_err();
        assert!(error.contains("hyper"), "got: {error}");
    }

    /// A whole unrecognized word never silently degrades to its first letter.
    #[test]
    fn rejects_an_unknown_multi_character_key() {
        let error = parse_key_combo("ctrl+banana").unwrap_err();
        assert!(error.contains("banana"), "got: {error}");
    }

    #[test]
    fn a_modifier_name_alone_is_a_valid_main_key() {
        // Rare, but "press just Alt" is a legitimate request (e.g. to open a menu bar).
        assert_eq!(parse_key_combo("alt").unwrap().main, Key::Alt);
    }
}
