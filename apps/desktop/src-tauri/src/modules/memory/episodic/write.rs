//! Caminho de escrita da memória episódica — porte de `episodic-write.ts`. Sem `WriteContext`
//! injetável: `generate_embedding` (`episodic::embedding`) é síncrono em Rust hoje (sempre `None`,
//! ver o cabeçalho daquele arquivo), então não há closure assíncrona para carregar por parâmetro
//! como o TS faz.

use uuid::Uuid;

use super::embedding::{find_duplicate_memory, generate_embedding, invalidate_stale_embedding};
use super::store::{locate_tier, write_entry};
use super::types::{index_key, key, now_millis, should_store_long, MemoryTier};
use crate::modules::memory::MemoryStore;

/// Config de escrita que varia por instância do serviço — porte dos campos de
/// `EpisodicMemoryService` que `write.ts` consome via `WriteContext`.
#[derive(Debug, Clone, Copy)]
pub struct WriteConfig {
    pub dedup_threshold: f64,
    pub short_ttl_seconds: i64,
    pub long_ttl_seconds: i64,
    pub embedding_enabled: bool,
}

fn ttl_seconds(cfg: &WriteConfig, tier: MemoryTier) -> i64 {
    match tier {
        MemoryTier::Short => cfg.short_ttl_seconds,
        MemoryTier::Long => cfg.long_ttl_seconds,
    }
}

/// Resultado de `store`/`update`: `None` = nada foi gravado (memória desligada em
/// `memory.enabled`) — nunca finge sucesso com um id vazio como o TS faz com `{ id: '' }`; o
/// chamador Rust distingue "não gravou" de "gravou com string vazia" de verdade.
pub type WriteOutcome = Option<String>;

/// = `rewriteEntry`: reescreve uma memória já existente em qualquer tier onde ela viva, mesclando
/// metadata antiga com nova e promovendo de tier quando a nova metadata pede `long`.
pub fn rewrite_entry(
    store: &MemoryStore,
    cfg: &WriteConfig,
    id: &str,
    content: &str,
    embedding: Option<&[f64]>,
    metadata: Option<&serde_json::Value>,
) -> Result<bool, String> {
    let Some(tier) = locate_tier(store, id)? else { return Ok(false) };
    let hash = store.h_get_all(&key(tier, id))?;
    let previous_content = hash.get("content").cloned();
    let old_meta: serde_json::Value = hash
        .get("metadata")
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let mut merged = old_meta.clone();
    if let (Some(merged_obj), Some(new_meta)) = (merged.as_object_mut(), metadata.and_then(|m| m.as_object())) {
        for (k, v) in new_meta {
            merged_obj.insert(k.clone(), v.clone());
        }
    }
    let entry_type = metadata
        .and_then(|m| m.get("type"))
        .and_then(|v| v.as_str())
        .or_else(|| hash.get("type").map(String::as_str))
        .unwrap_or("conversation")
        .to_string();
    let target_tier = if tier == MemoryTier::Short && should_store_long(Some(&merged)) { MemoryTier::Long } else { tier };

    let written = write_entry(
        store,
        target_tier,
        id,
        content,
        &entry_type,
        now_millis(),
        &merged.to_string(),
        ttl_seconds(cfg, target_tier),
        embedding,
    )?;
    if written {
        invalidate_stale_embedding(store, target_tier, id, previous_content.as_deref(), content, embedding)?;
    }
    if written && target_tier != tier {
        store.del(&key(tier, id))?;
        store.z_rem(&index_key(tier), id)?;
    }
    Ok(written)
}

/// O tipo com que `store` vai gravar a entrada (`metadata.type`, default `conversation`) — o mesmo
/// que `rewrite_entry` usaria; o dedup só funde entradas do MESMO tipo.
fn new_entry_type(metadata: Option<&serde_json::Value>) -> String {
    metadata.and_then(|m| m.get("type")).and_then(|v| v.as_str()).unwrap_or("conversation").to_string()
}

/// = `store`: dedup por embedding (quando há vetor), senão grava entrada nova. `recording_allowed`
/// é resolvido pelo chamador (lê `memory.enabled` do `ConfigStore` compartilhado) — este arquivo
/// não decide política, só respeita a decisão.
pub fn store(
    kv: &MemoryStore,
    cfg: &WriteConfig,
    recording_allowed: bool,
    content: &str,
    embedding: Option<Vec<f64>>,
    metadata: Option<&serde_json::Value>,
) -> Result<WriteOutcome, String> {
    if !recording_allowed {
        return Ok(None);
    }

    // Segredo, CPF, cartão e senha ditos no chat não podem ficar gravados nem seguir para o
    // provedor de embedding (que pode ser nuvem): a redação vem ANTES de qualquer um dos dois.
    let redacted = crate::modules::memory::redact::redact_for_storage(content);
    let content = redacted.as_str();

    let emb = embedding.or_else(|| generate_embedding(kv, content, cfg.embedding_enabled));
    if let Some(emb) = &emb {
        if let Some(dup_id) = find_duplicate_memory(kv, emb, cfg.dedup_threshold, &new_entry_type(metadata), content)? {
            if rewrite_entry(kv, cfg, &dup_id, content, Some(emb), metadata)? {
                return Ok(Some(dup_id));
            }
        }
    }

    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    let meta_value = metadata.cloned().unwrap_or_else(|| serde_json::json!({}));
    let tier = if should_store_long(Some(&meta_value)) { MemoryTier::Long } else { MemoryTier::Short };
    let entry_type = metadata.and_then(|m| m.get("type")).and_then(|v| v.as_str()).unwrap_or("conversation").to_string();

    let written = write_entry(
        kv,
        tier,
        &id,
        content,
        &entry_type,
        now,
        &meta_value.to_string(),
        ttl_seconds(cfg, tier),
        emb.as_deref(),
    )?;
    Ok(if written { Some(id) } else { None })
}

