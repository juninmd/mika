//! Persistência do catálogo — porte de
//! `apps/desktop/src/features/person-catalog/services/person-catalog-store.ts` +
//! `person-catalog.service.ts`.
//!
//! **JSON, não SQLite — divergência deliberada do padrão de `session_db.rs`/`scheduler/store.rs`.**
//! A TS não guarda o catálogo em SQLite: `PersonCatalogService` lê/escreve um `catalog.json` único
//! (`{version, persons: PersonRecord[], aliasMap, blocked?, socialImportedAt?}`), gravado
//! atomicamente via tempfile+rename (`writeCatalogIndex`), com backup-e-recomeço em corrupção
//! (`readCatalogIndex`). [`super::types::PersonCatalogIndex`] já foi portado ONTEM exatamente para
//! bater byte a byte com esse arquivo (ver o cabeçalho de `types.rs`: "o arquivo em disco é
//! literalmente o `PersonCatalogIndex` que o TypeScript já escreve"). Trocar isso por SQLite exigiria
//! desmontar um documento aninhado (menções/atributos/relacionamentos embutidos por pessoa) em
//! várias tabelas — o oposto de "casar com o modelo de dados da TS" que esta tarefa pede. `rusqlite`
//! continua reservado para os módulos cujo TS já é relacional (`session_db.rs`, `scheduler/store.rs`).
//!
//! `<app_data_dir>/plugins/mika.person-catalog/catalog.json` — mesma cadeia de resolução que
//! `db_path` de `gamer/session_db.rs` já confirmou (`pluginDataDir = join(dataRootDir, manifest.id)`,
//! `dataRootDir = join(userData, 'plugins')`, `manifest.id` de `person-catalog/index.ts`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::AppHandle;
use uuid::Uuid;

use super::enrichment::EnrichmentPatch;
use super::mutate::{
    add_mention_to_person, apply_enrichment_patch, block_person, delete_person, list_blocked as mutate_list_blocked, merge_two_persons,
    set_relationship_type, unblock_person, upsert_person, BlockOutcome, MergeOutcome,
};
use super::people_memory::{self, Ambiguity, ApplyCtx, ApplyReport, PeopleContext, PersonOp};
use super::query::{find_person_by_alias, get_person, list_persons, search_persons};
use super::types::{NewMention, PendingClarification, PersonCatalogIndex, PersonRecord, PersonType, RelationshipType, UpsertInput, UpsertOutcome};

const CATALOG_FILE: &str = "catalog.json";
const INDEX_VERSION: u32 = 2;

/// Mesmo gerador de `PersonCatalogService::id()` — `pc_<8 hex>`, reusado para id de pessoa E de
/// menção (ver o cabeçalho de `mutate/shared.rs::IdGen`).
pub(super) fn generate_id() -> String {
    let hex = Uuid::new_v4().simple().to_string();
    format!("pc_{}", &hex[..8])
}

pub struct CatalogStore {
    index: Mutex<PersonCatalogIndex>,
    path: PathBuf,
    /// Porte de `PersonCatalogService.loadFailed` — arquivo existia mas não deu pra ler (não
    /// `ENOENT`) ou estava corrompido E o backup falhou. `save` recusa gravar por cima nesse
    /// estado, mesma política do TS ("refusing to overwrite it").
    load_failed: bool,
}

