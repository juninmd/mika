//! Memória episódica — porte de `apps/desktop/src/main/services/memory/episodic/`.
//!
//! Barril do diretório: só declara os submódulos e reexporta a fachada. A divisão em arquivos veio
//! do TS (um por responsabilidade: escrita, consulta, busca, consolidação, contexto) e a regra de
//! 300 linhas do AGENTS.md a manteve.
//!
//! **Sem canal de IPC ainda.** Nada aqui é servido por `bridge.rs`: o canal continua no sidecar.
//! O que existe é a lógica, testada, pronta para o dia em que o canal for reivindicado.
//!
//! **Leitura exposta ao crate (2026-09-22).** `EpisodicMemoryService` passa a `pub(crate)` — a
//! MESMA fachada usada internamente, sem duplicar lógica de ordenação/TTL/poda em nenhum outro
//! lugar; seus métodos já devolvem `Vec<MemoryEntry>`, que não precisa de reexport próprio porque
//! ninguém fora daqui nomeia o tipo (só lê os campos `pub`/serializa via `Serialize`). Chamadores
//! fora deste diretório: `modules::brain::tools::memory_console`
//! (`snapshot`/`search_episodic`/`clear_episodic`, e agora `delete_episodic` via
//! [`service::EpisodicMemoryService::delete`]), `modules::features::privacy::list_memories`
//! (só leitura) e, desde a mesma rodada que `delete_episodic`,
//! `modules::features::person_catalog::tool.rs::handle_delete` via
//! [`service::EpisodicMemoryService::forget_mentions`] (a metade episódica de
//! `forgetPersonEverywhere` — a metade social mora em `modules::memory::social`). Ver
//! `modules::memory::episodic_reader()` para como os leitores sem escrita chegam numa instância sem
//! precisar de `ConfigStore`/`AppHandle` no próprio construtor. Busca semântica e consolidação
//! continuam privadas — nada além do necessário para leitura/apagar escapa deste submódulo.

mod consolidation;
mod context;
mod embedding;
mod queries;
mod search;
mod service;
mod store;
mod types;
mod write;

pub(crate) use embedding::{clear_caches as clear_embedding_caches, fact_similarity_to, test_embedding, warm_fact_vectors};
pub(crate) use search::tokenize_query;
pub(crate) use service::EpisodicMemoryService;
