use super::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-mcp-editor-test-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn input(value: Value) -> ServerInput {
    parse_spec(&value).unwrap()
}

// --- parse_name / parse_spec ---------------------------------------------------------------------

#[test]
fn name_is_trimmed_and_limited_to_a_safe_charset() {
    assert_eq!(parse_name(&json!("  my-fs.v2 ")).unwrap(), "my-fs.v2");
    assert!(parse_name(&json!("")).is_err());
    assert!(parse_name(&json!("   ")).is_err());
    assert!(parse_name(&json!("a/b")).is_err());
    assert!(parse_name(&json!("x".repeat(65))).is_err());
    assert!(parse_name(&json!(42)).is_err());
}

#[test]
fn spec_requires_a_non_empty_command_and_typed_args_and_env() {
    assert!(parse_spec(&json!({ "command": "   " })).is_err());
    assert!(parse_spec(&json!({ "command": "npx", "args": "mcp-fs" })).is_err());
    assert!(parse_spec(&json!({ "command": "npx", "args": [1] })).is_err());
    assert!(parse_spec(&json!({ "command": "npx", "env": ["A"] })).is_err());
    assert!(parse_spec(&json!({ "command": "npx", "env": { "1BAD": "x" } })).is_err());
    assert!(parse_spec(&json!({ "command": "npx", "env": { "A": 1 } })).is_err());
    assert!(parse_spec(&json!({ "command": "npx", "cwd": "relative/dir" })).is_err());
    assert!(parse_spec(&json!({ "command": "npx", "enabled": "yes" })).is_err());

    let ok = input(json!({ "command": " npx ", "args": ["-y", "mcp-fs"], "env": { "TOKEN": "s3cret", "KEEP": null }, "cwd": "" }));
    assert_eq!(ok.command, "npx");
    assert_eq!(ok.args, vec!["-y", "mcp-fs"]);
    assert_eq!(ok.env["TOKEN"], Some("s3cret".to_string()));
    assert_eq!(ok.env["KEEP"], None);
    assert_eq!(ok.cwd, None);
    assert!(ok.enabled, "enabled é true por padrão");
}

// --- upsert ----------------------------------------------------------------------------------------

#[test]
fn upsert_preserves_other_servers_unknown_top_level_keys_and_unknown_entry_keys() {
    let mut doc = json!({
        "mcpServers": {
            "fs": { "command": "npx", "args": ["mcp-fs"], "autoApprove": ["read"] },
            "git": { "command": "uvx", "args": ["mcp-git"] }
        },
        "comment": "do usuário"
    });

    let spec = upsert(&mut doc, "fs", input(json!({ "command": "node", "args": ["srv.js"] })), Some("fs")).unwrap();

    assert_eq!(spec.command, "node");
    assert_eq!(doc["comment"], json!("do usuário"));
    assert_eq!(doc["mcpServers"]["git"], json!({ "command": "uvx", "args": ["mcp-git"] }));
    assert_eq!(doc["mcpServers"]["fs"]["autoApprove"], json!(["read"]));
    assert_eq!(doc["mcpServers"]["fs"]["args"], json!(["srv.js"]));
}

#[test]
fn upsert_writes_under_the_legacy_key_when_that_is_the_one_the_file_uses() {
    let mut doc = json!({ "mcp_servers": { "fs": { "command": "npx" } } });
    upsert(&mut doc, "git", input(json!({ "command": "uvx" })), None).unwrap();
    assert!(doc.get("mcpServers").is_none());
    assert_eq!(doc["mcp_servers"]["git"]["command"], json!("uvx"));
}

#[test]
fn rename_moves_the_entry_and_null_env_values_keep_the_saved_secret() {
    let mut doc = json!({ "mcpServers": { "old": { "command": "npx", "env": { "TOKEN": "s3cret", "DROP": "x" } } } });

    let spec = upsert(&mut doc, "new", input(json!({ "command": "npx", "env": { "TOKEN": null } })), Some("old")).unwrap();

    assert!(doc["mcpServers"].get("old").is_none());
    assert_eq!(doc["mcpServers"]["new"]["env"], json!({ "TOKEN": "s3cret" }));
    assert_eq!(spec.env["TOKEN"], "s3cret");
}

/// NEGATIVO — `null` pede "mantenha o salvo"; sem valor salvo não há o que manter.
#[test]
fn null_env_value_without_a_saved_value_is_refused() {
    let mut doc = json!({ "mcpServers": {} });
    let error = upsert(&mut doc, "fs", input(json!({ "command": "npx", "env": { "TOKEN": null } })), None).unwrap_err();
    assert!(error.contains("TOKEN"), "got: {error}");
}

/// NEGATIVO — criar com nome já usado não sobrescreve em silêncio; colisão depois do `sanitize`
/// (`fs-a` x `fs.a` viram o mesmo prefixo de tool) também é recusada.
#[test]
fn creating_a_duplicate_or_a_sanitize_colliding_name_is_refused() {
    let mut doc = json!({ "mcpServers": { "fs-a": { "command": "npx" } } });
    assert!(upsert(&mut doc, "fs-a", input(json!({ "command": "node" })), None).is_err());
    assert!(upsert(&mut doc, "fs.a", input(json!({ "command": "node" })), None).is_err());
    assert_eq!(doc["mcpServers"]["fs-a"]["command"], json!("npx"));
}

#[test]
fn editing_a_server_that_no_longer_exists_is_refused() {
    let mut doc = json!({ "mcpServers": {} });
    assert!(upsert(&mut doc, "fs", input(json!({ "command": "npx" })), Some("ghost")).is_err());
}

