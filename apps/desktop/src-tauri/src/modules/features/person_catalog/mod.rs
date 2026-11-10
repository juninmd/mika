//! Catálogo de pessoas — porte de `apps/desktop/src/features/person-catalog/`.
//!
//! Barril do diretório. Nesta rodada o STORE nasceu: [`store::CatalogStore`] lê/escreve o mesmo
//! `catalog.json` que `PersonCatalogService` (TS) já grava
//! (`<app_data_dir>/plugins/mika.person-catalog/catalog.json`, ver o cabeçalho de `store.rs` para
//! por que é JSON e não SQLite), e [`tool`] passou a dispachar 7 das 11 ações de verdade contra ele
//! (`upsert`/`get`/`search`/`list`/`delete`/`add_mention`/`update_social` — ver a tabela no
//! cabeçalho de `tool.rs`). As outras 4 (`add_fact`/`delete_fact`/`add_relationship`/
//! `delete_relationship`) continuam degradando, agora nomeando a causa real: dependem do grafo
//! social do `modules::brain`, não deste store.
//!
//! `register_tools` passou a exigir `&AppHandle` (para resolver o caminho de `catalog.json`) e por
//! isso migrou da primeira passada de registro (`features::modules()`, sem `AppHandle`) para a
//! segunda (`modules::register_config_bound_tools`, fora deste diretório — mudança cirúrgica única,
//! nomeada no relatório desta rodada).
//!
//! `pii::redact_pii` é o chokepoint de redação: [`tool`] e [`rpc`] chamam ANTES de qualquer escrita
//! — bio, notas, valor de atributo, texto de menção — nunca depois. Nada sobre uma pessoa (nome,
//! relacionamento, foto) é logado; erros de armazenamento só nomeiam o tipo de falha, nunca o dado.
//!
//! [`rpc`] nasceu nesta rodada: os 10 métodos `person_catalog_*` de `mika:plugin-rpc`
//! (`super::super::plugins::rpc`) que o painel de Configurações e a janela de listagem usam, todos
//! contra o MESMO `CatalogStore` que [`tool`] já usa (via [`catalog_store`]); a foto por URL mora em
//! [`image`]. Registrados por `modules::register_config_bound_tools`.

mod app_context;
mod enrichment;
mod events;
mod forget;
mod image;
mod ingest;
mod lifecycle;
mod mention_extract;
mod mention_gate;
mod merge_fields;
mod mutate;
mod normalize;
pub(crate) mod people_memory;
mod plugin_test;
mod query;
mod rpc;
mod search_extract;
mod store;
mod store_singleton;

pub mod feature;
pub mod pii;
mod tool;
pub mod types;

pub(crate) use events::publish_updated as publish_person_updated;
pub(crate) use forget::forget_by_name as forget_person_everywhere;
pub(crate) use lifecycle::spawn_lifecycle_listener;
pub(crate) use rpc::register as register_rpc;
pub(crate) use store::{db_path, CatalogStore};
pub(crate) use store_singleton::{catalog_reader, catalog_store};
pub(crate) use tool::{register_stub, register_tools};
