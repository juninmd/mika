//! === features/character/slang-context-provider.ts ==============================================
//!
//! Porte PULL-BASED do provedor de contexto de gíria: em vez de assinar `GAME_CHANGED`/
//! `SCREEN_OBSERVATION`, [`build_slang_block`] lê `gamer::session_memory()` e, desde 2026-09-22,
//! `system_signals::active_window::last_known()` no instante em que o prompt é montado.
//! `OS_ACTIVE_WINDOW_CHANGED` TEM `EventKey` Rust agora (`ActiveWindowChanged`,
//! `modules/system_signals/active_window.rs`), mas este arquivo continua pull-based por
//! consistência com os outros dois modos — trocar só este por assinatura push criaria dois padrões
//! de leitura de sinal no mesmo arquivo sem ganho real (a leitura já é barata, um `Mutex` local).
//!
//! **`GAME_MOMENT` — porte HÍBRIDO desde 2026-09-22.** `EventKey::GameMoment` agora existe
//! (`gamer::rpc::MiniGameResult` publica); [`on_game_moment`] é chamada pelo assinante já existente
//! em `brain::awareness::mod.rs` (mesmo arm de `match` que alimenta `tension.rs`, um segundo efeito
//! colateral do mesmo evento) e só GRAVA `(game, at)` num `Mutex` estático — `current_mode`/
//! `detail_line` continuam PULL, lendo esse estado com o MESMO TTL de 5 minutos
//! (`SIGNAL_TTL_MS`) que `slang-context-provider.ts::setSignal` usava. Isso preserva "um só padrão
//! de leitura" (tudo em `current_mode`/`detail_line` é lido, nunca reage) enquanto fecha o gap real:
//! um `GAME_MOMENT` de boss/vitória força `combat` mesmo se `gamer::session_memory()` não tiver uma
//! sessão "atual" (ex.: mini-jogo, sem sessão Steam).
//!
//! ## Escopo menor que o TS, não escondido
//! `technical` (IDE em foco) foi fechado nesta rodada: `game_detection.rs` documentava que esse
//! ramo pedia uma crate nova (`windows`/X11/Wayland), que `system_signals::active_window` agora
//! resolve (Windows, Linux/X11 e macOS). Sem sonda (Wayland) `last_known()` nunca preenche, então
//! `technical` nunca dispara ali.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::config::store::ConfigStore;

use super::slang_dictionary;

const MAX_ENTRIES: usize = 4;
/// = `SIGNAL_TTL_MS` em `slang-context-provider.ts`.
const GAME_MOMENT_SIGNAL_TTL_MS: u64 = 5 * 60 * 1000;

/// Último `EventKey::GameMoment` visto (nome do jogo, epoch ms) — ver o cabeçalho: gravado por
/// [`on_game_moment`] (chamado do assinante de `brain::awareness::mod.rs`), lido por
/// `current_mode`/`detail_line`, nunca por um segundo assinante próprio deste arquivo.
static LAST_GAME_MOMENT: Mutex<Option<(String, u64)>> = Mutex::new(None);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Chamada pelo assinante de `EventKey::GameMoment` — grava o nome do jogo e o instante; sem
/// `game` no payload, é um no-op (mesmo gate de `d?.game` no TS).
pub fn on_game_moment(payload: &Value) {
    let Some(game) = payload.get("game").and_then(Value::as_str).filter(|g| !g.is_empty()) else { return };
    let at = payload.get("at").and_then(Value::as_u64).unwrap_or_else(now_ms);
    *LAST_GAME_MOMENT.lock().unwrap_or_else(|e| e.into_inner()) = Some((game.to_string(), at));
}

/// `Some(jogo)` só enquanto o sinal está dentro do TTL — porte de `currentSignal()` filtrando por
/// `SIGNAL_TTL_MS`, sem o restante da máquina de sinais genérica do TS (só este UM sinal existe do
/// lado Rust).
fn fresh_game_moment() -> Option<String> {
    let guard = LAST_GAME_MOMENT.lock().unwrap_or_else(|e| e.into_inner());
    let (game, at) = guard.as_ref()?;
    (now_ms().saturating_sub(*at) < GAME_MOMENT_SIGNAL_TTL_MS).then(|| game.clone())
}

