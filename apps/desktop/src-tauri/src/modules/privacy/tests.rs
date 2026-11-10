//! Testes de `privacy/mod.rs` — extraído para caber no teto de 300 linhas por arquivo (mesmo
//! motivo de `feature.rs` ter virado arquivo próprio nesta rodada).
//!
//! `#![cfg(test)]` interno, não `#[cfg(test)]` na declaração em `mod.rs` — mesmo padrão de
//! `modules/gamer/test_support.rs`. Sem o atributo interno, `state.rs::single_config_store_per_process`
//! lê este arquivo como um `.rs` de produção (ele só corta em `#[cfg(test)]` DENTRO do próprio
//! arquivo) e acusa `ConfigStore::open` de `store_with()` como abertura de store fora do
//! `ShellState` — falso positivo confirmado rodando a suíte antes desta linha existir.
#![cfg(test)]

use super::*;
use std::path::PathBuf;

/// Mesmo padrão de `config/store.rs`/`modules/telemetry.rs`: diretório isolado por teste, sem
/// trazer dependência só para isso.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-privacy-module-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store_with(offline_mode: bool, screen_capture: bool) -> (ConfigStore, TempDir) {
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;

    let dir = TempDir::new();
    let schema = SchemaRegistry::new(vec![Namespace {
        name: "privacy",
        fields: vec![boolean("offlineMode", false), boolean("screenCapture", false)],
    }]);
    let store = ConfigStore::open(&dir.0, schema).unwrap();
    store.set(OFFLINE_MODE_PATH, serde_json::json!(offline_mode)).unwrap();
    store.set(SCREEN_CAPTURE_PATH, serde_json::json!(screen_capture)).unwrap();
    (store, dir)
}

// --- paridade com a fonte TS (pin de drift silencioso) -------------------------------------

#[test]
fn config_paths_match_the_typescript_enum() {
    let enum_src =
        include_str!("../../../../../../packages/sdk/src/contracts/schemas/config-keys.enum.ts");
    assert!(
        enum_src.contains("PrivacyOfflineMode = 'privacy.offlineMode'"),
        "OFFLINE_MODE_PATH divergiu de ConfigKey.PrivacyOfflineMode"
    );
    assert!(
        enum_src.contains("PrivacyScreenCapture = 'privacy.screenCapture'"),
        "SCREEN_CAPTURE_PATH divergiu de ConfigKey.PrivacyScreenCapture"
    );
}


// `adapters/screen-capture-consent.ts` foi removido (backend Node deletado na migração para
// Tauri) — o Rust é agora o único adaptador. A chave que ele lia (`ConfigKey.PrivacyScreenCapture`)
// já é pinada contra `packages/sdk` (que sobrevive) em `config_paths_match_the_typescript_enum`
// acima, e `SCREEN_CAPTURE_PATH` é a mesma constante usada para escrever e ler nos testes do gate
// abaixo — não há mais um segundo lado a comparar.

// --- is_loopback_url -------------------------------------------------------------------------

#[test]
fn loopback_hosts_are_exempt() {
    for url in [
        "http://localhost:11434",
        "http://127.0.0.1:8080/api",
        "http://[::1]:9000/",
        "http://0.0.0.0/",
        "http://sub.localhost/",
        "HTTP://LOCALHOST/",
    ] {
        assert!(is_loopback_url(url), "{url} deveria ser loopback");
    }
}

#[test]
fn external_hosts_are_not_loopback() {
    for url in ["https://api.openai.com/v1", "https://example.com", "http://192.168.1.10:8080"] {
        assert!(!is_loopback_url(url), "{url} não deveria ser loopback");
    }
}

/// NEGATIVO: URL malformada é o caso que o comentário do TS chama explicitamente — falhar
/// aberto (tratar como loopback) transformaria todo erro de parsing numa fuga do modo offline.
#[test]
fn unparseable_url_is_not_loopback_fails_closed() {
    assert!(!is_loopback_url("not a url"));
    assert!(!is_loopback_url(""));
}

// --- is_offline_mode_enabled / assert_network_allowed ---------------------------------------

#[test]
fn offline_mode_disabled_by_default_allows_external_requests() {
    let (store, _dir) = store_with(false, false);
    assert!(!is_offline_mode_enabled(&store));
    assert!(assert_network_allowed(&store, "https://api.openai.com", "test").is_ok());
}

