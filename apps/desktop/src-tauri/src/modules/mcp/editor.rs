//! Edição de `mcp.json` pela UI — validação de payload, upsert/remove preservando o resto do
//! arquivo, escrita atômica e a visão mascarada que `mika:mcp-servers-list` devolve.
//!
//! Puro de propósito (sem `AppHandle`): o que decide se o arquivo do usuário sobrevive a uma edição
//! mora aqui e é testável com um diretório temporário. Quem liga isto a IPC e ao ciclo de vida das
//! conexões é `servers.rs`.
//!
//! Regras que valem para toda escrita:
//! - chave de topo desconhecida, outros servidores e chave desconhecida DENTRO da entrada editada
//!   (`autoApprove`, `type`, …) sobrevivem — o arquivo é do usuário, a UI só mexe no que conhece;
//! - JSON inválido no disco é ERRO, nunca template vazio: a próxima escrita gravaria por cima;
//! - valor de `env` nunca sai daqui para a UI (`describe` só expõe as chaves). Na volta, `null` num
//!   valor de `env` significa "mantenha o que está salvo" — é assim que a UI edita um servidor com
//!   segredo sem jamais ter visto o segredo.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use serde_json::{json, Map, Value};

use super::config::{sanitize, ServerSpec, MCP_CONFIG_FILE};

const MAX_NAME_CHARS: usize = 64;
const MAX_COMMAND_CHARS: usize = 1024;
const MAX_ARGS: usize = 64;
const MAX_ARG_CHARS: usize = 4096;
const MAX_ENV_ENTRIES: usize = 64;
const MAX_ENV_KEY_CHARS: usize = 128;
const MAX_ENV_VALUE_CHARS: usize = 8192;

/// Um servidor como a UI o manda: `env` com `None` = "mantenha o valor salvo".
#[derive(Debug, Clone, PartialEq)]
pub struct ServerInput {
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, Option<String>>,
    pub cwd: Option<String>,
    pub enabled: bool,
}

pub fn parse_name(value: &Value) -> Result<String, String> {
    let name = value.as_str().ok_or("name tem de ser string")?.trim();
    let valid_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.');
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || !name.chars().all(valid_char) {
        return Err(format!("nome inválido: use 1 a {MAX_NAME_CHARS} caracteres entre letras, números, '_', '-' e '.'"));
    }
    Ok(name.to_string())
}

/// Nome de uma entrada que JÁ está no `mcp.json` (remover, ou editar/ligar/desligar sem renomear):
/// o arquivo é do usuário e pode ter nomes fora do alfabeto de [`parse_name`] (`"brave search"`,
/// `"@org/srv"`) — recusá-los deixava a linha na lista sem como remover nem desligar pela UI.
pub fn parse_existing_name(value: &Value) -> Result<String, String> {
    let name = value.as_str().ok_or("name tem de ser string")?;
    if name.is_empty() || name.chars().count() > MAX_ARG_CHARS || name.contains('\0') {
        return Err("nome de servidor inválido".into());
    }
    Ok(name.to_string())
}

fn bounded_string(value: &Value, what: &str, max: usize) -> Result<String, String> {
    let text = value.as_str().ok_or_else(|| format!("{what} tem de ser string"))?;
    if text.chars().count() > max || text.contains('\0') {
        return Err(format!("{what} excede {max} caracteres ou contém NUL"));
    }
    Ok(text.to_string())
}

fn parse_args(value: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(value) = value.filter(|v| !v.is_null()) else { return Ok(Vec::new()) };
    let items = value.as_array().ok_or("args tem de ser lista de strings")?;
    if items.len() > MAX_ARGS {
        return Err(format!("no máximo {MAX_ARGS} args"));
    }
    items.iter().map(|item| bounded_string(item, "arg", MAX_ARG_CHARS)).collect()
}

fn is_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && key.len() <= MAX_ENV_KEY_CHARS
}

fn parse_env(value: Option<&Value>) -> Result<BTreeMap<String, Option<String>>, String> {
    let Some(value) = value.filter(|v| !v.is_null()) else { return Ok(BTreeMap::new()) };
    let map = value.as_object().ok_or("env tem de ser objeto { CHAVE: valor }")?;
    if map.len() > MAX_ENV_ENTRIES {
        return Err(format!("no máximo {MAX_ENV_ENTRIES} variáveis de ambiente"));
    }
    let mut env = BTreeMap::new();
    for (key, raw) in map {
        if !is_env_key(key) {
            return Err(format!("chave de env inválida: {key}"));
        }
        let parsed = if raw.is_null() { None } else { Some(bounded_string(raw, "valor de env", MAX_ENV_VALUE_CHARS)?) };
        env.insert(key.clone(), parsed);
    }
    Ok(env)
}

