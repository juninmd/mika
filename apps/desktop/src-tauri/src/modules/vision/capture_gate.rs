//! Bloqueio de captura — port de `isCaptureBlocked`
//! (`apps/desktop/src/main/infrastructure/adapters/screen-capture-consent.ts`), as duas metades:
//! `isAnyVisibleWindowBlocked`, que casa o título de CADA janela visível (não só a em foco) contra
//! a blocklist de privacidade (`vision.captureBlocklist`, `ConfigKey.VisionCaptureBlocklist`), e,
//! desde 2026-09-22, [`is_foreground_blocked`] — a metade que faltava (`isForegroundCaptureBlocked`).
//! `parseBlocklist`/`isBlockedWindowTitle` (`infrastructure/os/capture-blocklist.ts`) são as duas
//! funções que a primeira checagem usa, portadas aqui 1:1.
//!
//! **`is_foreground_blocked` não é 1:1 com `isForegroundCaptureBlocked`.** O TS chama
//! `getForegroundAppContext()` — um probe multiplataforma POR CHAMADA (addon no Windows, `xdotool`
//! no Linux, AppleScript no macOS) — e trata falha de sonda (`detectionFailed`) como BLOQUEADO
//! (fail-closed). Este port lê `system_signals::active_window::last_known()`: um CACHE pull-based
//! alimentado pelo watcher de janela ativa (`active_window.rs`: Windows, Linux/X11, macOS). Sem
//! snapshot (nenhuma sonda ainda, ou Wayland, que não tem sonda) devolve NÃO bloqueado, não
//! bloqueado-por-padrão: tratar "sem sonda" como bloqueio permanente derrubaria a captura de tela
//! para sempre nessas sessões, uma regressão maior que a lacuna que este gate fecha. `is_any_visible_window_blocked` (a defesa por janela VISÍVEL,
//! `screen_capture.rs`) continua cobrindo essas plataformas sem mudança nenhuma.
//!
//! Em uso pelo CHOKEPOINT real de captura (`super::screen_capture::capture_screen_frames`), que
//! `capture_tool.rs` (todas as actions de `vision_capture`/`vision_read`) e `watcher.rs` (o
//! observador periódico) já chamam — nenhum dos dois reimplementa o gate, ver o cabeçalho de
//! `screen_capture.rs`.
//!
//! **WP9 item 3 (2026-09-26):** [`is_foreground_blocked`] também bloqueia incondicionalmente quando
//! a janela em foco é a PRÓPRIA Configurações/Onboarding da Mika (`is_own_window`,
//! `crate::windows::is_own_window_title`) — nunca sujeito à blocklist do usuário. Sem isto, pedir
//! "o que tem na minha tela?" com as Configurações abertas mandaria a própria UI (chaves de API
//! incluídas) para o provedor de visão configurado.

use regex::Regex;

/// Espelha `DEFAULT_CAPTURE_BLOCKLIST` (`infrastructure/os/capture-blocklist.ts`).
///
/// Ampliada em 2026-10-06 (revisão de privacidade): além de senha e banco, e-mail, mensageiros,
/// navegação privada, carteiras cripto, saúde e autenticadores — o que aparece na tela e não é do
/// usuário só (conversas de terceiros, códigos). Termos que também são palavra de jogo ("exodus",
/// "keeper", "messenger", "bank", "ledger", "slack", "inter") só entram em forma específica ou como
/// `proc:<nome>` (casa só o NOME DO PROCESSO da janela em foco, nunca o título): "Metro Exodus" e
/// "Dungeon Keeper" não bloqueiam, a carteira Exodus e o app Signal/Slack bloqueiam. É o DEFAULT: quem editou a lista a mantém; quem
/// nunca editou ganha esta pela migração do config (`config/store.rs`).
pub(super) const DEFAULT_CAPTURE_BLOCKLIST: &str = "bitwarden, 1password, keepass, lastpass, dashlane, proton pass, keeper password, keepersecurity, nordpass, authy, authenticator, 2fa, banco, online banking, mobile banking, internet banking, bank of america, wells fargo, itau, nubank, bradesco, santander, banco inter, caixa, gmail, outlook, thunderbird, proton mail, yahoo mail, whatsapp, telegram, proc:signal, messenger.com, facebook messenger, discord, skype, proc:slack, slack.com, incognito, inprivate, private browsing, navegação privada, janela anônima, aba anônima, metamask, ledger live, ledger wallet, trezor, exodus wallet, proc:exodus, trust wallet, coinbase, binance, mychart, doctolib, teladoc, portal do paciente, prontuario";

