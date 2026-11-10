//! Camada de tabela genérica sobre [`MemoryStore`] — porte de `social-store.ts` (`RedisSocialStore`)
//! + `social-redis-index.ts`. Cada "tabela" social é uma fatia por prefixo de chave dentro do MESMO
//! `memory_entries` que `episodic` já usa (ver o cabeçalho de `modules/memory/mod.rs`), com um
//! sorted-set de índice (`zAdd`/`zRangeByScore`) para não precisar de `KEYS`/`SCAN` a cada leitura.
//!
//! **Compatibilidade com dados já gravados pelo TypeScript**: como este é o MESMO arquivo
//! `memory.db` que o sidecar já escreve hoje, o índice pode não existir ainda para uma tabela cujos
//! registros foram gravados antes do índice existir (backfill preguiçoso, MESMA estratégia do TS:
//! [`ensure_indexed`]/[`ensure_subject_indexed`] populam o índice na primeira leitura, não no boot).

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::modules::memory::{MemoryStore, RangeLimit, ScoreBound, ZRangeOpts};

const PREFIX: &str = "mika:social";

fn record_key(table: &str, id: &str) -> String {
    format!("{PREFIX}:{table}:{id}")
}

fn table_pattern(table: &str) -> String {
    format!("{PREFIX}:{table}:*")
}

fn index_key(table: &str) -> String {
    format!("{PREFIX}:index:{table}")
}

fn subject_index_key(table: &str, subject: &str) -> String {
    format!("{PREFIX}:index:{table}:subject:{}", super::rules::normalize_text(subject))
}

fn subject_backfill_done_key(table: &str) -> String {
    format!("{PREFIX}:index:{table}:subject-backfill-done")
}

fn record_score(value: &Value) -> f64 {
    value
        .get("updatedAt")
        .and_then(Value::as_f64)
        .or_else(|| value.get("createdAt").and_then(Value::as_f64))
        .unwrap_or(0.0)
}

/// Varre os registros crus de uma tabela por prefixo de chave — só usado pelo backfill de índice.
fn scan_table(mem: &MemoryStore, table: &str) -> Result<Vec<(String, Value, f64)>, String> {
    let mut out = Vec::new();
    for key in mem.keys(&table_pattern(table))? {
        let Some(raw) = mem.get(&key)? else { continue };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else { continue };
        let id = match value.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => continue,
        };
        let score = record_score(&value);
        out.push((id, value, score));
    }
    Ok(out)
}

fn index_all(mem: &MemoryStore, table: &str, records: &[(String, Value, f64)]) -> Result<(), String> {
    for (id, value, score) in records {
        mem.z_add(&index_key(table), *score, id)?;
        if let Some(subject) = value.get("subject").and_then(Value::as_str) {
            mem.z_add(&subject_index_key(table, subject), *score, id)?;
        }
    }
    Ok(())
}

fn first_sample(mem: &MemoryStore, key: &str) -> Result<Vec<String>, String> {
    mem.z_range_by_score(key, ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts { rev: false, limit: Some(RangeLimit { offset: 0, count: 1 }) })
}

/// = `ensureIndexed`: roda o backfill só se o índice principal está vazio.
fn ensure_indexed(mem: &MemoryStore, table: &str) -> Result<(), String> {
    if !first_sample(mem, &index_key(table))?.is_empty() {
        return Ok(());
    }
    let records = scan_table(mem, table)?;
    index_all(mem, table, &records)
}

/// = `ensureSubjectIndexed`: gate PRÓPRIO (não reaproveita o sentinela de `ensure_indexed`), porque
/// o índice principal pode já estar populado por outro caminho sem o índice-por-subject ter rodado.
fn ensure_subject_indexed(mem: &MemoryStore, table: &str) -> Result<(), String> {
    let done_key = subject_backfill_done_key(table);
    if !first_sample(mem, &done_key)?.is_empty() {
        return Ok(());
    }
    let records = scan_table(mem, table)?;
    index_all(mem, table, &records)?;
    mem.z_add(&done_key, 1.0, "done")?;
    Ok(())
}

