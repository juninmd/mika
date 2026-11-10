use std::sync::Mutex;

use super::*;
use crate::config::registry::{Namespace, SchemaRegistry};
use crate::config::schema::boolean;

struct TempDir(std::path::PathBuf);

impl TempDir {
    // Contador, não relógio: no Windows dois testes paralelos liam o mesmo nanossegundo e dividiam o
    // diretório — o `Drop` de um apagava o `ConfigStore` do outro.
    fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-guarded-fetch-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn offline_store(offline: bool) -> (ConfigStore, TempDir) {
    let dir = TempDir::new();
    let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", false)] }]);
    let store = ConfigStore::open(&dir.0, schema).unwrap();
    store.set("privacy.offlineMode", serde_json::json!(offline)).unwrap();
    (store, dir)
}

/// Fetcher fake: devolve respostas pré-programadas por URL e GRAVA cada URL que efetivamente
/// despachou — é essa gravação que prova (positivamente) que uma URL bloqueada nunca foi
/// despachada, sem depender de mock de rede real.
struct FakeFetcher {
    responses: std::collections::HashMap<String, HopResponse>,
    calls: Mutex<Vec<String>>,
}

impl FakeFetcher {
    fn new(responses: Vec<(&str, HopResponse)>) -> Self {
        Self {
            responses: responses.into_iter().map(|(url, response)| (url.to_string(), response)).collect(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

fn hop(status: u16, location: Option<&str>, body: &str) -> HopResponse {
    HopResponse { status, location: location.map(str::to_string), body: body.as_bytes().to_vec(), content_type: None }
}

#[async_trait]
impl HopFetcher for FakeFetcher {
    async fn fetch(&self, url: &str) -> Result<HopResponse, String> {
        self.calls.lock().unwrap().push(url.to_string());
        self.responses
            .get(url)
            .map(|response| HopResponse {
                status: response.status,
                location: response.location.clone(),
                body: response.body.clone(),
                content_type: response.content_type.clone(),
            })
            .ok_or_else(|| format!("FakeFetcher: sem resposta programada para {url}"))
    }
}

#[tokio::test]
async fn blocks_ssrf_target_before_any_dispatch() {
    let (store, _dir) = offline_store(false);
    let fetcher = FakeFetcher::new(vec![]);

    let result = guarded_get(&store, &fetcher, "http://127.0.0.1:22/").await;

    assert!(matches!(result, Err(GuardedFetchError::Ssrf(_))));
    assert!(fetcher.calls().is_empty(), "URL bloqueada não deveria nunca ser despachada");
}

#[tokio::test]
async fn blocks_offline_mode_before_any_dispatch() {
    let (store, _dir) = offline_store(true);
    let fetcher = FakeFetcher::new(vec![]);

    let result = guarded_get(&store, &fetcher, "https://example.com/").await;

    assert!(matches!(result, Err(GuardedFetchError::Offline(_))));
    assert!(fetcher.calls().is_empty());
}

/// O caso central da tarefa: uma URL pública aprovada redireciona pra um IP privado — a guarda
/// tem de barrar ANTES do segundo fetch, não depois. Prova POSITIVA: a lista de chamadas do
/// fetcher contém só o primeiro hop, nunca o destino do redirect.
#[tokio::test]
async fn revalidates_the_redirect_target_and_blocks_before_the_second_dispatch() {
    let (store, _dir) = offline_store(false);
    let fetcher =
        FakeFetcher::new(vec![("https://public.example/start", hop(302, Some("http://127.0.0.1/metadata"), ""))]);

    let result = guarded_get(&store, &fetcher, "https://public.example/start").await;

    assert!(matches!(result, Err(GuardedFetchError::Ssrf(_))));
    assert_eq!(fetcher.calls(), vec!["https://public.example/start"]);
}

#[tokio::test]
async fn follows_a_valid_redirect_chain_to_the_final_body() {
    let (store, _dir) = offline_store(false);
    let fetcher = FakeFetcher::new(vec![
        ("https://public.example/start", hop(302, Some("https://public.example/next"), "")),
        ("https://public.example/next", hop(200, None, "hello")),
    ]);

    let result = guarded_get(&store, &fetcher, "https://public.example/start").await.unwrap();

    assert_eq!(result.body, b"hello");
    assert_eq!(fetcher.calls(), vec!["https://public.example/start", "https://public.example/next"]);
}

#[tokio::test]
async fn errors_with_too_many_redirects_instead_of_leaking_the_uncapped_hop() {
    let (store, _dir) = offline_store(false);
    // Sempre redireciona pra si mesma: nunca resolve, tem de estourar o teto.
    let fetcher = FakeFetcher::new(vec![("https://public.example/loop", hop(302, Some("https://public.example/loop"), ""))]);

    let result = guarded_get(&store, &fetcher, "https://public.example/loop").await;

    assert!(matches!(result, Err(GuardedFetchError::TooManyRedirects)));
    assert_eq!(fetcher.calls().len(), 1 + MAX_REDIRECT_HOPS as usize);
}

#[tokio::test]
async fn surfaces_a_non_2xx_final_status_as_bad_status() {
    let (store, _dir) = offline_store(false);
    let fetcher = FakeFetcher::new(vec![("https://public.example/missing", hop(404, None, ""))]);

    let result = guarded_get(&store, &fetcher, "https://public.example/missing").await;

    assert_eq!(result.unwrap_err(), GuardedFetchError::BadStatus(404));
}

/// Pin: `navigation-guard.ts` foi removido (backend Node deletado na migração para Tauri) — os
/// mesmos limites (10 saltos, timeout de 15s) que o TS antes fixava viram literais Rust aqui.
#[test]
fn constants_are_pinned_against_the_expected_limits() {
    assert_eq!(MAX_REDIRECT_HOPS, 10);
    assert_eq!(HOP_TIMEOUT, Duration::from_secs(15));
}

// -- e2e_wiki_override_with -----------------------------------------------------------------------

#[test]
fn e2e_wiki_override_rewrites_a_wikipedia_summary_url_to_the_test_base() {
    let rewritten = e2e_wiki_override_with(
        Some("1"),
        Some("http://127.0.0.1:4321"),
        "https://en.wikipedia.org/api/rest_v1/page/summary/Elden_Ring",
    );
    assert_eq!(rewritten.as_deref(), Some("http://127.0.0.1:4321/api/rest_v1/page/summary/Elden_Ring"));
}

#[test]
fn e2e_wiki_override_preserves_the_query_string() {
    let rewritten = e2e_wiki_override_with(Some("1"), Some("http://127.0.0.1:4321"), "https://upload.wikimedia.org/thumb.jpg?width=100");
    assert_eq!(rewritten.as_deref(), Some("http://127.0.0.1:4321/thumb.jpg?width=100"));
}

#[test]
fn e2e_wiki_override_is_none_without_mika_e2e() {
    assert!(e2e_wiki_override_with(None, Some("http://127.0.0.1:4321"), "https://en.wikipedia.org/x").is_none());
    assert!(e2e_wiki_override_with(Some("0"), Some("http://127.0.0.1:4321"), "https://en.wikipedia.org/x").is_none());
}

#[test]
fn e2e_wiki_override_is_none_without_the_base_url() {
    assert!(e2e_wiki_override_with(Some("1"), None, "https://en.wikipedia.org/x").is_none());
    assert!(e2e_wiki_override_with(Some("1"), Some(""), "https://en.wikipedia.org/x").is_none());
}

#[test]
fn e2e_wiki_override_never_touches_a_non_wikipedia_host_even_when_both_env_vars_are_set() {
    assert!(e2e_wiki_override_with(Some("1"), Some("http://127.0.0.1:4321"), "https://example.com/x").is_none());
}

// -- authorize: o override só troca a URL, a validação continua rodando por cima --------------------

#[tokio::test]
async fn a_wikipedia_url_without_the_e2e_override_still_gets_rejected_by_ssrf_when_it_resolves_to_a_fake_local_host() {
    // Sem as env vars, uma URL apontando pra loopback continua batendo na mesma rejeição de
    // sempre — o override nunca afrouxa a guarda fora do par de env vars.
    let (store, _dir) = offline_store(false);
    let fetcher = FakeFetcher::new(vec![]);
    let result = guarded_get(&store, &fetcher, "http://127.0.0.1:4321/api/rest_v1/page/summary/Elden_Ring").await;
    assert!(matches!(result, Err(GuardedFetchError::Ssrf(_))), "loopback sem o override tinha de ser rejeitado: {result:?}");
    assert!(fetcher.calls().is_empty(), "nunca deveria ter despachado rede");
}

/// Fiação, não tabela: o cliente REAL do pipeline recusa o nome local na resolução e nunca abre
/// socket — o listener (loopback, sem rede externa) continua sem conexão pendente. `.no_proxy()`
/// porque um proxy de sistema resolveria o destino fora deste processo.
#[tokio::test]
async fn the_guarded_client_never_connects_to_a_name_the_resolver_blocks() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let client = guarded_client_builder().no_proxy().build().unwrap();

    let result = client.get(format!("http://app.localhost:{port}/")).timeout(Duration::from_secs(5)).send().await;

    assert!(result.is_err(), "o resolver tinha de recusar app.localhost");
    assert!(
        matches!(listener.accept(), Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "o cliente abriu conexão com um nome bloqueado"
    );
}

/// O charset do hop FINAL chega ao chamador: sem isso a página Shift-JIS/EUC-KR virava `�`.
#[tokio::test]
async fn the_final_hops_content_type_reaches_the_caller_for_charset_decoding() {
    let (store, _dir) = offline_store(false);
    let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode("日本語");
    let mut page = hop(200, None, "");
    page.body = bytes.into_owned();
    page.content_type = Some("text/html; charset=Shift_JIS".to_string());
    let fetcher = FakeFetcher::new(vec![("https://example.com/", page)]);

    let response = guarded_get(&store, &fetcher, "https://example.com/").await.unwrap();

    assert_eq!(response.text(), "日本語");
}
