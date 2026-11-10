//! Consolidação do tier curto — porte de `episodic-consolidation.ts` + o laço de
//! `EpisodicMemoryService.consolidateOld` (`episodic-memory.service.ts`).
//!
//! **Real, parametrizado por closure ASSÍNCRONA**: assim como o TS recebe `llmFn` de fora
//! (`consolidator.ts::consolidateMemoryIfNeeded` monta o prompt e chama `generateText`), a função
//! `consolidate_old` aqui recebe um `llm_fn: impl FnOnce(&[String]) -> Fut` — desde que
//! `post_turn::pipeline` passou a chamar isto de verdade (nesta rodada), a única fachada de LLM
//! alcançável é [`super::super::brain::turn::run_turn_llm_raw`], que é `async`; um `llm_fn` síncrono
//! obrigaria `block_on` dentro do runtime do Tauri, a mesma classe de bug de "reator não está
//! rodando" que este crate já documentou noutro lugar (ver AGENTS.md/histórico de `tokio::spawn`
//! fora de contexto). A varredura/reindexação do tier, que é o que este arquivo porta, é toda real.

use std::future::Future;

use super::store::{drop_orphan_index_entry, write_entry};
use super::types::{index_key, key, now_millis, parse_embedding, should_store_long, MemoryTier};
use crate::modules::memory::{MemoryStore, RangeLimit, ScoreBound, ZRangeOpts};

/// = `collectConsolidationBatch`: junta até `batch_size` entradas do tier curto, podando do índice
/// qualquer id órfão (hash expirado por TTL) encontrado no caminho — sem isso um órfão travava o
/// laço para sempre.
pub fn collect_consolidation_batch(store: &MemoryStore, batch_size: usize) -> Result<Vec<(String, String)>, String> {
    let mut collected = Vec::new();
    let mut offset: i64 = 0;
    let max_attempts = 10;
    for _ in 0..max_attempts {
        if collected.len() >= batch_size {
            break;
        }
        let remaining = (batch_size - collected.len()) as i64;
        let opts = ZRangeOpts { rev: false, limit: Some(RangeLimit { offset, count: remaining }) };
        let ids = store.z_range_by_score(&index_key(MemoryTier::Short), ScoreBound::NegInf, ScoreBound::PosInf, opts)?;
        if ids.is_empty() {
            break;
        }

        let mut orphans = 0i64;
        for id in &ids {
            let hash = store.h_get_all(&key(MemoryTier::Short, id))?;
            match hash.get("content") {
                Some(content) => collected.push((id.clone(), content.clone())),
                None => {
                    drop_orphan_index_entry(store, MemoryTier::Short, id)?;
                    orphans += 1;
                }
            }
        }
        // Um órfão podado sai do zset: a próxima página não pode pular o lugar dele.
        offset += ids.len() as i64 - orphans;
    }
    Ok(collected)
}

/// = `EpisodicMemoryService.consolidateOld`: some se o lote não atingir `batch_size`, senão resume
/// pelo `llm_fn` e substitui as entradas coletadas por um único resumo (gravado via `store_summary`,
/// que o chamador injeta — normalmente `episodic::write::store`).
pub async fn consolidate_old<F, Fut>(
    store: &MemoryStore,
    batch_size: usize,
    llm_fn: F,
    store_summary: impl FnOnce(&str) -> Result<Option<String>, String>,
) -> Result<usize, String>
where
    F: FnOnce(Vec<String>) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    let collected = collect_consolidation_batch(store, batch_size)?;
    if collected.len() < batch_size {
        return Ok(0);
    }

    let contents: Vec<String> = collected.iter().map(|(_, c)| c.clone()).collect();
    let summary = llm_fn(contents).await?;
    if summary.trim().is_empty() {
        return Ok(0);
    }

    let Some(_summary_id) = store_summary(&summary)? else { return Ok(0) };
    for (id, _) in &collected {
        store.del(&key(MemoryTier::Short, id))?;
        store.z_rem(&index_key(MemoryTier::Short), id)?;
    }
    Ok(collected.len())
}

