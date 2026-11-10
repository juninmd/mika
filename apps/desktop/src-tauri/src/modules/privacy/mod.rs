//! Módulo `privacy` em Rust — o PONTO DE DECISÃO do chokepoint de privacidade, não (ainda) um
//! dono de canal.
//!
//! ## Por que este módulo reivindica ZERO canais
//!
//! `scripts/checks/privacy-chokepoint.mts` (o contrato deste chokepoint no lado TS) e
//! `packages/sdk/src/contracts/desktop-ipc*.ts` foram lidos por inteiro: não existe nenhum canal de
//! IPC chamado `privacy:*` — nem hoje, nem no sidecar. A superfície real da feature `privacy`
//! (`apps/desktop/src/features/privacy/`) é:
//!
//! 1. **Config** (`privacy.offlineMode`, `privacy.screenCapture`,
//!    `privacy.conversationRetention`, `privacy.clipboardLogging`) — já coberta GENERICAMENTE por
//!    `mika:settings-get-all`/`mika:settings-set` em `modules/settings.rs`. Reivindicar um canal
//!    "privacy" próprio para isso duplicaria uma rota que já existe.
//! 2. **Tools de LLM** (`clear_history`, `list_memories`, `export_data`, ver
//!    `apps/desktop/src/features/privacy/tools/index.ts`) — não são canais de IPC do renderer; são
//!    registradas no laço de tool-calling do `brain` e chamadas dali. `privacy.module.ts` declara
//!    as deps reais desses tools: `MemoryService`, `BrowserService`, `TelemetryService`,
//!    `PersonsService`, `TTSFactory`, `ChatService` — NENHUMA portada ainda (fases 3/4/5/8 do
//!    roadmap, todas em aberto). Fingir portar `clear_history` aqui exigiria mockar essas 9
//!    dependências: violaria "zero mock" e, pior, um `clear_history` que reporta sucesso sem de
//!    fato apagar memória/telemetria/browser seria uma REGRESSÃO DE SEGURANÇA disfarçada de
//!    progresso — a app diria "apaguei tudo" e teria apagado só o que existe em Rust hoje (nada).
//!    Regra do pedido: "prefira NÃO reivindicar o canal a reivindicá-lo com o gate frouxo". Não
//!    reivindico.
//!
//! ## O que ESTE arquivo porta então
//!
//! As duas funções puras que hoje decidem, no lado TS, se uma captura de tela ou uma requisição de
//! rede pode acontecer — o chokepoint fica no ADAPTADOR de captura e em `createLoggedFetch`, nunca
//! no chamador:
//!
//! - [`assert_network_allowed`] / [`is_offline_mode_enabled`] — port de
//!   `packages/sdk/src/http/offline-mode.ts` (`assertNetworkAllowed`/`isOfflineModeEnabled`).
//! - [`is_screen_capture_allowed`] — port de
//!   `apps/desktop/src/main/infrastructure/adapters/screen-capture-consent.ts`
//!   (`isScreenCaptureAllowed`).
//!
//! As duas têm chamadores reais: a captura de tela (`vision/capture_tool.rs`,
//! `vision/screen_capture.rs`) consulta [`is_screen_capture_allowed`] antes de capturar, e toda
//! saída de rede — LLM (`llm.rs`, `vision/llm_client.rs`), STT/TTS em nuvem, download de modelo,
//! navegador, Steam — passa por [`assert_network_allowed`]. Um gate só, na nascente, em vez de cada
//! chamador inventar a própria checagem ad-hoc (a classe de defeito que já se repetiu aqui: redação
//! de PII duplicada por chamador).
//!
//! ## Endpoint digitado pelo usuário
//!
//! [`assert_user_endpoint_allowed`] (`endpoint_guard.rs`) é o chokepoint dos endpoints de LLM/TTS
//! configuráveis: guarda de SSRF (porte do `validateUrl` do TS) SEGUIDA de
//! [`assert_network_allowed`] — relaxar a primeira sem a segunda deixaria o modo offline sem efeito.
//!
//! ## Captura de tela
//!
//! O bloqueio por janela em foco/blocklist fica FORA daqui de propósito: depende de contexto de
//! janela do SO que este módulo, só-config, não tem. Ele vive em `vision/capture_gate.rs`, depois
//! de [`is_screen_capture_allowed`].
//!
//! ## `mod.rs` -> diretório
//!
//! Virou `privacy/mod.rs` (era `privacy.rs`) só para caber [`feature`] — a declaração de settings
//! da aba `privacy` — sem estourar o teto de 300 linhas por arquivo. O caminho externo
//! (`crate::modules::privacy::*`) não muda: `pub mod privacy;` em `modules/mod.rs` resolve para
//! `privacy/mod.rs` do mesmo jeito que resolvia para `privacy.rs`.

mod client_tests;
mod endpoint_guard;
pub mod feature;
pub mod senses;
mod tests;

pub use endpoint_guard::assert_user_endpoint_allowed;

use crate::config::store::ConfigStore;
use crate::native::NativeModule;

/// Espelha `ConfigKey.PrivacyOfflineMode` (`packages/sdk/src/contracts/schemas/config-keys.enum.ts`).
pub const OFFLINE_MODE_PATH: &str = "privacy.offlineMode";
/// Espelha `ConfigKey.PrivacyScreenCapture`.
pub const SCREEN_CAPTURE_PATH: &str = "privacy.screenCapture";

