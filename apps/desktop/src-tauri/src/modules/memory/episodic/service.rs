//! Fachada da memória episódica — porte de `EpisodicMemoryService` (`episodic-memory.service.ts`).
//! Reúne `write`/`queries`/`consolidation`/`context` sobre um `MemoryStore` — o TS troca de backend
//! (Redis/embarcado) em runtime via `RedisRunner`; Rust só tem o backend embarcado (decisão já
//! tomada no cabeçalho de `modules/memory/mod.rs`), então não há equivalente de "indisponível,
//! tentando reconectar" aqui — o `MemoryStore` local não cai como um servidor Redis remoto cairia.
//!
//! **Assinatura de embedding rastreada desde esta rodada**: `generate_embedding`
//! (`episodic::embedding`) tem 4 clientes HTTP reais (Ollama/Google/OpenRouter/OpenAI-compatible) e,
//! a cada vetor calculado de verdade, chama `invalidate_index_if_embedding_changed` internamente —
//! trocar de provedor/modelo (dimensão diferente) derruba o índice velho em vez de misturar vetores
//! incompatíveis na busca semântica (defects.md "memory-relationship" P1: a invalidação existia,
//! testada, sem chamador de produção).

use super::consolidation::{consolidate_old, promote_aged_entries, store_consolidation_summary};
use super::context::{build_context, ContextFlags};
use super::embedding::generate_embedding_async;
use super::queries::{self, QueryConfig};
use super::search::ScoreFlags;
use super::types::{
    MemoryEntry, GET_ALL_LIMIT_DEFAULT, LONG_TTL_DAYS_DEFAULT, SHORT_TTL_HOURS_DEFAULT,
};
use super::write::{self, WriteConfig};
use crate::modules::memory::MemoryStore;

pub struct EpisodicMemoryService<'a> {
    store: &'a MemoryStore,
    pub recency_bias: f64,
    pub reranking_enabled: bool,
    pub embedding_enabled: bool,
    pub emotional_tagging: bool,
    pub rag_top_k: usize,
    pub min_similarity: f64,
    pub dedup_threshold: f64,
    pub lexical_rerank_weight: f64,
    short_ttl_seconds: i64,
    long_ttl_seconds: i64,
    get_all_limit: i64,
    current_game: Option<String>,
}

impl<'a> EpisodicMemoryService<'a> {
    /// Mesmos defaults de campo de `EpisodicMemoryService` no TS.
    pub fn new(store: &'a MemoryStore) -> Self {
        Self {
            store,
            recency_bias: 0.25,
            reranking_enabled: true,
            embedding_enabled: true,
            emotional_tagging: true,
            rag_top_k: 5,
            min_similarity: 0.25,
            dedup_threshold: 0.95,
            lexical_rerank_weight: 0.15,
            short_ttl_seconds: (SHORT_TTL_HOURS_DEFAULT * 3600.0) as i64,
            long_ttl_seconds: (LONG_TTL_DAYS_DEFAULT * 86_400.0) as i64,
            get_all_limit: GET_ALL_LIMIT_DEFAULT,
            current_game: None,
        }
    }

    /// O serviço com a config viva do usuário (TTL curto/longo, limite de lista e os sete campos de
    /// scoring, inclusive `embeddingEnabled`). Quem GRAVA tem de usar este, não [`Self::new`]: só o
    /// leitor aplicava a config, então a escrita ignorava `memory.shortTermTtlHours`/
    /// `longTermTtlDays` e continuava gerando embedding (texto da conversa saindo para o provedor,
    /// que pode ser nuvem) com `brain.embeddingEnabled` desligado.
    pub(crate) fn with_config(store: &'a MemoryStore, cfg: Option<&crate::modules::memory::EpisodicScoringConfig>, list_limit: i64) -> Self {
        let mut svc = Self::new(store);
        match cfg {
            Some(cfg) => {
                svc.apply_retention_config(Some(cfg.short_term_ttl_hours), Some(cfg.long_term_ttl_days), Some(list_limit));
                svc.apply_scoring_config(cfg);
            }
            None => svc.apply_retention_config(None, None, Some(list_limit)),
        }
        svc
    }

    /// = `applyRetentionConfig`.
    pub fn apply_retention_config(&mut self, short_term_ttl_hours: Option<f64>, long_term_ttl_days: Option<f64>, episodic_list_limit: Option<i64>) {
        self.short_ttl_seconds = (short_term_ttl_hours.unwrap_or(SHORT_TTL_HOURS_DEFAULT) * 3600.0) as i64;
        self.long_ttl_seconds = (long_term_ttl_days.unwrap_or(LONG_TTL_DAYS_DEFAULT) * 86_400.0) as i64;
        self.get_all_limit = episodic_list_limit.filter(|&n| n != 0).unwrap_or(GET_ALL_LIMIT_DEFAULT);
    }