/// NEGATIVO — o caminho que o chokepoint existe para negar: modo offline ligado, host externo,
/// tem de ser recusado.
#[test]
fn offline_mode_enabled_denies_external_request() {
    let (store, _dir) = store_with(true, false);
    assert!(is_offline_mode_enabled(&store));
    let result = assert_network_allowed(&store, "https://api.openai.com/v1", "llm:openai");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains(OFFLINE_MODE_PATH));
}

#[test]
fn offline_mode_enabled_still_allows_loopback() {
    let (store, _dir) = store_with(true, false);
    assert!(assert_network_allowed(&store, "http://127.0.0.1:11434/api/tags", "llm:ollama").is_ok());
}

/// NEGATIVO — o mesmo caso malformado acima, mas pela porta que importa de verdade: uma URL que
/// não parseia não pode virar uma fuga silenciosa do modo offline.
#[test]
fn offline_mode_enabled_denies_unparseable_url() {
    let (store, _dir) = store_with(true, false);
    assert!(assert_network_allowed(&store, "not a url", "test").is_err());
}

#[test]
fn env_override_forces_offline_mode_regardless_of_config() {
    assert!(is_offline_mode_enabled_with(&store_with(false, false).0, true));
}

#[test]
fn env_value_must_be_exactly_one() {
    assert!(external_http_blocked_by_env_value(Some("1")));
    assert!(!external_http_blocked_by_env_value(Some("true")));
    assert!(!external_http_blocked_by_env_value(Some("")));
    assert!(!external_http_blocked_by_env_value(None));
}

// --- is_screen_capture_allowed ----------------------------------------------------------------

#[test]
fn screen_capture_allowed_only_when_explicitly_true() {
    let (store, _dir) = store_with(false, true);
    assert!(is_screen_capture_allowed(&store));
}

/// NEGATIVO — o padrão de fábrica é negar. Sem isto, um schema que não populasse o campo
/// liberaria a câmera do usuário por omissão.
#[test]
fn screen_capture_denied_by_default() {
    let (store, _dir) = store_with(false, false);
    assert!(!is_screen_capture_allowed(&store));
}

/// "Pausar mic e tela" (bandeja) vence a config: com a captura LIGADA e os sentidos pausados, nega.
#[test]
fn paused_senses_deny_screen_capture_even_when_the_config_allows_it() {
    let (store, _dir) = store_with(false, true);
    assert!(screen_capture_allowed(&store, false));
    assert!(!screen_capture_allowed(&store, true));
}

#[test]
fn no_channels_claimed_yet() {
    assert!(modules().is_empty());
}

