//! Leitura/remoção da memória episódica — porte de `episodic-queries.ts`.

use super::search::{
    mentions_name, rank_semantic_candidates, score_keyword_candidates, tokenize_query, LexicalRerank, ScoreFlags,
};
use super::store::{load_tier, locate_tier};
use super::types::{index_key, key, now_millis, MemoryEntry, MemoryTier, EMBEDDING_SCAN_LIMIT, LONG_TIER_SCAN_LIMIT};
use crate::modules::memory::MemoryStore;

/// Config de leitura que varia por instância — porte dos campos que `queries.ts`/`tier-scoring.ts`
/// consomem via `QueryContext`/`TierScoreFlags`.
#[derive(Debug, Clone, Copy)]
pub struct QueryConfig {
    pub get_all_limit: i64,
    pub min_similarity: f64,
    pub score_flags: ScoreFlags,
    pub lexical_rerank_weight: f64,
}

/// = `getAll`: os dois tiers juntos, mais recentes primeiro, cortados no limite configurado.
pub fn get_all(store: &MemoryStore, cfg: &QueryConfig) -> Result<Vec<MemoryEntry>, String> {
    let mut all = load_tier(store, MemoryTier::Short, Some(cfg.get_all_limit))?;
    all.extend(load_tier(store, MemoryTier::Long, Some(cfg.get_all_limit))?);
    all.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    all.truncate(cfg.get_all_limit as usize);
    Ok(all)
}

/// = `deleteMemory`.
pub fn delete_memory(store: &MemoryStore, id: &str) -> Result<bool, String> {
    let Some(tier) = locate_tier(store, id)? else { return Ok(false) };
    store.del(&key(tier, id))?;
    store.z_rem(&index_key(tier), id)?;
    Ok(true)
}

/// = `forgetMentions`: remove toda memória cujo CONTEÚDO cite um dos termos como palavra inteira
/// ([`mentions_name`]: mínimo de 3 caracteres e capitalização do termo). O `metadata` não conta —
/// ids, tags e origem não são o que a pessoa disse. Apagar é irreversível, então na dúvida não casa.
pub fn forget_mentions(store: &MemoryStore, terms: &[String]) -> Result<usize, String> {
    sweep_mentions(store, terms, true)
}

/// A prévia do [`forget_mentions`]: quantas memórias ele removeria, sem remover nenhuma. Mesma regra
/// de casamento, pelo mesmo caminho — a contagem mostrada ao usuário é a do apagar.
pub fn count_mentions(store: &MemoryStore, terms: &[String]) -> Result<usize, String> {
    sweep_mentions(store, terms, false)
}

fn sweep_mentions(store: &MemoryStore, terms: &[String], delete: bool) -> Result<usize, String> {
    let mut matched = 0usize;
    for tier in [MemoryTier::Short, MemoryTier::Long] {
        for entry in load_tier(store, tier, None)? {
            if !terms.iter().any(|term| mentions_name(&entry.content, term)) {
                continue;
            }
            if delete {
                store.del(&key(tier, &entry.id))?;
                store.z_rem(&index_key(tier), &entry.id)?;
            }
            matched += 1;
        }
    }
    Ok(matched)
}

/// = `search`: busca por palavra-chave, sem embedding nenhum — sempre disponível.
pub fn search(store: &MemoryStore, query: &str, limit: usize) -> Result<Vec<MemoryEntry>, String> {
    let terms = tokenize_query(query);
    if terms.is_empty() {
        return Ok(vec![]);
    }
    // O tier longo tem o MESMO teto da busca semântica: com 500 aqui, o fallback (o único caminho
    // sem embeddings) nunca alcançava as memórias duradouras além das 500 mais novas.
    let mut candidates = load_tier(store, MemoryTier::Short, Some(EMBEDDING_SCAN_LIMIT))?;
    let long = load_tier(store, MemoryTier::Long, Some(LONG_TIER_SCAN_LIMIT))?;
    if long.len() as i64 >= LONG_TIER_SCAN_LIMIT {
        log::warn!("busca por palavra-chave atingiu o teto do tier longo ({LONG_TIER_SCAN_LIMIT}) — memória antiga pode estar fora do alcance");
    }
    candidates.extend(long);
    Ok(score_keyword_candidates(&candidates, &terms, limit))
}

