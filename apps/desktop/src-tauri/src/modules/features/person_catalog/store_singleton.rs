//! `CatalogStore` do processo — separado de `store.rs` só por causa do limite de 300 linhas do
//! arquivo, não por camada nova.
//!
//! **Por que isto precisa existir.** Antes de 2026-09-22, `tool::register_tools` era o ÚNICO
//! chamador de `CatalogStore::open`, então "uma instância por processo" acontecia por acaso. Isso
//! deixou de ser verdade quando `features::privacy::clear::register` passou a precisar do MESMO
//! catálogo para `clear_history`: cada `CatalogStore` mantém o índice INTEIRO em memória, e
//! `save()` regrava o arquivo por cima do que está lá. Duas instâncias independentes sobre o MESMO
//! `catalog.json` fariam a próxima escrita de uma desfazer o `clear_all()` da outra — a cópia em
//! memória mais velha "ressuscitaria" as pessoas apagadas no primeiro `save()` seguinte. Mesma
//! classe de bug que este repo já nomeou como "duas passadas de registro" em outra frente.

use std::sync::OnceLock;
use tauri::AppHandle;

use super::store::{db_path, CatalogStore};

static CATALOG_STORE: OnceLock<CatalogStore> = OnceLock::new();

/// O `CatalogStore` do processo, aberto sob demanda na primeira chamada — mesmo padrão de
/// `crate::modules::memory::memory_store`. Compartilhado de propósito entre `tool::register_tools`
/// e `features::privacy::clear::register` (ver o cabeçalho do arquivo).
pub fn catalog_store(app: &AppHandle) -> Result<&'static CatalogStore, String> {
    if let Some(store) = CATALOG_STORE.get() {
        return Ok(store);
    }
    let store = CatalogStore::open(&db_path(app)?)?;
    let _ = CATALOG_STORE.set(store);
    Ok(CATALOG_STORE.get().expect("acabou de inicializar"))
}

/// Leitura pull-based sem `AppHandle` — mesmo papel que `memory::memory_store_reader()` cumpre para
/// o `MemoryStore`: `brain::persona::compose_system_prompt` (turno de chat regular, síncrono) não
/// tem `AppHandle` no próprio ponto de chamada. `None` só antes do primeiro `catalog_store(app)` do
/// processo (nunca em runtime normal — `tool::register_tools` já roda no boot).
pub fn catalog_reader() -> Option<&'static CatalogStore> {
    CATALOG_STORE.get()
}