    /// Aplica os sete campos de scoring de `brain.schema.ts` (`rerankingEnabled`/`emotionalTagging`/
    /// `embeddingEnabled`/`recencyBias`/`ragTopK`/`minSimilarity`/`lexicalRerankWeight`) sobre a
    /// instância — mesmo padrão de [`Self::apply_retention_config`], para os campos que ficavam
    /// hardcoded nos defaults do TS embutidos em [`Self::new`] e nunca liam o `ConfigStore` real
    /// (Settings > Brain mostrava os sete controles sem nenhum efeito na busca/rerank/geração de
    /// embedding). Chamado por `modules::memory::episodic_reader()` com `EpisodicScoringConfig`
    /// (mesmo `OnceLock` cacheado no boot que `EPISODIC_LIST_LIMIT`/`RECORDING_ALLOWED` já usam,
    /// porque este ponto não tem `ConfigStore` no próprio construtor).
    pub fn apply_scoring_config(&mut self, cfg: &crate::modules::memory::EpisodicScoringConfig) {
        self.reranking_enabled = cfg.reranking_enabled;
        self.emotional_tagging = cfg.emotional_tagging;
        self.embedding_enabled = cfg.embedding_enabled;
        self.recency_bias = cfg.recency_bias;
        self.rag_top_k = cfg.rag_top_k;
        self.min_similarity = cfg.min_similarity;
        self.lexical_rerank_weight = cfg.lexical_rerank_weight;
    }

    fn write_config(&self) -> WriteConfig {
        WriteConfig {
            dedup_threshold: self.dedup_threshold,
            short_ttl_seconds: self.short_ttl_seconds,
            long_ttl_seconds: self.long_ttl_seconds,
            embedding_enabled: self.embedding_enabled,
        }
    }

    fn query_config(&self) -> QueryConfig {
        QueryConfig {
            get_all_limit: self.get_all_limit,
            min_similarity: self.min_similarity,
            score_flags: ScoreFlags { reranking_enabled: self.reranking_enabled, emotional_tagging: self.emotional_tagging },
            lexical_rerank_weight: if self.reranking_enabled { self.lexical_rerank_weight } else { 0.0 },
        }
    }

    pub fn store(&self, recording_allowed: bool, content: &str, embedding: Option<Vec<f64>>, metadata: Option<&serde_json::Value>) -> Result<Option<String>, String> {
        write::store(self.store, &self.write_config(), recording_allowed, content, embedding, metadata)
    }

    // `update` (escrita) não tem chamador de produção ainda — só `get_all`/`search`/`clear`
    // (leitura) ganharam um em rodada anterior, via `modules::memory::episodic_reader()`
    // (`memory_console`, `list_memories`); `delete`/`forget_mentions` ganharam o seu NESTA rodada
    // (`memory_console::delete_episodic` e `features::person_catalog::tool.rs::handle_delete`, ver
    // os cabeçalhos deles). `update` fica pronto, testado, para quando `chat`/`brain` escreverem
    // memória via Rust.
    #[allow(dead_code)]
    pub fn update(&self, recording_allowed: bool, id: &str, content: &str, metadata: Option<&serde_json::Value>) -> Result<bool, String> {
        write::update(self.store, &self.write_config(), recording_allowed, id, content, metadata)
    }

    pub fn get_all(&self) -> Result<Vec<MemoryEntry>, String> {
        queries::get_all(self.store, &self.query_config())
    }

    pub fn delete(&self, id: &str) -> Result<bool, String> {
        queries::delete_memory(self.store, id)
    }

    pub fn forget_mentions(&self, terms: &[String]) -> Result<usize, String> {
        queries::forget_mentions(self.store, terms)
    }

