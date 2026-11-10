//! `learnGameDetails` — porte de `services/brain/awareness/awareness-game.ts::learnGameDetails` +
//! `fetchGameSummary`: quando `EventKey::GameChanged` chega com um jogo NOVO (não `initial`, mesmo
//! gate de `brain::awareness::event_reactions::on_game_changed`), busca o jogo na web
//! (`modules::browser::search`), pede pro LLM (`brain::turn::run_turn_llm_raw`) compilar um resumo
//! spoiler-free, guarda em `MemoryStore` (`wiki:<nome normalizado>`, TTL de 7 dias — mesma janela de
//! `GAME_WIKI_CACHE_TTL_MS` no TS) e reage uma vez através do MESMO `ProactiveEngine` que o resto de
//! `brain::awareness` usa (`key: game:{game}`, `priority: 8` — igual ao `withProactive` do TS em
//! `awareness-event-handlers.ts::GAME_CHANGED`; a cadência por chave é compartilhada com a reação
//! "steam game" de prioridade 9 de `event_reactions.rs::on_game_changed`, mesma chave lá).
//!
//! **`wikiCtx` (lado de LEITURA) — porte direto desde 2026-09-22.** `maybeLoadGameWiki` no TS faz
//! busca semântica por vetor sobre até 2 memórias já armazenadas; sem `vectorStore`/`embedding` no
//! lado Rust, [`wiki_context_for`] faz uma leitura DIRETA pela MESMA chave normalizada que
//! [`learn_game_details`] grava (`wiki:<nome>`) — equivalente aceitável quando só existe UM resumo
//! por jogo (não vários chunks a rankear), documentado aqui em vez de escondido. Consumido por
//! `GameProvider::due` (`providers.rs`) para preencher `wikiCtx` em `gaming_loop_comment`.
//!
//! **`redactSecrets`** — reaproveita `features::person_catalog::pii::redact_secrets` (mesmo padrão
//! de import cross-feature que `features::desktop::window.rs` já usa para `redact_pii`) ANTES de
//! gravar em `MemoryStore`: nunca aceita segredo (API key, token) num resumo de LLM persistido.
//!
//! **Simplificação nomeada restante:** `llmFactory.resolveLanguageModel('game_wiki')` (papel de
//! modelo dedicado) não existe no lado Rust — usa a MESMA resolução de provedor de qualquer outro
//! `run_turn_llm_raw`.
//!
//! Testável sem rede/LLM real: [`GameResearch`] é injetado (ver AGENTS.md — "Tests never touch the
//! network or a real LLM: inject").

use async_trait::async_trait;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::config::store::ConfigStore;
use crate::events::EventKey;
use crate::modules::brain::awareness::engine::ProactiveEngine;
use crate::modules::brain::awareness::types::ProactiveCandidate;
use crate::modules::brain::turn::{self, NullObserver, TurnAbortHandle};
use crate::modules::chat::load_prompt_no_handle;
use crate::modules::features::person_catalog::pii::redact_secrets;
use crate::modules::memory::MemoryStore;

const WIKI_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const SEARCH_RESULTS_TAKEN: usize = 4;

/// Cópia local da mesma tabela fixa que `brain::guards`/`memory::text`/`features::person_catalog`
/// já têm — mesma decisão de não puxar `unicode-normalization` como dependência (ver o cabeçalho de
/// `guards.rs`).
fn strip_diacritics(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            'ý' | 'ÿ' => 'y',
            'Ý' => 'Y',
            other => other,
        })
        .collect()
}

/// Port de `normalizeGameName`.
fn normalize_game_name(name: &str) -> String {
    strip_diacritics(name.trim()).to_lowercase()
}

pub(crate) fn wiki_cache_key(game_name: &str) -> String {
    format!("wiki:{}", normalize_game_name(game_name))
}

/// Port de `maybeLoadGameWiki`, leitura direta (ver o cabeçalho): mesmo texto de bloco
/// (`"\n\n[CONHECIMENTO DA WIKI DA MIKA SOBRE ESTE JOGO]:\n..."`), string vazia quando não há nada
/// aprendido ainda para este jogo.
pub fn wiki_context_for(memory: &MemoryStore, game_name: &str) -> String {
    match memory.get(&wiki_cache_key(game_name)) {
        Ok(Some(summary)) if !summary.trim().is_empty() => {
            format!("\n\n[CONHECIMENTO DA WIKI DA MIKA SOBRE ESTE JOGO]:\n{}", summary.trim())
        }
        Ok(_) => String::new(),
        Err(error) => {
            log::debug!("wiki_context_for({game_name}): leitura de memória falhou: {error}");
            String::new()
        }
    }
}

/// Um resultado de busca reduzido a (título, snippet, url) — o que `fetchGameSummary` usa do lado
/// TS, sem o resto da forma de `SearchResult`.
pub struct SearchHit {
    pub title: String,
    pub snippet: String,
    pub url: String,
}

