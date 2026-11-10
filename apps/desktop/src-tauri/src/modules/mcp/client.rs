//! Conecta a servidores MCP declarados em `mcp.json` e descobre as tools de cada um — port de
//! `apps/desktop/src/features/mcp/mcp-client.ts` (`connect`/`connectAll`/`closeAll`), sobre a crate
//! `rmcp` (papel de cliente + transporte stdio por processo filho).
//!
//! `rmcp::transport::which_command` substitui `isRunnable` (`where`/`which` via `execFile`):
//! resolve o executável pelo PATH do SO e falha ANTES de qualquer spawn se ele não existir — mesmo
//! efeito da checagem do TS, sem abrir um processo cujo destino já se sabe que vai falhar, e
//! resolve shims `.cmd` do Windows (`npx.cmd`) que `tokio::process::Command` sozinho não acha.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::future::Future;
use std::time::Duration;

use rmcp::model::Tool;
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, IntoTransport, TokioChildProcess, which_command};
use rmcp::{RoleClient, ServiceExt};

use super::config::{ServerSpec, sanitize};

/// Um servidor MCP conectado: a conexão viva (segurar `service` mantém o processo filho de pé —
/// dropá-lo o mata, ver `TokioChildProcess`/`ChildWithCleanup` na própria crate) mais o catálogo de
/// tools já com a chave prefixada.
#[derive(Debug)]
pub struct ConnectedServer {
    pub name: String,
    pub service: RunningService<RoleClient, ()>,
    pub tools: Vec<DiscoveredTool>,
}

/// Uma tool descoberta num servidor: `key` é o que `mika:tools-list` expõe (`mcp_<servidor>_<tool>`,
/// mesma fórmula de `connect()` no TS), `upstream_name` é o nome ORIGINAL que o servidor espera de
/// volta em `tools/call` — os dois divergem depois do `sanitize`.
#[derive(Debug)]
pub struct DiscoveredTool {
    pub key: String,
    pub upstream_name: String,
    pub tool: Tool,
}

/// Aplica um prazo a um future e traduz o estouro na MESMA mensagem do TS (`"no answer in {n}s"`).
/// Separado de `connect_one` para ser testável sem spawnar processo nenhum (ver os testes com
/// `std::future::pending`). `pub(super)`: `register.rs` reusa para aplicar o MESMO prazo a
/// `tools/call` — ver o comentário de `register_tools`.
pub(super) async fn with_deadline<T>(fut: impl Future<Output = Result<T, String>>, timeout: Duration) -> Result<T, String> {
    match tokio::time::timeout(timeout, fut).await {
        Ok(result) => result,
        Err(_elapsed) => Err(format!("no answer in {}s", timeout.as_secs())),
    }
}

/// Handshake + descoberta de tools sobre um transporte já aberto. Separado de `connect_one` para
/// ser exercitado, em teste, sobre um par `tokio::io::duplex` com um servidor MCP fake do outro
/// lado — sem spawnar processo nenhum (ver os testes deste arquivo).
async fn handshake_and_discover<T, E, A>(
    transport: T,
    timeout: Duration,
) -> Result<(RunningService<RoleClient, ()>, Vec<Tool>), String>
where
    T: IntoTransport<RoleClient, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    with_deadline(
        async move {
            let service = ().serve(transport).await.map_err(|error| error.to_string())?;
            let tools = service.peer().list_all_tools().await.map_err(|error| error.to_string())?;
            Ok((service, tools))
        },
        timeout,
    )
    .await
}