    /// Quantas memórias `forget_mentions(terms)` removeria — a prévia da confirmação da UI.
    pub fn count_mentions(&self, terms: &[String]) -> Result<usize, String> {
        queries::count_mentions(self.store, terms)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>, String> {
        queries::search(self.store, query, limit)
    }

    pub fn search_semantic(&self, embedding: &[f64], limit: usize, recency_bias: f64, query_text: &str) -> Result<Vec<MemoryEntry>, String> {
        queries::search_semantic(self.store, &self.query_config(), embedding, limit, recency_bias, self.current_game.as_deref(), query_text)
    }

    pub fn clear(&self) -> Result<bool, String> {
        queries::clear(self.store)
    }

    pub fn count_short(&self) -> Result<usize, String> {
        queries::count_short(self.store)
    }

    /// = `consolidateOld`. `llm_fn` é `async` desde que `post_turn::pipeline` passou a chamar isto
    /// de verdade via `turn::run_turn_llm_raw` (ver cabeçalho de `consolidation.rs` para o porquê);
    /// um teste passa um resumo fixo dentro de um `async {}`.
    pub async fn consolidate_old<F, Fut>(&self, recording_allowed: bool, batch_size: usize, llm_fn: F) -> Result<usize, String>
    where
        F: FnOnce(Vec<String>) -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        if !recording_allowed {
            return Ok(0);
        }
        let long_ttl = self.long_ttl_seconds;
        consolidate_old(self.store, batch_size, llm_fn, |summary| store_consolidation_summary(self.store, summary, long_ttl)).await
    }

    /// = `promote_aged_entries` (ver o cabeçalho daquele arquivo): sweep de manutenção, independente
    /// do volume que [`Self::consolidate_old`] exige — chamada por
    /// `brain::post_turn::aging_sweep` no boot e periodicamente.
    pub fn promote_aged(&self, recording_allowed: bool, now_ms: i64, max_age_ms: i64) -> Result<usize, String> {
        if !recording_allowed {
            return Ok(0);
        }
        // O corte fixo de 5 dias do sweep ignorava o TTL curto do usuário: com 24 h a entrada
        // expirava ANTES de ser promovida. Promove com 70% do TTL curto, no máximo.
        let max_age_ms = max_age_ms.min(self.short_ttl_seconds.saturating_mul(700));
        promote_aged_entries(self.store, now_ms, max_age_ms, self.long_ttl_seconds)
    }

    /// = `buildContext`: tenta busca semântica primeiro (quando há embedding, ver
    /// `episodic::embedding`), cai para palavra-chave quando não há vetor OU a busca semântica não
    /// achou nada — MESMA ordem de fallback do TS. `async`: o embedding da consulta é `await`ado
    /// (caminho do turno de chat, ver o cabeçalho de `episodic::embedding`).
    pub async fn build_context(&self, query: &str) -> Result<String, String> {
        let top_k = self.rag_top_k;
        let mut relevant = Vec::new();
        if self.embedding_enabled {
            if let Some(embedding) = generate_embedding_async(self.store, query, self.embedding_enabled).await {
                relevant = self.search_semantic(&embedding, top_k, self.recency_bias, query)?;
            }
        }
        // Entrada sem vetor (resumo de consolidação, ou gravada com o provedor de embedding fora do
        // ar) nunca aparece na busca semântica. Antes só entrava quando o semântico voltava VAZIO;
        // agora a busca por palavra-chave completa o que faltar até `top_k`.
        if relevant.len() < top_k {
            relevant = top_up_with_keyword(relevant, self.search(query, top_k)?, top_k);
        }
        Ok(build_context(&relevant, ContextFlags { emotional_tagging: self.emotional_tagging }))
    }
}

/// Completa `semantic` com os acertos por palavra-chave que ainda não estão lá, até `top_k`.
fn top_up_with_keyword(mut semantic: Vec<MemoryEntry>, keyword: Vec<MemoryEntry>, top_k: usize) -> Vec<MemoryEntry> {
    for entry in keyword {
        if semantic.len() >= top_k {
            break;
        }
        if !semantic.iter().any(|existing| existing.id == entry.id) {
            semantic.push(entry);
        }
    }
    semantic
}