fn parse_cwd(value: Option<&Value>) -> Result<Option<String>, String> {
    let Some(value) = value.filter(|v| !v.is_null()) else { return Ok(None) };
    let cwd = bounded_string(value, "cwd", MAX_ARG_CHARS)?.trim().to_string();
    if cwd.is_empty() {
        return Ok(None);
    }
    if !Path::new(&cwd).is_absolute() {
        return Err("cwd tem de ser caminho absoluto".into());
    }
    Ok(Some(cwd))
}

pub fn parse_spec(value: &Value) -> Result<ServerInput, String> {
    let spec = value.as_object().ok_or("spec tem de ser objeto")?;
    let command = bounded_string(spec.get("command").unwrap_or(&Value::Null), "command", MAX_COMMAND_CHARS)?;
    let command = command.trim().to_string();
    if command.is_empty() {
        return Err("command não pode ser vazio".into());
    }
    let enabled = match spec.get("enabled") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => return Err("enabled tem de ser booleano".into()),
    };
    Ok(ServerInput {
        command,
        args: parse_args(spec.get("args"))?,
        env: parse_env(spec.get("env"))?,
        cwd: parse_cwd(spec.get("cwd"))?,
        enabled,
    })
}

/// Valor de um documento JÁ aberto (`env_secrets::load_plain_document`) que ainda tem o prefixo da
/// cifra: não decifrou com a `master.key` atual. Fica no arquivo, nunca no `env` do processo.
fn is_unreadable(value: &Value) -> bool {
    value.as_str().is_some_and(crate::config::crypto::Encryption::is_encrypted)
}

/// `null` = valor salvo em `previous_env`; sem valor salvo (ou ilegível), recusa nomeando a chave.
pub fn resolve_env(input: &ServerInput, previous_env: Option<&Value>) -> Result<BTreeMap<String, String>, String> {
    input
        .env
        .iter()
        .map(|(key, value)| {
            let resolved = match value {
                Some(value) => Some(value.clone()),
                None => previous_env.and_then(|env| env.get(key)).filter(|v| !is_unreadable(v)).and_then(Value::as_str).map(str::to_string),
            };
            resolved.map(|v| (key.clone(), v)).ok_or_else(|| format!("env {key} sem valor salvo para manter"))
        })
        .collect()
}

/// Mesma precedência de `read_servers`: `mcpServers`, e `mcp_servers` só se aquela estiver ausente.
fn servers_key(doc: &Map<String, Value>) -> &'static str {
    if !doc.contains_key("mcpServers") && doc.get("mcp_servers").is_some_and(Value::is_object) {
        "mcp_servers"
    } else {
        "mcpServers"
    }
}

fn servers_mut(doc: &mut Value) -> Result<&mut Map<String, Value>, String> {
    let root = doc.as_object_mut().ok_or("mcp.json não é um objeto JSON")?;
    let key = servers_key(root);
    root.entry(key).or_insert_with(|| json!({})).as_object_mut().ok_or_else(|| format!("{key} não é objeto"))
}

pub fn find_entry<'a>(doc: &'a Value, name: &str) -> Option<&'a Value> {
    let root = doc.as_object()?;
    root.get(servers_key(root))?.get(name)
}

fn ensure_name_is_free(servers: &Map<String, Value>, name: &str, source: &str) -> Result<(), String> {
    let wanted = sanitize(name);
    match servers.keys().find(|key| key.as_str() != source && sanitize(key) == wanted) {
        Some(taken) => Err(format!("já existe um servidor '{taken}' com esse nome (ou equivalente)")),
        None => Ok(()),
    }
}