/// Variáveis que o servidor MCP herda do app — o mínimo para `npx`/`node`/`python` acharem o
/// executável, o HOME e a pasta temporária; a sessão gráfica (X11/Wayland/DBus: sem elas um servidor
/// que abre janela ou usa o chaveiro morre); a CA corporativa (`SSL_CERT_*`, `*_CA_BUNDLE`: sem ela
/// nenhum TLS valida); e o ambiente-base do Windows. Uma variável que o SO não define simplesmente
/// não existe para herdar. Comparação sem diferenciar caixa (no Windows `Path` e `PATH` são a mesma
/// variável). Todo o resto — `MIKA_MASTER_KEY`, `*_API_KEY`, tokens de nuvem e os `*_PROXY` (a URL
/// pode levar usuário:senha; o usuário os declara no `env` do servidor se quiser) — NÃO passa: o
/// servidor é código de terceiro, e herdar o ambiente inteiro entregaria a ele a chave que decifra
/// o `config.json`.
const INHERITED_ENV: &[&str] = &[
    "PATH", "HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "TEMP", "TMP", "TMPDIR", "SYSTEMROOT", "WINDIR",
    "COMSPEC", "PATHEXT", "LANG", "LANGUAGE", "LC_ALL", "NODE_PATH", "PROGRAMDATA", "PROGRAMFILES",
    // Linux/BSD: sessão gráfica e identidade.
    "DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "DBUS_SESSION_BUS_ADDRESS", "USER", "LOGNAME", "SHELL",
    // CA corporativa (todos os SOs).
    "SSL_CERT_FILE", "SSL_CERT_DIR", "REQUESTS_CA_BUNDLE", "NODE_EXTRA_CA_CERTS", "CURL_CA_BUNDLE",
    // Windows.
    "SYSTEMDRIVE", "HOMEDRIVE", "HOMEPATH", "PROCESSOR_ARCHITECTURE", "PROCESSOR_ARCHITEW6432", "NUMBER_OF_PROCESSORS",
    "USERNAME", "ALLUSERSPROFILE", "PROGRAMW6432", "COMMONPROGRAMFILES", "COMMONPROGRAMFILES(X86)",
    "COMMONPROGRAMW6432", "PROGRAMFILES(X86)",
];
/// Prefixos aceitos (`XDG_*` dirs, `LC_*` locale, `npm_config_*` do `npx`) — menos o que cheira a segredo.
const INHERITED_ENV_PREFIXES: &[&str] = &["xdg_", "lc_", "npm_config_"];
const SECRET_HINTS: &[&str] = &["token", "auth", "password", "secret", "key", "cert_pass"];

fn inherits(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if INHERITED_ENV.iter().any(|allowed| allowed.eq_ignore_ascii_case(&lower)) {
        return true;
    }
    INHERITED_ENV_PREFIXES.iter().any(|prefix| lower.starts_with(prefix)) && !SECRET_HINTS.iter().any(|hint| lower.contains(hint))
}

/// O ambiente do filho: a allowlist acima do ambiente do app, e POR CIMA o `env` que o usuário
/// declarou para o servidor (esse sim pode ser segredo — é o que ele escolheu entregar).
fn child_env(parent: impl IntoIterator<Item = (String, String)>, declared: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut env: BTreeMap<String, String> = parent
        .into_iter()
        // `Path` herdado (Windows) não pode coexistir com um `PATH` declarado: `Command` compara os
        // nomes sem caixa e deixaria o herdado vencer.
        .filter(|(name, _)| inherits(name) && !declared.keys().any(|own| own.eq_ignore_ascii_case(name)))
        .collect();
    env.extend(declared.iter().map(|(name, value)| (name.clone(), value.clone())));
    env
}

/// Resolve o comando pelo PATH (equivalente a `isRunnable`, ver o cabeçalho do arquivo) e aplica
/// args/env/cwd. NUNCA loga `spec` inteiro: `env` pode carregar segredo (chave de API repassada por
/// variável de ambiente ao servidor). `env_clear` + [`child_env`]: o filho não herda o ambiente do app.
fn build_command(spec: &ServerSpec) -> std::io::Result<tokio::process::Command> {
    let args = spec.args.clone();
    let env = child_env(std::env::vars(), &spec.env);
    let cwd = spec.cwd.clone();
    Ok(which_command(&spec.command)?.configure(|cmd| {
        cmd.args(&args);
        cmd.env_clear();
        cmd.envs(&env);
        if let Some(cwd) = &cwd {
            cmd.current_dir(cwd);
        }
        cmd.kill_on_drop(true);
    }))
}

/// Port de `connect()`: spawna o processo, faz o handshake e descobre as tools sob o MESMO prazo
/// que o TS aplica às duas etapas somadas (`timeoutMs`). Colisão de nome de tool depois do
/// `sanitize`, dentro do MESMO servidor, mantém a primeira e descarta a segunda com log — mesma
/// política de `connect()`.
pub(super) async fn connect_one(name: &str, spec: &ServerSpec, timeout: Duration) -> Result<ConnectedServer, String> {
    let command = build_command(spec).map_err(|error| format!("command '{}' not on PATH: {error}", spec.command))?;
    let transport = TokioChildProcess::new(command).map_err(|error| format!("spawn failed: {error}"))?;

    let (service, discovered) = handshake_and_discover(transport, timeout).await?;

    let prefix = sanitize(name);
    let mut seen = BTreeSet::new();
    let mut tools = Vec::new();
    for tool in discovered {
        let key = format!("mcp_{prefix}_{}", sanitize(&tool.name));
        if !seen.insert(key.clone()) {
            log::error!("MCP tool name collision after sanitize — keeping the first, dropping this one (server={name}, key={key})");
            continue;
        }
        tools.push(DiscoveredTool { key, upstream_name: tool.name.to_string(), tool });
    }

    Ok(ConnectedServer { name: name.to_string(), service, tools })
}

