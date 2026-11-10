//! Tools `clear_history`, `list_memories`, `export_data` — port de
//! `apps/desktop/src/features/privacy/tools/index.ts`. NÃO confundir com
//! `crate::modules::privacy` (top-level, fora do meu escopo): aquele é o CHOKEPOINT de rede/tela
//! (`assertNetworkAllowed`/`isScreenCaptureAllowed`) e a declaração de settings da aba `privacy`
//! (`privacy::feature::register_feature`, já chamada em produção). Este diretório é a feature
//! TARDIA (`apps/desktop/src/features/privacy/`) — as TOOLS de LLM, que `modules/privacy/mod.rs`
//! documentava como "não reivindico" numa rodada anterior.
//!
//! ## Por que esta rodada reivindica o que a anterior recusou
//!
//! A rodada anterior mediu 9 dependências do `PrivacyZdrModule`
//! (`MemoryService`/`BrowserService`/`ChatService`/`PersonsService`/`TTSFactory`/`TelemetryService`
//! + 2 caches de visão) como "NENHUMA portada" e concluiu que fingir `clear_history` exigiria
//! mockar todas as 9 — uma regressão de segurança disfarçada de progresso. Essa medição está
//! DESATUALIZADA: `crate::modules::memory::memory_store` (episódica), `crate::modules::features::
//! scheduler` (lembretes) e `crate::modules::voice` (cache de TTS) existem hoje com API pública, e
//! `apps/desktop/src/features/user/` (perfil) sempre foi `ConfigStore` puro. O design ORIGINAL do
//! TS já tolera isso: `wipeEveryStore`/`exportUserData` (`tools/index.ts`, `export-store.ts`) tratam
//! cada dependência do `PrivacyRuntime`/`ExportRuntime` como OPCIONAL — quando o container DI não
//! resolve um serviço (feature desativada, dependência não ativada), o `step()` correspondente
//! marca `skipped`/`not-covered` em vez de falhar. Este porte reproduz EXATAMENTE esse contrato: o
//! que este build de Rust alcança de verdade é limpo/exportado de verdade; o resto entra na MESMA
//! lista de "não coberto" que o TS já usaria se rodasse com metade dos serviços desativados — não é
//! um atalho inventado, é o comportamento de degradação que o desenho original já previa. Ver
//! [`clear`] e [`export`] para a lista exata de wired vs. skipped, medida linha a linha.
//!
//! `list_memories` (2026-09-22, atualizado): `crate::modules::memory` abriu uma porta estreita de
//! LEITURA (`episodic_reader()`) sobre o `EpisodicMemoryService` real — `get_all`/`search`/`clear`,
//! com a MESMA ordenação/TTL/poda de índice órfão que o submódulo PRIVADO
//! `crate::modules::memory::episodic` já testa, sem duplicar aquela lógica aqui por fora. A tool
//! deixa de degradar `unavailable` — ver [`list_memories`] para o recorte exato do que ela devolve.

mod clear;
mod clear_ext;
mod clear_more;
mod export;
mod export_ext;
mod list_memories;
mod plugin_test;
mod wipe_files;

use std::sync::Arc;

use tauri::AppHandle;

use crate::config::store::ConfigStore;

/// Chamado por `modules::register_config_bound_tools(app)`. Registra as 3 tools desta feature no
/// `ToolRegistry` nativo — `clear_history` via `register_destructive` (gate de aprovação),
/// `list_memories`/`export_data` via `register` normal (leitura, sem risco) — e o teste de conexão
/// de `mika:plugin-test` (`plugin_test::register`).
pub fn register(store: Arc<ConfigStore>, app: &AppHandle) -> Result<(), String> {
    clear::register(Arc::clone(&store), app)?;
    list_memories::register();
    plugin_test::register(Arc::clone(&store));
    export::register(store, app)?;
    Ok(())
}
