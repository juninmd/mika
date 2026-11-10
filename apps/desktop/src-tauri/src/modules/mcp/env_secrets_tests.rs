//! `env` de servidor MCP é segredo: nunca em claro no disco, 0600, e o arquivo legado migra.

use std::path::PathBuf;

use serde_json::json;

use super::*;
use crate::config::registry::SchemaRegistry;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("mika-mcp-env-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `MIKA_MASTER_KEY` fixa pela variável de ambiente seria global ao processo; um `data_dir` próprio
/// gera uma `master.key` própria — cada teste tem a sua.
fn store(dir: &TempDir) -> ConfigStore {
    ConfigStore::open(&dir.0.join("cfg"), SchemaRegistry::new(vec![])).unwrap()
}

fn doc_with(env: serde_json::Value) -> Value {
    json!({ "mcpServers": { "gh": { "command": "npx", "args": ["x"], "env": env } } })
}

fn on_disk(dir: &TempDir) -> String {
    std::fs::read_to_string(dir.0.join("mcp").join(super::super::config::MCP_CONFIG_FILE)).unwrap()
}

#[test]
fn a_saved_env_secret_is_never_plaintext_on_disk_and_round_trips() {
    let dir = TempDir::new();
    let (store, data_dir) = (store(&dir), dir.0.join("mcp"));
    let doc = doc_with(json!({ "GITHUB_TOKEN": "ghp_super_secret_123" }));

    write_sealed_document(&data_dir, &doc, &store).unwrap();

    let raw = on_disk(&dir);
    assert!(!raw.contains("ghp_super_secret_123"), "segredo em claro no mcp.json: {raw}");
    assert!(raw.contains("enc:aes256:"), "{raw}");
    let loaded = load_plain_document(&data_dir, &store).unwrap();
    assert_eq!(loaded["mcpServers"]["gh"]["env"]["GITHUB_TOKEN"], json!("ghp_super_secret_123"));
    // Outras chaves do arquivo do usuário sobrevivem.
    assert_eq!(loaded["mcpServers"]["gh"]["args"], json!(["x"]));
}

#[cfg(unix)]
#[test]
fn the_written_file_is_private_to_the_owner() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new();
    let (store, data_dir) = (store(&dir), dir.0.join("mcp"));

    write_sealed_document(&data_dir, &doc_with(json!({ "K": "v" })), &store).unwrap();

    let mode = std::fs::metadata(data_dir.join(super::super::config::MCP_CONFIG_FILE)).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

/// Arquivo escrito antes da cifra (ou à mão): continua lendo, é regravado cifrado e apertado a 0600.
#[test]
fn a_legacy_plaintext_file_is_migrated_in_place() {
    let dir = TempDir::new();
    let (store, data_dir) = (store(&dir), dir.0.join("mcp"));
    std::fs::create_dir_all(&data_dir).unwrap();
    let path = data_dir.join(super::super::config::MCP_CONFIG_FILE);
    std::fs::write(&path, doc_with(json!({ "BRAVE_API_KEY": "legacy-plain" })).to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    assert!(seal_file_in_place(&data_dir, &store).unwrap(), "tinha texto claro, tinha de regravar");

    assert!(!on_disk(&dir).contains("legacy-plain"));
    assert_eq!(load_plain_document(&data_dir, &store).unwrap()["mcpServers"]["gh"]["env"]["BRAVE_API_KEY"], json!("legacy-plain"));
    assert!(!seal_file_in_place(&data_dir, &store).unwrap(), "já cifrado: nada a fazer");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn sealing_twice_does_not_double_encrypt() {
    let dir = TempDir::new();
    let (store, data_dir) = (store(&dir), dir.0.join("mcp"));
    write_sealed_document(&data_dir, &doc_with(json!({ "K": "v" })), &store).unwrap();
    let again = editor::load_document(&data_dir).unwrap();

    write_sealed_document(&data_dir, &again, &store).unwrap();

    assert_eq!(load_plain_document(&data_dir, &store).unwrap()["mcpServers"]["gh"]["env"]["K"], json!("v"));
}

/// Master.key trocada: o valor ilegível NÃO vira o `enc:...` passado ao servidor como se fosse a
/// chave — mas o ciphertext continua no documento, senão a próxima gravação o destruiria.
#[test]
fn a_value_that_no_longer_decrypts_stays_in_the_document_but_never_reaches_the_server() {
    let (dir_a, dir_b) = (TempDir::new(), TempDir::new());
    let data_dir = dir_a.0.join("mcp");
    write_sealed_document(&data_dir, &doc_with(json!({ "K": "v" })), &store(&dir_a)).unwrap();
    let ciphertext = editor::load_document(&data_dir).unwrap()["mcpServers"]["gh"]["env"]["K"].as_str().unwrap().to_string();

    let other_key = store(&dir_b);
    let loaded = load_plain_document(&data_dir, &other_key).unwrap();
    assert_eq!(loaded["mcpServers"]["gh"]["env"]["K"], json!(ciphertext), "o ciphertext original tem de sobreviver");

    let mut specs = std::collections::BTreeMap::from([(
        "gh".to_string(),
        ServerSpec { command: "npx".into(), env: [("K".to_string(), ciphertext)].into(), ..Default::default() },
    )]);
    assert_eq!(unreadable_env(&specs["gh"], &other_key), vec!["K".to_string()]);
    open_specs(&mut specs, &other_key);
    assert!(specs["gh"].env.is_empty());
}

/// O defeito: salvar o servidor A regravava o arquivo inteiro sem o env ilegível do B — perda
/// permanente do ciphertext (e, com a `master.key` antiga de volta, nada para decifrar).
#[test]
fn saving_server_a_does_not_destroy_the_unreadable_env_of_server_b() {
    let (dir_old, dir_new) = (TempDir::new(), TempDir::new());
    let data_dir = dir_old.0.join("mcp");
    let (old_key, new_key) = (store(&dir_old), store(&dir_new));
    write_sealed_document(&data_dir, &doc_with(json!({ "TOKEN": "ghp_old" })), &old_key).unwrap();

    let mut doc = load_plain_document(&data_dir, &new_key).unwrap();
    let input = editor::parse_spec(&json!({ "command": "npx", "env": { "OTHER": "novo" } })).unwrap();
    editor::upsert(&mut doc, "a", input, None).unwrap();
    write_sealed_document(&data_dir, &doc, &new_key).unwrap();

    let restored = load_plain_document(&data_dir, &old_key).unwrap();
    assert_eq!(restored["mcpServers"]["gh"]["env"]["TOKEN"], json!("ghp_old"), "o env ilegível do B foi destruído");
    assert_eq!(load_plain_document(&data_dir, &new_key).unwrap()["mcpServers"]["a"]["env"]["OTHER"], json!("novo"));
}

#[test]
fn open_specs_decrypts_sealed_values_and_keeps_plain_ones() {
    let dir = TempDir::new();
    let store = store(&dir);
    let sealed = store.seal_secret("secret").unwrap();
    let mut specs = std::collections::BTreeMap::from([(
        "gh".to_string(),
        ServerSpec {
            command: "npx".into(),
            env: [("A".to_string(), sealed), ("B".to_string(), "plain".to_string())].into(),
            ..Default::default()
        },
    )]);

    open_specs(&mut specs, &store);

    assert_eq!(specs["gh"].env["A"], "secret");
    assert_eq!(specs["gh"].env["B"], "plain");
}