/// Port de `rejectSanitizedNameCollisions`: dois nomes de SERVIDOR que colidem depois do
/// `sanitize` — o segundo é recusado, com log, antes de qualquer tentativa de conexão.
fn reject_sanitized_name_collisions(servers: BTreeMap<String, ServerSpec>) -> Vec<(String, ServerSpec)> {
    let mut seen_by_sanitized_name: HashMap<String, String> = HashMap::new();
    let mut accepted = Vec::new();
    for (name, spec) in servers {
        let sanitized_name = sanitize(&name);
        if let Some(first_name) = seen_by_sanitized_name.get(&sanitized_name) {
            log::error!(
                "MCP server name collision after sanitize — refusing to register the second server (first={first_name}, colliding={name}, sanitizedName={sanitized_name})"
            );
            continue;
        }
        seen_by_sanitized_name.insert(sanitized_name, name.clone());
        accepted.push((name, spec));
    }
    accepted
}

/// Port de `connectAll`: conecta todos os servidores em paralelo (mesmo `Promise.all` do TS) —
/// falha de um não derruba os outros. Devolve os conectados e, à parte, `(servidor, erro)` de quem
/// falhou: é o que `mika:mcp-servers-list` mostra como status `error` (`servers.rs`).
pub async fn connect_all(servers: BTreeMap<String, ServerSpec>, timeout: Duration) -> (Vec<ConnectedServer>, Vec<(String, String)>) {
    let accepted = reject_sanitized_name_collisions(servers);
    let results = futures::future::join_all(accepted.into_iter().map(|(name, spec)| async move {
        let result = connect_one(&name, &spec, timeout).await;
        match &result {
            Ok(connected) => log::info!("MCP server connected (server={}, tools={})", connected.name, connected.tools.len()),
            Err(error) => log::error!("MCP server failed (server={name}): {error}"),
        }
        result.map_err(|error| (name, error))
    }))
    .await;
    let mut connected = Vec::new();
    let mut failed = Vec::new();
    for result in results {
        match result {
            Ok(server) => connected.push(server),
            Err(failure) => failed.push(failure),
        }
    }
    (connected, failed)
}

