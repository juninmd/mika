//! Leitura de `mcp.json` e normalização de nomes — port de
//! `apps/desktop/src/features/mcp/mcp-client.ts` (`readServers`/`sanitize`/`ServerSpec`). A parte
//! de rede/spawn fica em `client.rs`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::AppHandle;

pub const MCP_CONFIG_FILE: &str = "mcp.json";

/// Espelha `ServerSpec` (`mcp-client.ts`): um servidor declarado em `mcp.json`.
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct ServerSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// `"disabled": true` no `mcp.json` (mesma grafia de Cursor/Cline): o servidor fica declarado
    /// mas não sobe. Ausente = ligado — arquivo antigo continua valendo sem mudança.
    #[serde(default)]
    pub disabled: bool,
}

/// As duas grafias que `readServers` aceita para a chave de topo — `mcpServers` primeiro, e
/// `mcp_servers` (legado) só se a primeira estiver AUSENTE, mesma ordem do `??` no TS.
#[derive(Debug, Deserialize, Default)]
struct McpConfigFile {
    #[serde(rename = "mcpServers")]
    mcp_servers: Option<BTreeMap<String, ServerSpec>>,
    #[serde(rename = "mcp_servers")]
    mcp_servers_snake_case: Option<BTreeMap<String, ServerSpec>>,
}

/// `<app_data_dir>/plugins/mika.mcp` — o mesmo `ctx.dataDir` que `plugin-activation.ts` monta para
/// o plugin `mika.mcp` hoje.
pub fn mcp_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = crate::state::app_data_dir(app)
        .map_err(|error| format!("não resolveu app_data_dir: {error}"))?;
    Ok(data_dir.join("plugins").join("mika.mcp"))
}

/// Espelha `sanitize`: tudo fora de `[a-zA-Z0-9_]` vira `_`. Não apara pontas nem colapsa
/// sequências — esse é o `normalize_name` do `ToolRegistry` (`modules/tools/registry.rs`), um normalizador
/// DIFERENTE deste; os dois compõem a chave final da tool (`mcp_<servidor>_<tool>`), ver `client.rs`.
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

/// Espelha `readServers`. Primeira leitura sem `mcp.json` cria o template vazio sem lançar; JSON
/// inválido loga e devolve vazio SEM tocar no arquivo — nunca sobrescreve config que o usuário já
/// tem, mesmo quebrada.
pub fn read_servers(data_dir: &Path) -> BTreeMap<String, ServerSpec> {
    let path = data_dir.join(MCP_CONFIG_FILE);

    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            // Existe mas não leu (UTF-16, permissão, lock): o arquivo é do usuário, nunca o recria.
            log::error!("mcp.json could not be read — no server was loaded, file left untouched: {error}");
            return BTreeMap::new();
        }
        Err(_) => {
            match write_template(data_dir, &path) {
                Ok(()) => log::info!("mcp: created empty mcp.json ({})", path.display()),
                Err(error) => log::debug!("mcp: could not write the mcp.json template: {error}"),
            }
            return BTreeMap::new();
        }
    };

    match serde_json::from_str::<McpConfigFile>(&text) {
        Ok(raw) => raw.mcp_servers.or(raw.mcp_servers_snake_case).unwrap_or_default(),
        Err(error) => {
            log::error!("mcp.json is not valid JSON — no server was loaded, file left untouched: {error}");
            BTreeMap::new()
        }
    }
}

