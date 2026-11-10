//! O config store em Rust — alicerce da migração do backend.
//!
//! 62 arquivos do backend TypeScript leem configuração. Enquanto ela viver no sidecar Node, nenhum
//! outro módulo pode migrar de verdade: ou leria config pelo IPC de volta ao Node (inventando uma
//! dependência ao contrário), ou teria duas fontes da verdade. Por isso este é o primeiro módulo
//! portado, e não os menores — a ordem que o plano tinha no começo estava errada e foi corrigida
//! quando `telemetry` mostrou depender de `IConfigStore` e do banco.
//!
//! Ver `docs/tauri-rust-migration.md`.

pub mod atomic;
pub mod crypto;
pub mod locale;
pub mod registry;
pub mod schema;
pub mod schemas;
pub mod store;
