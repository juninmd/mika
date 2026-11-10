//! Lógica pura da janela ativa — porte de `infrastructure/os/foreground-context.ts`
//! (`isLikelyGameApp`) + `foreground-native.ts::fromNativeWindow` (normalização) +
//! `active-window-watcher.ts::identity`/`tick` (decisão de emitir). Separado de `active_window.rs`
//! (que faz a sonda de SO e o laço) pelo limite de 300 linhas do AGENTS.md — e porque isto é o que
//! os testes de unidade exercitam, sem tocar SO nem o estado compartilhado.

use std::sync::OnceLock;

use super::redact::redact_pii;

/// Sonda crua do SO, sem redação/normalização — porte de `NativeForegroundWindow`.
pub(super) struct RawForeground {
    pub title: String,
    pub process_name: String,
    pub pid: u32,
}

/// Já redigido, trimado, `.exe` removido — o que o evento publica e o que `last_known` guarda.
pub(super) struct Probe {
    pub title: Option<String>,
    pub owner: Option<String>,
    pub pid: u32,
    pub is_likely_game: bool,
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Porte de `processName.replace(/\.exe$/i, '')` (`fromNativeWindow`).
pub(super) fn strip_exe_suffix(name: &str) -> &str {
    // `get`, não índice: o nome vem do SO e os 4 bytes finais podem cair no meio de um char.
    match name.len().checked_sub(4).and_then(|cut| Some((cut, name.get(cut..)?))) {
        Some((cut, tail)) if tail.eq_ignore_ascii_case(".exe") => &name[..cut],
        _ => name,
    }
}

fn game_hint() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\b(game|steam|unity|unreal|directx|vulkan|launcher|elden ring|valorant|cs2|dota|league|minecraft|fortnite)\b").unwrap()
    })
}

fn non_game_hint() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\b(code|visual studio|cursor|chrome|edge|firefox|figma|notion|slack|discord)\b").unwrap()
    })
}

/// Porte de `isLikelyGameApp` (`foreground-context.ts`).
pub(super) fn is_likely_game_app(title: &str, process_name: &str) -> bool {
    let source = format!("{title} {process_name}");
    let source = source.trim();
    if source.is_empty() {
        return false;
    }
    if non_game_hint().is_match(source) {
        return false;
    }
    game_hint().is_match(source)
}

pub(super) fn to_probe(raw: &RawForeground) -> Probe {
    let is_likely_game = is_likely_game_app(&raw.title, &raw.process_name);
    let title = non_empty(&redact_pii(raw.title.trim()));
    let owner = non_empty(&redact_pii(strip_exe_suffix(raw.process_name.trim())));
    Probe { title, owner, pid: raw.pid, is_likely_game }
}

/// Porte de `identity(title, owner)` — `dono|título`.
pub(super) fn identity(owner: Option<&str>, title: Option<&str>) -> String {
    format!("{}|{}", owner.unwrap_or(""), title.unwrap_or(""))
}

pub(super) enum Decision {
    /// Nem dono nem título — sonda inútil, identidade não muda (`isFirstProbe` também não).
    Skip,
    /// Primeira sonda válida: só grava a identidade, nunca emite (mesma regra do TS).
    Remember(String),
    /// Identidade igual à anterior — nada a fazer.
    Unchanged,
    /// Identidade mudou depois da primeira sonda — emite.
    Emit(String),
}

/// Porte de `tick()`: decide entre pular, só lembrar (primeira sonda), ignorar (sem mudança) ou
/// emitir — nunca chama o barramento sozinho, quem decide publicar é o laço de `active_window.rs`.
pub(super) fn decide(last_identity: Option<&str>, probe: &Probe) -> Decision {
    if probe.owner.is_none() && probe.title.is_none() {
        return Decision::Skip;
    }
    let current = identity(probe.owner.as_deref(), probe.title.as_deref());
    if Some(current.as_str()) == last_identity {
        return Decision::Unchanged;
    }
    if last_identity.is_none() {
        return Decision::Remember(current);
    }
    Decision::Emit(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(owner: Option<&str>, title: Option<&str>) -> Probe {
        Probe { title: title.map(String::from), owner: owner.map(String::from), pid: 1, is_likely_game: false }
    }

    #[test]
    fn strips_the_exe_suffix_case_insensitively() {
        assert_eq!(strip_exe_suffix("Code.EXE"), "Code");
        assert_eq!(strip_exe_suffix("chrome.exe"), "chrome");
        assert_eq!(strip_exe_suffix("notepad"), "notepad");
    }

    /// O nome do processo vem do SO e pode ter multibyte: cortar 4 bytes do fim caía no meio de um char.
    #[test]
    fn strip_exe_suffix_never_cuts_inside_a_multibyte_char() {
        assert_eq!(strip_exe_suffix("ããa"), "ããa");
        assert_eq!(strip_exe_suffix("jogo-é"), "jogo-é");
        assert_eq!(strip_exe_suffix("ação.exe"), "ação");
    }

    #[test]
    fn is_likely_game_app_matches_the_game_hint_when_no_non_game_hint_is_present() {
        assert!(is_likely_game_app("Elden Ring", "eldenring.exe"));
        assert!(!is_likely_game_app("index.ts - Visual Studio Code", "code.exe"), "IDE não é jogo");
        assert!(!is_likely_game_app("", ""), "fonte vazia nunca é jogo");
    }

    #[test]
    fn the_first_probe_only_remembers_and_never_emits() {
        assert!(matches!(decide(None, &probe(Some("code.exe"), Some("index.ts"))), Decision::Remember(_)));
    }

    #[test]
    fn a_changed_identity_after_the_first_probe_emits() {
        let last = identity(Some("code.exe"), Some("a.ts"));
        match decide(Some(&last), &probe(Some("chrome.exe"), Some("b.com"))) {
            Decision::Emit(id) => assert_eq!(id, "chrome.exe|b.com"),
            _ => panic!("deveria emitir"),
        }
    }

    #[test]
    fn an_unchanged_identity_does_not_emit() {
        let last = identity(Some("code.exe"), Some("a.ts"));
        assert!(matches!(decide(Some(&last), &probe(Some("code.exe"), Some("a.ts"))), Decision::Unchanged));
    }

    #[test]
    fn an_owner_only_probe_is_a_valid_identity_change() {
        let last = identity(Some("code.exe"), None);
        match decide(Some(&last), &probe(Some("chrome.exe"), None)) {
            Decision::Emit(id) => assert_eq!(id, "chrome.exe|"),
            _ => panic!("deveria emitir"),
        }
    }

    #[test]
    fn a_probe_with_neither_owner_nor_title_is_skipped() {
        assert!(matches!(decide(None, &probe(None, None)), Decision::Skip));
        let last = identity(Some("code.exe"), Some("a.ts"));
        assert!(matches!(decide(Some(&last), &probe(None, None)), Decision::Skip));
    }

    #[test]
    fn to_probe_redacts_pii_and_strips_the_exe_suffix() {
        let raw = RawForeground {
            title: "Caixa — ada@empresa.com — Outlook".to_string(),
            process_name: "OUTLOOK.EXE".to_string(),
            pid: 42,
        };
        let probe = to_probe(&raw);
        assert_eq!(probe.title.as_deref(), Some("Caixa — [EMAIL] — Outlook"));
        assert_eq!(probe.owner.as_deref(), Some("OUTLOOK"));
    }
}
