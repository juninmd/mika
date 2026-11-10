//! Canais `mika:mcp-*` — a UI lista, cria, edita, remove e testa servidores MCP sem o usuário
//! editar `mcp.json` à mão. Antes daqui, o único caminho era abrir o arquivo no editor.
//!
//! Toda rota: superfície do dono (`is_owner_surface`, mesmo `assertOwnerSurface` de sempre —
//! salvar aqui SPAWNA processo arbitrário, então `plugin-view` nunca chega), payload validado em
//! `editor.rs` antes de qualquer I/O, e valor de `env` nunca logado nem devolvido. Erro de negócio
//! (nome repetido, JSON quebrado no disco, servidor que não sobe) volta como `{ ok: false, error }`;
//! recusa de superfície volta como `Err`.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

use super::{client, config, editor, env_secrets, lifecycle};

pub const CH_LIST: &str = "mika:mcp-servers-list";
pub const CH_SAVE: &str = "mika:mcp-server-save";
pub const CH_REMOVE: &str = "mika:mcp-server-remove";
pub const CH_TEST: &str = "mika:mcp-server-test";
pub const CH_OPEN_DIR: &str = "mika:mcp-open-config-dir";

const CHANNELS: &[&str] = &[CH_LIST, CH_SAVE, CH_REMOVE, CH_TEST, CH_OPEN_DIR];

pub struct McpServersModule;

fn edit_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[async_trait]
impl NativeModule for McpServersModule {
    fn name(&self) -> &'static str {
        "mcp"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(&self, app: &AppHandle, ctx: &InvokeContext, channel: &str, payload: Value) -> Result<Value, String> {
        if !is_owner_surface(ctx.window_type) {
            return Err(format!("{channel} recusado para {:?}", ctx.window_type));
        }
        let data_dir = config::mcp_data_dir(app)?;
        // Salvar/remover é ler-modificar-gravar do mesmo arquivo: dois cliques simultâneos não
        // podem intercalar e um apagar a escrita do outro.
        let _edit = if matches!(channel, CH_SAVE | CH_REMOVE) { Some(edit_lock().lock().await) } else { None };
        let outcome = match channel {
            CH_LIST => return Ok(list(app, &data_dir)),
            CH_SAVE => save(app, &data_dir, &payload).await,
            CH_REMOVE => remove(app, &data_dir, &payload).await,
            CH_TEST => test(app, &data_dir, &payload).await,
            CH_OPEN_DIR => open_dir(&data_dir),
            other => return Err(format!("canal '{other}' declarado em `mcp` mas não tratado")),
        };
        Ok(outcome.unwrap_or_else(|error| json!({ "ok": false, "error": error })))
    }
}

fn list(app: &AppHandle, data_dir: &std::path::Path) -> Value {
    let store = crate::state::config_store(app).ok();
    let flag = |path: &str| store.as_ref().and_then(|s| s.get(path)).and_then(|v| v.as_bool()).unwrap_or(false);
    let offline = store.as_ref().is_some_and(|s| crate::modules::privacy::is_offline_mode_enabled(s));
    if let Some(store) = &store {
        if let Err(error) = env_secrets::seal_file_in_place(data_dir, store) {
            log::warn!("mcp: mcp.json não migrou para env cifrado: {error}");
        }
    }
    let (servers, error) = match editor::load_document(data_dir) {
        Ok(doc) => (editor::entries(&doc).into_iter().map(|(name, spec)| row(&name, spec, store.as_deref())).collect(), None),
        Err(error) => (Vec::new(), Some(error)),
    };
    json!({
        "configPath": data_dir.join(config::MCP_CONFIG_FILE),
        "configDir": data_dir,
        "enabled": flag("mcp.enabled"),
        "offlineMode": offline,
        "error": error,
        "servers": servers,
    })
}

