//! Casos de `game_learning.rs`, extraídos por causa do teto de 300 linhas do AGENTS.md.

use super::*;
use std::sync::Mutex;

struct FakeResearch {
    hits: Vec<SearchHit>,
    summary: Result<String, String>,
    search_calls: Mutex<usize>,
    summarize_calls: Mutex<usize>,
}

impl FakeResearch {
    fn with_summary(summary: &str) -> Self {
        Self { hits: vec![], summary: Ok(summary.to_string()), search_calls: Mutex::new(0), summarize_calls: Mutex::new(0) }
    }
}

#[async_trait]
impl GameResearch for FakeResearch {
    async fn search(&self, _query: &str) -> Result<Vec<SearchHit>, String> {
        *self.search_calls.lock().unwrap() += 1;
        Ok(self.hits.iter().map(|h| SearchHit { title: h.title.clone(), snippet: h.snippet.clone(), url: h.url.clone() }).collect())
    }
    async fn summarize(&self, _prompt: &str) -> Result<String, String> {
        *self.summarize_calls.lock().unwrap() += 1;
        self.summary.clone()
    }
}

fn memory() -> MemoryStore {
    MemoryStore::open_in_memory().unwrap()
}

#[test]
fn normalize_game_name_folds_case_and_diacritics() {
    assert_eq!(normalize_game_name("  Hollow Knight  "), "hollow knight");
    assert_eq!(normalize_game_name("É um Jogo"), "e um jogo");
}

#[tokio::test]
async fn learn_game_details_redacts_a_secret_before_persisting() {
    let memory = memory();
    let research = FakeResearch::with_summary("A chave da API é sk-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789, guarde bem.");

    let learned = learn_game_details(&memory, &research, "Hades", "pt-BR").await.unwrap();

    assert!(!learned.contains("sk-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"), "segredo vazou para o resumo aprendido: {learned}");
    let cached = memory.get("wiki:hades").unwrap().unwrap();
    assert!(!cached.contains("sk-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"), "segredo vazou para MemoryStore: {cached}");
}

#[test]
fn wiki_context_for_is_empty_before_anything_is_learned() {
    let memory = memory();
    assert_eq!(wiki_context_for(&memory, "Hades"), "");
}

#[test]
fn wiki_context_for_wraps_the_cached_summary_after_learning() {
    let memory = memory();
    memory.set("wiki:hades", "Hades é um roguelike.", Some(WIKI_TTL_SECONDS)).unwrap();
    let ctx = wiki_context_for(&memory, "Hades");
    assert!(ctx.contains("Hades é um roguelike."));
    assert!(ctx.contains("CONHECIMENTO DA WIKI"));
}