#[test]
fn disabling_writes_disabled_true_and_reenabling_removes_it() {
    let mut doc = json!({ "mcpServers": {} });
    let spec = upsert(&mut doc, "fs", input(json!({ "command": "npx", "enabled": false })), None).unwrap();
    assert!(spec.disabled);
    assert_eq!(doc["mcpServers"]["fs"]["disabled"], json!(true));

    upsert(&mut doc, "fs", input(json!({ "command": "npx", "enabled": true })), Some("fs")).unwrap();
    assert!(doc["mcpServers"]["fs"].get("disabled").is_none());
}

// --- remove ----------------------------------------------------------------------------------------

#[test]
fn remove_drops_only_the_named_server() {
    let mut doc = json!({ "mcpServers": { "fs": { "command": "npx" }, "git": { "command": "uvx" } }, "x": 1 });
    remove(&mut doc, "fs").unwrap();
    assert_eq!(doc, json!({ "mcpServers": { "git": { "command": "uvx" } }, "x": 1 }));
    assert!(remove(&mut doc, "fs").is_err(), "remover o que não existe é erro nomeado");
}

// --- load_document / write_atomic -----------------------------------------------------------------

#[test]
fn write_then_load_round_trips_and_leaves_no_temp_file_behind() {
    let dir = TempDir::new();
    let data_dir = dir.0.join("plugins").join("mika.mcp");
    let doc = json!({ "mcpServers": { "fs": { "command": "npx" } }, "keep": true });

    write_atomic(&data_dir, &doc).unwrap();

    assert_eq!(load_document(&data_dir).unwrap(), doc);
    let names: Vec<_> = fs::read_dir(&data_dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from(super::super::config::MCP_CONFIG_FILE)]);
}

#[test]
fn missing_file_loads_as_the_empty_template() {
    let dir = TempDir::new();
    assert_eq!(load_document(&dir.0).unwrap(), json!({ "mcpServers": {} }));
}

/// NEGATIVO — JSON quebrado não vira template vazio (que a próxima escrita gravaria por cima,
/// apagando a config do usuário): é erro, e o arquivo fica intocado.
#[test]
fn invalid_json_is_an_error_not_an_empty_document() {
    let dir = TempDir::new();
    let path = dir.0.join(super::super::config::MCP_CONFIG_FILE);
    fs::write(&path, "{ quebrado").unwrap();
    assert!(load_document(&dir.0).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ quebrado");
}

// --- entries / describe -----------------------------------------------------------------------------

#[test]
fn describe_exposes_env_keys_but_never_env_values() {
    let doc = json!({ "mcpServers": { "fs": { "command": "npx", "args": ["a"], "env": { "TOKEN": "s3cret" }, "cwd": "/tmp" } } });
    let (name, spec) = entries(&doc).into_iter().next().unwrap();
    let described = describe(&name, &spec.unwrap());

    assert_eq!(described["envKeys"], json!(["TOKEN"]));
    assert_eq!(described["enabled"], json!(true));
    assert_eq!(described["cwd"], json!("/tmp"));
    assert!(!described.to_string().contains("s3cret"), "valor de env vazou: {described}");
}

/// Uma entrada malformada não esconde as outras — diferente de `read_servers`, que desserializa o
/// mapa inteiro de uma vez.
#[test]
fn a_malformed_entry_is_reported_per_server() {
    let doc = json!({ "mcpServers": { "bad": { "url": "http://x" }, "fs": { "command": "npx" } } });
    let listed = entries(&doc);
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().find(|(n, _)| n == "bad").unwrap().1.is_err());
    assert!(listed.iter().find(|(n, _)| n == "fs").unwrap().1.is_ok());
}

/// Review 2026-09-28: a name already in the user's mcp.json may be outside `parse_name`'s alphabet;
/// removing/toggling it from the UI must still work.
#[test]
fn an_existing_name_outside_the_strict_alphabet_is_accepted_for_existing_entries() {
    assert!(parse_name(&json!("brave search")).is_err());
    assert_eq!(parse_existing_name(&json!("brave search")).unwrap(), "brave search");
    assert_eq!(parse_existing_name(&json!("@org/srv")).unwrap(), "@org/srv");
    assert!(parse_existing_name(&json!("")).is_err());
    assert!(parse_existing_name(&json!("a\u{0}b")).is_err());
    assert!(parse_existing_name(&json!(3)).is_err());
}

/// Editar um servidor cujo env não decifra: o ciphertext fica no arquivo (nada o apaga), mas o
/// `ServerSpec` que sobe o processo não o recebe — o servidor nunca vê `enc:aes256:...` como chave.
#[test]
fn editing_a_server_keeps_its_unreadable_env_in_the_file_but_out_of_the_spawned_spec() {
    let mut doc = json!({ "mcpServers": { "s": { "command": "npx", "env": { "LOST": "enc:aes256:abc", "OK": "plain" } } } });

    let spec = upsert(&mut doc, "s", input(json!({ "command": "npx", "env": { "OK": null, "NEW": "v" } })), Some("s")).unwrap();

    assert_eq!(spec.env.keys().collect::<Vec<_>>(), ["NEW", "OK"]);
    assert_eq!(doc["mcpServers"]["s"]["env"]["LOST"], json!("enc:aes256:abc"));
    // "mantenha o salvo" para um valor ilegível é o erro nomeado de sempre, não o ciphertext.
    let err = resolve_env(&input(json!({ "command": "npx", "env": { "LOST": null } })), find_entry(&doc, "s").unwrap().get("env")).unwrap_err();
    assert!(err.contains("LOST"), "{err}");
}