/// As duas chamadas de rede/LLM que `learnGameDetails` faz, abstraídas para teste sem rede real —
/// mesma técnica de `ProactiveSender` em `engine.rs`.
#[async_trait]
pub trait GameResearch: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<SearchHit>, String>;
    async fn summarize(&self, prompt: &str) -> Result<String, String>;
}

/// Implementação de produção: `modules::browser::search` (motor "auto", SSRF-guardado) +
/// `brain::turn::run_turn_llm_raw` (resolução de provedor padrão, sem `tools`/histórico).
pub struct LiveGameResearch {
    store: Arc<ConfigStore>,
}

impl LiveGameResearch {
    pub fn new(store: Arc<ConfigStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl GameResearch for LiveGameResearch {
    async fn search(&self, query: &str) -> Result<Vec<SearchHit>, String> {
        // O nome do jogo detectado vai a Bing/DuckDuckGo/Yahoo: só com a busca web ligada pelo
        // usuário (`browser.enabled`, opt-in). Sem ela, `fetch_game_summary` cai no prompt sem busca.
        if !crate::modules::browser::web_enabled(&self.store) {
            return Err("busca web desligada (Configurações → Navegador)".to_string());
        }
        crate::modules::browser::search(&self.store, query, crate::modules::browser::AUTO_ENGINE)
            .await
            .map(|hits| hits.into_iter().map(|h| SearchHit { title: h.title, snippet: h.snippet, url: h.url }).collect())
            .map_err(|error| format!("{error:?}"))
    }

    async fn summarize(&self, prompt: &str) -> Result<String, String> {
        let (_handle, signal) = TurnAbortHandle::new();
        turn::run_turn_llm_raw(&self.store, None, prompt, signal, &NullObserver)
            .await
            .map(|result| result.text)
            .map_err(|error| error.to_string())
    }
}

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Port de `fetchGameSummary`: monta a query de busca, formata até 4 resultados como
/// `Título:/Snippet:/Link:` (mesmo formato do TS) e escolhe o prompt COM ou SEM busca.
async fn fetch_game_summary(research: &dyn GameResearch, game_name: &str, language: &str) -> Result<String, String> {
    let query = format!("\"{game_name}\" game story characters gameplay details wiki");
    let hits = research.search(&query).await.unwrap_or_default();

    let prompt = if hits.is_empty() {
        load_prompt_no_handle("brain", "awareness-game-summary", &vars(&[("gameName", game_name), ("language", language)]))
    } else {
        let search_results_text = hits
            .iter()
            .take(SEARCH_RESULTS_TAKEN)
            .map(|h| format!("Título: {}\nSnippet: {}\nLink: {}", h.title, h.snippet, h.url))
            .collect::<Vec<_>>()
            .join("\n\n");
        load_prompt_no_handle(
            "brain",
            "awareness-game-summary-with-search",
            &vars(&[("gameName", game_name), ("searchResultsText", &search_results_text), ("language", language)]),
        )
    }?;

    research.summarize(&prompt).await
}

/// Port de `learnGameDetails`. `memory` é o `MemoryStore` global do processo (ver
/// `modules::memory::memory_store`) — o cache de dedup do TS (`gameWikiCache` TTL-FIFO em memória +
/// `memory.search('Wiki: ...')`) vira uma checagem só: a MESMA chave que grava também gateia a
/// releitura, sem cache em memória duplicado.
pub async fn learn_game_details(memory: &MemoryStore, research: &dyn GameResearch, game_name: &str, language: &str) -> Option<String> {
    let game_name = game_name.trim();
    if game_name.is_empty() {
        return None;
    }

    let key = wiki_cache_key(game_name);
    match memory.get(&key) {
        Ok(Some(_)) => {
            log::info!("já tenho conhecimento de wiki para {game_name}, aprendizado pulado");
            return None;
        }
        Ok(None) => {}
        Err(error) => log::warn!("checagem de cache de wiki falhou para {game_name}: {error}"),
    }

    log::info!("aprendendo sobre o jogo \"{game_name}\"...");

    let summary = match fetch_game_summary(research, game_name, language).await {
        // Mesma redação seletiva de `context.ts`/`social-memory.service.ts` (ver o cabeçalho):
        // segredo (API key, token) nunca chega a `MemoryStore`, mesmo vindo de um resumo de LLM.
        Ok(text) if !text.trim().is_empty() => redact_secrets(text.trim()),
        Ok(_) => return None,
        Err(error) => {
            log::warn!("falha ao aprender detalhes do jogo {game_name}: {error}");
            return None;
        }
    };

    if let Err(error) = memory.set(&key, &summary, Some(WIKI_TTL_SECONDS)) {
        log::warn!("falha ao gravar wiki de {game_name} em memória: {error}");
        return None;
    }
    log::info!("wiki de {game_name} aprendida e gravada com sucesso");
    Some(summary)
}

/// Reage à wiki recém-aprendida através do MESMO `ProactiveEngine` — `awareness/game-research-
/// learned.md`, `key: game:{game}`, `priority: 8`, mesma cadência por chave que
/// `event_reactions::on_game_changed` (prioridade 9, "steam game").
async fn react_to_learned_game(engine: &ProactiveEngine, game_name: &str, summary: &str, now_ms: u64) {
    let map = vars(&[("game", game_name), ("summary", summary)]);
    let prompt = match load_prompt_no_handle("brain", "awareness/game-research-learned", &map) {
        Ok(p) => p,
        Err(error) => {
            log::error!("reação 'jogo aprendido' sem prompt (game-research-learned): {error}");
            return;
        }
    };

    engine
        .request(
            ProactiveCandidate { key: format!("game:{game_name}"), priority: 8, label: "game wiki learned".to_string(), prompt },
            now_ms,
        )
        .await;
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Port de `registerHandler(Events.GAME_CHANGED, ...)`, só o recorte de `learnGameDetails`: mesmo
/// gate de `initial` que `event_reactions::on_game_changed` já aplica para a reação irmã.
pub async fn on_game_changed(
    engine: &ProactiveEngine,
    memory: &MemoryStore,
    research: &dyn GameResearch,
    payload: &Value,
    language: &str,
) {
    let initial = payload.get("initial").and_then(Value::as_bool).unwrap_or(false);
    if initial {
        return;
    }
    let Some(game) = payload.get("game").and_then(Value::as_str).filter(|g| !g.is_empty()) else { return };

    if let Some(summary) = learn_game_details(memory, research, game, language).await {
        react_to_learned_game(engine, game, &summary, now_ms()).await;
    }
}

/// Jogos cuja pesquisa de wiki está EM ANDAMENTO agora — fecha a corrida entre dois `GameChanged`
/// do MESMO jogo chegando antes do primeiro `learn_game_details` terminar e gravar o cache: a
/// checagem de `MemoryStore` em [`learn_game_details`] é ler-antes-de-escrever (`memory.get` == None
/// para os dois), então sem este guarda os dois disparariam busca+LLM em paralelo para o mesmo jogo.
#[derive(Default)]
struct InFlightGames(Mutex<HashSet<String>>);

impl InFlightGames {
    /// `true` se este chamador ganhou a corrida (a chave NÃO estava em andamento e agora está).
    fn claim(&self, key: &str) -> bool {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(key.to_string())
    }

    fn release(&self, key: &str) {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(key);
    }
}

/// Sobe o assinante de `EventKey::GameChanged` para o aprendizado de wiki — segundo assinante do
/// mesmo evento (o primeiro, em `brain::awareness::mod.rs`, cuida da reação "steam game"); os dois
/// são independentes, `broadcast` entrega a AMBOS. Assina ANTES do `spawn` (mesma regra de
/// AGENTS.md/`modules/mod.rs`), `tauri::async_runtime::spawn`, nunca `tokio::spawn`.
///
/// `on_game_changed` busca na web e chama o LLM — pode levar minutos (ver o cabeçalho do módulo).
/// Await-lo DENTRO deste laço travaria o `recv()` deste assinante por todo esse tempo: `broadcast`
/// não enfileira sem limite (`events/bus.rs`), então qualquer `GameChanged` publicado enquanto este
/// assinante está preso vira `Lagged` e é descartado — inclusive o próprio evento que este
/// assinante existe para consumir. Por isso cada evento vira uma task própria
/// (`tauri::async_runtime::spawn`), com [`InFlightGames`] evitando que dois `GameChanged` do MESMO
/// jogo pesquisem em paralelo.
pub fn spawn(store: Arc<ConfigStore>, memory: &'static MemoryStore, engine: Arc<ProactiveEngine>) -> tauri::async_runtime::JoinHandle<()> {
    let mut sub = crate::events::bus().subscribe();
    let research = Arc::new(LiveGameResearch::new(Arc::clone(&store)));
    let in_flight = Arc::new(InFlightGames::default());

    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if envelope.key != EventKey::GameChanged {
                continue;
            }
            let Some(game) = envelope.payload.get("game").and_then(Value::as_str).filter(|g| !g.is_empty()) else { continue };
            let key = normalize_game_name(game);
            if !in_flight.claim(&key) {
                log::debug!("aprendizado de wiki para \"{game}\" já em andamento, evento ignorado");
                continue;
            }

            // Lido por evento: o idioma pode mudar com o app aberto.
            let language = crate::config::locale::app_language(&store);
            let (engine, research, in_flight_task, payload) =
                (Arc::clone(&engine), Arc::clone(&research), Arc::clone(&in_flight), envelope.payload.clone());
            tauri::async_runtime::spawn(async move {
                on_game_changed(&engine, memory, &*research, &payload, &language).await;
                in_flight_task.release(&key);
            });
        }
    })
}

// Teto de 300 linhas do AGENTS.md: os casos de teste moram em `game_learning_tests.rs`.
#[cfg(test)]
#[path = "game_learning_tests.rs"]
mod tests;
