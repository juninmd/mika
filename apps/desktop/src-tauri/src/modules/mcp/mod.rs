//! Cliente MCP em Rust, sobre a crate `rmcp` (papel de cliente, transporte stdio por processo
//! filho — `client`, `transport-child-process`, `which-command`, ver `Cargo.toml`).
//!
//! Hoje `apps/desktop/src/features/mcp/mcp-client.ts` sobe servidores MCP com `execFile` +
//! `Experimental_StdioMCPTransport`. Isso é Node puro e some com o sidecar.
//!
//! **É a única extensibilidade de terceiros que o produto tem de verdade** — o usuário edita
//! `mcp.json` e aponta para servidores externos. Diferente dos 12 "plugins" (todos do bundle), isto
//! precisa continuar funcionando depois da migração, e é por isso que tem módulo próprio.
//!
//! Servidor MCP é processo do SO separado falando JSON-RPC por stdio: não é sidecar Node, é a
//! arquitetura do protocolo. Manter isso não viola a meta de apagar o sidecar.
//!
//! ## Layout
//!
//! - [`config`] — `mcp.json` (`readServers`/`sanitize`/`ServerSpec`).
//! - [`client`] — conecta cada servidor (`rmcp::transport::TokioChildProcess`), com o MESMO prazo e
//!   as MESMAS regras de colisão de nome do TS.
//! - [`register`] — registra as tools descobertas no `ToolRegistry` nativo
//!   (`crate::modules::tools::registry()`), via `register_destructive` — **leia o cabeçalho daquele
//!   arquivo antes de mexer aqui**: toda tool MCP passa pelo `ApprovalGateService`
//!   (`modules/tools/approval.rs`) antes de despachar, mesma política `destructive: true`
//!   incondicional que o TS aplicava a QUALQUER tool descoberta (`mcp/index.ts::connectAndRegister`,
//!   nunca lia `readOnlyHint`/anotação nenhuma do servidor). Sem handler instalado, nega por padrão;
//!   com o handler real (`tools::install_approval_handler`, `modules/mod.rs`), aprovado despacha de
//!   verdade contra o servidor MCP.
//!
//! ## Canais `mika:mcp-*` — gestão de servidores pela UI
//!
//! [`servers`] reivindica `mika:mcp-servers-list`/`-server-save`/`-server-remove`/`-server-test`/
//! `mika:mcp-open-config-dir`: listar (com status vivo e tools), criar/editar/remover (gravando o
//! `mcp.json` atomicamente via [`editor`] e reiniciando SÓ aquele servidor via [`lifecycle`]),
//! testar sem gravar e abrir a pasta. As tools descobertas continuam chegando a `mika:tools-list`
//! pelo catálogo compartilhado (`register.rs`). Mudança de estado emite
//! `mika:event:mcp:servers-changed` ([`lifecycle::SERVERS_CHANGED_EVENT`]).
//!
//! ## Ciclo de vida
//!
//! Port de `mcp/index.ts` (`activate`/`deactivate`/`connectAndRegister`/`disconnectAndUnregister`/
//! reação a `onChange(PrivacyOfflineMode)`):
//!
//! - [`boot`] — chamada síncrona de UMA linha para dentro de `setup()` (não é async): dispara
//!   [`connect_and_register`] em background. Equivale a `activate()`.
//! - [`disconnect_all`] — fecha tudo. Chamar no shutdown do app (equivale a `deactivate()`).
//! - [`on_privacy_offline_mode_changed`] — modo offline ligando DERRUBA conexão viva (MCP é
//!   processo externo com rede própria, fora do funil de rede da app); desligando, tenta reconectar.
//! - [`on_mcp_enabled_changed`] — equivalente ao `activate`/`deactivate` que o HOST de plugin do TS
//!   roda quando `enabledConfigPath` (`mcp.enabled`) muda. Isso não mora dentro de `mcp/index.ts` no
//!   TS — é o runtime de plugin, fora do escopo deste arquivo — mas Rust ainda não tem esse host
//!   (`modules/plugins` vazio), então esta função é o que ele chamaria.
//!
//! **As duas reações de config estão ligadas** por `watch_config`, chamada de [`boot`]:
//! `ConfigStore::watch` (`src/config/store.rs`) notifica depois de cada `set` bem-sucedido, e o
//! observador manda cada handler para o runtime do Tauri em vez de bloquear a thread do `set` —
//! `disconnect_all` espera o processo do servidor MCP morrer, e segurar o `set` por isso congelaria
//! a tela de settings. [`connect_and_register`] é idempotente (no-op se já há conexão viva) e
//! resolve corrida contra [`disconnect_all`] por geração (mesmo `activationGeneration` do TS), então
//! as duas funções podem ser chamadas sem coordenação adicional.

