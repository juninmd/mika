//! As "features tardias" em Rust — `mood`, `desktop`, `scheduler`, `user`, `character`,
//! `person-catalog`, `privacy`, mais `mcp`/`gamer-companion` (frentes próprias, fora daqui).
//!
//! São 9 no TypeScript (`apps/desktop/src/features/<ctx>/index.ts`, ~11,1k LOC de backend).
//! **`desktop` aqui NÃO é `modules/desktop.rs`** — aquele nome já existe em Rust para outra coisa
//! (auto-launch do SO + assets do wizard); a feature tardia `desktop` (a tool `launch`/`window`/
//! `process`) é distinta e mora em [`desktop`] (submódulo deste diretório). Ver o cabeçalho de
//! `desktop.rs` para o porquê da colisão de nome não ser um erro.
//!
//! **`code_runner` não tem predecessor TypeScript** — é tool nova (`code`, action `run_js`),
//! Rust-nativa desde o primeiro commit. Ver o cabeçalho de `code_runner/mod.rs`.
//!
//! Cada uma registra tools e declarações de settings no boot, e é isso que enche
//! `mika:tools-list`, `mika:tools-list-by-plugin` e `mika:settings-declarations`. Enquanto elas não
//! executarem em Rust, aqueles canais têm o que listar apenas em parte — por isso este módulo
//! existe como frente separada e não como apêndice de outro.
//!
//! Nenhuma feature aqui reivindica canal de IPC (`modules()` sempre devolve `vec![]`) — quem lista
//! o que já existe é o `ToolRegistry` (`crate::modules::tools::registry()`).
//!
//! **`person_catalog` e `privacy` ganharam tool.** [`privacy::register`] (segunda passada, com
//! `ConfigStore`+`AppHandle` — ver o cabeçalho de `privacy/mod.rs`) e `person_catalog::register_tools`
//! (idem, só `AppHandle` — ver abaixo) fecham duas das cinco lacunas nomeadas em
//! `modules/tools/mod.rs` (`clear_history`/`list_memories`/`export_data`/`person_catalog`/`voice`).
//!
//! ## Duas passadas de registro (rodada 2026-09-21, `ebb5d7eb`)
//!
//! [`modules()`] abaixo é a PRIMEIRA passada — roda dentro de `native::registry()`, ANTES do Tauri
//! montar o app, sem `AppHandle`/`ConfigStore`. Só [`mood`] usa essa passada (`register_tools`,
//! sem persistência de afinidade — ver o cabeçalho de `mood/mod.rs`) e [`desktop`] (idem).
//!
//! [`mood::register`], [`user::register`], [`character::register`] e [`scheduler::register`] são a
//! SEGUNDA passada — `pub fn register(...)` que `modules::register_config_bound_tools(app)`
//! (`modules/mod.rs`, fora do meu escopo, chamado de `lib.rs::setup`) chama depois que o
//! `AppHandle` existe. `mood`/`user`/`character` recebem `Arc<ConfigStore>`; `scheduler` recebe
//! `&AppHandle` direto (precisa de `crate::state::app_data_dir(app)` para `scheduler.db`, não só do
//! `ConfigStore` — ver o cabeçalho de `scheduler/mod.rs`). As assinaturas exatas estão no relatório
//! desta rodada.
//!
//! **`person_catalog::register_tools` migrou da primeira passada para a segunda nesta rodada** (o
//! store que ele passou a abrir precisa de `crate::state::app_data_dir(app)` para achar `catalog.json` —
//! ver o cabeçalho de `person_catalog/store.rs`). `modules()` abaixo não chama mais
//! `person_catalog::register_tools()`; quem chama agora é `modules::register_config_bound_tools`
//! (`modules/mod.rs`, fora deste diretório — uma linha cirúrgica adicionada lá, nomeada no relatório
//! desta rodada por pedido explícito do coordenador).
//!
//! `mika:settings-declarations`/`mika:plugin-ui-catalog` são alimentados por
//! `crate::feature_registry::shared()` (frente própria, fora do meu escopo — não construo nem
//! edito, só consumo a API pública). [`settings_declaration`] monta a `Declaration` a partir do
//! `SchemaRegistry` já portado (`config/schemas/`); `user::register`/`character::register` chamam
//! `feature_registry::shared().register(...)` no fim do próprio `register(store)`. `scheduler` e
//! `person_catalog` têm `feature.rs` próprio, declarado à mão no padrão de `mcp`/`privacy`, e são
//! registrados por `modules::register_config_bound_tools` — os namespaces dos dois já existem em
//! `config/schemas/misc.rs`.
//!
//! Invariante do repositório que vale aqui: núcleo nunca importa de feature. O caminho de volta é
//! token de DI tolerando ausência, nunca `use` direto.

pub mod character;
/// `pub` (não `mod` privado): `modules::register_config_bound_tools` (`modules/mod.rs`, fora do
/// meu escopo) precisa chamar `computer_use::register(store)` — ver o cabeçalho de
/// `computer_use/mod.rs`. Sem primeira passada (ao contrário de `mood`/`desktop`): a tool `computer`
/// não tem capacidade config-livre que valha expor antes do `ConfigStore` existir.
pub mod computer_use;
/// Config-livre, primeira passada só — sem segunda passada, ao contrário de [`desktop`]/[`mood`]:
/// a tool `code` não depende de `AppHandle`/`ConfigStore` nenhum. Ver o cabeçalho de
/// `code_runner/mod.rs`.
mod code_runner;
/// `pub` pela mesma razão de [`mood`]: desde 2026-09-22 `desktop` também tem segunda passada —
/// `modules::register_config_bound_tools` chama `desktop::register(store)` para trocar o handler
/// sem config (onde `launch` degrada nomeado) pelo que abre app/URL de verdade.
pub mod desktop;
/// `pub` (não `mod` privado): `modules::register_config_bound_tools` (`modules/mod.rs`, fora do
/// meu escopo) precisa chamar `mood::register(store)` — ver o cabeçalho de `mood/mod.rs`.
pub mod mood;
/// `pub` pela mesma razão de [`mood`]: `modules::register_config_bound_tools` precisa chamar
/// `person_catalog::register_tools(app)` — ver o cabeçalho de `person_catalog/mod.rs`.
pub mod person_catalog;
/// `pub` pela mesma razão de [`mood`]: `modules::register_config_bound_tools` precisa chamar
/// `privacy::register(store, app)` — ver o cabeçalho de `privacy/mod.rs`.
pub mod privacy;
pub mod scheduler;
mod settings_declaration;
pub mod user;

use crate::native::NativeModule;

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    mood::register_tools();
    desktop::register_tools();
    code_runner::register_tools();
    // Handler-tampão sem `AppHandle` — garante que `person_catalog` já aparece em
    // `mika:tools-list`/`ToolRegistry::get` nesta passada; `register_tools(app)` (segunda
    // passada, `modules::register_config_bound_tools`) sobrescreve pelo handler de verdade
    // assim que o store de `catalog.json` puder ser aberto (ver o cabeçalho de
    // `person_catalog/tool.rs`, struct `StubHandler`).
    person_catalog::register_stub();
    vec![]
}

#[cfg(test)]
mod tests {
    /// `modules()` continua sem reivindicar canal nenhum — nada de invoke aqui, só registro de
    /// tools como efeito colateral (ver o cabeçalho do arquivo).
    #[test]
    fn claims_no_channels() {
        assert!(super::modules().is_empty());
    }
}