/// = `hydrate`: um id morto (registro apagado mas ainda no índice) é removido do índice em vez de
/// virar `null` na lista — auto-cura o mesmo jeito que o TS já faz.
fn hydrate<T: DeserializeOwned>(mem: &MemoryStore, table: &str, ids: &[String], subject_idx_key: Option<&str>) -> Result<Vec<T>, String> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        match mem.get(&record_key(table, id))? {
            None => {
                let _ = mem.z_rem(&index_key(table), id);
                if let Some(sub_key) = subject_idx_key {
                    let _ = mem.z_rem(sub_key, id);
                }
            }
            Some(raw) => {
                if let Ok(value) = serde_json::from_str::<T>(&raw) {
                    out.push(value);
                }
            }
        }
    }
    Ok(out)
}

pub(super) fn get_by_id<T: DeserializeOwned>(mem: &MemoryStore, table: &str, id: &str) -> Result<Option<T>, String> {
    match mem.get(&record_key(table, id))? {
        None => Ok(None),
        Some(raw) => serde_json::from_str(&raw).map(Some).map_err(|e| format!("social/{table}/{id}: registro corrompido ({e})")),
    }
}

/// = `getAll` (mais recente primeiro, sem teto — só chamado sobre tabelas do grafo social, que já
/// são naturalmente pequenas por usuário).
pub(super) fn get_all<T: DeserializeOwned>(mem: &MemoryStore, table: &str) -> Result<Vec<T>, String> {
    ensure_indexed(mem, table)?;
    let ids = mem.z_range_by_score(&index_key(table), ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts { rev: true, limit: None })?;
    hydrate(mem, table, &ids, None)
}

/// = `getRecentBySubject`: teto explícito, o que [`super::fact_store::add_fact`] usa para nunca
/// escanear o histórico inteiro de um subject.
pub(super) fn get_recent_by_subject<T: DeserializeOwned>(mem: &MemoryStore, table: &str, subject: &str, limit: i64) -> Result<Vec<T>, String> {
    ensure_subject_indexed(mem, table)?;
    let sub_key = subject_index_key(table, subject);
    let ids = mem.z_range_by_score(&sub_key, ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts { rev: true, limit: Some(RangeLimit { offset: 0, count: limit }) })?;
    hydrate(mem, table, &ids, Some(&sub_key))
}

pub(super) fn upsert<T: Serialize>(mem: &MemoryStore, table: &str, id: &str, score: f64, value: &T) -> Result<(), String> {
    let as_value = serde_json::to_value(value).map_err(|e| format!("social/{table}/{id}: falha ao serializar ({e})"))?;
    mem.set(&record_key(table, id), &as_value.to_string(), None)?;
    mem.z_add(&index_key(table), score, id)?;
    if let Some(subject) = as_value.get("subject").and_then(Value::as_str) {
        mem.z_add(&subject_index_key(table, subject), score, id)?;
    }
    Ok(())
}

/// = `delete`: apaga o registro, o índice principal e — quando o registro tinha um campo `subject`
/// (só a tabela de fatos tem) — o índice por subject também.
pub(super) fn delete(mem: &MemoryStore, table: &str, id: &str) -> Result<(), String> {
    let raw = mem.get(&record_key(table, id))?;
    mem.del(&record_key(table, id))?;
    mem.z_rem(&index_key(table), id)?;
    if let Some(raw) = raw {
        if let Ok(value) = serde_json::from_str::<Value>(&raw) {
            if let Some(subject) = value.get("subject").and_then(Value::as_str) {
                mem.z_rem(&subject_index_key(table, subject), id)?;
            }
        }
    }
    Ok(())
}