pub mod client;
pub mod config;
pub mod editor;
pub mod env_secrets;
pub mod feature;
pub mod lifecycle;
mod plugin_test;
pub mod register;
pub mod servers;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::AppHandle;

use crate::native::NativeModule;

/// Conexões vivas deste processo. Precisam ficar de pé aqui, não na pilha de quem chama
/// `connect_and_register`: dropar um `RunningService`/`TokioChildProcess` mata o processo filho
/// (ver `client.rs`). Mesmo padrão do `OnceLock<ToolRegistry>` de `modules/tools/registry.rs`.
static CONNECTIONS: OnceLock<Mutex<Vec<client::ConnectedServer>>> = OnceLock::new();

fn connections() -> &'static Mutex<Vec<client::ConnectedServer>> {
    CONNECTIONS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Port de `activationGeneration` (`mcp/index.ts`): avança a cada `connect_and_register`/
/// `disconnect_all`. Uma rodada de conexão que termina depois de o contador ter avançado (porque um
/// `disconnect_all` ou outra rodada rodou NO MEIO do `await`) descarta o próprio resultado — ver
/// [`was_superseded_during_connect`].
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Edit epoch per server name, bumped by every UI save/remove/toggle of that name
/// ([`invalidate_server`]). A connect round snapshots the epochs of the servers it starts
/// ([`edit_epochs`]); a name that moved while it was connecting was edited under it, and the round
/// drops that server ([`split_stale`]) instead of starting a spec the user already changed, disabled
/// or removed. Per name, not [`GENERATION`]: bumping that would also throw away every other server
/// the round had connected.
fn edits() -> &'static Mutex<BTreeMap<String, u64>> {
    static EDITS: OnceLock<Mutex<BTreeMap<String, u64>>> = OnceLock::new();
    EDITS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// The UI changed `name`: any connect round already in flight for it is stale.
pub(crate) fn invalidate_server(name: &str) {
    *edits().lock().unwrap_or_else(|e| e.into_inner()).entry(name.to_string()).or_default() += 1;
}

pub(crate) fn edit_epochs<'a>(names: impl Iterator<Item = &'a str>) -> BTreeMap<String, u64> {
    let edits = edits().lock().unwrap_or_else(|e| e.into_inner());
    names.map(|name| (name.to_string(), edits.get(name).copied().unwrap_or(0))).collect()
}

/// `true` when `name` was edited in the UI after `snapshot` was taken.
pub(crate) fn edited_since(name: &str, snapshot: &BTreeMap<String, u64>) -> bool {
    edits().lock().unwrap_or_else(|e| e.into_inner()).get(name).copied().unwrap_or(0) != snapshot.get(name).copied().unwrap_or(0)
}

/// `(stale, fresh)`: items whose server was edited since `snapshot` was taken.
pub(crate) fn split_stale<T>(items: Vec<T>, name: impl Fn(&T) -> &str, snapshot: &BTreeMap<String, u64>) -> (Vec<T>, Vec<T>) {
    items.into_iter().partition(|item| edited_since(name(item), snapshot))
}