/// Cria (`original = None`) ou edita/renomeia (`original = Some`) uma entrada, preservando as
/// chaves que a UI não conhece. Devolve o `ServerSpec` efetivo, já com o `env` resolvido.
pub fn upsert(doc: &mut Value, name: &str, input: ServerInput, original: Option<&str>) -> Result<ServerSpec, String> {
    let servers = servers_mut(doc)?;
    let source = original.unwrap_or(name);
    if original.is_some() && !servers.contains_key(source) {
        return Err(format!("servidor '{source}' não existe mais no mcp.json"));
    }
    ensure_name_is_free(servers, name, if original.is_some() { source } else { "" })?;

    let mut entry = servers.get(source).and_then(Value::as_object).cloned().unwrap_or_default();
    let env = resolve_env(&input, entry.get("env"))?;
    entry.insert("command".into(), json!(input.command));
    entry.insert("args".into(), json!(input.args));
    let mut stored_env = env.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<Map<_, _>>();
    // Variável ilegível que o usuário não reenviou: o ciphertext segue no arquivo (nada o apaga
    // sem querer), só não entra no `ServerSpec` devolvido.
    for (key, value) in entry.get("env").and_then(Value::as_object).into_iter().flatten() {
        if is_unreadable(value) && !stored_env.contains_key(key) {
            stored_env.insert(key.clone(), value.clone());
        }
    }
    entry.insert("env".into(), Value::Object(stored_env));
    match &input.cwd {
        Some(cwd) => entry.insert("cwd".into(), json!(cwd)),
        None => entry.remove("cwd"),
    };
    match input.enabled {
        true => entry.remove("disabled"),
        false => entry.insert("disabled".into(), json!(true)),
    };

    servers.remove(source);
    servers.insert(name.to_string(), Value::Object(entry));
    Ok(ServerSpec { command: input.command, args: input.args, env, cwd: input.cwd, disabled: !input.enabled })
}

pub fn remove(doc: &mut Value, name: &str) -> Result<(), String> {
    servers_mut(doc)?.remove(name).map(|_| ()).ok_or_else(|| format!("servidor '{name}' não existe no mcp.json"))
}

/// Cada entrada desserializada sozinha: uma malformada vira erro só dela.
pub fn entries(doc: &Value) -> Vec<(String, Result<ServerSpec, String>)> {
    let Some(root) = doc.as_object() else { return Vec::new() };
    let Some(servers) = root.get(servers_key(root)).and_then(Value::as_object) else { return Vec::new() };
    servers
        .iter()
        .map(|(name, raw)| {
            let spec = serde_json::from_value::<ServerSpec>(raw.clone()).map_err(|error| format!("entrada inválida no mcp.json: {error}"));
            (name.clone(), spec)
        })
        .collect()
}

/// Visão para a UI: chaves de `env`, nunca valores.
pub fn describe(name: &str, spec: &ServerSpec) -> Value {
    json!({
        "name": name,
        "command": spec.command,
        "args": spec.args,
        "envKeys": spec.env.keys().collect::<Vec<_>>(),
        "cwd": spec.cwd,
        "enabled": !spec.disabled,
    })
}

pub fn load_document(data_dir: &Path) -> Result<Value, String> {
    let path = data_dir.join(MCP_CONFIG_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(json!({ "mcpServers": {} })),
        Err(error) => return Err(format!("não leu {}: {error}", path.display())),
    };
    serde_json::from_str(&text).map_err(|error| format!("mcp.json não é JSON válido ({error}) — corrija o arquivo à mão; nada foi gravado"))
}

/// Cria o arquivo já 0600 (só o dono lê): `mcp.json` carrega `env` de servidor, que é segredo. No
/// Windows não há bit POSIX — vale a ACL herdada da pasta de dados do usuário (mesmo limite da
/// `master.key`, ver `config/crypto.rs`).
fn create_private(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// Aperta um `mcp.json` que já existia (criado antes da cifra, com a permissão padrão do usuário).
pub fn restrict_to_owner(data_dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(data_dir.join(MCP_CONFIG_FILE), fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = data_dir;
}

/// Temporário no MESMO diretório + `rename`: o leitor vê o arquivo antigo ou o novo, nunca meio.
pub fn write_atomic(data_dir: &Path, doc: &Value) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|error| format!("não criou {}: {error}", data_dir.display()))?;
    let target = data_dir.join(MCP_CONFIG_FILE);
    let temp = data_dir.join(format!(".{MCP_CONFIG_FILE}.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(doc).map_err(|error| error.to_string())?;
    let written = create_private(&temp).and_then(|mut file| {
        file.write_all(&bytes)?;
        file.sync_all()
    });
    if let Err(error) = written.and_then(|()| fs::rename(&temp, &target)) {
        let _ = fs::remove_file(&temp);
        return Err(format!("não gravou {}: {error}", target.display()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