#[tokio::test]
async fn learn_game_details_stores_and_returns_the_summary_for_a_new_game() {
    let memory = memory();
    let research = FakeResearch::with_summary("Hades é um roguelike sobre fugir do submundo.");

    let learned = learn_game_details(&memory, &research, "Hades", "pt-BR").await;

    assert_eq!(learned.as_deref(), Some("Hades é um roguelike sobre fugir do submundo."));
    assert_eq!(memory.get("wiki:hades").unwrap().as_deref(), Some("Hades é um roguelike sobre fugir do submundo."));
    assert_eq!(*research.summarize_calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn a_cached_wiki_skips_research_entirely() {
    let memory = memory();
    memory.set("wiki:hades", "já sei tudo sobre Hades.", Some(WIKI_TTL_SECONDS)).unwrap();
    let research = FakeResearch::with_summary("nunca deveria chegar aqui");

    let learned = learn_game_details(&memory, &research, "Hades", "pt-BR").await;

    assert!(learned.is_none());
    assert_eq!(*research.summarize_calls.lock().unwrap(), 0, "cache já presente não deveria disparar pesquisa");
}

#[tokio::test]
async fn an_empty_game_name_is_a_no_op() {
    let memory = memory();
    let research = FakeResearch::with_summary("x");
    assert!(learn_game_details(&memory, &research, "   ", "pt-BR").await.is_none());
    assert_eq!(*research.summarize_calls.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_failed_summary_does_not_poison_the_cache() {
    let memory = memory();
    let research = FakeResearch { hits: vec![], summary: Err("provider indisponível".into()), search_calls: Mutex::new(0), summarize_calls: Mutex::new(0) };

    let learned = learn_game_details(&memory, &research, "Celeste", "pt-BR").await;

    assert!(learned.is_none());
    assert!(memory.get("wiki:celeste").unwrap().is_none(), "resumo vazio/falho não deveria ser cacheado");
}

struct NoopSender;
#[async_trait]
impl crate::modules::brain::awareness::engine::ProactiveSender for NoopSender {
    async fn send(&self, _prompt: &str) -> String {
        String::new()
    }
}

#[tokio::test]
async fn on_game_changed_skips_the_initial_detection() {
    let memory = memory();
    let research = FakeResearch::with_summary("resumo");
    let engine =
        ProactiveEngine::new(vec![], Box::new(NoopSender), false, 120_000, Box::new(|| (23, 7)), Box::new(|| 50.0), Box::new(|| 12));

    on_game_changed(&engine, &memory, &research, &serde_json::json!({ "game": "Hades", "initial": true }), "pt-BR").await;

    assert_eq!(*research.summarize_calls.lock().unwrap(), 0, "detecção inicial não deveria disparar pesquisa");
}

#[test]
fn wiki_cache_key_is_stable_across_case_and_spacing() {
    assert_eq!(wiki_cache_key("Hollow Knight"), wiki_cache_key("hollow knight"));
}

// -- InFlightGames: a corrida que `spawn` existe para fechar -------------------------------

#[test]
fn in_flight_games_claim_rejects_a_key_already_in_progress() {
    let guard = InFlightGames::default();
    assert!(guard.claim("hades"), "primeira reivindicação devia ganhar a corrida");
    assert!(!guard.claim("hades"), "segunda reivindicação concorrente devia perder");
    assert!(guard.claim("celeste"), "chave diferente não é afetada pelo guarda de outra");
}

#[test]
fn in_flight_games_release_lets_the_key_be_claimed_again() {
    let guard = InFlightGames::default();
    assert!(guard.claim("hades"));
    guard.release("hades");
    assert!(guard.claim("hades"), "depois de liberar, uma nova rodada de aprendizado pode reivindicar");
}

/// Prova de ponta a ponta da corrida que [`InFlightGames`] fecha: dois `GameChanged` do MESMO
/// jogo, tratados em tasks concorrentes (mesmo desenho de `spawn`), só disparam UMA pesquisa —
/// sem o guarda, os dois veriam `memory.get(&key) == None` e pesquisariam em paralelo.
#[tokio::test]
async fn concurrent_game_changed_events_for_the_same_game_only_research_once() {
    let memory = Arc::new(memory());
    let research = Arc::new(FakeResearch::with_summary("Hades é um roguelike."));
    let in_flight = Arc::new(InFlightGames::default());
    let barrier = Arc::new(tokio::sync::Barrier::new(2));

    let mut tasks = Vec::new();
    for _ in 0..2 {
        let (memory, research, in_flight, barrier) =
            (Arc::clone(&memory), Arc::clone(&research), Arc::clone(&in_flight), Arc::clone(&barrier));
        tasks.push(tokio::spawn(async move {
            barrier.wait().await; // força as duas tasks a chegarem juntas na checagem do guarda
            let key = normalize_game_name("Hades");
            if !in_flight.claim(&key) {
                return;
            }
            let _ = learn_game_details(&memory, &*research, "Hades", "pt-BR").await;
            in_flight.release(&key);
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*research.summarize_calls.lock().unwrap(), 1, "as duas tasks concorrentes deveriam ter pesquisado o jogo só uma vez");
}

/// O ponto central desta rodada: `on_game_changed` pode levar minutos (busca + LLM). Rodando-o
/// numa task própria em vez de inline no laço do assinante, um evento LENTO não impede que o
/// PRÓXIMO evento do mesmo assinante seja recebido a tempo — a mesma garantia que
/// `events/bus.rs::a_lagging_subscriber_is_notified_with_lagged_instead_of_blocking_the_publisher`
/// prova para o publicador, aqui provada para quem consome.
#[tokio::test]
async fn a_slow_game_changed_handler_does_not_block_the_next_bus_receive() {
    struct SlowResearch;
    #[async_trait]
    impl GameResearch for SlowResearch {
        async fn search(&self, _query: &str) -> Result<Vec<SearchHit>, String> {
            Ok(vec![])
        }
        async fn summarize(&self, _prompt: &str) -> Result<String, String> {
            std::future::pending::<()>().await; // nunca resolve dentro da janela do teste
            unreachable!()
        }
    }

    let bus = crate::events::EventBus::with_capacity(4);
    let mut sub = bus.subscribe();
    bus.publish(EventKey::GameChanged, serde_json::json!({ "game": "Slowpoke", "initial": false }));
    bus.publish(EventKey::VadSample, serde_json::json!({ "speechProbability": 0.5 }));

    // O "assinante" só dispara a task lenta (nunca a espera) antes de continuar drenando —
    // mesmo desenho de `spawn` depois desta rodada.
    let first = sub.recv().await.unwrap();
    assert_eq!(first.key, EventKey::GameChanged);
    let engine = ProactiveEngine::new(vec![], Box::new(NoopSender), false, 120_000, Box::new(|| (23, 7)), Box::new(|| 50.0), Box::new(|| 12));
    tokio::spawn(async move {
        on_game_changed(&engine, &memory(), &SlowResearch, &first.payload, "pt-BR").await;
    });

    // Se `on_game_changed` tivesse sido `.await`ado inline aqui, este `recv()` nunca chegaria a
    // rodar dentro do timeout do teste.
    let second = tokio::time::timeout(std::time::Duration::from_millis(500), sub.recv())
        .await
        .expect("o segundo evento devia chegar sem esperar a task lenta terminar")
        .unwrap();
    assert_eq!(second.key, EventKey::VadSample);
}

/// Pesquisar o jogo na web é busca iniciada pela Mika: sem `browser.enabled` (default de perfil
/// novo) nada sai — o erro volta antes de qualquer rede e `fetch_game_summary` segue sem busca.
#[tokio::test]
async fn live_game_research_never_searches_the_web_until_the_user_enables_it() {
    let dir = std::env::temp_dir().join(format!("mika-game-learning-web-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Arc::new(ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap());

    let error = LiveGameResearch::new(store).search("Hades").await.err().expect("busca desligada deve falhar");

    assert!(error.contains("busca web desligada"), "{error}");
    let _ = std::fs::remove_dir_all(&dir);
}