/// `envKeys` são só as variáveis que o servidor de fato recebe; as que a `master.key` atual não
/// decifra vão em `unreadableEnv` (a UI avisa — o ciphertext continua no arquivo).
fn row(name: &str, spec: Result<config::ServerSpec, String>, store: Option<&crate::config::store::ConfigStore>) -> Value {
    let spec = match spec {
        Ok(spec) => spec,
        Err(error) => return json!({ "name": name, "status": "error", "error": error, "tools": [], "envKeys": [], "unreadableEnv": [], "args": [], "enabled": false, "command": "", "cwd": null }),
    };
    let unreadable = store.map(|store| env_secrets::unreadable_env(&spec, store)).unwrap_or_default();
    let mut described = editor::describe(name, &spec);
    described["envKeys"] = json!(spec.env.keys().filter(|key| !unreadable.contains(key)).collect::<Vec<_>>());
    described["unreadableEnv"] = json!(unreadable);
    if let (Some(target), Value::Object(runtime)) = (described.as_object_mut(), lifecycle::live_status(name)) {
        target.extend(runtime);
    }
    described
}

fn server_row(data_dir: &std::path::Path, name: &str, store: &crate::config::store::ConfigStore) -> Value {
    let spec = editor::load_document(data_dir)
        .ok()
        .and_then(|doc| editor::entries(&doc).into_iter().find(|(n, _)| n == name))
        .map(|(_, spec)| spec)
        .unwrap_or_else(|| Err("sumiu do mcp.json".into()));
    row(name, spec, Some(store))
}

async fn save(app: &AppHandle, data_dir: &std::path::Path, payload: &Value) -> Result<Value, String> {
    let original = optional_name(payload.get("originalName"))?;
    let raw_name = payload.get("name").unwrap_or(&Value::Null);
    // Sem renomear, o nome é o que já está no arquivo — vale mesmo fora do alfabeto de `parse_name`.
    let name = match &original {
        Some(original) if raw_name.as_str() == Some(original.as_str()) => original.clone(),
        _ => editor::parse_name(raw_name)?,
    };
    let input = editor::parse_spec(payload.get("spec").unwrap_or(&Value::Null))?;

    let store = crate::state::config_store(app)?;
    let mut doc = env_secrets::load_plain_document(data_dir, &store)?;
    let spec = editor::upsert(&mut doc, &name, input, original.as_deref())?;
    env_secrets::write_sealed_document(data_dir, &doc, &store)?;
    log::info!("mcp: servidor salvo pela UI (server={name}, renamedFrom={original:?})");

    if let Some(original) = &original {
        super::invalidate_server(original);
        lifecycle::stop_server(original).await;
    }
    super::invalidate_server(&name);
    lifecycle::stop_server(&name).await;
    lifecycle::start_server(app, &name, &spec).await?;
    lifecycle::notify_changed(app);
    Ok(json!({ "ok": true, "server": server_row(data_dir, &name, &store) }))
}

async fn remove(app: &AppHandle, data_dir: &std::path::Path, payload: &Value) -> Result<Value, String> {
    let name = editor::parse_existing_name(payload.get("name").unwrap_or(&Value::Null))?;
    let store = crate::state::config_store(app)?;
    let mut doc = env_secrets::load_plain_document(data_dir, &store)?;
    editor::remove(&mut doc, &name)?;
    env_secrets::write_sealed_document(data_dir, &doc, &store)?;
    log::info!("mcp: servidor removido pela UI (server={name})");
    super::invalidate_server(&name);
    lifecycle::stop_server(&name).await;
    lifecycle::notify_changed(app);
    Ok(json!({ "ok": true }))
}