/// Resultado observável de uma rodada de `connect_and_register` — para log/telemetria de quem
/// chamar, nenhuma lógica interna decide em cima disto.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectOutcome {
    AlreadyConnected,
    Disabled,
    OfflineModeBlocked,
    NoServersDeclared,
    Connected { servers: usize, tools: usize },
    AllServersFailed { attempted: usize },
    SupersededDuringConnect,
}

/// Port de `myGeneration !== activationGeneration`: verdadeiro quando outra rodada de
/// `connect_and_register`/`disconnect_all` avançou o contador DURANTE o `await` de conexão desta
/// rodada, invalidando o resultado que acabou de chegar. Puro — testável sem AppHandle nem conexão
/// real.
fn was_superseded_during_connect(my_generation: u64, current_generation: u64) -> bool {
    my_generation != current_generation
}

/// Fachada real de `connectAndRegister` (`mcp/index.ts`): lê `mcp.json`, checa `mcp.enabled`, passa
/// pelo chokepoint de modo offline — servidor MCP é processo externo com rede própria, fora do
/// funil de `createLoggedFetch`/`assert_network_allowed`, por isso o gate aqui é
/// `privacy::is_offline_mode_enabled` direto, não `assert_network_allowed` (perguntas diferentes,
/// ver `modules/privacy.rs`) — conecta cada servidor e registra as tools descobertas.
///
/// Idempotente: uma segunda chamada enquanto já há conexão viva é um no-op (`AlreadyConnected`),
/// mesmo guard `if (connected.length > 0) return` do TS — importa porque tanto [`boot`] quanto o
/// futuro observador de `mcp.enabled`/modo offline podem chamar isto sem coordenar entre si.
pub async fn connect_and_register(app: &AppHandle) -> Result<ConnectOutcome, String> {
    if !connections().lock().unwrap_or_else(|e| e.into_inner()).is_empty() {
        return Ok(ConnectOutcome::AlreadyConnected);
    }

    let store = crate::state::config_store(app)?;

    if !store.get("mcp.enabled").and_then(|value| value.as_bool()).unwrap_or(false) {
        return Ok(ConnectOutcome::Disabled);
    }

    if crate::modules::privacy::is_offline_mode_enabled(&store) {
        log::warn!(
            "MCP não conecta com o modo offline ligado — servidor MCP é processo externo com rede própria (configPath=privacy.offlineMode)"
        );
        return Ok(ConnectOutcome::OfflineModeBlocked);
    }

    let data_dir = config::mcp_data_dir(app)?;
    if let Err(error) = env_secrets::seal_file_in_place(&data_dir, &store) {
        log::warn!("mcp: mcp.json não migrou para env cifrado: {error}");
    }
    let mut servers = config::read_servers(&data_dir);
    env_secrets::open_specs(&mut servers, &store);
    servers.retain(|_, spec| !spec.disabled);
    if servers.is_empty() {
        return Ok(ConnectOutcome::NoServersDeclared);
    }
    let attempted = servers.len();

    let timeout_seconds = store.get("mcp.timeoutSeconds").and_then(|value| value.as_u64()).unwrap_or(20);

    // Snapshot ANTES do `await`: o `myGeneration = ++activationGeneration` do TS.
    let my_generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let epochs = edit_epochs(servers.keys().map(String::as_str));

    let (connected, failed) = client::connect_all(servers, Duration::from_secs(timeout_seconds)).await;

    if was_superseded_during_connect(my_generation, GENERATION.load(Ordering::SeqCst)) {
        log::info!("MCP desligou/reativou durante a conexão — fechando os servidores que subiram");
        client::close_all(connected).await;
        return Ok(ConnectOutcome::SupersededDuringConnect);
    }

    // Removed/disabled/edited in the UI during the `await`: the UI path already stopped (and, if
    // still enabled, restarted) it with the new spec — this round's copy must not come back.
    let (edited, connected) = split_stale(connected, |server| server.name.as_str(), &epochs);
    let (_, failed) = split_stale(failed, |(name, _)| name.as_str(), &epochs);
    client::close_all(edited).await;

    lifecycle::replace_failures(failed);
    if connected.is_empty() {
        lifecycle::notify_changed(app);
        return Ok(ConnectOutcome::AllServersFailed { attempted });
    }

    // `lifecycle::start_server` (salvar pela UI) não avança `GENERATION`: um servidor salvo durante
    // o `await` acima já pode estar vivo com o spec NOVO — o desta rodada (spec lido antes) sai.
    let (outcome, duplicates) = {
        let mut live = connections().lock().unwrap_or_else(|e| e.into_inner());
        let (duplicates, fresh): (Vec<_>, Vec<_>) =
            connected.into_iter().partition(|server| live.iter().any(|other| other.name == server.name));
        // MESMO prazo usado para conectar — ver o comentário de `register_tools` sobre por que
        // `tools/call` precisa de um deadline que o TS nunca teve.
        let tools = register::register_tools(&fresh, Duration::from_secs(timeout_seconds));
        let outcome = ConnectOutcome::Connected { servers: fresh.len(), tools };
        live.extend(fresh);
        (outcome, duplicates)
    };
    client::close_all(duplicates).await;

    lifecycle::notify_changed(app);
    Ok(outcome)
}