/// Port de `closeAll`: fecha todo mundo, mesmo se um travar — `RunningService::close` já aplica o
/// próprio desligamento gracioso; a falha de um close não pode derrubar os outros.
pub async fn close_all(servers: Vec<ConnectedServer>) {
    futures::future::join_all(servers.into_iter().map(|mut connected| async move {
        let _ = connected.service.close().await;
    }))
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::ErrorData as McpError;
    use rmcp::model::{CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams};
    use rmcp::service::{RequestContext, RoleServer};
    use serde_json::json;
    use std::sync::Arc;

    // --- with_deadline -----------------------------------------------------------------------------

    #[tokio::test]
    async fn resolves_promptly_when_the_inner_future_finishes_before_the_deadline() {
        let result = with_deadline(async { Ok::<_, String>(42) }, Duration::from_secs(5)).await;
        assert_eq!(result, Ok(42));
    }

    /// NEGATIVO — o caminho que este helper existe para cobrir: um future que nunca resolve (o
    /// handshake que trava) tem de estourar com a MESMA mensagem do TS, não travar o chamador.
    #[tokio::test]
    async fn times_out_with_the_same_message_shape_as_the_typescript_client() {
        let never: std::future::Pending<Result<(), String>> = std::future::pending();
        let result = with_deadline(never, Duration::from_millis(20)).await;
        assert_eq!(result, Err("no answer in 0s".to_string()));
    }

    // --- reject_sanitized_name_collisions -----------------------------------------------------------

    #[test]
    fn keeps_the_first_server_and_drops_the_colliding_second_one() {
        let mut servers = BTreeMap::new();
        servers.insert("fs-a".to_string(), ServerSpec { command: "npx".into(), ..Default::default() });
        servers.insert("fs.a".to_string(), ServerSpec { command: "node".into(), ..Default::default() });

        let accepted = reject_sanitized_name_collisions(servers);

        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].0, "fs-a");
    }

    #[test]
    fn accepts_every_server_when_no_name_collides_after_sanitize() {
        let mut servers = BTreeMap::new();
        servers.insert("fs".to_string(), ServerSpec { command: "npx".into(), ..Default::default() });
        servers.insert("git".to_string(), ServerSpec { command: "npx".into(), ..Default::default() });

        assert_eq!(reject_sanitized_name_collisions(servers).len(), 2);
    }

    // --- ambiente do filho: allowlist, nunca o ambiente inteiro do app ------------------------------

    fn parent_env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn the_child_never_inherits_secrets_from_the_app_environment() {
        let env = child_env(
            parent_env(&[
                ("PATH", "/usr/bin"),
                ("Home", "/home/u"),
                ("MIKA_MASTER_KEY", "deadbeef"),
                ("OPENAI_API_KEY", "sk-1"),
                ("GROQ_API_KEY", "gsk"),
                ("AWS_SECRET_ACCESS_KEY", "aws"),
                ("GITHUB_TOKEN", "ghp"),
                ("npm_config_registry", "https://registry.npmjs.org"),
                ("npm_config__authToken", "npm-secret"),
                ("XDG_CONFIG_HOME", "/home/u/.config"),
                ("NODE_OPTIONS", "--require /tmp/evil.js"),
                ("LD_PRELOAD", "/tmp/evil.so"),
            ]),
            &BTreeMap::new(),
        );
        let names: Vec<&str> = env.keys().map(String::as_str).collect();
        assert_eq!(names, vec!["Home", "PATH", "XDG_CONFIG_HOME", "npm_config_registry"]);
    }

    #[test]
    fn the_declared_env_is_added_and_wins_over_the_inherited_one() {
        let declared = BTreeMap::from([("API_TOKEN".to_string(), "mine".to_string()), ("PATH".to_string(), "/opt/bin".to_string())]);
        let env = child_env(parent_env(&[("PATH", "/usr/bin")]), &declared);
        assert_eq!(env["API_TOKEN"], "mine");
        assert_eq!(env["PATH"], "/opt/bin");
    }

    /// No Windows o ambiente herdado chega como `Path` e o usuário declara `PATH`: o `BTreeMap` (sensível
    /// a caixa) guardaria as duas, `Command` as compara sem caixa e a herdada venceria a declarada.
    #[test]
    fn a_declared_path_replaces_the_inherited_one_whatever_its_case() {
        let declared = BTreeMap::from([("PATH".to_string(), "/opt/bin".to_string())]);
        let env = child_env(parent_env(&[("Path", "C:\\Windows"), ("HOME", "/h")]), &declared);
        assert_eq!(env.iter().filter(|(name, _)| name.eq_ignore_ascii_case("path")).collect::<Vec<_>>(), vec![(&"PATH".to_string(), &"/opt/bin".to_string())]);
        assert_eq!(env["HOME"], "/h");
    }

    /// Sessão gráfica, CA corporativa e ambiente do Windows: sem eles o servidor sobe mas não abre
    /// janela/não valida TLS/não acha `ProgramFiles`. Proxies FICAM de fora (podem ter credencial na URL).
    #[test]
    fn the_session_certificate_and_windows_basics_are_inherited_but_proxies_are_not() {
        let inherited = [
            "DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "DBUS_SESSION_BUS_ADDRESS", "USER", "LOGNAME", "SHELL", "SSL_CERT_FILE",
            "SSL_CERT_DIR", "REQUESTS_CA_BUNDLE", "NODE_EXTRA_CA_CERTS", "CURL_CA_BUNDLE", "SYSTEMDRIVE", "HOMEDRIVE", "HOMEPATH",
            "PROCESSOR_ARCHITECTURE", "PROCESSOR_ARCHITEW6432", "NUMBER_OF_PROCESSORS", "USERNAME", "ALLUSERSPROFILE", "PROGRAMW6432",
            "COMMONPROGRAMFILES", "COMMONPROGRAMFILES(X86)", "COMMONPROGRAMW6432", "PROGRAMFILES(X86)",
        ];
        let denied = ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "SSH_AUTH_SOCK", "GPG_AGENT_INFO"];
        let parent: Vec<(String, String)> = inherited.iter().chain(&denied).map(|name| (name.to_string(), "x".into())).collect();
        let env = child_env(parent, &BTreeMap::new());
        for name in inherited {
            assert!(env.contains_key(name), "{name} tinha de ser herdada");
        }
        for name in denied {
            assert!(!env.contains_key(name), "{name} não pode ser herdada");
        }
    }

    /// De ponta a ponta no processo de verdade: `env` imprime o que o filho recebeu. Variável com
    /// nome único (não mexe em nada que outro teste leia) e só em Unix, onde `env` existe.
    #[cfg(unix)]
    #[test]
    fn a_spawned_server_does_not_see_the_apps_secrets() {
        std::env::set_var("MIKA_TEST_LEAK_API_KEY", "must-not-leak");
        let spec = ServerSpec {
            command: "env".into(),
            env: BTreeMap::from([("MIKA_TEST_DECLARED".to_string(), "declared-ok".to_string())]),
            ..Default::default()
        };
        let mut command = build_command(&spec).expect("env está no PATH");
        let output = command.as_std_mut().output().expect("env roda");
        let printed = String::from_utf8_lossy(&output.stdout);
        assert!(printed.contains("MIKA_TEST_DECLARED=declared-ok"), "{printed}");
        assert!(printed.contains("PATH="), "{printed}");
        assert!(!printed.contains("MIKA_TEST_LEAK_API_KEY"), "o filho herdou o ambiente do app: {printed}");
    }

    // --- connect_one: falha sem spawnar nada ---------------------------------------------------------

    /// Portável: o nome do binário simplesmente não existe em PATH nenhum. Prova que
    /// `which_command` barra ANTES do spawn, com um erro nomeado — nunca panic.
    #[tokio::test]
    async fn command_not_on_path_is_a_named_error_not_a_panic() {
        let spec = ServerSpec { command: "this-binary-does-not-exist-mika-mcp-test".into(), ..Default::default() };
        let error = connect_one("ghost", &spec, Duration::from_secs(5)).await.unwrap_err();
        assert!(error.contains("not on PATH"), "got: {error}");
    }

    // --- round trip real, sobre um par tokio::io::duplex (sem spawnar processo) --------------------

    /// Servidor MCP fake, em processo, só para o teste: uma tool ("echo") que devolve os próprios
    /// argumentos como `structured_content`. Prova o handshake, a descoberta e o `call_tool` de
    /// ponta a ponta sem depender de nenhum binário externo — o que o spawn de verdade faz depois é
    /// coberto pelos próprios testes da crate `rmcp` (`TokioChildProcess`) e pelo teste acima
    /// (`which_command`).
    #[derive(Clone)]
    struct FakeMcpServer;

    impl rmcp::ServerHandler for FakeMcpServer {
        async fn list_tools(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListToolsResult, McpError> {
            let schema = json!({ "type": "object", "properties": { "text": { "type": "string" } } });
            let tool = Tool::new("echo", "echoes the input back", Arc::new(schema.as_object().unwrap().clone()));
            Ok(ListToolsResult::with_all_items(vec![tool]))
        }

        async fn call_tool(
            &self,
            request: CallToolRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> Result<rmcp::model::CallToolResponse, McpError> {
            Ok(CallToolResult::structured(json!({ "echoed": request.arguments })).into())
        }
    }

    async fn connected_fake_pair() -> (RunningService<RoleClient, ()>, Vec<Tool>) {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            let _ = FakeMcpServer.serve(server_io).await.expect("fake server handshake").waiting().await;
        });
        handshake_and_discover(client_io, Duration::from_secs(5)).await.expect("handshake against the fake server")
    }

    #[tokio::test]
    async fn discovers_the_tool_the_fake_server_declares() {
        let (_service, tools) = connected_fake_pair().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
    }

    #[tokio::test]
    async fn calling_the_discovered_tool_round_trips_through_the_fake_server() {
        let (service, _tools) = connected_fake_pair().await;

        let mut params = CallToolRequestParams::new("echo");
        params = params.with_arguments(json!({ "text": "oi" }).as_object().unwrap().clone());
        let response = service.peer().call_tool_once(params).await.unwrap();

        let rmcp::model::CallToolResponse::Complete(result) = response else {
            panic!("expected a Complete response, got {response:?}");
        };
        assert_eq!(result.structured_content, Some(json!({ "echoed": { "text": "oi" } })));
    }
}