/// Sobe o servidor descrito, lista as tools e fecha — nada é gravado nem registrado. `env: null`
/// resolve contra a entrada salva em `originalName`, para testar uma edição sem reenviar segredo.
async fn test(app: &AppHandle, data_dir: &std::path::Path, payload: &Value) -> Result<Value, String> {
    let input = editor::parse_spec(payload.get("spec").unwrap_or(&Value::Null))?;
    let original = optional_name(payload.get("originalName"))?;
    let store = crate::state::config_store(app)?;
    if crate::modules::privacy::is_offline_mode_enabled(&store) {
        return Err("Modo offline ligado: nenhum servidor MCP sobe".into());
    }

    let doc = if original.is_some() { env_secrets::load_plain_document(data_dir, &store)? } else { json!({}) };
    let previous_env = original.as_deref().and_then(|name| editor::find_entry(&doc, name)).and_then(|entry| entry.get("env"));
    let env = editor::resolve_env(&input, previous_env)?;
    let spec = config::ServerSpec { command: input.command, args: input.args, env, cwd: input.cwd, disabled: false };

    let connected = client::connect_one("test", &spec, lifecycle::timeout_from(&store)).await?;
    let tools: Vec<Value> = connected
        .tools
        .iter()
        .map(|t| json!({ "name": t.upstream_name, "description": t.tool.description }))
        .collect();
    client::close_all(vec![connected]).await;
    Ok(json!({ "ok": true, "tools": tools }))
}

fn open_dir(data_dir: &std::path::Path) -> Result<Value, String> {
    // Garante a pasta (e o template) antes de abrir: pasta inexistente abriria um erro do SO.
    config::read_servers(data_dir);
    #[allow(deprecated)] // mesma escolha de `features/desktop/launch.rs::open_url`
    tauri_plugin_shell::open::open(None, data_dir.to_string_lossy(), None).map_err(|error| error.to_string())?;
    Ok(json!({ "ok": true, "path": data_dir }))
}

fn optional_name(value: Option<&Value>) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => editor::parse_existing_name(value).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contrato cross-language: cada canal existe no contrato TS — senão a UI nunca o manda.
    #[test]
    fn every_channel_is_declared_in_the_typescript_contract() {
        let contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        for channel in CHANNELS {
            assert!(contract.contains(&format!("'{channel}'")), "canal {channel} não existe em desktop-ipc-channels.ts");
        }
    }

    /// Entrada malformada vira linha de erro com o MESMO formato das outras — a UI não precisa de
    /// um caminho à parte para ela.
    #[test]
    fn a_malformed_entry_row_has_the_same_keys_as_a_healthy_one() {
        let healthy = row("mcp-servers-row-ok", Ok(config::ServerSpec { command: "npx".into(), ..Default::default() }), None);
        let broken = row("mcp-servers-row-bad", Err("entrada inválida".into()), None);
        // Ordenado: com `serde_json/preserve_order` (ligado pela `rig-core` ≥ 0.43) a ordem das chaves é a
        // de inserção, que difere entre as duas linhas; o que se afirma é o mesmo CONJUNTO de chaves.
        let keys = |v: &Value| {
            let mut keys = v.as_object().unwrap().keys().filter(|k| *k != "error").cloned().collect::<Vec<_>>();
            keys.sort_unstable();
            keys
        };
        assert_eq!(keys(&healthy), keys(&broken));
        assert_eq!(healthy["status"], json!("stopped"));
        assert_eq!(broken["status"], json!("error"));
    }

    /// Env que a `master.key` atual não decifra: a UI o vê em `unreadableEnv` e NÃO em `envKeys`
    /// (que alimenta o formulário de edição e a contagem de segredos "salvos").
    #[test]
    fn an_unreadable_env_is_reported_apart_from_the_keys_the_server_receives() {
        let dir = std::env::temp_dir().join(format!("mika-mcp-row-{}", uuid::Uuid::new_v4()));
        let store = crate::config::store::ConfigStore::open(&dir, crate::config::registry::SchemaRegistry::new(vec![])).unwrap();
        let env = [("GOOD".to_string(), store.seal_secret("v").unwrap()), ("LOST".to_string(), "enc:aes256:00:00:00:00".to_string())];
        let spec = config::ServerSpec { command: "npx".into(), env: env.into(), ..Default::default() };

        let described = row("mcp-servers-row-env", Ok(spec), Some(&store));

        assert_eq!(described["envKeys"], json!(["GOOD"]));
        assert_eq!(described["unreadableEnv"], json!(["LOST"]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