/// = `SocialMemoryService.deleteIfExists`.
pub(super) fn delete_if_exists(mem: &MemoryStore, table: &str, id: &str) -> Result<bool, String> {
    if get_by_id::<Value>(mem, table, id)?.is_none() {
        return Ok(false);
    }
    delete(mem, table, id)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct Row {
        id: String,
        #[serde(rename = "updatedAt")]
        updated_at: f64,
        #[serde(default)]
        subject: Option<String>,
    }

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn upsert_then_get_by_id_round_trips() {
        let mem = store();
        let row = Row { id: "a".into(), updated_at: 1.0, subject: None };
        upsert(&mem, "t", "a", 1.0, &row).unwrap();
        assert_eq!(get_by_id::<Row>(&mem, "t", "a").unwrap(), Some(row));
    }

    #[test]
    fn get_by_id_of_missing_record_is_none_not_an_error() {
        let mem = store();
        assert_eq!(get_by_id::<Row>(&mem, "t", "ghost").unwrap(), None);
    }

    #[test]
    fn get_all_orders_newest_first() {
        let mem = store();
        upsert(&mem, "t", "old", 1.0, &Row { id: "old".into(), updated_at: 1.0, subject: None }).unwrap();
        upsert(&mem, "t", "new", 2.0, &Row { id: "new".into(), updated_at: 2.0, subject: None }).unwrap();
        let all: Vec<Row> = get_all(&mem, "t").unwrap();
        assert_eq!(all.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec!["new".to_string(), "old".to_string()]);
    }

    /// Mesma classe de bug que `session_db.rs::history`/`game_locations.rs`/`z_range_by_score`
    /// documentam: score empatado (mesmo milissegundo) não pode devolver ordem instável — o
    /// desempate por `rowid` de `MemoryStore::z_range_by_score` é o que evita isso aqui.
    #[test]
    fn get_all_breaks_same_score_ties_deterministically() {
        let mem = store();
        upsert(&mem, "t", "first", 100.0, &Row { id: "first".into(), updated_at: 100.0, subject: None }).unwrap();
        upsert(&mem, "t", "second", 100.0, &Row { id: "second".into(), updated_at: 100.0, subject: None }).unwrap();
        let all: Vec<Row> = get_all(&mem, "t").unwrap();
        assert_eq!(
            all.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
            vec!["second".to_string(), "first".to_string()],
            "empate de score tem de desempatar pela ordem de inserção, não por acaso"
        );
    }

    #[test]
    fn get_recent_by_subject_only_returns_matching_subject_bounded_by_limit() {
        let mem = store();
        upsert(&mem, "t", "a1", 1.0, &Row { id: "a1".into(), updated_at: 1.0, subject: Some("ada".into()) }).unwrap();
        upsert(&mem, "t", "a2", 2.0, &Row { id: "a2".into(), updated_at: 2.0, subject: Some("ada".into()) }).unwrap();
        upsert(&mem, "t", "b1", 3.0, &Row { id: "b1".into(), updated_at: 3.0, subject: Some("bea".into()) }).unwrap();

        let ada: Vec<Row> = get_recent_by_subject(&mem, "t", "ada", 10).unwrap();
        assert_eq!(ada.len(), 2);
        assert!(ada.iter().all(|r| r.subject.as_deref() == Some("ada")));

        let capped: Vec<Row> = get_recent_by_subject(&mem, "t", "ada", 1).unwrap();
        assert_eq!(capped.len(), 1, "teto de leitura tem de ser respeitado");
    }

    #[test]
    fn delete_removes_record_from_main_and_subject_index() {
        let mem = store();
        upsert(&mem, "t", "a", 1.0, &Row { id: "a".into(), updated_at: 1.0, subject: Some("ada".into()) }).unwrap();
        delete(&mem, "t", "a").unwrap();

        assert_eq!(get_by_id::<Row>(&mem, "t", "a").unwrap(), None);
        assert!(get_all::<Row>(&mem, "t").unwrap().is_empty());
        assert!(get_recent_by_subject::<Row>(&mem, "t", "ada", 10).unwrap().is_empty());
    }

    #[test]
    fn delete_if_exists_distinguishes_found_from_not_found() {
        let mem = store();
        upsert(&mem, "t", "a", 1.0, &Row { id: "a".into(), updated_at: 1.0, subject: None }).unwrap();
        assert!(delete_if_exists(&mem, "t", "a").unwrap());
        assert!(!delete_if_exists(&mem, "t", "a").unwrap(), "segunda chamada não finge sucesso");
    }

    /// Prova o backfill: um registro gravado FORA desta camada (sem passar por `upsert`, simulando
    /// um `memory.db` já populado pelo TypeScript antes do índice existir) ainda aparece em
    /// `get_all` depois do backfill preguiçoso de `ensure_indexed`.
    #[test]
    fn get_all_finds_records_written_before_any_index_existed() {
        let mem = store();
        let raw = serde_json::to_string(&Row { id: "legacy".into(), updated_at: 5.0, subject: None }).unwrap();
        mem.set(&record_key("t", "legacy"), &raw, None).unwrap();

        let all: Vec<Row> = get_all(&mem, "t").unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "legacy");
    }
}
