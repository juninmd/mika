//! "O que está na tela agora" — fecha a lacuna documentada no cabeçalho de `system_prompt.rs`
//! (`chat::PersonaState.screen_context` nunca teve fonte Rust) e em `turn.rs` ("continua sem
//! tela/contexto de jogo PRÓPRIO"). specs/08-gamer-companion.md §"Ajuda no jogo" (SHOULD) promete
//! que, com visão ligada, a Mika "ajuda com o que está na tela" através do chat REGULAR consumindo o
//! contexto que `specs/06-vision.md` já publica — até esta rodada isso nunca acontecia: só o
//! `ScreenProvider` do motor proativo (`brain::awareness`) via a análise, o turno de chat comum não.
//!
//! **Holder compartilhado, atualizado por assinante do barramento — não pull direto.** Diferente de
//! [`super::gaming_context`] (lê `SessionMemory::current_session()` na hora), aqui não existe um
//! `SessionMemory` equivalente para tela: a única fonte é o evento `EventKey::ScreenObservation` que
//! `vision::watcher` publica a cada tick. [`spawn`] assina esse evento (mesmo padrão de
//! `gamer::game_learning::spawn` — assina ANTES do `spawn`, `tauri::async_runtime::spawn`, seleciona
//! `shutdown::until_triggered`) e grava só o TEXTO (`analysis`, o resumo em prosa que
//! `vision::watcher::observation_payload` já produz) e o instante da captura num
//! `OnceLock<Mutex<Option<ScreenSnapshot>>>` — mesma forma de `gamer::SESSION_MEMORY`. Nunca imagem,
//! nunca OCR bruto: só o que a observação JÁ publicou como prosa.
//!
//! **Leitura pull-based.** [`build_screen_context_block`] é chamada por
//! [`super::compose_system_prompt`] a cada turno de chat, igual às outras cinco fontes de
//! `dynamic_context` — sem `EventKey` novo do lado de leitura, só o holder.
//!
//! **Três guardas antes de injetar, todas testadas puras:**
//! 1. `vision.enabled` (mesmo `ConfigKey` que `vision::watcher` já usa para o próprio tick) —
//!    sem visão ligada, sem bloco, mesmo que uma observação antiga ainda esteja no holder.
//! 2. `privacy::is_screen_capture_allowed` — nega por padrão (chokepoint único, ver o cabeçalho de
//!    `privacy/mod.rs`); a visão pode estar "ligada" na config e ainda assim sem consentimento.
//! 3. Frescor: TTL = `min(2 × vision.captureIntervalMs, SCREEN_CONTEXT_MAX_TTL_MS)` — uma
//!    observação de 10 minutos atrás não é "agora", mesmo com intervalo configurado longo.
//!
//! Char-cap (`SCREEN_CONTEXT_CHAR_CAP`) pela mesma razão de `FACT_CHAR_CAP`/`PERSON_BIO_CHAR_CAP`
//! em `persona/mod.rs`: `num_ctx` do Ollama é limitado (`project_ollama_num_ctx_4096`), um resumo de
//! tela não pode competir pelo orçamento inteiro do prompt.

use std::sync::{Mutex, OnceLock};

use crate::config::store::ConfigStore;
use crate::events::EventKey;

/// Teto absoluto de frescor, independente do intervalo configurado — "at most a few minutes" do
/// pedido. Maior que o pior caso de `vision::watcher::MIN_INTERVAL_MS` (60s) × 2 (120s), então só
/// entra em jogo quando `vision.captureIntervalMs` está configurado acima de 2 minutos.
const SCREEN_CONTEXT_MAX_TTL_MS: u64 = 4 * 60_000;

/// Intervalo assumido quando `vision.captureIntervalMs` está ausente/inválido — mesmo default de
/// `vision::watcher::MIN_INTERVAL_MS` (cópia do literal, não importado: aquele é `pub(super)` do
/// módulo `vision`, e duplicar uma constante de 60_000 não vale acoplar os dois módulos).
const DEFAULT_CAPTURE_INTERVAL_MS: u64 = 60_000;

/// Teto de caracteres do resumo injetado — mesma régua de `FACT_CHAR_CAP`/`PERSON_BIO_CHAR_CAP`
/// (`persona/mod.rs`), maior porque o texto aqui é a análise inteira de um frame, não um fato curto.
const SCREEN_CONTEXT_CHAR_CAP: usize = 500;

/// Última observação de tela conhecida — só o texto em prosa (`analysis`) e o instante da captura,
/// nunca imagem nem OCR bruto além do que a observação já publicou.
#[derive(Debug, Clone)]
pub struct ScreenSnapshot {
    pub text: String,
    pub captured_at_ms: u64,
}

/// `Mutex<Option<_>>`, não `OnceLock<ScreenSnapshot>` direto — mesmo motivo de
/// `gamer::SESSION_MEMORY`: o valor muda a cada tick de visão, e testes de outros módulos precisam
/// poder trocar/limpar entre casos.
static LATEST_SCREEN_SNAPSHOT: OnceLock<Mutex<Option<ScreenSnapshot>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<ScreenSnapshot>> {
    LATEST_SCREEN_SNAPSHOT.get_or_init(|| Mutex::new(None))
}

