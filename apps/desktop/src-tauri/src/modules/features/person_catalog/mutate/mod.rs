//! Escritas sobre o índice — porte de
//! `apps/desktop/src/features/person-catalog/services/person-catalog-mutations.ts` +
//! `person-upsert.ts` + `person-catalog-deletion.ts` + `person-catalog-moderation.ts` +
//! `person-catalog-merge-ops.ts::mergePersons` + `person-merge.ts::mergeRecords`. Toda função aqui
//! recebe `&mut PersonCatalogIndex` já com o lock segurado por [`super::store::CatalogStore`] —
//! nenhuma toca disco. Cinco arquivos por responsabilidade ([`shared`] helpers comuns, [`upsert`] o
//! caminho mais denso, [`lifecycle`] menção/apagar, [`moderation`] bloqueio, [`merge`] fusão de dois
//! registros) em vez de um só — o original ultrapassava 300 linhas.
//!
//! [`moderation`] e [`merge`] nasceram nesta rodada para `person_catalog_block`/`_unblock`/
//! `_blocked_list`/`_merge` (RPC de `mika:plugin-rpc`, ver `super::rpc`) — ficaram de fora da
//! primeira passada porque não são ação da tool (`tools/schema.ts::ACTIONS` não as lista), não
//! porque fossem descartadas.
//!
//! **Não portado, de propósito** (nem ação da tool, nem método RPC — só chamadas no boot do plugin
//! TS): `autoMergeExactDuplicates`, `purgeNonPersonRecords`, `importFromSocial`.
//!
//! **Emissão de evento não portada.** O TS publica `Events.PERSON_CREATED/UPDATED/DELETED/
//! MENTION_ADDED` (`packages/sdk/src/svc/events.ts`) a cada mutação. O barramento Rust
//! (`crate::events`) não declara essas chaves ainda — adicioná-las mexeria num arquivo compartilhado
//! fora deste diretório, e nenhum assinante Rust existe hoje para elas. Lacuna nomeada, não fingida.

mod enrich;
mod lifecycle;
mod merge;
mod moderation;
mod shared;
mod upsert;

pub(super) use enrich::apply_enrichment_patch;
pub(super) use lifecycle::{add_mention_to_person, delete_person, set_relationship_type};
pub(super) use merge::{merge_two_persons, MergeOutcome};
pub(super) use moderation::{block_person, list_blocked, unblock_person, BlockOutcome};
pub(super) use shared::rebuild_alias_map;
pub(super) use upsert::upsert_person;