/// = uma consolidação/promoção por IDADE, independente do volume que [`consolidate_old`] exige
/// (`CONSOLIDATION_THRESHOLD` = 200 entradas no tier curto — `post_turn::consolidation`). Um usuário
/// de baixo uso nunca acumula 200 entradas; a memória "da semana passada" simplesmente expirava pelo
/// TTL de 7 dias do tier curto antes de qualquer consolidação rodar (defects.md
/// "memory-relationship" P1). Aqui a ação é PROMOVER (mover para o tier longo tal como está), não
/// resumir via LLM — resumir um lote de 1-2 entradas isoladas perderia detalhe sem ganhar espaço
/// nenhum, e chamar o modelo a cada varredura periódica custaria uma chamada de LLM por usuário
/// ocioso, o oposto do que uma sweep de manutenção deveria fazer.
///
/// `now_ms`/`max_age_ms` são parâmetros explícitos (não `SystemTime::now()` interno) para o relógio
/// ser injetável em teste — mesma exigência do brief desta rodada ("test with an injected clock").
/// `max_age_ms <= 0` degrada para "não promove nada" em vez de interpretar um cutoff no futuro como
/// "promova tudo": um bug de configuração (idade negativa) não pode esvaziar o tier curto inteiro.
///
/// Promove por DUAS portas, OU: idade (`timestamp <= cutoff`) OU importância
/// ([`should_store_long`] — mesmo limiar `importance >= 4`/`important: true` que já decide se uma
/// entrada NOVA nasce direto no tier longo, reaplicado aqui para uma entrada que nasceu curta e só
/// depois foi marcada importante). Por isso a varredura é o índice INTEIRO do tier curto (não só o
/// intervalo `..cutoff`) — uma entrada importante e jovem nunca cairia num range limitado por idade.
pub fn promote_aged_entries(store: &MemoryStore, now_ms: i64, max_age_ms: i64, long_ttl_seconds: i64) -> Result<usize, String> {
    if max_age_ms <= 0 {
        return Ok(0);
    }
    let cutoff = now_ms - max_age_ms;
    let ids = store.z_range_by_score(&index_key(MemoryTier::Short), ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts::default())?;

    let mut promoted = 0usize;
    for id in ids {
        let hash = store.h_get_all(&key(MemoryTier::Short, &id))?;
        let Some(content) = hash.get("content").cloned() else {
            drop_orphan_index_entry(store, MemoryTier::Short, &id)?;
            continue;
        };
        let entry_type = hash.get("type").cloned().unwrap_or_else(|| "conversation".to_string());
        let metadata = hash.get("metadata").cloned().unwrap_or_else(|| "{}".to_string());
        let timestamp = hash.get("timestamp").and_then(|v| v.parse().ok()).unwrap_or(now_ms);

        let is_old = timestamp <= cutoff;
        let is_important = serde_json::from_str::<serde_json::Value>(&metadata).ok().is_some_and(|meta| should_store_long(Some(&meta)));
        if !is_old && !is_important {
            continue;
        }

        let embedding = parse_embedding(hash.get("embedding"));
        let written = write_entry(store, MemoryTier::Long, &id, &content, &entry_type, timestamp, &metadata, long_ttl_seconds, embedding.as_deref())?;
        if written {
            store.del(&key(MemoryTier::Short, &id))?;
            store.z_rem(&index_key(MemoryTier::Short), &id)?;
            promoted += 1;
        }
    }
    Ok(promoted)
}