/// Escrito pelo assinante de [`spawn`] a cada `EventKey::ScreenObservation`; exposto `pub(crate)`
/// para os testes de integração deste arquivo poderem popular o holder sem publicar um evento real.
pub(crate) fn set_latest_screen_snapshot(text: String, captured_at_ms: u64) {
    *slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(ScreenSnapshot { text, captured_at_ms });
}

/// Leitura pull-based do que [`super::compose_system_prompt`] consulta a cada turno. `None` antes da
/// primeira observação (nunca acontece em produção depois do primeiro tick de `vision::watcher`, ou
/// em qualquer teste que não popule o holder).
fn latest_screen_snapshot() -> Option<ScreenSnapshot> {
    slot().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn get_bool(store: &ConfigStore, path: &str, default: bool) -> bool {
    store.get(path).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn get_u64(store: &ConfigStore, path: &str, default: u64) -> u64 {
    store.get(path).and_then(|v| v.as_u64()).unwrap_or(default)
}

/// `TTL = min(2 × intervalo configurado, SCREEN_CONTEXT_MAX_TTL_MS)` — pura, testável sem
/// `ConfigStore`.
fn screen_context_ttl_ms(configured_interval_ms: u64) -> u64 {
    configured_interval_ms.saturating_mul(2).min(SCREEN_CONTEXT_MAX_TTL_MS)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    format!("{}…", text.chars().take(max_chars).collect::<String>())
}

/// `true` quando `captured_at_ms` ainda está dentro do TTL a partir de `now_ms` — `saturating_sub`
/// absorve um `captured_at_ms` no futuro (relógio ajustado entre a captura e a leitura) como "ainda
/// fresco" em vez de estourar/subtrair com overflow.
fn is_fresh(captured_at_ms: u64, now_ms: u64, ttl_ms: u64) -> bool {
    now_ms.saturating_sub(captured_at_ms) <= ttl_ms
}

/// Monta o bloco "## What Is On Screen Right Now" — string vazia (mesmo padrão de todo bloco opcional
/// de `persona/mod.rs`/`gaming_context.rs`) quando qualquer uma das guardas falha: visão desligada,
/// privacidade não permite captura, sem observação ainda, observação obsoleta ou texto vazio.
fn build_screen_context_block(snapshot: Option<&ScreenSnapshot>, vision_enabled: bool, screen_capture_allowed: bool, now_ms: u64, ttl_ms: u64) -> String {
    if !vision_enabled || !screen_capture_allowed {
        return String::new();
    }
    let Some(snapshot) = snapshot else { return String::new() };
    if !is_fresh(snapshot.captured_at_ms, now_ms, ttl_ms) {
        return String::new();
    }
    let text = snapshot.text.trim();
    if text.is_empty() {
        return String::new();
    }
    format!("\n\n## What Is On Screen Right Now\n{}", truncate_chars(text, SCREEN_CONTEXT_CHAR_CAP))
}

/// Fachada chamada por [`super::compose_system_prompt`] — resolve as três guardas contra o
/// `ConfigStore`/holder reais e delega a [`build_screen_context_block`], pura.
pub fn build_screen_context_from_store(store: &ConfigStore) -> String {
    let snapshot = latest_screen_snapshot();
    let vision_enabled = get_bool(store, "vision.enabled", false);
    let screen_capture_allowed = crate::modules::privacy::is_screen_capture_allowed(store);
    let interval_ms = get_u64(store, "vision.captureIntervalMs", DEFAULT_CAPTURE_INTERVAL_MS);
    build_screen_context_block(snapshot.as_ref(), vision_enabled, screen_capture_allowed, now_ms(), screen_context_ttl_ms(interval_ms))
}

/// Sobe o assinante de `EventKey::ScreenObservation` que alimenta [`LATEST_SCREEN_SNAPSHOT`] — só
/// guarda `analysis`/`at` do payload (ver `vision::watcher::observation_payload`), nunca o resto
/// (pessoas, sinal de jogo etc. não são "o que está na tela" para efeito de prompt). Assina ANTES do
/// `spawn` (mesma regra de AGENTS.md/`modules/mod.rs`), `tauri::async_runtime::spawn`, seleciona
/// `shutdown::until_triggered` (gate `check:lifecycle-shutdown`).
pub fn spawn() -> tauri::async_runtime::JoinHandle<()> {
    let mut sub = crate::events::bus().subscribe();
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if envelope.key != EventKey::ScreenObservation {
                continue;
            }
            let Some(text) = envelope.payload.get("analysis").and_then(|v| v.as_str()).filter(|t| !t.trim().is_empty()) else { continue };
            let at = envelope.payload.get("at").and_then(|v| v.as_u64()).unwrap_or_else(now_ms);
            set_latest_screen_snapshot(text.to_string(), at);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// Serializa os dois testes que escrevem em [`LATEST_SCREEN_SNAPSHOT`] (holder global de
    /// processo, `cargo test` roda em paralelo) — mesmo padrão de `gamer::SESSION_MEMORY_TEST_ORDER`.
    static HOLDER_TEST_ORDER: StdMutex<()> = StdMutex::new(());

    fn snap(text: &str, captured_at_ms: u64) -> ScreenSnapshot {
        ScreenSnapshot { text: text.to_string(), captured_at_ms }
    }

    // -- screen_context_ttl_ms ------------------------------------------------------------------

    #[test]
    fn ttl_is_twice_the_configured_interval_when_under_the_cap() {
        assert_eq!(screen_context_ttl_ms(60_000), 120_000);
    }

    #[test]
    fn ttl_caps_at_a_few_minutes_even_with_a_long_configured_interval() {
        assert_eq!(screen_context_ttl_ms(10 * 60_000), SCREEN_CONTEXT_MAX_TTL_MS);
    }

    // -- is_fresh ---------------------------------------------------------------------------------

    #[test]
    fn is_fresh_accepts_a_capture_inside_the_ttl() {
        assert!(is_fresh(1_000, 1_000 + 60_000, 120_000));
    }

    #[test]
    fn is_fresh_rejects_a_capture_past_the_ttl() {
        assert!(!is_fresh(1_000, 1_000 + 200_000, 120_000));
    }

    #[test]
    fn is_fresh_tolerates_a_capture_timestamp_in_the_future() {
        // Relógio ajustado entre a captura e a leitura: `saturating_sub` nunca estoura, e "capturado
        // no futuro" conta como fresco em vez de derrubar o bloco inteiro por causa de um clock skew.
        assert!(is_fresh(5_000, 1_000, 120_000));
    }

    // -- build_screen_context_block: as três guardas ------------------------------------------------

    #[test]
    fn no_block_without_any_observation_yet() {
        assert_eq!(build_screen_context_block(None, true, true, 10_000, 120_000), "");
    }

    #[test]
    fn no_block_when_vision_is_disabled_even_with_a_fresh_observation() {
        let s = snap("terminal aberto com um erro de compilação", 10_000);
        assert_eq!(build_screen_context_block(Some(&s), false, true, 10_000, 120_000), "");
    }

    #[test]
    fn no_block_when_privacy_denies_screen_capture_even_with_vision_enabled() {
        let s = snap("terminal aberto com um erro de compilação", 10_000);
        assert_eq!(build_screen_context_block(Some(&s), true, false, 10_000, 120_000), "");
    }

    #[test]
    fn no_block_when_the_observation_is_stale() {
        let s = snap("terminal aberto com um erro de compilação", 10_000);
        assert_eq!(build_screen_context_block(Some(&s), true, true, 10_000 + 500_000, 120_000), "");
    }

    #[test]
    fn no_block_when_the_observation_text_is_blank() {
        let s = snap("   ", 10_000);
        assert_eq!(build_screen_context_block(Some(&s), true, true, 10_000, 120_000), "");
    }

    #[test]
    fn injects_the_block_when_every_guard_passes() {
        let s = snap("terminal aberto com um erro de compilação em Rust", 10_000);
        let block = build_screen_context_block(Some(&s), true, true, 10_500, 120_000);
        assert_eq!(block, "\n\n## What Is On Screen Right Now\nterminal aberto com um erro de compilação em Rust");
    }

    #[test]
    fn caps_the_injected_text_at_the_character_budget() {
        let long_text = "x".repeat(SCREEN_CONTEXT_CHAR_CAP + 50);
        let s = snap(&long_text, 10_000);
        let block = build_screen_context_block(Some(&s), true, true, 10_000, 120_000);
        assert!(block.contains('…'), "texto acima do teto tem de ser truncado, não estourar o orçamento");
        assert!(block.len() < long_text.len(), "bloco truncado não pode ser tão grande quanto o texto original");
    }

    // -- build_screen_context_from_store: integração com ConfigStore/holder -------------------------

    fn store_with(pairs: &[(&str, serde_json::Value)]) -> ConfigStore {
        let dir = std::env::temp_dir().join(format!("mika-screen-context-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        for (path, value) in pairs {
            store.set(path, value.clone()).unwrap();
        }
        store
    }

    #[test]
    fn from_store_is_empty_by_default_deny_by_default_privacy_and_vision_off() {
        let _guard = HOLDER_TEST_ORDER.lock().unwrap_or_else(|e| e.into_inner());
        let store = store_with(&[]);
        set_latest_screen_snapshot("qualquer coisa".to_string(), now_ms());
        assert_eq!(build_screen_context_from_store(&store), "", "vision.enabled e privacy.screenCapture negam por padrão");
    }

    #[test]
    fn from_store_injects_the_block_when_vision_and_privacy_both_allow_it() {
        let _guard = HOLDER_TEST_ORDER.lock().unwrap_or_else(|e| e.into_inner());
        let store = store_with(&[("vision.enabled", serde_json::json!(true)), ("privacy.screenCapture", serde_json::json!(true))]);
        set_latest_screen_snapshot("editor de código com um teste falhando".to_string(), now_ms());
        let block = build_screen_context_from_store(&store);
        assert!(block.contains("editor de código com um teste falhando"), "bloco ausente: {block}");
    }
}