impl CatalogStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("não criou {}: {e}", parent.display()))?;
        }
        let (index, load_failed) = load_index(path);
        Ok(Self { index: Mutex::new(index), path: path.to_path_buf(), load_failed })
    }

    fn save(&self, index: &PersonCatalogIndex) -> Result<(), String> {
        if self.load_failed {
            return Err("Person catalog was not loaded successfully; refusing to overwrite the file on disk.".into());
        }
        let json = serde_json::to_string_pretty(index).map_err(|e| format!("serializar catálogo: {e}"))?;
        crate::config::atomic::write_atomic(&self.path, json.as_bytes()).map_err(|e| format!("gravar {}: {e}", self.path.display()))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, PersonCatalogIndex> {
        self.index.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, id: &str) -> Option<PersonRecord> {
        get_person(&self.lock(), id).cloned()
    }

    pub fn find_by_alias(&self, alias: &str) -> Option<PersonRecord> {
        find_person_by_alias(&self.lock(), alias).cloned()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<PersonRecord> {
        search_persons(&self.lock(), query, limit).into_iter().cloned().collect()
    }

    pub fn list(&self, kind: Option<PersonType>, sort_by: Option<&str>, limit: usize) -> Vec<PersonRecord> {
        list_persons(&self.lock(), kind, sort_by, limit).into_iter().cloned().collect()
    }

    pub fn upsert(&self, input: UpsertInput, now: i64, max_mentions: usize) -> Result<UpsertOutcome, String> {
        let mut index = self.lock();
        let outcome = upsert_person(&mut index, input, now, max_mentions, generate_id);
        if outcome.person.is_some() {
            self.save(&index)?;
        }
        Ok(outcome)
    }

    pub fn add_mention(&self, identifier: &str, mention: NewMention, now: i64, max_mentions: usize) -> Result<PersonRecord, String> {
        let mut index = self.lock();
        let person = add_mention_to_person(&mut index, identifier, mention, now, max_mentions, generate_id)?;
        self.save(&index)?;
        Ok(person)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let mut index = self.lock();
        delete_person(&mut index, id)?;
        self.save(&index)
    }

    /// specs/08-gamer-companion.md ("Pessoas no jogo") — único chamador de produção:
    /// `gamer::game_partners::promote_recurring_partner`.
    pub fn set_relationship_type(&self, id: &str, relationship_type: RelationshipType, now: i64) -> Result<PersonRecord, String> {
        let mut index = self.lock();
        let person = set_relationship_type(&mut index, id, relationship_type, now)?;
        self.save(&index)?;
        Ok(person)
    }

    /// Porte de `PersonCatalogService.block` — bloqueia a chave (nome + aliases quando a pessoa
    /// existe) E apaga o registro catalogado, mesma ordem do TS (`blockPerson`): a lista de
    /// bloqueio sobrevive à remoção do registro, então um upsert futuro com o mesmo nome é
    /// recusado por [`super::mutate::upsert_person`]'s `is_blocked`.
    pub fn block(&self, identifier: &str) -> Result<BlockOutcome, String> {
        let mut index = self.lock();
        let outcome = block_person(&mut index, identifier);
        self.save(&index)?;
        Ok(outcome)
    }

    /// Porte de `PersonCatalogService.unblock` — erro nomeado quando a chave não estava bloqueada
    /// (mesmo `fail('Not blocked: ...')` do TS), nunca um sucesso silencioso.
    pub fn unblock(&self, identifier: &str) -> Result<Vec<String>, String> {
        let mut index = self.lock();
        let blocked = unblock_person(&mut index, identifier)?;
        self.save(&index)?;
        Ok(blocked)
    }

    /// Porte de `PersonCatalogService.listBlocked` — leitura pura, sem gravar.
    pub fn list_blocked(&self) -> Vec<String> {
        mutate_list_blocked(&self.lock())
    }

    /// Porte de `PersonCatalogService.merge` — funde `source` em `target` e apaga `source`; nunca
    /// grava quando `already_merged` (fonte ausente ou igual ao alvo), mesma condição de
    /// `mergePersons` do TS.
    pub fn merge(&self, target_id: &str, source_id: &str, now: i64, max_mentions: usize) -> Result<MergeOutcome, String> {
        let mut index = self.lock();
        let outcome = merge_two_persons(&mut index, target_id, source_id, now, max_mentions)?;
        if !outcome.already_merged {
            self.save(&index)?;
        }
        Ok(outcome)
    }

    /// Aplica um [`EnrichmentPatch`] (`super::enrichment::enrich_person`) sobre a pessoa `id` —
    /// porte de `service.patch(id, patch)` no handler `PERSON_CREATED` de
    /// `person-catalog-lifecycle.ts`. `None` quando `id` não existe mais (pessoa apagada/bloqueada
    /// entre o disparo do enriquecimento e a gravação); nunca cria um registro novo.
    pub(super) fn apply_enrichment(&self, id: &str, patch: EnrichmentPatch, now: i64) -> Result<Option<PersonRecord>, String> {
        let mut index = self.lock();
        let person = apply_enrichment_patch(&mut index, id, patch, now);
        if person.is_some() {
            self.save(&index)?;
        }
        Ok(person)
    }

    /// Post-turn people memory: resolve + write the extractor's ops (see `people_memory`). Saves only
    /// when something changed; publishes one `PersonUpdated` per touched person.
    pub(super) fn apply_people_ops(&self, ops: &[PersonOp], text: &str, now: i64, max_mentions: usize) -> Result<ApplyReport, String> {
        let mut index = self.lock();
        let report = people_memory::apply_ops(&mut index, ops, &ApplyCtx { text, now, gen_id: generate_id, max_mentions });
        if report.changed() {
            self.save(&index)?;
            for id in &report.touched {
                if let Some(person) = get_person(&index, id) {
                    super::events::publish_updated(person, report.created.contains(id));
                }
            }
        }
        Ok(report)
    }

    /// Chat-turn read side: cards pulled in by name/alias/relation, ambiguities to ask about now,
    /// and clarifications opened by an earlier extraction that Mika has not asked yet (marked asked
    /// here, so each is asked once).
    pub(crate) fn people_for_turn(&self, text: &str, limit: usize) -> (PeopleContext, Vec<Ambiguity>) {
        let mut index = self.lock();
        let context = people_memory::prompt_context(&index, text, limit);
        let mut unasked = people_memory::unasked_clarifications(&index);
        unasked.retain(|u| !context.ambiguities.iter().any(|a| a.relation == u.relation));
        if !unasked.is_empty() {
            index.pending_clarifications.iter_mut().for_each(|p| p.asked = true);
            if let Err(error) = self.save(&index) {
                log::warn!("people memory: could not mark clarifications asked: {error}");
            }
        }
        (context, unasked)
    }

    pub(crate) fn pending_clarifications(&self) -> Vec<PendingClarification> {
        self.lock().pending_clarifications.clone()
    }

    /// Settings UI: "it was Pedro" (`person_id`) or dismiss (`None`).
    pub(super) fn settle_clarification(&self, pending_id: &str, person_id: Option<&str>, now: i64) -> Result<bool, String> {
        let mut index = self.lock();
        let ctx = ApplyCtx { text: "", now, gen_id: generate_id, max_mentions: 0 };
        let settled = people_memory::settle(&mut index, pending_id, person_id, &ctx);
        if settled {
            self.save(&index)?;
        }
        Ok(settled)
    }

    /// Settings UI edit of one fact; an empty text deletes it.
    pub(super) fn edit_fact(&self, person_id: &str, fact_id: &str, text: Option<&str>, now: i64) -> Result<PersonRecord, String> {
        let mut index = self.lock();
        let person = index.persons.iter_mut().find(|p| p.id == person_id).ok_or_else(|| format!("Person not found: {person_id}"))?;
        let position = person.facts.iter().position(|f| f.id == fact_id).ok_or_else(|| format!("Fact not found: {fact_id}"))?;
        match text.map(str::trim).filter(|t| !t.is_empty()) {
            Some(text) => {
                person.facts[position].text = text.to_string();
                person.facts[position].confidence = 1.0;
                person.facts[position].updated_at = now;
            }
            None => {
                person.facts.remove(position);
            }
        }
        person.updated_at = now;
        let updated = person.clone();
        self.save(&index)?;
        Ok(updated)
    }

    /// Settings UI: drop one alias chip.
    pub(super) fn remove_alias(&self, person_id: &str, alias: &str, now: i64) -> Result<PersonRecord, String> {
        let mut index = self.lock();
        let person = index.persons.iter_mut().find(|p| p.id == person_id).ok_or_else(|| format!("Person not found: {person_id}"))?;
        person.aliases.retain(|a| a != alias);
        person.updated_at = now;
        let updated = person.clone();
        super::mutate::rebuild_alias_map(&mut index);
        self.save(&index)?;
        Ok(updated)
    }

    /// Espelha `PersonCatalogService.clearAll`: `persons`/`aliasMap`/`blocked` voltam a vazio, mas
    /// `socialImportedAt` sobrevive — mesma política do TS (lida linha a linha, não assumida). Não
    /// cobre o cache de fotos em disco (`PersonImageCache.clearAll`) — isso é responsabilidade de
    /// quem chama (ver `features::privacy::wipe_files`), porque este store nunca soube de fotos.
    pub fn clear_all(&self) -> Result<usize, String> {
        let mut index = self.lock();
        let removed = index.persons.len();
        let index_new = PersonCatalogIndex {
            version: INDEX_VERSION,
            persons: vec![],
            alias_map: Default::default(),
            blocked: None,
            social_imported_at: index.social_imported_at,
            pending_clarifications: vec![],
        };
        self.save(&index_new)?;
        *index = index_new;
        Ok(removed)
    }
}

/// Porte de `readCatalogIndex` — `ENOENT` vira índice vazio (sem erro); qualquer outro erro de
/// leitura, ou corrupção cujo backup também falhe, marca `load_failed` (recusa sobrescrever depois).
/// JSON corrompido faz backup em `<path>.corrupt-<epoch_ms>.bak` e recomeça vazio, mesma política do
/// TS — nunca perde o arquivo original, só para de confiar nele.
fn load_index(path: &Path) -> (PersonCatalogIndex, bool) {
    let empty = PersonCatalogIndex { version: INDEX_VERSION, persons: vec![], alias_map: Default::default(), blocked: None, social_imported_at: None, ..Default::default() };

    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (empty, false),
        Err(e) => {
            log::error!("person catalog unreadable, refusing to overwrite it: {e}");
            return (empty, true);
        }
    };

    match serde_json::from_str::<PersonCatalogIndex>(&raw) {
        Ok(parsed) => {
            let mut index = PersonCatalogIndex { version: INDEX_VERSION, ..parsed };
            super::mutate::rebuild_alias_map(&mut index);
            (index, false)
        }
        Err(parse_error) => {
            let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let backup = path.with_extension(format!("json.corrupt-{now_ms}.bak"));
            match std::fs::rename(path, &backup) {
                Ok(()) => {
                    log::error!("person catalog corrupted ({parse_error}); original preserved at {}", backup.display());
                    (empty, false)
                }
                Err(rename_error) => {
                    log::error!("person catalog corrupted and backup failed, refusing to write: {rename_error}");
                    (empty, true)
                }
            }
        }
    }
}

/// `<app_data_dir>/plugins/mika.person-catalog/catalog.json` — ver o cabeçalho do arquivo.
pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?;
    Ok(data_dir.join("plugins").join("mika.person-catalog").join(CATALOG_FILE))
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