/// = `update`: sempre reescreve (nunca cria) — `false` quando o id não existe em tier nenhum.
pub fn update(
    kv: &MemoryStore,
    cfg: &WriteConfig,
    recording_allowed: bool,
    id: &str,
    content: &str,
    metadata: Option<&serde_json::Value>,
) -> Result<bool, String> {
    if !recording_allowed {
        return Ok(false);
    }
    let emb = generate_embedding(kv, content, cfg.embedding_enabled);
    rewrite_entry(kv, cfg, id, content, emb.as_deref(), metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::load_tier;
    use serde_json::json;

    fn cfg() -> WriteConfig {
        WriteConfig { dedup_threshold: 0.95, short_ttl_seconds: 3600, long_ttl_seconds: 86_400, embedding_enabled: true }
    }

    #[test]
    fn secrets_cpf_cards_and_spoken_passwords_never_reach_the_store() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let key = format!("sk-ant-{}", "a".repeat(40));
        store(&kv, &cfg(), true, &format!("User: meu cpf é 529.982.247-25 e a chave {key}\nMika: anotado"), None, None).unwrap();
        let saved = load_tier(&kv, MemoryTier::Short, None).unwrap().remove(0).content;
        assert!(!saved.contains("529.982") && !saved.contains(&key), "{saved}");
        assert!(saved.contains("[CPF]") && saved.contains("[KEY]"));
        assert!(saved.contains("anotado"), "o resto da conversa fica");
    }

    #[test]
    fn store_blocked_by_memory_enabled_false_writes_nothing() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let id = store(&kv, &cfg(), false, "conteudo", None, None).unwrap();
        assert_eq!(id, None);
        assert!(load_tier(&kv, MemoryTier::Short, None).unwrap().is_empty());
    }

    #[test]
    fn store_without_embedding_lands_in_short_tier_by_default() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let id = store(&kv, &cfg(), true, "conteudo qualquer", None, None).unwrap();
        assert!(id.is_some());
        assert_eq!(load_tier(&kv, MemoryTier::Short, None).unwrap().len(), 1);
        assert!(load_tier(&kv, MemoryTier::Long, None).unwrap().is_empty());
    }

    #[test]
    fn store_with_important_metadata_lands_directly_in_long_tier() {
        let kv = MemoryStore::open_in_memory().unwrap();
        store(&kv, &cfg(), true, "fato importante", None, Some(&json!({ "important": true }))).unwrap();
        assert_eq!(load_tier(&kv, MemoryTier::Long, None).unwrap().len(), 1);
    }

    #[test]
    fn store_with_embedding_above_dedup_threshold_rewrites_instead_of_duplicating() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let first = store(&kv, &cfg(), true, "gosta de rust", Some(vec![1.0, 0.0]), None).unwrap().unwrap();
        let second = store(&kv, &cfg(), true, "gosta MUITO de rust", Some(vec![1.0, 0.0]), None).unwrap().unwrap();
        assert_eq!(first, second, "mesmo embedding acima do limiar reescreve o id existente");
        assert_eq!(load_tier(&kv, MemoryTier::Short, None).unwrap().len(), 1);
    }

    #[test]
    fn update_on_unknown_id_returns_false_without_creating_anything() {
        let kv = MemoryStore::open_in_memory().unwrap();
        assert!(!update(&kv, &cfg(), true, "ghost", "novo conteudo", None).unwrap());
    }

    #[test]
    fn update_merges_metadata_and_can_promote_short_to_long() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let id = store(&kv, &cfg(), true, "fato comum", None, Some(&json!({ "source": "chat" }))).unwrap().unwrap();
        assert!(update(&kv, &cfg(), true, &id, "fato agora importante", Some(&json!({ "important": true }))).unwrap());

        assert!(load_tier(&kv, MemoryTier::Short, None).unwrap().is_empty(), "promovido, não deve sobrar no tier antigo");
        let long = load_tier(&kv, MemoryTier::Long, None).unwrap();
        assert_eq!(long.len(), 1);
        assert_eq!(long[0].content, "fato agora importante");
        let meta: serde_json::Value = serde_json::from_str(&long[0].metadata).unwrap();
        assert_eq!(meta["source"], json!("chat"), "metadata antiga sobrevive à mesclagem");
        assert_eq!(meta["important"], json!(true));
    }

    #[test]
    fn update_blocked_by_memory_enabled_false_leaves_the_entry_untouched() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let id = store(&kv, &cfg(), true, "original", None, None).unwrap().unwrap();
        assert!(!update(&kv, &cfg(), false, &id, "tentativa de sobrescrever", None).unwrap());
        assert_eq!(load_tier(&kv, MemoryTier::Short, None).unwrap()[0].content, "original");
    }
}