/// Fecha toda conexão viva — port de `closeAll` MAIS o `activationGeneration++` de
/// `disconnectAndUnregister`. Chamado no shutdown do app e nas duas reações de config abaixo.
///
/// O `fetch_add` roda SEMPRE, mesmo sem nada conectado — é o que invalida um `connect_and_register`
/// que esteja em voo (no meio do próprio `await`) neste exato instante; o TS garante o mesmo
/// incrementando `activationGeneration` ANTES do `if (connected.length === 0) return`.
///
/// As tools de cada servidor saem do catálogo (`register::unregister_tools`) antes de o processo
/// fechar — antes disso ficavam em `mika:tools-list` presas a um `Peer` de processo morto.
pub async fn disconnect_all() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let servers = std::mem::take(&mut *connections().lock().unwrap_or_else(|e| e.into_inner()));
    lifecycle::replace_failures(Vec::new());
    register::unregister_tools(&servers);
    client::close_all(servers).await;
}

/// Porta de entrada síncrona para `setup()` do Tauri, que não é async: dispara
/// [`connect_and_register`] em background e loga o resultado. Equivale a `activate()` do TS.
pub fn boot(app: &AppHandle) {
    watch_config(app);
    plugin_test::register_from_boot(app);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match connect_and_register(&app).await {
            Ok(outcome) => log::info!("mcp boot: {outcome:?}"),
            Err(error) => log::error!("mcp boot failed: {error}"),
        }
    });
}

/// Liga [`on_privacy_offline_mode_changed`] e [`on_mcp_enabled_changed`] ao `ConfigStore`.
///
/// Registrado ANTES do `spawn` do boot, não dentro dele: `tokio::spawn` só AGENDA, e uma mudança de
/// config feita entre o agendamento e a primeira execução da task passaria despercebida — a mesma
/// corrida que o barramento de eventos já documenta.
///
/// O observador roda na thread do `set` (um invoke de IPC) e os dois handlers são `async`, então
/// cada um vai para o runtime do Tauri. Não bloquear aqui é obrigatório: `disconnect_all` espera o
/// processo do servidor MCP morrer, e segurar o `set` por isso congelaria a tela de settings.
fn watch_config(app: &AppHandle) {
    let Ok(store) = crate::state::config_store(app) else {
        log::error!("mcp: sem ConfigStore — mudanças de mcp.enabled/offlineMode não terão efeito até o próximo boot");
        return;
    };

    let app = app.clone();
    store.watch(Box::new(move |path, value| {
        let Some(enabled) = value.as_bool() else { return };
        let app = app.clone();
        match path {
            "privacy.offlineMode" => {
                tauri::async_runtime::spawn(async move {
                    on_privacy_offline_mode_changed(&app, enabled).await;
                });
            }
            "mcp.enabled" => {
                tauri::async_runtime::spawn(async move {
                    on_mcp_enabled_changed(&app, enabled).await;
                });
            }
            _ => {}
        }
    }));
}