/// Gira as entradas mostradas entre chamadas sucessivas — mesmo papel do `turn++` do TS
/// (`createSlangContextProvider`), só que sem instância própria: o prompt inteiro é montado por uma
/// fachada só (`compose_system_prompt`), então um contador de processo tem o mesmo efeito.
static ROTATION: AtomicUsize = AtomicUsize::new(0);

struct Entry {
    term: String,
    meaning: String,
    register: String,
}

fn dictionary_value(store: &ConfigStore) -> serde_json::Value {
    store.get("character.slangDictionary").unwrap_or(serde_json::Value::Null)
}

fn entries_for_mode(value: &serde_json::Value, mode: &str) -> Vec<Entry> {
    value
        .get(mode)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let term = item.get("term")?.as_str()?.to_string();
                    let meaning = item.get("meaning")?.as_str()?.to_string();
                    let register = item.get("register").and_then(|v| v.as_str()).unwrap_or("cyberpunk").to_string();
                    Some(Entry { term, meaning, register })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn has_any_entries(value: &serde_json::Value) -> bool {
    ["technical", "combat", "idle"].iter().any(|mode| !entries_for_mode(value, mode).is_empty())
}

fn default_entries(mode: &str) -> Vec<Entry> {
    let source: &[slang_dictionary::SlangEntry] = match mode {
        "technical" => slang_dictionary::TECHNICAL,
        "combat" => slang_dictionary::COMBAT,
        _ => slang_dictionary::IDLE,
    };
    source.iter().map(|e| Entry { term: e.term.to_string(), meaning: e.meaning.to_string(), register: e.register.to_string() }).collect()
}

/// Espelha `resolveDictionary`: só cai no default embutido quando o dicionário do usuário está
/// TOTALMENTE vazio (nenhuma entrada em nenhum modo) — um dicionário customizado, mesmo que vazio só
/// no modo atual, é respeitado como está. O dicionário embutido é em português: fora do pt-* ele
/// ensinaria a Mika a salpicar gíria brasileira numa conversa em coreano, então o bloco some
/// (dicionário do próprio usuário, no idioma que ele escreveu, continua valendo).
fn resolve_entries(store: &ConfigStore, mode: &str) -> Vec<Entry> {
    let value = dictionary_value(store);
    if has_any_entries(&value) {
        entries_for_mode(&value, mode)
    } else if crate::config::locale::language_code(&crate::config::locale::app_locale(store)) == "pt" {
        default_entries(mode)
    } else {
        Vec::new()
    }
}

/// Espelha `pickEntries`: até [`MAX_ENTRIES`], girando pelo pool com `offset` quando ele é maior.
fn pick_entries(entries: &[Entry], offset: usize) -> Vec<&Entry> {
    if entries.is_empty() {
        return Vec::new();
    }
    if entries.len() <= MAX_ENTRIES {
        return entries.iter().collect();
    }
    let start = offset % entries.len();
    (0..MAX_ENTRIES).map(|i| &entries[(start + i) % entries.len()]).collect()
}

fn ide_pattern() -> &'static regex::Regex {
    // Porte de `packages/sdk/src/utils/process-patterns.ts::IDE_PATTERN` — cópia literal, mesmo
    // padrão de `default_entries` já embutir os dicionários em vez de importar do TS.
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"(?i)\b(code|vscode|webstorm|intellij|rider|pycharm|cursor|windsurf|neovim|vim|sublime|atom|fleet)\b").unwrap()
    })
}

/// `Some(detail)` quando a janela ativa conhecida bate `IDE_PATTERN` — porte de
/// `isIDE(d)`/`probeFromWindow(d)`. `detail` é `owner ?? title`, mesma preferência de
/// `slang-context-provider.ts::setSignal({ detail: d?.owner ?? d?.title })`.
fn technical_detail() -> Option<String> {
    let window = crate::modules::system_signals::active_window::last_known()?;
    let probe = format!("{} {}", window.owner.as_deref().unwrap_or(""), window.title.as_deref().unwrap_or(""));
    if !ide_pattern().is_match(&probe) {
        return None;
    }
    window.owner.or(window.title)
}