/// `parseBlocklist`. `None` (chave ausente) cai no default; `Some("")` (valor configurado vazio)
/// NÃO cai no default — o `??` do TS só substitui `null`/`undefined`, nunca string vazia — produz
/// lista vazia (nenhuma entrada sobrevive ao filtro de tamanho mínimo).
pub(super) fn parse_blocklist(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or(DEFAULT_CAPTURE_BLOCKLIST)
        .split(',')
        .map(|entry| entry.trim().to_lowercase())
        .filter(|entry| entry.chars().count() >= 3)
        .collect()
}

/// Match por fronteira de palavra, não substring solta — mesma razão do comentário original em
/// `capture-blocklist.ts`: "banco"/"caixa"/"inter" são substrings de palavras comuns em
/// pt-BR/tech ("banco de dados", "caixa de texto", "Internet", "Interface"); sem fronteira de
/// palavra isso bloquearia captura em qualquer janela com essas palavras soltas no título.
///
/// `\b` do crate `regex` é Unicode-aware por padrão; o `\b` do V8 (TS) é ASCII-only. Os padrões da
/// blocklist são todos ASCII, então a diferença de engine não muda nenhum resultado real.
/// Prefixo de entrada que restringe o casamento ao nome do processo (dono da janela).
const PROCESS_PREFIX: &str = "proc:";

///
/// A fronteira só vale na ponta que é caractere de palavra: `\b` colado a `.`/`+`/`:` exige uma letra do
/// OUTRO lado, então uma entrada do usuário como `.env` ou `c++` nunca casava com nada (o gate de
/// privacidade ignorava a entrada em silêncio).
fn matches_pattern(haystack: &str, pattern: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let edge = |c: Option<char>| if c.is_some_and(is_word) { r"\b" } else { "" };
    let (start, end) = (edge(pattern.chars().next()), edge(pattern.chars().next_back()));
    let escaped = regex::escape(pattern);
    Regex::new(&format!("{start}{escaped}{end}")).map(|re| re.is_match(haystack)).unwrap_or(false)
}

/// `isBlockedWindowTitle` — checa UM título; o chamador (quando a fase 6 existir) varre TODAS as
/// janelas visíveis via `capture::get_sources({ types: ["window"] })`, já portado e capaz de listar
/// títulos sem precisar de thumbnail nenhum (`want_thumbnail = false`).
pub(super) fn is_blocked_window_title(title: Option<&str>, patterns: &[String]) -> Option<String> {
    if patterns.is_empty() {
        return None;
    }
    let haystack = title.unwrap_or_default().to_lowercase();
    if haystack.trim().is_empty() {
        return None;
    }
    patterns.iter().filter(|pattern| !pattern.starts_with(PROCESS_PREFIX)).find(|pattern| matches_pattern(&haystack, pattern)).cloned()
}

/// WP9 item 3: a Mika nunca analisa a PRÓPRIA janela de Configurações/Onboarding — checagem
/// incondicional, nunca sujeita à blocklist configurável pelo usuário (essa é para apps de
/// TERCEIROS: banco, gerenciador de senha). Ver `crate::windows::is_own_window_title` para por que
/// é o título, não o nome de processo, que identifica a janela própria.
fn is_own_window(title: Option<&str>) -> bool {
    title.is_some_and(crate::windows::is_own_window_title)
}