/// Port do handler `ctx.config.onChange(ConfigKey.PrivacyOfflineMode, …)` de `mcp/index.ts`: modo
/// offline LIGANDO com servidor vivo tem de DERRUBAR a conexão, não só parar de abrir novas — MCP é
/// processo externo com rede própria, fora do funil de rede da app. Desligando, tenta reconectar.
///
/// Ligada ao `ConfigStore` por [`watch_config`], chamada do boot.
pub async fn on_privacy_offline_mode_changed(app: &AppHandle, offline_mode_enabled: bool) {
    if offline_mode_enabled {
        log::info!("mcp: modo offline ligado — desconectando servidores MCP vivos");
        disconnect_all().await;
        lifecycle::notify_changed(app);
        return;
    }

    if let Err(error) = connect_and_register(app).await {
        log::error!("mcp: não reconectou depois do modo offline desligar: {error}");
    }
}

/// Equivalente ao `activate`/`deactivate` que o HOST de plugin do TS roda quando
/// `enabledConfigPath` (`mcp.enabled`) muda — no TS isso não mora em `mcp/index.ts`, é o runtime de
/// plugin (`definePlugin`) por trás dele. Rust ainda não tem esse host (`modules/plugins` vazio),
/// então quem a chama é o observador de config de [`watch_config`], direto.
pub async fn on_mcp_enabled_changed(app: &AppHandle, enabled: bool) {
    if enabled {
        if let Err(error) = connect_and_register(app).await {
            log::error!("mcp: não conectou depois de mcp.enabled ligar: {error}");
        }
        return;
    }

    log::info!("mcp: mcp.enabled desligado — desconectando servidores MCP vivos");
    disconnect_all().await;
    lifecycle::notify_changed(app);
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(servers::McpServersModule)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_the_server_management_channels() {
        let claimed: Vec<&str> = modules().iter().flat_map(|m| m.channels().iter().copied()).collect();
        assert!(claimed.contains(&servers::CH_LIST) && claimed.contains(&servers::CH_SAVE));
    }

    #[test]
    fn connections_registry_is_a_stable_singleton() {
        assert!(std::ptr::eq(connections(), connections()));
    }

    // --- was_superseded_during_connect -----------------------------------------------------------

    #[test]
    fn same_generation_is_not_superseded() {
        assert!(!was_superseded_during_connect(3, 3));
    }

    /// NEGATIVO — o caso que a checagem existe para pegar: `disconnect_all` (ou outra rodada de
    /// connect) avançou o contador enquanto a rodada anterior ainda estava em voo.
    #[test]
    fn advanced_generation_is_superseded() {
        assert!(was_superseded_during_connect(3, 4));
    }

    // --- disconnect_all ----------------------------------------------------------------------------

    /// O contador avança mesmo sem nada conectado — é o que invalida um `connect_and_register` que
    /// esteja no meio do próprio `await` neste instante.
    /// A server removed/disabled/edited in the UI while a boot or re-enable round is connecting it
    /// must not come back when that round lands: the round drops exactly the edited names and keeps
    /// the untouched ones (bumping the global generation would throw away every other server too).
    #[test]
    fn a_server_edited_during_a_connect_round_is_dropped_from_that_round_only() {
        let names = ["mcp-race-edited".to_string(), "mcp-race-untouched".to_string()];
        let snapshot = edit_epochs(names.iter().map(String::as_str));
        invalidate_server("mcp-race-edited");

        let (stale, fresh) = split_stale(names.to_vec(), |name| name.as_str(), &snapshot);
        assert_eq!(stale, ["mcp-race-edited"]);
        assert_eq!(fresh, ["mcp-race-untouched"]);
    }

    #[tokio::test]
    async fn disconnect_all_bumps_generation_even_with_nothing_connected() {
        let before = GENERATION.load(Ordering::SeqCst);
        disconnect_all().await;
        assert!(GENERATION.load(Ordering::SeqCst) > before);
    }
}