/// Único par de sinais pull-based disponível hoje. `technical` vem primeiro: é o sinal mais
/// instantâneo (a janela em foco NESTE momento), enquanto `combat` reflete uma sessão de jogo que
/// pode continuar "atual" no banco mesmo com o usuário alt-tab'ado para o editor — o TS resolvia
/// isso por recência de evento (`setAt`/`SIGNAL_TTL_MS`, nenhum dos dois pull-based aqui tem
/// equivalente), então "o que está literalmente em foco agora" é a aproximação mais fiel sem
/// reintroduzir estado de TTL.
fn current_mode() -> &'static str {
    if technical_detail().is_some() {
        return "technical";
    }
    if fresh_game_moment().is_some() {
        return "combat";
    }
    match crate::modules::gamer::session_memory().and_then(|memory| memory.current_session()) {
        Some(_) => "combat",
        None => "idle",
    }
}

/// Título do bloco no prompt (instruções em inglês, como o resto do prompt).
fn mode_title(mode: &str) -> &'static str {
    match mode {
        "technical" => "Technical (Noir/Cyberpunk)",
        "combat" => "Combat (Cyberpunk/Noir)",
        _ => "Idle (Noir)",
    }
}

/// Espelha o `detailLine` de `renderSlangBlock`: em `technical`, a janela ativa (dono ou título);
/// em `combat`, o nome do jogo. Os dois rotulados como conteúdo não confiável (mesmo utilitário que
/// `browser::search` já usa para snippet de busca) — título de janela é literalmente texto que
/// outro processo escolheu, e o nome do jogo vem do casamento processo↔biblioteca Steam, mas segue
/// o mesmo tratamento por precaução.
fn detail_line(mode: &str) -> String {
    match mode {
        "technical" => match technical_detail() {
            Some(detail) => format!("Detected context: {}.\n", crate::modules::browser::label_untrusted_content("CONTEXTO_JANELA", &detail)),
            None => String::new(),
        },
        "combat" => {
            let game_name = fresh_game_moment().or_else(|| {
                crate::modules::gamer::session_memory().and_then(|memory| memory.current_session()).map(|s| s.game_name)
            });
            match game_name {
                Some(game_name) => format!("Detected context: {}.\n", crate::modules::browser::label_untrusted_content("CONTEXTO_JOGO", &game_name)),
                None => String::new(),
            }
        }
        _ => String::new(),
    }
}