fn write_template(data_dir: &Path, path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let template = serde_json::json!({ "mcpServers": {} });
    fs::write(path, serde_json::to_vec_pretty(&template).expect("static template always serializes"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Mesmo padrão de `modules/privacy.rs`: diretório isolado por teste, sem trazer dependência
    /// só para isso.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);

            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-mcp-config-test-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // --- sanitize --------------------------------------------------------------------------------

    #[test]
    fn sanitize_keeps_alnum_and_underscore_replaces_the_rest() {
        assert_eq!(sanitize("my-server.v2"), "my_server_v2");
        assert_eq!(sanitize("café_ñ"), "caf___");
    }

    // --- read_servers ------------------------------------------------------------------------------

    #[test]
    fn first_read_without_mcp_json_creates_the_empty_template_without_throwing() {
        let dir = TempDir::new();
        let servers = read_servers(&dir.0);
        assert!(servers.is_empty());

        let written = fs::read_to_string(dir.0.join(MCP_CONFIG_FILE)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert_eq!(parsed, serde_json::json!({ "mcpServers": {} }));
    }

    #[test]
    fn creates_mcp_json_even_when_the_data_dir_does_not_exist_yet() {
        let parent = TempDir::new();
        let data_dir = parent.0.join("plugins").join("mika.mcp");

        let servers = read_servers(&data_dir);

        assert!(servers.is_empty());
        assert!(data_dir.join(MCP_CONFIG_FILE).exists());
    }

    #[test]
    fn reads_mcp_servers_from_an_existing_file() {
        let dir = TempDir::new();
        fs::write(
            dir.0.join(MCP_CONFIG_FILE),
            serde_json::json!({ "mcpServers": { "fs": { "command": "npx", "args": ["mcp-fs"] } } }).to_string(),
        )
        .unwrap();

        let servers = read_servers(&dir.0);

        assert_eq!(servers.len(), 1);
        assert_eq!(servers["fs"].command, "npx");
        assert_eq!(servers["fs"].args, vec!["mcp-fs".to_string()]);
    }

    /// A grafia legada é aceita como alias — mesma regra `raw.mcpServers ?? raw.mcp_servers` do TS.
    #[test]
    fn accepts_the_legacy_snake_case_spelling_as_an_alias() {
        let dir = TempDir::new();
        fs::write(
            dir.0.join(MCP_CONFIG_FILE),
            serde_json::json!({ "mcp_servers": { "fs": { "command": "npx" } } }).to_string(),
        )
        .unwrap();

        let servers = read_servers(&dir.0);

        assert_eq!(servers.len(), 1);
        assert_eq!(servers["fs"].command, "npx");
    }

    /// NEGATIVO — o caso que o comentário do TS chama pelo nome: JSON quebrado não pode apagar a
    /// config do usuário. Devolve vazio e deixa o arquivo intocado.
    #[test]
    fn invalid_json_does_not_throw_returns_empty_and_leaves_the_file_untouched() {
        let dir = TempDir::new();
        let path = dir.0.join(MCP_CONFIG_FILE);
        fs::write(&path, "{ isto nao e json").unwrap();

        let servers = read_servers(&dir.0);

        assert!(servers.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ isto nao e json");
    }

    /// `mcp.json` salvo em UTF-16 (PowerShell `>`, Notepad antigo) falha em `read_to_string` com
    /// `InvalidData`, não `NotFound`: não é "arquivo ausente", e o template não pode sobrescrevê-lo.
    #[test]
    fn an_unreadable_but_existing_file_is_never_overwritten_by_the_template() {
        let dir = TempDir::new();
        let path = dir.0.join(MCP_CONFIG_FILE);
        let utf16: Vec<u8> = "{\"mcpServers\":{}}".encode_utf16().flat_map(u16::to_le_bytes).collect();
        fs::write(&path, &utf16).unwrap();

        assert!(read_servers(&dir.0).is_empty());
        assert_eq!(fs::read(&path).unwrap(), utf16);
    }

    #[test]
    fn does_not_overwrite_an_existing_valid_file() {
        let dir = TempDir::new();
        let path = dir.0.join(MCP_CONFIG_FILE);
        fs::write(&path, serde_json::json!({ "mcpServers": { "files": { "command": "npx" } } }).to_string()).unwrap();

        let servers = read_servers(&dir.0);

        assert_eq!(servers.len(), 1);
        assert!(fs::read_to_string(&path).unwrap().contains("files"));
    }

    #[test]
    fn valid_json_without_either_recognized_key_falls_back_to_an_empty_map() {
        let dir = TempDir::new();
        fs::write(dir.0.join(MCP_CONFIG_FILE), serde_json::json!({ "unrelated": true }).to_string()).unwrap();

        assert!(read_servers(&dir.0).is_empty());
    }

    /// NEGATIVO — mesmo caso do teste `mcp-client-edge-cases.test.ts` do TS: o diretório do
    /// template não pode ser criado (o "pai" é um arquivo, não uma pasta). Devolve vazio em vez de
    /// lançar.
    #[test]
    fn logs_and_returns_empty_when_the_template_cannot_be_written() {
        let parent = TempDir::new();
        let blocker = parent.0.join("blocked");
        fs::write(&blocker, "not a directory").unwrap();
        let data_dir = blocker.join("nested");

        assert!(read_servers(&data_dir).is_empty());
    }
}