/// Grava um resumo já pronto como entrada nova no tier longo — porte do trecho de `store()` que
/// `consolidateOld` reusa (`type: 'summary', source: 'memory:consolidation'`), sem passar pelo
/// caminho de dedup (um resumo nunca é duplicata de si mesmo).
pub fn store_consolidation_summary(store: &MemoryStore, summary: &str, long_ttl_seconds: i64) -> Result<Option<String>, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let metadata = serde_json::json!({ "type": "summary", "source": "memory:consolidation" }).to_string();
    let written = write_entry(store, MemoryTier::Long, &id, summary, "summary", now_millis(), &metadata, long_ttl_seconds, None)?;
    Ok(if written { Some(id) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::write::{store as write_store, WriteConfig};

    fn write_cfg() -> WriteConfig {
        WriteConfig { dedup_threshold: 0.95, short_ttl_seconds: 3600, long_ttl_seconds: 86_400, embedding_enabled: true }
    }

    #[test]
    fn collect_consolidation_batch_skips_orphans_and_keeps_paging() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "vivo-1", None, None).unwrap();
        // Órfão: índice aponta pra um id sem hash (simula TTL expirado que já sumiu do hash mas não do zset).
        kv.z_add(&index_key(MemoryTier::Short), 500.0, "orfao").unwrap();
        write_store(&kv, &write_cfg(), true, "vivo-2", None, None).unwrap();

        let batch = collect_consolidation_batch(&kv, 2).unwrap();
        assert_eq!(batch.len(), 2);
        assert!(batch.iter().all(|(id, _)| id != "orfao"));
    }

    #[tokio::test]
    async fn consolidate_old_does_nothing_below_the_batch_size() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "unico", None, None).unwrap();
        let removed =
            consolidate_old(&kv, 5, |_| async { Ok("resumo".to_string()) }, |s| store_consolidation_summary(&kv, s, 86_400)).await.unwrap();
        assert_eq!(removed, 0);
    }

    #[tokio::test]
    async fn consolidate_old_replaces_the_batch_with_one_summary() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "fato 1", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "fato 2", None, None).unwrap();

        let removed = consolidate_old(&kv, 2, |texts| async move { Ok(format!("resumo de {} fatos", texts.len())) }, |s| {
            store_consolidation_summary(&kv, s, 86_400)
        })
        .await
        .unwrap();
        assert_eq!(removed, 2);

        let long = super::super::store::load_tier(&kv, MemoryTier::Long, None).unwrap();
        assert_eq!(long.len(), 1);
        assert_eq!(long[0].content, "resumo de 2 fatos");
        assert!(super::super::store::load_tier(&kv, MemoryTier::Short, None).unwrap().is_empty());
    }

    #[tokio::test]
    async fn consolidate_old_does_nothing_when_the_llm_summary_is_blank() {
        let kv = MemoryStore::open_in_memory().unwrap();
        write_store(&kv, &write_cfg(), true, "fato 1", None, None).unwrap();
        write_store(&kv, &write_cfg(), true, "fato 2", None, None).unwrap();

        let removed =
            consolidate_old(&kv, 2, |_| async { Ok("   ".to_string()) }, |s| store_consolidation_summary(&kv, s, 86_400)).await.unwrap();
        assert_eq!(removed, 0);
        assert_eq!(super::super::store::load_tier(&kv, MemoryTier::Short, None).unwrap().len(), 2);
    }

    // --- promote_aged_entries: consolidação por idade, relógio injetado --------------------------

    const ONE_DAY_MS: i64 = 24 * 3_600_000;

    #[test]
    fn promote_aged_entries_moves_only_entries_past_the_cutoff_to_the_long_tier() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let now = 20 * ONE_DAY_MS;
        write_entry(&kv, MemoryTier::Short, "velho", "da semana passada", "conversation", now - 6 * ONE_DAY_MS, "{}", 3600, None).unwrap();
        write_entry(&kv, MemoryTier::Short, "novo", "de agora", "conversation", now - ONE_DAY_MS, "{}", 3600, None).unwrap();

        let promoted = promote_aged_entries(&kv, now, 5 * ONE_DAY_MS, 86_400).unwrap();
        assert_eq!(promoted, 1, "só a entrada de 6 dias passou do corte de 5");

        let short = super::super::store::load_tier(&kv, MemoryTier::Short, None).unwrap();
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].id, "novo", "a entrada recente fica no tier curto");

        let long = super::super::store::load_tier(&kv, MemoryTier::Long, None).unwrap();
        assert_eq!(long.len(), 1);
        assert_eq!(long[0].content, "da semana passada");
    }

    /// Cobre exatamente o cenário do defeito: um usuário de baixo uso nunca junta as 200 entradas
    /// que `consolidate_old`/`CONSOLIDATION_THRESHOLD` exigem — a promoção por idade não depende de
    /// volume nenhum, só de idade.
    #[test]
    fn promote_aged_entries_works_with_a_single_entry_far_below_the_consolidation_batch_size() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let now = 10 * ONE_DAY_MS;
        write_entry(&kv, MemoryTier::Short, "unico", "memoria isolada de usuario esporadico", "conversation", now - 6 * ONE_DAY_MS, "{}", 3600, None).unwrap();

        assert_eq!(promote_aged_entries(&kv, now, 5 * ONE_DAY_MS, 86_400).unwrap(), 1);
        assert!(super::super::store::load_tier(&kv, MemoryTier::Short, None).unwrap().is_empty());
    }

    #[test]
    fn promote_aged_entries_preserves_content_metadata_timestamp_and_embedding() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let now = 10 * ONE_DAY_MS;
        write_entry(&kv, MemoryTier::Short, "a", "conteudo original", "note", now - 6 * ONE_DAY_MS, "{\"k\":1}", 3600, Some(&[0.1, 0.2])).unwrap();

        assert_eq!(promote_aged_entries(&kv, now, 5 * ONE_DAY_MS, 86_400).unwrap(), 1);

        let long = super::super::store::load_tier(&kv, MemoryTier::Long, None).unwrap();
        assert_eq!(long[0].content, "conteudo original");
        assert_eq!(long[0].r#type, "note");
        assert_eq!(long[0].timestamp, now - 6 * ONE_DAY_MS, "carimbo de tempo original preservado, não o do boot");
        assert_eq!(long[0].metadata, "{\"k\":1}");
        assert_eq!(long[0].embedding, Some(vec![0.1, 0.2]));
    }

    #[test]
    fn promote_aged_entries_does_nothing_when_nothing_is_old_enough() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let now = 10 * ONE_DAY_MS;
        write_entry(&kv, MemoryTier::Short, "recente", "de ontem", "conversation", now - ONE_DAY_MS, "{}", 3600, None).unwrap();

        assert_eq!(promote_aged_entries(&kv, now, 5 * ONE_DAY_MS, 86_400).unwrap(), 0);
        assert_eq!(super::super::store::load_tier(&kv, MemoryTier::Short, None).unwrap().len(), 1);
    }

    #[test]
    fn promote_aged_entries_also_promotes_a_young_entry_marked_important() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let now = 10 * ONE_DAY_MS;
        write_entry(&kv, MemoryTier::Short, "importante", "lembrar sempre", "note", now - ONE_DAY_MS, r#"{"importance":5}"#, 3600, None).unwrap();
        write_entry(&kv, MemoryTier::Short, "banal", "de ontem tambem", "conversation", now - ONE_DAY_MS, "{}", 3600, None).unwrap();

        let promoted = promote_aged_entries(&kv, now, 5 * ONE_DAY_MS, 86_400).unwrap();
        assert_eq!(promoted, 1, "só a entrada com importance>=4 é jovem o bastante para escapar do corte de idade");

        let long = super::super::store::load_tier(&kv, MemoryTier::Long, None).unwrap();
        assert_eq!(long.len(), 1);
        assert_eq!(long[0].id, "importante");

        let short = super::super::store::load_tier(&kv, MemoryTier::Short, None).unwrap();
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].id, "banal", "a entrada jovem e não-importante fica no tier curto");
    }

    /// NEGATIVO — uma idade máxima zero/negativa (bug de configuração) não pode ser lida como "sem
    /// piso, promova tudo": degrada para nenhuma promoção.
    #[test]
    fn promote_aged_entries_with_a_non_positive_max_age_promotes_nothing() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let now = 10 * ONE_DAY_MS;
        write_entry(&kv, MemoryTier::Short, "a", "x", "conversation", now - 9 * ONE_DAY_MS, "{}", 3600, None).unwrap();

        assert_eq!(promote_aged_entries(&kv, now, 0, 86_400).unwrap(), 0);
        assert_eq!(promote_aged_entries(&kv, now, -1, 86_400).unwrap(), 0);
    }

    #[test]
    fn promote_aged_entries_prunes_orphans_found_along_the_way() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let now = 10 * ONE_DAY_MS;
        // Índice aponta para um id sem hash vivo (TTL já expirou) — mesma simulação de órfão que
        // `collect_consolidation_batch` já cobre acima.
        kv.z_add(&index_key(MemoryTier::Short), (now - 9 * ONE_DAY_MS) as f64, "orfao").unwrap();

        assert_eq!(promote_aged_entries(&kv, now, 5 * ONE_DAY_MS, 86_400).unwrap(), 0);
        let ids = kv.z_range_by_score(&index_key(MemoryTier::Short), ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts::default()).unwrap();
        assert!(ids.is_empty(), "órfão tinha de sair do índice mesmo sem ser promovido");
    }
}
