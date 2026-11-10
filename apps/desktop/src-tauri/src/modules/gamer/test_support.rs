//! Dublê de filesystem só para teste (`#[cfg(test)]`) — permitido por AGENTS.md; nunca compilado
//! em produção. Cria um diretório escrachável único por teste e apaga no `Drop`, mesmo em panic.

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::registry::{Namespace, SchemaRegistry};
use crate::config::schema::boolean;
use crate::config::store::ConfigStore;

/// `ConfigStore` real com só o namespace `gamer-companion.enabled` declarado — usado pelos testes
/// de toggle mestre da feature (`feature.rs`, `game_watcher.rs`, `game_moment_commentary.rs`,
/// tools) que só precisam ler/escrever essa chave, sem montar o schema inteiro do namespace.
/// `ScratchDir` acompanha o `ConfigStore` para não ser dropado antes dele.
pub fn empty_config_store() -> (ConfigStore, ScratchDir) {
    let dir = ScratchDir::new("empty-config-store");
    let schema =
        SchemaRegistry::new(vec![Namespace { name: "gamer-companion", fields: vec![boolean("enabled", true)] }]);
    (ConfigStore::open(dir.path(), schema).unwrap(), dir)
}

pub struct ScratchDir(PathBuf);

impl ScratchDir {
    pub fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mika-gamer-test-{prefix}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("failed to create scratch dir");
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