/// Fachada real: `""` quando `character.slangEnabled` está desligado ou não há entrada para
/// mostrar. Chamada por `brain::persona::compose_system_prompt`. Sem `AppHandle` — usa
/// `chat::load_prompt_no_handle` (mesma fachada que `brain::persona::system_prompt` já usa para o
/// `system.md` rico, ver o cabeçalho de `chat/prompt.rs`), não o override de usuário.
pub fn build_slang_block(store: &ConfigStore) -> String {
    let enabled = store.get("character.slangEnabled").and_then(|v| v.as_bool()).unwrap_or(true);
    if !enabled {
        return String::new();
    }

    let mode = current_mode();
    let entries = resolve_entries(store, mode);
    let offset = ROTATION.fetch_add(1, Ordering::Relaxed);
    let picked = pick_entries(&entries, offset);
    if picked.is_empty() {
        return String::new();
    }

    let lines = picked.iter().map(|e| format!("- \"{}\" — {} ({})", e.term, e.meaning, e.register)).collect::<Vec<_>>().join("\n");
    let language = crate::config::locale::app_language(store);

    let mut vars = HashMap::new();
    vars.insert("modeTitle".to_string(), mode_title(mode).to_string());
    vars.insert("detailLine".to_string(), detail_line(mode));
    vars.insert("language".to_string(), language);
    vars.insert("lines".to_string(), lines);

    match crate::modules::chat::load_prompt_no_handle("character", "slang-block", &vars) {
        Ok(rendered) => format!("\n\n{rendered}\n"),
        Err(err) => {
            log::warn!("character: slang-block.md indisponível, seguindo sem gíria ({err})");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_entries_returns_everything_when_the_pool_fits() {
        let pool = vec![
            Entry { term: "a".into(), meaning: "A".into(), register: "noir".into() },
            Entry { term: "b".into(), meaning: "B".into(), register: "noir".into() },
        ];
        assert_eq!(pick_entries(&pool, 0).len(), 2);
    }

    #[test]
    fn pick_entries_rotates_the_start_by_offset_when_the_pool_is_larger_than_the_cap() {
        let pool: Vec<Entry> = (0..6).map(|i| Entry { term: i.to_string(), meaning: String::new(), register: "noir".into() }).collect();
        let first = pick_entries(&pool, 0);
        let rotated = pick_entries(&pool, 1);
        assert_eq!(first.len(), MAX_ENTRIES);
        assert_ne!(first[0].term, rotated[0].term);
    }

    #[test]
    fn resolve_entries_falls_back_to_the_embedded_default_when_the_config_dictionary_is_entirely_empty() {
        let value = serde_json::json!({ "technical": [], "combat": [], "idle": [] });
        assert!(entries_for_mode(&value, "combat").is_empty());
        assert!(!has_any_entries(&value));
    }

    #[test]
    fn resolve_entries_respects_a_customized_but_partially_empty_dictionary() {
        let value = serde_json::json!({
            "technical": [], "idle": [],
            "combat": [{ "term": "custom", "meaning": "m", "register": "noir" }],
        });
        assert!(has_any_entries(&value));
        assert!(entries_for_mode(&value, "idle").is_empty());
        assert_eq!(entries_for_mode(&value, "combat")[0].term, "custom");
    }

    #[test]
    fn mode_title_covers_technical_combat_and_idle() {
        assert_eq!(mode_title("technical"), "Technical (Noir/Cyberpunk)");
        assert_eq!(mode_title("combat"), "Combat (Cyberpunk/Noir)");
        assert_eq!(mode_title("idle"), "Idle (Noir)");
    }

    /// O dicionário embutido é português: só o pt-* o recebe; as outras línguas ficam sem o bloco
    /// até o usuário escrever o próprio dicionário.
    #[test]
    fn the_embedded_portuguese_dictionary_is_only_used_for_portuguese() {
        let dir = std::env::temp_dir().join(format!("mika-slang-locale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        assert!(!resolve_entries(&store, "combat").is_empty(), "pt-BR mantém o dicionário embutido");

        store.set("app.locale", serde_json::json!("ko-KR")).unwrap();
        assert!(resolve_entries(&store, "combat").is_empty());
        assert!(build_slang_block(&store).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_entries_covers_the_technical_mode() {
        assert!(!default_entries("technical").is_empty());
    }

    /// Porte de `IDE_PATTERN` — mesmo regex que `system_signals::foreground` usa para
    /// `NON_GAME_HINT_RE`, mas testado aqui contra o próprio uso deste arquivo (`ide_pattern`), não
    /// importado de lá (cada arquivo é dono da cópia que consome).
    #[test]
    fn ide_pattern_matches_known_editors_and_ignores_unrelated_apps() {
        assert!(ide_pattern().is_match("code.exe index.ts"));
        assert!(ide_pattern().is_match("Cursor — main.rs"));
        assert!(!ide_pattern().is_match("chrome.exe github.com"));
    }

    // `LAST_GAME_MOMENT` é `static` de processo — os 3 testes abaixo serializam via `MOMENT_LOCK`
    // para não correr entre si sob `cargo test` paralelo (mesma técnica de `TEST_LOCK` em
    // `gamer::rpc_tests`).
    static MOMENT_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn on_game_moment_without_a_game_field_is_a_no_op() {
        let _guard = MOMENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        *LAST_GAME_MOMENT.lock().unwrap() = None;
        on_game_moment(&serde_json::json!({ "type": "boss" }));
        assert!(fresh_game_moment().is_none());
    }

    #[test]
    fn a_fresh_game_moment_forces_combat_mode_even_without_a_session() {
        let _guard = MOMENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        on_game_moment(&serde_json::json!({ "game": "Hades", "at": now_ms() }));
        assert_eq!(fresh_game_moment().as_deref(), Some("Hades"));
        assert_eq!(current_mode(), "combat");
        assert!(detail_line("combat").contains("Hades"));
        *LAST_GAME_MOMENT.lock().unwrap() = None;
    }

    #[test]
    fn a_stale_game_moment_past_the_ttl_is_ignored() {
        let _guard = MOMENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let stale_at = now_ms().saturating_sub(GAME_MOMENT_SIGNAL_TTL_MS + 1_000);
        on_game_moment(&serde_json::json!({ "game": "Celeste", "at": stale_at }));
        assert!(fresh_game_moment().is_none());
        *LAST_GAME_MOMENT.lock().unwrap() = None;
    }
}