/// Porte PURO de `isBlockedForeground` (`infrastructure/os/capture-blocklist.ts`), adaptado à
/// fonte pull-based deste backend: `title`/`owner` já vêm resolvidos (o chamador extrai do
/// snapshot), nunca lidos daqui. Haystack é `título + dono`, mesma composição do TS
/// (`${title ?? ''} ${processName ?? ''}`).
fn is_foreground_context_blocked(title: Option<&str>, owner: Option<&str>, patterns: &[String]) -> Option<String> {
    if patterns.is_empty() {
        return None;
    }
    let owner = owner.unwrap_or("").to_lowercase();
    let haystack = format!("{} {owner}", title.unwrap_or("")).to_lowercase();
    if haystack.trim().is_empty() {
        return None;
    }
    patterns
        .iter()
        .find(|pattern| match pattern.strip_prefix(PROCESS_PREFIX) {
            Some(process) => matches_pattern(&owner, process),
            None => matches_pattern(&haystack, pattern),
        })
        .cloned()
}

/// Fachada real — `pub(crate)` porque `super::screen_capture` (o chokepoint) chama daqui de fora
/// deste diretório de teste. Lê `vision.captureBlocklist` e o último snapshot de janela ativa
/// (`system_signals::active_window::last_known()`); ver o cabeçalho do arquivo para a diferença de
/// contrato com `isForegroundCaptureBlocked` (fail-open sem snapshot, não fail-closed).
pub(crate) fn is_foreground_blocked(store: &crate::config::store::ConfigStore) -> bool {
    let Some(window) = crate::modules::system_signals::active_window::last_known() else {
        return false;
    };
    if is_own_window(window.title.as_deref()) {
        return true;
    }
    let raw = store.get("vision.captureBlocklist");
    let patterns = parse_blocklist(raw.as_ref().and_then(|v| v.as_str()));
    is_foreground_context_blocked(window.title.as_deref(), window.owner.as_deref(), &patterns).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_blocklist -------------------------------------------------------------------------
    //
    // `capture-blocklist.ts` foi removido (backend Node deletado na migração para Tauri); o pin de
    // conteúdo do `DEFAULT_CAPTURE_BLOCKLIST` que essa suíte tinha contra o fonte TS não tem mais
    // twin vivo para comparar — as entradas concretas ("bitwarden", "internet banking") continuam
    // cobertas abaixo, direto sobre a constante Rust, que agora é a única fonte de verdade.

    #[test]
    fn parse_blocklist_uses_the_default_when_absent() {
        assert_eq!(parse_blocklist(None), parse_blocklist(Some(DEFAULT_CAPTURE_BLOCKLIST)));
        assert!(parse_blocklist(None).contains(&"bitwarden".to_string()));
        assert!(parse_blocklist(None).contains(&"internet banking".to_string()));
    }

    /// NEGATIVO — mesmo `??` peculiar do TS: string configurada vazia é um VALOR, não ausência.
    #[test]
    fn parse_blocklist_of_empty_string_is_empty_not_the_default() {
        assert!(parse_blocklist(Some("")).is_empty());
    }

    #[test]
    fn parse_blocklist_drops_entries_shorter_than_three_chars() {
        // "ok" (2) e "ab" (2) caem fora; "aaa" (3) e "banco" (5) ficam.
        assert_eq!(parse_blocklist(Some("ok, ab, aaa, banco")), vec!["aaa".to_string(), "banco".to_string()]);
    }

    #[test]
    fn parse_blocklist_trims_and_lowercases_entries() {
        assert_eq!(parse_blocklist(Some("  NuBank ,BANCO ")), vec!["nubank".to_string(), "banco".to_string()]);
    }

    #[test]
    fn the_default_blocklist_covers_mail_messengers_private_browsing_wallets_health_and_authenticators() {
        let patterns = parse_blocklist(None);
        for title in [
            "Caixa de Entrada - Gmail",
            "Outlook",
            "WhatsApp",
            "Telegram Desktop",
            "Mozilla Firefox Private Browsing",
            "Nova guia - Google Chrome (Incognito)",
            "Microsoft Edge InPrivate",
            "MetaMask",
            "Ledger Live",
            "Google Authenticator",
            "Doctolib - Mes rendez-vous",
            "Portal do paciente",
        ] {
            assert!(is_blocked_window_title(Some(title), &patterns).is_some(), "deveria bloquear: {title}");
        }
        for title in ["Visual Studio Code", "Elden Ring", "Rust tutorial - YouTube", "Internet Explorer"] {
            assert!(is_blocked_window_title(Some(title), &patterns).is_none(), "não deveria bloquear: {title}");
        }
    }

    /// M4: palavras comuns de jogo não podem bloquear a captura ("Metro Exodus", "Dungeon Keeper", "The
    /// Messenger", "Ledger" de um RPG...), e o app de verdade tem de bloquear pelo NOME DO PROCESSO
    /// (`proc:`), que é o que o título de jogo/site não imita.
    #[test]
    fn game_titles_that_share_a_word_with_a_sensitive_app_are_not_blocked_but_the_app_is() {
        let patterns = parse_blocklist(None);
        let blocked = |title: &str, owner: &str| is_foreground_context_blocked(Some(title), Some(owner), &patterns).is_some();
        for (title, owner) in [
            ("Metro Exodus", "MetroExodus.exe"), ("Dungeon Keeper", "DUNGEON.EXE"), ("The Messenger", "TheMessenger.exe"),
            ("Bank Heist Simulator", "heist.exe"), ("Ledger Quest", "ledgerquest.exe"), ("Slack Off Simulator", "slackoff.exe"),
            ("Inter Milan Manager", "manager.exe"), ("Signal Simulator", "SignalSimulator.exe"),
        ] {
            assert!(!blocked(title, owner), "jogo bloqueado por engano: {title} / {owner}");
        }
        for (title, owner) in [
            ("Exodus", "Exodus.exe"), ("Signal", "signal.exe"), ("Signal", "Signal"), ("Signal", "signal-desktop"),
            ("general | Acme - Slack", "Slack"), ("Chat", "Bitwarden.exe"),
        ] {
            assert!(blocked(title, owner), "app sensível não bloqueado: {title} / {owner}");
        }
        for title in ["Facebook Messenger", "messenger.com | Chats", "Keeper Password Manager", "Online Banking - Meu Banco", "Exodus Wallet", "Ledger Live"] {
            assert!(is_blocked_window_title(Some(title), &patterns).is_some(), "site/app por título não bloqueado: {title}");
        }
        assert!(is_blocked_window_title(Some("Exodus"), &patterns).is_none(), "`proc:` nunca casa por título");
    }

    /// O default do schema (o que o painel mostra e o que perfil novo grava) tem de ser o MESMO que o
    /// gate usa quando a chave não existe — duas listas que divergem em silêncio já foram um defeito.
    #[test]
    fn the_schema_default_and_the_gate_default_are_the_same_list() {
        let defaults = crate::config::schemas::registry().defaults();
        assert_eq!(defaults["vision"]["captureBlocklist"], serde_json::json!(DEFAULT_CAPTURE_BLOCKLIST));
    }

    // --- is_blocked_window_title -----------------------------------------------------------------

    #[test]
    fn matches_a_whole_word_pattern() {
        let patterns = parse_blocklist(Some("nubank"));
        assert_eq!(is_blocked_window_title(Some("Nubank - App"), &patterns), Some("nubank".to_string()));
    }

    /// NEGATIVO — o caso que a fronteira de palavra existe para evitar: "inter" não pode casar
    /// dentro de "Internet", senão qualquer navegador desligaria a captura.
    #[test]
    fn does_not_match_a_pattern_that_is_only_a_substring_of_a_longer_word() {
        let patterns = parse_blocklist(Some("inter"));
        assert_eq!(is_blocked_window_title(Some("Internet Explorer"), &patterns), None);
    }

    /// Entrada do usuário que começa/termina em símbolo (`.env`, `c++`, `senha:`): `\b` colado a um
    /// não-caractere-de-palavra nunca casava, e a entrada não bloqueava nada.
    #[test]
    fn a_pattern_edged_by_a_symbol_still_blocks() {
        let patterns = parse_blocklist(Some(".env, c++, senha:"));
        for title in ["Notepad - .env", ".env", "learning c++ today", "Senha: 1234 - Notas"] {
            assert!(is_blocked_window_title(Some(title), &patterns).is_some(), "deveria bloquear: {title}");
        }
        assert_eq!(is_blocked_window_title(Some("Cplusplus notes"), &patterns), None);
    }

    #[test]
    fn empty_pattern_list_never_blocks() {
        assert_eq!(is_blocked_window_title(Some("Bitwarden"), &[]), None);
    }

    #[test]
    fn missing_or_blank_title_never_blocks() {
        let patterns = parse_blocklist(Some("banco"));
        assert_eq!(is_blocked_window_title(None, &patterns), None);
        assert_eq!(is_blocked_window_title(Some("   "), &patterns), None);
    }

    #[test]
    fn unrelated_title_does_not_match() {
        let patterns = parse_blocklist(Some(DEFAULT_CAPTURE_BLOCKLIST));
        assert_eq!(is_blocked_window_title(Some("Visual Studio Code"), &patterns), None);
    }

    // --- is_own_window -----------------------------------------------------------------------------

    /// Reprodução do item 3 do WP9: a janela de Configurações da Mika em primeiro plano tem de ser
    /// excluída da captura, incondicionalmente — mesmo sem NENHUMA entrada na blocklist do usuário.
    #[test]
    fn is_own_window_matches_mikas_settings_and_wizard_titles() {
        assert!(is_own_window(Some(crate::windows::SETTINGS_TITLE)));
        assert!(is_own_window(Some(crate::windows::WIZARD_TITLE)));
    }

    #[test]
    fn is_own_window_is_false_for_any_other_title_including_none() {
        assert!(!is_own_window(Some("Chrome")));
        assert!(!is_own_window(None));
    }

    // --- is_foreground_context_blocked ------------------------------------------------------------
    // Puras — recebem título/dono injetados, nunca tocam `system_signals::active_window` nem o
    // `ConfigStore` (essa fiação impura fica em `is_foreground_blocked`, sem teste de unidade
    // próprio pelo mesmo motivo que `game_watcher.rs::tick` não testa o SO real).

    #[test]
    fn foreground_blocked_matches_a_pattern_against_the_owner() {
        let patterns = parse_blocklist(Some("bitwarden"));
        assert_eq!(
            is_foreground_context_blocked(Some("Cofre"), Some("Bitwarden.exe"), &patterns),
            Some("bitwarden".to_string())
        );
    }

    #[test]
    fn foreground_blocked_matches_a_pattern_against_the_title() {
        let patterns = parse_blocklist(Some("nubank"));
        assert_eq!(is_foreground_context_blocked(Some("Nubank - App"), Some("chrome.exe"), &patterns), Some("nubank".to_string()));
    }

    /// NEGATIVO — a fronteira de palavra vale aqui também, mesmo `matches_pattern` de
    /// `is_blocked_window_title`.
    #[test]
    fn foreground_blocked_respects_the_word_boundary() {
        let patterns = parse_blocklist(Some("inter"));
        assert_eq!(is_foreground_context_blocked(Some("Internet Explorer"), Some("iexplore.exe"), &patterns), None);
    }

    #[test]
    fn foreground_blocked_is_none_with_no_patterns_or_no_identity() {
        assert_eq!(is_foreground_context_blocked(Some("Bitwarden"), None, &[]), None);
        let patterns = parse_blocklist(Some("banco"));
        assert_eq!(is_foreground_context_blocked(None, None, &patterns), None);
        assert_eq!(is_foreground_context_blocked(Some("   "), Some(""), &patterns), None);
    }
}