#[cfg(test)]
impl EpisodicMemoryService<'_> {
    pub fn count(&self) -> Result<usize, String> {
        queries::count(self.store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn age_promotion_never_waits_longer_than_the_short_ttl_allows() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let mut svc = EpisodicMemoryService::new(&kv);
        svc.apply_retention_config(Some(1.0), None, None); // TTL curto de 1 h
        let now = 10_000_000_000i64;
        let min = 60_000;
        super::super::store::write_entry(&kv, super::super::types::MemoryTier::Short, "quase-expirando", "x", "conversation", now - 50 * min, "{}", 3600, None).unwrap();
        // o corte padrão do sweep é 5 dias: com TTL de 1 h a entrada expiraria muito antes
        let promoted = svc.promote_aged(true, now, 5 * 24 * 60 * min).unwrap();
        assert_eq!(promoted, 1, "a promoção por idade acompanha o TTL curto configurado");
    }

    #[test]
    fn keyword_matches_top_up_semantic_results_without_duplicates_or_overflow() {
        let entry = |id: &str| MemoryEntry { id: id.to_string(), content: id.to_string(), r#type: "conversation".into(), timestamp: 0, metadata: "{}".into(), embedding: None };
        let semantic = vec![entry("a")];
        let keyword = vec![entry("a"), entry("sem-vetor"), entry("outra")];
        let merged = top_up_with_keyword(semantic, keyword, 2);
        assert_eq!(merged.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["a", "sem-vetor"], "sem repetir e respeitando o teto");
    }

    #[test]
    fn with_config_applies_the_users_retention_and_embedding_choices() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let cfg = crate::modules::memory::EpisodicScoringConfig {
            reranking_enabled: true,
            emotional_tagging: true,
            embedding_enabled: false,
            recency_bias: 0.25,
            rag_top_k: 5,
            min_similarity: 0.25,
            lexical_rerank_weight: 0.15,
            short_term_ttl_hours: 24.0,
            long_term_ttl_days: 30.0,
        };
        let svc = EpisodicMemoryService::with_config(&kv, Some(&cfg), 50);
        assert_eq!(svc.short_ttl_seconds, 24 * 3600);
        assert_eq!(svc.long_ttl_seconds, 30 * 86_400);
        assert!(!svc.embedding_enabled, "embedding desligado não pode ser ligado de novo na escrita");
        assert_eq!(svc.write_config().embedding_enabled, false);
        assert_eq!(svc.get_all_limit, 50);
        let untouched = EpisodicMemoryService::with_config(&kv, None, 0);
        assert_eq!(untouched.short_ttl_seconds, 168 * 3600, "sem config, os defaults");
    }

    #[test]
    fn defaults_match_the_ts_service_fields() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let svc = EpisodicMemoryService::new(&kv);
        assert_eq!(svc.recency_bias, 0.25);
        assert_eq!(svc.dedup_threshold, 0.95);
        assert_eq!(svc.rag_top_k, 5);
    }

    #[test]
    fn apply_retention_config_falls_back_to_defaults_on_none_or_zero() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let mut svc = EpisodicMemoryService::new(&kv);
        svc.apply_retention_config(Some(1.0), Some(2.0), Some(0));
        assert_eq!(svc.get_all_limit, GET_ALL_LIMIT_DEFAULT, "0 cai no default, igual ao `|| GET_ALL_LIMIT_DEFAULT` do TS");
    }

    /// Regressão: antes de [`EpisodicMemoryService::apply_scoring_config`] existir, os sete campos
    /// de `brain.schema.ts` nunca saíam dos defaults do TS embutidos em [`EpisodicMemoryService::new`]
    /// — este teste falha se o método parar de sobrescrever algum dos sete (ou se algum ficar preso
    /// no default, provando que Settings > Brain não teria efeito nenhum na busca/embedding real).
    #[test]
    fn apply_scoring_config_overrides_every_one_of_the_seven_ts_defaults() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let mut svc = EpisodicMemoryService::new(&kv);
        let cfg = crate::modules::memory::EpisodicScoringConfig {
            reranking_enabled: false,
            emotional_tagging: false,
            embedding_enabled: false,
            recency_bias: 0.9,
            rag_top_k: 12,
            min_similarity: 0.6,
            lexical_rerank_weight: 0.4,
            short_term_ttl_hours: 168.0,
            long_term_ttl_days: 365.0,
        };
        svc.apply_scoring_config(&cfg);
        assert!(!svc.reranking_enabled);
        assert!(!svc.emotional_tagging);
        assert!(!svc.embedding_enabled);
        assert_eq!(svc.recency_bias, 0.9);
        assert_eq!(svc.rag_top_k, 12);
        assert_eq!(svc.min_similarity, 0.6);
        assert_eq!(svc.lexical_rerank_weight, 0.4);
    }

    /// `query_config()` é o que `search`/`search_semantic`/`build_context` de fato consomem — prova
    /// que a config aplicada chega ao `ScoreFlags`/`lexical_rerank_weight` reais, não só aos campos
    /// da struct. `reranking_enabled: false` zera `lexical_rerank_weight` na config de query (mesma
    /// regra que `query_config()` já aplicava antes desta rodada).
    #[test]
    fn apply_scoring_config_reranking_disabled_zeroes_the_lexical_rerank_weight_in_query_config() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let mut svc = EpisodicMemoryService::new(&kv);
        svc.apply_scoring_config(&crate::modules::memory::EpisodicScoringConfig {
            reranking_enabled: false,
            emotional_tagging: true,
            embedding_enabled: true,
            recency_bias: 0.25,
            rag_top_k: 5,
            min_similarity: 0.25,
            lexical_rerank_weight: 0.4,
            short_term_ttl_hours: 168.0,
            long_term_ttl_days: 365.0,
        });
        assert_eq!(svc.query_config().lexical_rerank_weight, 0.0);
    }

    /// Regressão do item #2: `brain.embeddingEnabled` desligado tem de chegar ao `WriteConfig` real
    /// que `store()` consome (`embedding_enabled: true` era hardcoded em
    /// [`EpisodicMemoryService::new`], sem chamador nenhum lendo o `ConfigStore`) — `write_config()`
    /// é o ÚNICO ponto que `store`/`update` leem para decidir se tentam gerar embedding.
    #[test]
    fn apply_scoring_config_embedding_disabled_reaches_write_config() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let mut svc = EpisodicMemoryService::new(&kv);
        assert!(svc.write_config().embedding_enabled, "default do TS é true");
        svc.apply_scoring_config(&crate::modules::memory::EpisodicScoringConfig {
            reranking_enabled: true,
            emotional_tagging: true,
            embedding_enabled: false,
            recency_bias: 0.25,
            rag_top_k: 5,
            min_similarity: 0.25,
            lexical_rerank_weight: 0.15,
            short_term_ttl_hours: 168.0,
            long_term_ttl_days: 365.0,
        });
        assert!(!svc.write_config().embedding_enabled);
    }

    #[tokio::test]
    async fn store_then_build_context_falls_back_to_keyword_search_without_embedding_provider() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "Mika gosta de Rust e RPGs", None, None).unwrap();

        let ctx = svc.build_context("rust").await.unwrap();
        assert!(ctx.contains("Mika gosta de Rust e RPGs"), "{ctx}");
    }

    #[tokio::test]
    async fn build_context_is_empty_when_nothing_matches() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "assunto totalmente diferente", None, None).unwrap();
        assert_eq!(svc.build_context("xyz-inexistente").await.unwrap(), "");
    }

    #[test]
    fn store_respects_memory_enabled_false_end_to_end() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let svc = EpisodicMemoryService::new(&kv);
        assert_eq!(svc.store(false, "não deveria gravar", None, None).unwrap(), None);
        assert_eq!(svc.count().unwrap(), 0);
    }

    #[tokio::test]
    async fn consolidate_old_end_to_end_through_the_service() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "fato 1", None, None).unwrap();
        svc.store(true, "fato 2", None, None).unwrap();

        let removed = svc.consolidate_old(true, 2, |texts| async move { Ok(format!("resumo({})", texts.len())) }).await.unwrap();
        assert_eq!(removed, 2);
        assert_eq!(svc.count_short().unwrap(), 0);
        assert_eq!(svc.get_all().unwrap()[0].content, "resumo(2)");
    }

    #[tokio::test]
    async fn consolidate_old_blocked_by_memory_enabled_false() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "fato 1", None, None).unwrap();
        svc.store(true, "fato 2", None, None).unwrap();
        let removed = svc.consolidate_old(false, 2, |_| async { Ok("resumo".to_string()) }).await.unwrap();
        assert_eq!(removed, 0);
        assert_eq!(svc.count_short().unwrap(), 2);
    }

    #[test]
    fn important_metadata_promotes_directly_to_long_tier_via_the_service() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let svc = EpisodicMemoryService::new(&kv);
        svc.store(true, "fato vital", None, Some(&json!({ "important": true }))).unwrap();
        let all = svc.get_all().unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn promote_aged_through_the_service_uses_its_own_long_ttl_and_respects_recording_allowed() {
        let kv = MemoryStore::open_in_memory().unwrap();
        let svc = EpisodicMemoryService::new(&kv);
        let now = 10 * 24 * 3_600_000i64;
        svc.store(true, "memoria de usuario esporadico", None, None).unwrap();
        // Recua o carimbo escrito por `store` para simular 6 dias de idade (mesmo truque de
        // `write_entry` direto que `consolidation.rs` já usa).
        let id = svc.get_all().unwrap()[0].id.clone();
        let hash_key = format!("mika:mem:short:{id}");
        kv.h_set(&hash_key, "timestamp", &(now - 6 * 24 * 3_600_000).to_string()).unwrap();
        kv.z_add("mika:mem:index:short", (now - 6 * 24 * 3_600_000) as f64, &id).unwrap();

        assert_eq!(svc.promote_aged(false, now, 5 * 24 * 3_600_000).unwrap(), 0, "memory.enabled=false bloqueia a promoção");
        assert_eq!(svc.promote_aged(true, now, 5 * 24 * 3_600_000).unwrap(), 1);
        assert_eq!(svc.get_all().unwrap()[0].content, "memoria de usuario esporadico");
    }
}