/// = `scoreTier` (`episodic-tier-scoring.ts`): pontua as entradas de UM tier contra um embedding de
/// consulta já calculado — REAL, só precisa do vetor pronto (ver `episodic::embedding` para por que
/// gerar esse vetor ainda degrada).
fn score_tier(
    store: &MemoryStore,
    tier: MemoryTier,
    query_embedding: &[f64],
    recency_bias: f64,
    now_ms: i64,
    min_similarity: f64,
    flags: ScoreFlags,
    current_game: Option<&str>,
) -> Result<Vec<(MemoryEntry, f64)>, String> {
    let scan_limit = if tier == MemoryTier::Long { LONG_TIER_SCAN_LIMIT } else { EMBEDDING_SCAN_LIMIT };
    let entries = load_tier(store, tier, Some(scan_limit))?;
    if entries.len() as i64 >= scan_limit {
        log::warn!("varredura de memória atingiu o teto — memória antiga pode estar fora do alcance (tier={:?}, scanLimit={scan_limit})", tier);
    }

    let mut candidates = Vec::new();
    for entry in entries {
        let Some(embedding) = &entry.embedding else { continue };
        // Vetor de outro modelo/dimensão não é comparável: virava similaridade 0 e, com
        // `minSimilarity = 0` (permitido), entrava no resultado como se fosse um acerto.
        if embedding.len() != query_embedding.len() {
            continue;
        }
        let scored = super::search::calculate_candidate_score(
            &entry.r#type,
            &entry.metadata,
            embedding,
            query_embedding,
            recency_bias,
            now_ms,
            entry.timestamp,
            flags,
            current_game,
        );
        if scored.sim < min_similarity {
            continue;
        }
        candidates.push((entry, scored.score));
    }
    Ok(candidates)
}

/// = `searchSemantic`: recebe o embedding de consulta já pronto (ver `episodic::embedding`).
#[allow(clippy::too_many_arguments)]
pub fn search_semantic(
    store: &MemoryStore,
    cfg: &QueryConfig,
    query_embedding: &[f64],
    limit: usize,
    recency_bias: f64,
    current_game: Option<&str>,
    query_text: &str,
) -> Result<Vec<MemoryEntry>, String> {
    let now = now_millis();
    let mut candidates = score_tier(store, MemoryTier::Short, query_embedding, recency_bias, now, cfg.min_similarity, cfg.score_flags, current_game)?;
    candidates.extend(score_tier(store, MemoryTier::Long, query_embedding, recency_bias, now, cfg.min_similarity, cfg.score_flags, current_game)?);

    let lexical_rerank = if cfg.lexical_rerank_weight > 0.0 && !query_text.is_empty() {
        Some(LexicalRerank { query: query_text, weight: cfg.lexical_rerank_weight })
    } else {
        None
    };
    Ok(rank_semantic_candidates(candidates, limit, lexical_rerank))
}

/// = `clear`: apaga TODAS as chaves `mika:mem:*` (índices e hashes dos dois tiers, e o meta de
/// embedding).
pub fn clear(store: &MemoryStore) -> Result<bool, String> {
    // Os caches em RAM guardam o TEXTO e o vetor do que foi embeddado: "apagar tudo" que os deixasse
    // de pé manteria dados do usuário em memória até o fim do processo.
    super::embedding::clear_caches();
    for k in store.keys("mika:mem:*")? {
        store.del(&k)?;
    }
    Ok(true)
}

/// = `countShort`: só o tier curto — `consolidateOld` só encolhe esse tier; gatear pela contagem
/// total travaria em loop quando o crescimento é majoritariamente no tier longo.
pub fn count_short(store: &MemoryStore) -> Result<usize, String> {
    Ok(store.keys(&key(MemoryTier::Short, "*"))?.len())
}

