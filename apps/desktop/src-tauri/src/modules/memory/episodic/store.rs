//! Leitura/escrita de tier sobre o `MemoryStore` KV — porte de `loadTier`/`locateTier`/
//! `dropOrphanIndexEntry`/`writeEntry` (`episodic-store.ts`). O KV em si (`hSet`/`zAdd`/...) já é o
//! `MemoryStore` de `modules/memory/mod.rs`; este arquivo é só a camada de tier por cima dele.

use crate::modules::memory::{MemoryStore, RangeLimit, ScoreBound, ZRangeOpts};

use super::types::{hash_to_entry, index_key, key, MemoryEntry, MemoryTier};

/// = `dropOrphanIndexEntry`: poda um id sem memória viva do índice. Erro de I/O aqui é só log no
/// TS (`log.debug`), nunca propagado — mesmo aqui: melhor esforço, `Result` descartado pelo chamador.
pub fn drop_orphan_index_entry(store: &MemoryStore, tier: MemoryTier, id: &str) -> Result<(), String> {
    store.z_rem(&index_key(tier), id)?;
    Ok(())
}

/// = `locateTier`: em qual tier (se algum) o id vive hoje, pelo campo `content` do hash.
pub fn locate_tier(store: &MemoryStore, id: &str) -> Result<Option<MemoryTier>, String> {
    if store.h_get_all(&key(MemoryTier::Short, id))?.contains_key("content") {
        return Ok(Some(MemoryTier::Short));
    }
    if store.h_get_all(&key(MemoryTier::Long, id))?.contains_key("content") {
        return Ok(Some(MemoryTier::Long));
    }
    Ok(None)
}

/// = `loadTier`: entradas do tier, mais recentes primeiro, podando do índice qualquer id cujo hash
/// já expirou (TTL do `MemoryStore` limpa o hash mas não o índice sozinho).
pub fn load_tier(store: &MemoryStore, tier: MemoryTier, count: Option<i64>) -> Result<Vec<MemoryEntry>, String> {
    let opts = ZRangeOpts { rev: true, limit: count.map(|c| RangeLimit { offset: 0, count: c }) };
    let ids = store.z_range_by_score(&index_key(tier), ScoreBound::NegInf, ScoreBound::PosInf, opts)?;

    let mut entries = Vec::with_capacity(ids.len());
    for id in ids {
        let hash = store.h_get_all(&key(tier, &id))?;
        match hash_to_entry(&id, &hash) {
            Some(entry) => entries.push(entry),
            None => drop_orphan_index_entry(store, tier, &id)?,
        }
    }
    Ok(entries)
}

/// = `writeEntry`: grava os campos do hash, aplica TTL e indexa por timestamp. Sempre `Ok(true)`
/// quando não há erro de I/O — o TS devolve `Promise<boolean>` mas nunca `false` de fato aqui, o
/// booleano fica pela paridade de assinatura com o chamador.
#[allow(clippy::too_many_arguments)]
pub fn write_entry(
    store: &MemoryStore,
    tier: MemoryTier,
    id: &str,
    content: &str,
    entry_type: &str,
    timestamp: i64,
    metadata: &str,
    ttl_seconds: i64,
    embedding: Option<&[f64]>,
) -> Result<bool, String> {
    let k = key(tier, id);
    // Uma entrada NOVA gravada pela metade (falha de I/O entre os 6 comandos, antes do `expire`)
    // ficaria para sempre sem TTL: desfaz o que escreveu. Entrada existente não é desfeita — o
    // estado anterior dela já era válido e é o que o chamador ainda enxerga.
    let is_new = store.h_get_all(&k)?.is_empty();
    let result = write_entry_fields(store, tier, &k, id, content, entry_type, timestamp, metadata, ttl_seconds, embedding);
    if result.is_err() && is_new {
        let _ = store.del(&k);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn write_entry_fields(
    store: &MemoryStore,
    tier: MemoryTier,
    k: &str,
    id: &str,
    content: &str,
    entry_type: &str,
    timestamp: i64,
    metadata: &str,
    ttl_seconds: i64,
    embedding: Option<&[f64]>,
) -> Result<bool, String> {
    let k = k.to_string();
    store.h_set(&k, "content", content)?;
    store.h_set(&k, "type", entry_type)?;
    store.h_set(&k, "timestamp", &timestamp.to_string())?;
    store.h_set(&k, "metadata", metadata)?;
    if let Some(emb) = embedding {
        let serialized = serde_json::to_string(emb).map_err(|e| format!("serializar embedding: {e}"))?;
        store.h_set(&k, "embedding", &serialized)?;
    }
    store.expire(&k, ttl_seconds)?;
    store.z_add(&index_key(tier), timestamp as f64, id)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn locate_tier_finds_the_right_tier_and_none_when_absent() {
        let store = store();
        write_entry(&store, MemoryTier::Long, "id1", "conteudo", "conversation", 100, "{}", 3600, None).unwrap();
        assert_eq!(locate_tier(&store, "id1").unwrap(), Some(MemoryTier::Long));
        assert_eq!(locate_tier(&store, "ghost").unwrap(), None);
    }

    #[test]
    fn load_tier_orders_newest_first_and_respects_count() {
        let store = store();
        write_entry(&store, MemoryTier::Short, "a", "primeiro", "conversation", 1, "{}", 3600, None).unwrap();
        write_entry(&store, MemoryTier::Short, "b", "segundo", "conversation", 2, "{}", 3600, None).unwrap();
        write_entry(&store, MemoryTier::Short, "c", "terceiro", "conversation", 3, "{}", 3600, None).unwrap();

        let all = load_tier(&store, MemoryTier::Short, None).unwrap();
        assert_eq!(all.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["c", "b", "a"]);

        let top1 = load_tier(&store, MemoryTier::Short, Some(1)).unwrap();
        assert_eq!(top1.len(), 1);
        assert_eq!(top1[0].id, "c");
    }

    #[test]
    fn load_tier_prunes_orphan_index_entries_whose_hash_expired() {
        let store = store();
        write_entry(&store, MemoryTier::Short, "expira-ja", "x", "conversation", 1, "{}", -1, None).unwrap();
        write_entry(&store, MemoryTier::Short, "vivo", "y", "conversation", 2, "{}", 3600, None).unwrap();

        let entries = load_tier(&store, MemoryTier::Short, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "vivo");
        // O índice foi podado, não só o hash: uma segunda leitura não recria o órfão.
        let ids = store
            .z_range_by_score(&index_key(MemoryTier::Short), ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts::default())
            .unwrap();
        assert_eq!(ids, vec!["vivo".to_string()]);
    }

    #[test]
    fn write_entry_round_trips_unicode_content_and_embedding() {
        let store = store();
        write_entry(
            &store,
            MemoryTier::Long,
            "id1",
            "olá 🎮 — テスト",
            "note",
            42,
            "{\"k\":1}",
            3600,
            Some(&[0.1, 0.2, 0.3]),
        )
        .unwrap();
        let entries = load_tier(&store, MemoryTier::Long, None).unwrap();
        assert_eq!(entries[0].content, "olá 🎮 — テスト");
        assert_eq!(entries[0].embedding, Some(vec![0.1, 0.2, 0.3]));
        assert_eq!(entries[0].metadata, "{\"k\":1}");
    }
}