/// Espelha o `process.env.MIKA_BLOCK_EXTERNAL_HTTP === '1'` de `isExternalHttpBlockedByEnv`.
const OFFLINE_MODE_ENV_VAR: &str = "MIKA_BLOCK_EXTERNAL_HTTP";

/// Espelha `LOOPBACK_HOSTNAMES` de `offline-mode.ts`, sem a entrada `[::1]` com colchetes: o
/// literal IPv6 é normalizado (colchetes removidos) antes da comparação em [`is_loopback_url`], ao
/// contrário do V8 (`new URL().hostname`), que devolve `[::1]` com colchetes e por isso o TS
/// precisa das duas formas na mesma lista.
const LOOPBACK_HOSTNAMES: [&str; 4] = ["localhost", "127.0.0.1", "::1", "0.0.0.0"];

fn external_http_blocked_by_env_value(raw: Option<&str>) -> bool {
    raw == Some("1")
}

/// Lê a variável de ambiente real. Sem `_with`: nada aqui é lógica, só a leitura — a lógica
/// testável é [`external_http_blocked_by_env_value`].
fn is_external_http_blocked_by_env() -> bool {
    let value = std::env::var(OFFLINE_MODE_ENV_VAR).ok();
    external_http_blocked_by_env_value(value.as_deref())
}

/// URL sem host parseável NÃO é loopback — espelha o comentário de `isLoopbackUrl` no TS: fica
/// bloqueada em modo offline, nunca isenta por acidente de parsing.
pub fn is_loopback_url(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    // `url::Url::host_str()` devolve o literal IPv6 COM colchetes (`[::1]`), diferente do `::1`
    // que este módulo compara — remove-los aqui em vez de duplicar a entrada na lista de hosts.
    let normalized = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    LOOPBACK_HOSTNAMES.contains(&normalized.as_str()) || normalized.ends_with(".localhost")
}

fn is_offline_mode_enabled_with(store: &ConfigStore, env_blocked: bool) -> bool {
    if env_blocked {
        return true;
    }
    store
        .get(OFFLINE_MODE_PATH)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

/// Espelha `isOfflineModeEnabled`. `ConfigStore::get` nunca lança (devolve `Option`), então não há
/// `try/catch` para replicar — ausência ou tipo errado já caem no mesmo `false` do lado TS.
pub fn is_offline_mode_enabled(store: &ConfigStore) -> bool {
    is_offline_mode_enabled_with(store, is_external_http_blocked_by_env())
}

/// Espelha `assertNetworkAllowed`. Loopback (Ollama local, servidor de dev) segue isento mesmo com
/// modo offline ligado — é o mesmo comportamento do TS.
///
/// Nunca loga a URL nem o payload da requisição: pode carregar segredo/consulta pessoal na query
/// string. Só o `provider_tag` (rótulo fixo do chamador, ex. `"browser"`, `"llm:google"`) é
/// logado — diferente do TS, que loga a URL; a regra de segurança desta tarefa (nunca logar
/// credencial ou dado do usuário) tem precedência sobre paridade literal de log aqui.
pub fn assert_network_allowed(store: &ConfigStore, url: &str, provider_tag: &str) -> Result<(), String> {
    if !is_offline_mode_enabled(store) || is_loopback_url(url) {
        return Ok(());
    }
    log::warn!("modo offline: requisição externa bloqueada (provider={provider_tag}, configPath={OFFLINE_MODE_PATH})");
    Err(format!(
        "Modo offline ligado: requisição de '{provider_tag}' bloqueada ({OFFLINE_MODE_PATH})"
    ))
}

/// Espelha `isScreenCaptureAllowed`: nega por padrão, só libera com `privacy.screenCapture ===
/// true` explícito. Ausência de schema/valor cai em `false`, nunca em `true` por omissão — negar
/// é o modo de falha seguro de um chokepoint.
///
/// Com os sentidos pausados pela bandeja ([`senses`]) nega sempre — toda captura (watcher, tools de
/// visão, contexto de tela) passa por aqui.
pub fn is_screen_capture_allowed(store: &ConfigStore) -> bool {
    screen_capture_allowed(store, senses::paused())
}

fn screen_capture_allowed(store: &ConfigStore, senses_paused: bool) -> bool {
    !senses_paused
        && store
            .get(SCREEN_CAPTURE_PATH)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
}

/// Cliente HTTP compartilhado dos endpoints FIXOS (wizard, perfil de modelo, Steam, download do
/// Supertonic): `Client::new()` não tem timeout e segue 10 redirects, então um servidor mudo prendia
/// a task para sempre. Aqui: 10 s para conectar, 60 s entre leituras (não é o total: download grande
/// segue vivo enquanto chegar byte) e no máximo 3 redirects — os destinos são constantes. Quem
/// precisa de teto total ainda passa `.timeout()` por requisição.
pub fn fixed_endpoint_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| build_fixed_client(std::time::Duration::from_secs(10), std::time::Duration::from_secs(60)))
}

fn build_fixed_client(connect: std::time::Duration, read: std::time::Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(connect)
        .read_timeout(read)
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .expect("cliente http de endpoint fixo")
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![]
}
