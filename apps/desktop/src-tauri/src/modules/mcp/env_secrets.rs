//! `env` dos servidores de `mcp.json` cifrado em repouso. É onde o usuário cola a chave de API que
//! o servidor MCP precisa (`GITHUB_TOKEN`, `BRAVE_API_KEY`...) — antes ficava em texto claro num
//! arquivo que o `config.json` jamais teria (as chaves de provedor lá são `enc:aes256:`).
//!
//! Em memória o documento é sempre texto claro ([`load_plain_document`]); só [`write_sealed_document`]
//! cifra, com a `master.key` do `ConfigStore` (mesmo `Encryption`), e grava com permissão 0600.
//! Arquivo antigo em texto claro continua lendo (valor sem prefixo passa) e é regravado cifrado por
//! [`seal_file_in_place`]. Valor que não decifra (master.key trocada) FICA no documento como o
//! ciphertext original — apagá-lo aqui faria o próximo `save`/`remove` de QUALQUER servidor regravar
//! o arquivo sem ele, perda permanente (a `master.key` antiga de volta já não teria o que abrir). O
//! que ele não faz é chegar ao servidor: [`open_specs`] e `editor::upsert` o excluem do `env` que
//! sobe o processo (que falha com mensagem própria em vez de receber `enc:aes256:...` como se fosse
//! a chave), e [`unreadable_env`] diz à UI quais variáveis estão assim. Num documento já aberto,
//! valor ainda com o prefixo `enc:aes256:` é, por construção, um valor ilegível.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use super::config::ServerSpec;
use super::editor;
use crate::config::store::ConfigStore;

/// Todo `env` (objeto) de todo servidor, com o nome do servidor para o log.
fn env_maps(doc: &mut Value) -> Vec<(String, &mut serde_json::Map<String, Value>)> {
    let Some(root) = doc.as_object_mut() else { return Vec::new() };
    let mut maps = Vec::new();
    for (key, servers) in root.iter_mut() {
        if key != "mcpServers" && key != "mcp_servers" {
            continue;
        }
        let Some(servers) = servers.as_object_mut() else { continue };
        for (server, entry) in servers.iter_mut() {
            if let Some(env) = entry.get_mut("env").and_then(Value::as_object_mut) {
                maps.push((server.clone(), env));
            }
        }
    }
    maps
}

/// Ao ABRIR: valor que não decifra continua como ciphertext (ver o cabeçalho), com log do NOME.
fn open_values(doc: &mut Value, store: &ConfigStore) {
    for (server, env) in env_maps(doc) {
        for name in env.keys().cloned().collect::<Vec<_>>() {
            let Some(text) = env.get(&name).and_then(Value::as_str) else { continue };
            match store.open_secret(text) {
                Ok(plain) => {
                    env.insert(name, Value::String(plain));
                }
                Err(error) => {
                    log::warn!("mcp: env {name} do servidor {server} não decifrou ({error}) — mantido cifrado, fora do processo");
                }
            }
        }
    }
}

/// Ao GRAVAR: falha ao cifrar ABORTA — nunca apaga nem grava em claro o segredo do usuário.
fn seal_values(doc: &mut Value, store: &ConfigStore) -> Result<(), String> {
    for (server, env) in env_maps(doc) {
        for name in env.keys().cloned().collect::<Vec<_>>() {
            let Some(text) = env.get(&name).and_then(Value::as_str) else { continue };
            let sealed = store.seal_secret(text).map_err(|error| format!("env {name} do servidor {server} não foi cifrado: {error}"))?;
            env.insert(name, Value::String(sealed));
        }
    }
    Ok(())
}

fn has_plaintext_env(doc: &Value) -> bool {
    let mut probe = doc.clone();
    env_maps(&mut probe)
        .iter()
        .any(|(_, env)| env.values().filter_map(Value::as_str).any(|text| !crate::config::crypto::Encryption::is_encrypted(text)))
}

/// O documento com todo `env` em texto claro, pronto para `editor::upsert`/`resolve_env`.
pub fn load_plain_document(data_dir: &Path, store: &ConfigStore) -> Result<Value, String> {
    let mut doc = editor::load_document(data_dir)?;
    open_values(&mut doc, store);
    Ok(doc)
}

/// Cifra todo `env` e grava (atômico, 0600). O `doc` do chamador continua em texto claro.
pub fn write_sealed_document(data_dir: &Path, doc: &Value, store: &ConfigStore) -> Result<(), String> {
    let mut sealed = doc.clone();
    seal_values(&mut sealed, store)?;
    editor::write_atomic(data_dir, &sealed)
}

/// Migração de `mcp.json` legado: se há `env` em texto claro, regrava cifrado (e 0600). Sem
/// `mcp.json` ou com JSON quebrado não toca em nada. Devolve se regravou.
pub fn seal_file_in_place(data_dir: &Path, store: &ConfigStore) -> Result<bool, String> {
    if !data_dir.join(super::config::MCP_CONFIG_FILE).exists() {
        return Ok(false);
    }
    editor::restrict_to_owner(data_dir);
    let doc = editor::load_document(data_dir)?;
    if !has_plaintext_env(&doc) {
        return Ok(false);
    }
    write_sealed_document(data_dir, &doc, store)?;
    Ok(true)
}

/// Nomes do `env` (valores como estão no ARQUIVO) que a `master.key` atual não decifra — o que a UI
/// avisa. Texto claro (arquivo legado) é legível.
pub fn unreadable_env(spec: &ServerSpec, store: &ConfigStore) -> Vec<String> {
    spec.env.iter().filter(|(_, value)| store.open_secret(value).is_err()).map(|(name, _)| name.clone()).collect()
}

/// `ServerSpec`s lidos por `config::read_servers` têm o `env` como está no arquivo; abre os cifrados.
pub fn open_specs(servers: &mut BTreeMap<String, ServerSpec>, store: &ConfigStore) {
    for (name, spec) in servers.iter_mut() {
        spec.env.retain(|var, value| match store.open_secret(value) {
            Ok(plain) => {
                *value = plain;
                true
            }
            Err(error) => {
                log::warn!("mcp: env {var} do servidor {name} não decifrou ({error}) — não passado ao processo");
                false
            }
        });
    }
}

#[cfg(test)]
#[path = "env_secrets_tests.rs"]
mod tests;