/// = `count`: soma das chaves distintas de cada tier.
#[cfg(test)]
pub fn count(store: &MemoryStore) -> Result<usize, String> {
    let short = store.keys(&key(MemoryTier::Short, "*"))?.len();
    let long = store.keys(&key(MemoryTier::Long, "*"))?.len();
    Ok(short + long)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::write::{store as write_store, WriteConfig};
    use serde_json::json;

    fn cfg() -> QueryConfig {
        QueryConfig {
            get_all_limit: 500,
            min_similarity: 0.25,
            score_flags: ScoreFlags { reranking_enabled: true, emotional_tagging: true },
            lexical_rerank_weight: 0.15,
        }
    }

    fn write_cfg() -> WriteConfig {
        WriteConfig { dedup_threshold: 0.95, short_ttl_seconds: 3600, long_ttl_seconds: 86_400, embedding_enabled: true }
    }

    #[test]
    fn vectors_of_another_dimension_never_surface_even_with_a_zero_similarity_floor() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "mesma dimensão", Some(vec![1.0, 0.0]), Some(&json!({ "type": "note" }))).unwrap();
        write_store(&kv, &write_cfg(), true, "outro modelo", Some(vec![1.0, 0.0, 0.0]), Some(&json!({ "type": "other" }))).unwrap();
        let floor_zero = QueryConfig { min_similarity: 0.0, ..cfg() };
        let found = search_semantic(&kv, &floor_zero, &[1.0, 0.0], 5, 0.0, None, "").unwrap();
        assert_eq!(found.iter().map(|e| e.content.as_str()).collect::<Vec<_>>(), ["mesma dimensão"], "dimensão diferente virava similaridade 0 e passava");
    }

    #[test]
    fn clearing_memory_also_drops_the_cached_vectors_of_what_was_erased() {
        crate::modules::memory::episodic::embedding::cache_put_for_test("texto apagado", vec![1.0]);
        let kv = MemoryStore::open_in_memory().unwrap();
        clear(&kv).unwrap();
        assert!(!crate::modules::memory::episodic::embedding::cache_has_for_test("texto apagado"), "o cache em RAM guardava o texto e o vetor do que o usuário mandou apagar");
    }

    #[test]
    fn get_all_merges_both_tiers_newest_first() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "curto", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "longo", None, Some(&json!({ "important": true }))).unwrap();
        let all = get_all(&kv, &cfg()).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_memory_removes_from_whichever_tier_and_is_idempotent() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let id = write_store(&kv, &write_cfg(), true, "apagar", None, None).unwrap().unwrap();
        assert!(delete_memory(&kv, &id).unwrap());
        assert!(!delete_memory(&kv, &id).unwrap(), "segunda chamada não finge sucesso");
    }

    #[test]
    fn forget_mentions_matches_the_whole_word_in_the_content() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "Ana foi ao cinema", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "comi banana", None, None).unwrap();
        let removed = forget_mentions(&kv, &["Ana".to_string()]).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(get_all(&kv, &cfg()).unwrap().len(), 1);
    }

    /// ja/zh/ko não separam palavra por espaço (partícula colada: "田中太郎さん", "김철수가") — casar
    /// `" 김철수 "` nunca achava nada e "esqueça" ficava sem efeito. Mínimo de 3 caracteres também aqui:
    /// um alias de 2 (`ゆき`) casaria dentro de `ゆきだるま` e apagaria memória de outra coisa.
    #[test]
    fn forget_mentions_matches_cjk_terms_glued_to_particles() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "田中太郎さんが来た", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "김철수가 게임을 했다", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "山田花子さんは寝た", None, None).unwrap();
        assert_eq!(forget_mentions(&kv, &["田中太郎".to_string()]).unwrap(), 1);
        assert_eq!(forget_mentions(&kv, &["김철수".to_string()]).unwrap(), 1);
        assert_eq!(get_all(&kv, &cfg()).unwrap().len(), 1, "só a memória sobre 山田花子 sobrevive");
    }

    /// Apagar uma ficha é irreversível: nome curto/comum não pode levar memórias de outras coisas junto.
    #[test]
    fn forget_mentions_refuses_terms_shorter_than_three_characters() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "Al foi à feira", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "Ed comprou pão", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "ゆきだるまを作った", None, None).unwrap();

        let terms: Vec<String> = ["Al", "Ed", "ゆき"].iter().map(|t| t.to_string()).collect();
        assert_eq!(forget_mentions(&kv, &terms).unwrap(), 0);
        assert_eq!(get_all(&kv, &cfg()).unwrap().len(), 3);
    }

    /// "Link" é nome do personagem E palavra comum: só a ocorrência capitalizada é dele.
    #[test]
    fn forget_mentions_needs_the_capitalization_of_the_term() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "clique no link do tutorial", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "Link salvou Hyrule", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "joguei como LINK ontem", None, None).unwrap();

        assert_eq!(forget_mentions(&kv, &["Link".to_string()]).unwrap(), 2);
        let left: Vec<String> = get_all(&kv, &cfg()).unwrap().into_iter().map(|e| e.content).collect();
        assert_eq!(left, ["clique no link do tutorial"]);
    }

    /// O `metadata` (ids, tags, origem) não é o que a pessoa disse: um nome ali não autoriza apagar.
    #[test]
    fn forget_mentions_ignores_the_metadata() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "comprei pão", None, Some(&json!({ "source": "Marina" }))).unwrap();

        assert_eq!(forget_mentions(&kv, &["Marina".to_string()]).unwrap(), 0);
        assert_eq!(get_all(&kv, &cfg()).unwrap().len(), 1);
    }

    /// A prévia da UI: a MESMA regra de `forget_mentions`, sem apagar nada.
    #[test]
    fn count_mentions_predicts_what_forget_mentions_removes_without_removing() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "Marina gosta de rock", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "Marina viajou", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "outra coisa", None, None).unwrap();
        let terms = vec!["Marina".to_string()];

        assert_eq!(count_mentions(&kv, &terms).unwrap(), 2);
        assert_eq!(get_all(&kv, &cfg()).unwrap().len(), 3, "contar não apaga");
        assert_eq!(forget_mentions(&kv, &terms).unwrap(), 2);
    }

    #[test]
    fn search_returns_empty_for_a_query_with_no_usable_terms() {
        let kv = MemoryStore::open_in_memory().unwrap();
        assert_eq!(search(&kv, "", 5).unwrap(), vec![]);
    }

    #[test]
    fn clear_removes_everything_including_index_keys() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "a", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "b", None, Some(&json!({ "important": true }))).unwrap();
        assert_eq!(count(&kv).unwrap(), 2);
        assert!(clear(&kv).unwrap());
        assert_eq!(count(&kv).unwrap(), 0);
        assert!(get_all(&kv, &cfg()).unwrap().is_empty());
    }

    #[test]
    fn count_short_only_counts_the_short_tier() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "curto", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "longo", None, Some(&json!({ "important": true }))).unwrap();
        assert_eq!(count_short(&kv).unwrap(), 1);
        assert_eq!(count(&kv).unwrap(), 2);
    }

    #[test]
    fn search_semantic_ranks_by_similarity_and_respects_the_floor() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "perto", Some(vec![1.0, 0.0]), None).unwrap();
        write_store(&kv, &write_cfg(), true, "longe", Some(vec![0.0, 1.0]), None).unwrap();

        let results = search_semantic(&kv, &cfg(), &[1.0, 0.0], 5, 0.0, None, "").unwrap();
        assert_eq!(results.len(), 1, "só o candidato acima do piso de similaridade (0.25) aparece");
        assert_eq!(results[0].content, "perto");
    }
}