// --- check:privacy-chokepoint (metade Rust) -----------------------------------------------------
//
// `scripts/checks/privacy-chokepoint.mts` escopou a si mesmo a `apps/desktop/src/features/**`
// depois que `src/main` (as nascentes TS de redação/rede) morreu na migração para Tauri — o
// cabeçalho dele confessa: "isto é um buraco de verdade, não decoração". A metade de REDAÇÃO já é
// fechada pelo próprio compilador: [`redact_pii`] em `system_signals/redact.rs` e `RawForeground`
// em `system_signals/foreground.rs` são `pub(super)` — nenhum arquivo fora de `system_signals` pode
// sequer nomeá-los, muito menos chamá-los por fora do funil.
//
// A metade de REDE não tinha gate nenhum: nada impedia um `reqwest::Client::builder()` novo,
// solto, sem passar por [`assert_network_allowed`]. Medido em 2026-09-22 (grep por
// `reqwest::Client::(new|builder)` em código de produção, `src-tauri/src` inteiro): 4 sites, cada
// um já com seu PRÓPRIO funil — não existe UM ponto central de rede neste crate (browser lida com
// URL arbitrária vinda de busca/modelo e precisa também de revalidação de SSRF por hop;
// desktop/voice não). O invariante real não é "um Client só”; é "todo Client construído em
// produção mora numa lista fechada, e cada dono chama `assert_network_allowed` antes de despachar".
#[test]
fn every_reqwest_client_construction_site_is_an_allowlisted_funnel_that_calls_assert_network_allowed() {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    // Caminho relativo a `src-tauri/src/`, ponto e vírgula incluído no comentário, não no valor.
    const ALLOWED_FUNNELS: &[(&str, &str)] = &[
        (
            "modules/browser/guarded_fetch/mod.rs",
            "único despacho de URL arbitrária (busca/`action=fetch`) — soma SSRF + offline por hop, ver o cabeçalho do arquivo",
        ),
        (
            "modules/privacy/mod.rs",
            "`fixed_endpoint_client`: cliente único (timeouts + 3 redirects) dos endpoints fixos — \
             wizard, perfil de modelo, Steam, download do Supertonic; cada chamador passa pelo \
             próprio `assert_network_allowed`",
        ),
        (
            "modules/llm.rs",
            "cliente único de chat/visão/embeddings/catálogo (connect 10 s, 120 s entre chunks) — \
             provedor configurado pelo usuário; cada chamador passa pelo próprio `assert_network_allowed`",
        ),
        (
            "modules/voice/stt_cloud.rs",
            "provedor de STT em nuvem configurado pelo usuário — endpoint do provider, não arbitrário",
        ),
        (
            "modules/voice/tts/providers/remote/http.rs",
            "provedor de TTS remoto configurado pelo usuário — mesmo motivo do STT",
        ),
        (
            "modules/voice/tts/providers/piper.rs",
            "download do modelo de voz do Piper (endpoint fixo do repositório de vozes) — mesmo \
             chokepoint (`assert_network_allowed`) que `providers/supertonic/assets.rs` já usa para \
             o download equivalente do Supertonic",
        ),
    ];

    fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    fn production_only(source: &str) -> &str {
        source.split("mod tests {").next().unwrap_or(source)
    }

    fn is_test_only_file(file: &Path) -> bool {
        file.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with("_tests.rs") || n == "tests.rs")
    }

    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut all_files = Vec::new();
    rs_files(&src_root, &mut all_files);

    let mut sites_found: BTreeSet<String> = BTreeSet::new();
    for file in &all_files {
        if is_test_only_file(file) {
            continue;
        }
        let source = fs::read_to_string(file).unwrap_or_default();
        let production = production_only(&source);

        // `Client::builder()`/`Client::new(` cru só conta quando `Client` É `reqwest::Client` neste
        // arquivo — vários provedores de LLM (`rig_core`: `gemini::Client`, `openrouter::Client`,
        // `openai::Client`, `ollama::Client`, todos com `.builder()`) reusam o mesmo nome de tipo
        // para o cliente DELES, não para `reqwest`. Sem essa distinção o gate confundia os quatro
        // com construção de `reqwest::Client` de verdade e acusava `brain::turn`/`llm_provider`/
        // `vision::llm_client` como funil não-listado — falso positivo, não achado.
        let imports_reqwest_client = production.contains("use reqwest::Client");
        let raw_reqwest = production.contains("reqwest::Client::new") || production.contains("reqwest::Client::builder");
        let bare_reqwest = imports_reqwest_client
            && (production.contains("Client::new(") || production.contains("Client::builder()"));

        if raw_reqwest || bare_reqwest {
            let relative = file.strip_prefix(&src_root).unwrap_or(file).to_string_lossy().replace('\\', "/");
            sites_found.insert(relative);
        }
    }

    let allowed: BTreeSet<String> = ALLOWED_FUNNELS.iter().map(|(path, _)| path.to_string()).collect();

    let unexpected: Vec<&String> = sites_found.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "reqwest::Client construído fora da lista fechada de funis (`ALLOWED_FUNNELS` em \
         privacy/tests.rs) — redação/SSRF/offline não cobrem quem despacha rede sem passar por um \
         funil conhecido: {unexpected:?}. Rotule por `assert_network_allowed` e adicione à lista, \
         com justificativa, ou volte a chamada a passar por um funil já existente"
    );

    let stale: Vec<&String> = allowed.difference(&sites_found).collect();
    assert!(
        stale.is_empty(),
        "funil(is) declarado(s) em ALLOWED_FUNNELS e o arquivo não constrói mais nenhum \
         reqwest::Client — remova a entrada, virou decoração: {stale:?}"
    );

    for (path, _reason) in ALLOWED_FUNNELS {
        let full = src_root.join(path);
        let source = fs::read_to_string(&full).unwrap_or_else(|e| panic!("lendo funil {path}: {e}"));
        assert!(
            production_only(&source).contains("assert_network_allowed"),
            "{path} constrói reqwest::Client mas não chama `assert_network_allowed` em produção — \
             a metade do par que impede rede em modo offline está faltando"
        );
    }
}
