//! Tipos e chaves da memória episódica — porte de `episodic-types.ts` + as constantes/helpers de
//! chave de `episodic-store.ts` (`key`, `indexKey`, `hashToEntry`, `parseEmbedding`).

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Mesmo padrão de `now_millis` em `modules/memory/mod.rs` — cada arquivo mantém a própria cópia
/// (função de 4 linhas, sem estado) em vez de importar do módulo pai, que não a expõe `pub`.
pub fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// = `MemoryTier` (`episodic-types.ts`). `Copy`: é só uma tag de duas variantes, carregada por
/// valor em toda a árvore de chamadas — igual ao TS passar a string literal `'short' | 'long'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryTier {
    Short,
    Long,
}

impl MemoryTier {
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryTier::Short => "short",
            MemoryTier::Long => "long",
        }
    }
}

pub const EMBEDDING_SCAN_LIMIT: i64 = 500;
pub const GET_ALL_LIMIT_DEFAULT: i64 = 500;
pub const LONG_TIER_SCAN_LIMIT: i64 = 5_000;
pub const SHORT_TTL_HOURS_DEFAULT: f64 = 168.0;
pub const LONG_TTL_DAYS_DEFAULT: f64 = 365.0;
pub const EMBEDDING_META_KEY: &str = "mika:mem:embedding-meta";

/// = `key(tier, id)`.
pub fn key(tier: MemoryTier, id: &str) -> String {
    format!("mika:mem:{}:{id}", tier.as_str())
}

/// = `indexKey(tier)`.
pub fn index_key(tier: MemoryTier) -> String {
    format!("mika:mem:index:{}", tier.as_str())
}

/// Espelha `MemoryEntry` (`episodic-search.ts`). `embedding: None` = nunca calculado OU
/// provider indisponível (ver `episodic::embedding`) — igual ao TS tratar ausência de campo.
///
/// `Serialize` com `rename = "type"` (`r#type` não pode ser nome de campo JSON) e
/// `skip_serializing_if` no embedding — a MESMA forma que `JSON.stringify` dá ao objeto TS
/// (`embedding?: number[]` ausente vira chave ausente, não `null`), consumida por
/// `memory_console`/`list_memories` para devolver a lista sem reconstruir o objeto campo a campo.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub timestamp: i64,
    pub metadata: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f64>>,
}

/// = `parseEmbedding`: `None` para ausente OU JSON inválido/não-array-de-números — nunca panica.
pub fn parse_embedding(raw: Option<&String>) -> Option<Vec<f64>> {
    serde_json::from_str(raw?).ok()
}

/// = `hashToEntry`: `None` quando o hash não tem `content` (chave nunca escrita, ou registro
/// órfão de índice) — nunca confunde ausência com string vazia.
pub fn hash_to_entry(id: &str, hash: &HashMap<String, String>) -> Option<MemoryEntry> {
    let content = hash.get("content")?.clone();
    Some(MemoryEntry {
        id: id.to_string(),
        content,
        r#type: hash.get("type").cloned().unwrap_or_else(|| "conversation".to_string()),
        timestamp: hash.get("timestamp").and_then(|v| v.parse().ok()).unwrap_or(0),
        metadata: hash.get("metadata").cloned().unwrap_or_else(|| "{}".to_string()),
        embedding: parse_embedding(hash.get("embedding")),
    })
}

/// = `shouldStoreLong`: `metadata` é o JSON já parseado (`serde_json::Value::Object` esperado;
/// qualquer outra forma cai nos `unwrap_or` abaixo, igual ao TS tratar `metadata?.foo` em valor
/// não-objeto como `undefined`).
pub fn should_store_long(metadata: Option<&serde_json::Value>) -> bool {
    let Some(meta) = metadata else { return false };
    let is_summary = meta.get("type").and_then(|v| v.as_str()) == Some("summary");
    let important = matches!(meta.get("important"), Some(serde_json::Value::Bool(true)))
        || meta.get("important").and_then(|v| v.as_str()) == Some("true");
    if is_summary || important {
        return true;
    }
    meta.get("importance").and_then(numeric_value).map(|n| n >= 4.0).unwrap_or(false)
}

/// `Number(metadata?.importance)`: aceita número OU string numérica, igual ao `Number()` do JS.
fn numeric_value(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_and_index_key_match_the_ts_naming() {
        assert_eq!(key(MemoryTier::Short, "abc"), "mika:mem:short:abc");
        assert_eq!(index_key(MemoryTier::Long), "mika:mem:index:long");
    }

    #[test]
    fn parse_embedding_rejects_non_numeric_arrays_and_garbage() {
        assert_eq!(parse_embedding(None), None);
        assert_eq!(parse_embedding(Some(&"not json".to_string())), None);
        assert_eq!(parse_embedding(Some(&"[1, \"x\", 3]".to_string())), None);
        assert_eq!(parse_embedding(Some(&"[1.5, 2.5]".to_string())), Some(vec![1.5, 2.5]));
    }

    #[test]
    fn hash_to_entry_is_none_without_content() {
        let hash = HashMap::from([("type".to_string(), "note".to_string())]);
        assert_eq!(hash_to_entry("id1", &hash), None);
    }

    #[test]
    fn hash_to_entry_defaults_type_and_metadata_when_missing() {
        let hash = HashMap::from([("content".to_string(), "olá".to_string())]);
        let entry = hash_to_entry("id1", &hash).unwrap();
        assert_eq!(entry.r#type, "conversation");
        assert_eq!(entry.metadata, "{}");
        assert_eq!(entry.embedding, None);
    }

    #[test]
    fn memory_entry_serializes_type_field_and_omits_embedding_when_absent() {
        let entry = MemoryEntry {
            id: "id1".to_string(),
            content: "olá".to_string(),
            r#type: "conversation".to_string(),
            timestamp: 42,
            metadata: "{}".to_string(),
            embedding: None,
        };
        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value["type"], json!("conversation"), "campo Rust r#type vira \"type\" no JSON");
        assert!(!value.as_object().unwrap().contains_key("embedding"), "embedding ausente não vira null");
    }

    #[test]
    fn memory_entry_serializes_embedding_when_present() {
        let entry = MemoryEntry {
            id: "id1".to_string(),
            content: "olá".to_string(),
            r#type: "note".to_string(),
            timestamp: 1,
            metadata: "{}".to_string(),
            embedding: Some(vec![0.1, 0.2]),
        };
        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value["embedding"], json!([0.1, 0.2]));
    }

    #[test]
    fn should_store_long_covers_summary_important_and_importance_threshold() {
        assert!(should_store_long(Some(&json!({ "type": "summary" }))));
        assert!(should_store_long(Some(&json!({ "important": true }))));
        assert!(should_store_long(Some(&json!({ "important": "true" }))));
        assert!(should_store_long(Some(&json!({ "importance": 4 }))));
        assert!(should_store_long(Some(&json!({ "importance": "5" }))));
        assert!(!should_store_long(Some(&json!({ "importance": 3 }))));
        assert!(!should_store_long(None));
    }
}
