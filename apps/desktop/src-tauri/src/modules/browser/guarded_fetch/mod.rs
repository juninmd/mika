//! guarded_fetch.rs — porte do núcleo de segurança de `services/navigation-guard.ts`: cada hop de
//! redirect é revalidado (SSRF + modo offline) ANTES do próximo fetch, num loop explícito e
//! bounded. Único ponto do módulo `browser` que despacha rede pra uma URL vinda de busca ou de
//! `action='fetch'` — `search.rs`/`page.rs` nunca chamam `reqwest` diretamente (gate:
//! `mod.rs::search_and_page_never_dispatch_network_outside_the_guarded_pipeline`).
//!
//! ## Por que não dá pra usar a policy padrão do `reqwest`
//!
//! Com `redirect::Policy::default()` (segue redirect sozinho), o `reqwest` já teria despachado a
//! requisição pro destino do redirect ANTES de `send()` devolver o controle a este código —
//! `response.status()` chegaria aqui como o 200 do destino final, e o hop intermediário nunca
//! passaria por `validate_navigable_url`/`assert_network_allowed`. É exatamente a classe de bug que
//! `navigation-guard.ts` existe pra fechar (URL pública aprovada que redireciona pra
//! `127.0.0.1`/`169.254.169.254`). [`guarded_http_client`] desliga o redirect automático
//! (`Policy::none()`) e [`guarded_get`] segue os hops manualmente.

use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;

use crate::config::store::ConfigStore;
use crate::modules::privacy;

use super::validate_navigable_url;

/// = `MAX_REDIRECT_HOPS` em `services/navigation-guard.ts`.
pub(super) const MAX_REDIRECT_HOPS: u8 = 10;
/// = `DEFAULT_TIMEOUT_MS` em `services/navigation-guard.ts`, aplicado por hop.
const HOP_TIMEOUT: Duration = Duration::from_secs(15);
/// Bound novo — `navigation-guard.ts` não tem: lá quem administra a memória do corpo é o Chromium.
/// 10 MiB cobre qualquer HTML de busca/artigo real; acima disso o corpo é truncado, nunca
/// acumulado sem limite (defesa contra DoS de memória por página hostil ou enorme).
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";

/// Erro honesto por categoria — cada variante vira um `errorCode` de tool diferente em
/// `tools/web.rs` (ver a nota de divergência deliberada no cabeçalho daquele arquivo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedFetchError {
    /// `validate_navigable_url` recusou (esquema/host bloqueado) — nunca despachou rede.
    Ssrf(String),
    /// `privacy::assert_network_allowed` recusou (modo offline) — nunca despachou rede.
    Offline(String),
    /// Respondeu, mas com status fora de 2xx/3xx depois de resolver os redirects válidos.
    BadStatus(u16),
    /// Ainda redirecionando depois de [`MAX_REDIRECT_HOPS`] hops.
    TooManyRedirects,
    /// Falha de transporte (DNS, timeout, conexão recusada, TLS, etc.).
    Network(String),
}

#[derive(Debug)]
pub(super) struct GuardedResponse {
    pub(super) body: Vec<u8>,
    /// Status of the final (non-redirect) hop — `search.rs` reads this to tell a DuckDuckGo
    /// anti-bot challenge page (HTTP 202, still inside the 2xx range `guarded_get` already accepts)
    /// apart from a real result page, which `BadStatus` alone cannot express.
    pub(super) status: u16,
    /// `Content-Type` cru do hop final — o charset da página (ver `decode::decode_html`).
    pub(super) content_type: Option<String>,
}

impl GuardedResponse {
    /// Corpo como texto, no charset que a resposta declara (cabeçalho, depois `<meta>`).
    pub(super) fn text(&self) -> String {
        super::decode::decode_html(&self.body, self.content_type.as_deref())
    }
}

/// Resposta de UM hop — quem decide se é redirect ou final é [`guarded_get`], nunca o fetcher.
#[derive(Debug, Clone)]
pub(super) struct HopResponse {
    pub(super) status: u16,
    pub(super) location: Option<String>,
    pub(super) body: Vec<u8>,
    pub(super) content_type: Option<String>,
}

/// Abstrai o transporte pra permitir testar o LOOP sem rede real (fetcher fake injetado nos
/// testes). [`ReqwestFetcher`] é a única implementação que fala HTTP de verdade.
#[async_trait]
pub(super) trait HopFetcher {
    async fn fetch(&self, url: &str) -> Result<HopResponse, String>;
}

static GUARDED_HTTP: std::sync::OnceLock<Client> = std::sync::OnceLock::new();

/// Cliente compartilhado do pipeline SSRF-guardado: redirect automático DESLIGADO (ver o porquê no
/// cabeçalho do módulo) e cookie jar LIGADO — o Yahoo faz um bounce same-origin de verificação
/// (redirect pra `/_bv/v.gif?...`, seta um cookie, redireciona de volta) antes de servir HTML real;
/// confirmado com captura real (`curl -c/-b`) nesta rodada. Sem cookie jar esse bounce nunca
/// resolve. Compartilhado processo-a-processo de propósito: um jar por processo imita o
/// comportamento de UM navegador, não é estado por chamador.
pub(super) fn guarded_http_client() -> &'static Client {
    GUARDED_HTTP.get_or_init(|| guarded_client_builder().build().expect("cliente HTTP guardado"))
}

/// O cliente inteiro menos o `build()`: os testes o constroem com `.no_proxy()` (o proxy do CI
/// resolveria o destino fora deste processo) sem criar um segundo caminho de produção.
/// `PublicOnlyResolver` filtra o DNS NA CONEXÃO — nome público que aponta para IP privado, ou que
/// muda de resposta entre a validação e o connect (rebinding), nunca chega a abrir socket.
fn guarded_client_builder() -> reqwest::ClientBuilder {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .user_agent(USER_AGENT)
        .dns_resolver(super::net_guard::PublicOnlyResolver)
}

pub(super) struct ReqwestFetcher<'a> {
    pub(super) client: &'a Client,
}

#[async_trait]
impl HopFetcher for ReqwestFetcher<'_> {
    async fn fetch(&self, url: &str) -> Result<HopResponse, String> {
        let response = self.client.get(url).timeout(HOP_TIMEOUT).send().await.map_err(|error| error.to_string())?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            return Ok(HopResponse { status: status.as_u16(), location, body: Vec::new(), content_type: None });
        }
        let content_type =
            response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).map(str::to_string);
        let body = read_capped_body(response).await?;
        Ok(HopResponse { status: status.as_u16(), location: None, body, content_type })
    }
}

/// Acumula o corpo em streaming e TRUNCA em [`MAX_BODY_BYTES`] em vez de crescer sem limite — o
/// resto da resposta é descartado, nunca lido.
async fn read_capped_body(response: reqwest::Response) -> Result<Vec<u8>, String> {
    let mut stream = response.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        let remaining = MAX_BODY_BYTES.saturating_sub(buf.len());
        if remaining == 0 {
            break;
        }
        let take = remaining.min(chunk.len());
        buf.extend_from_slice(&chunk[..take]);
        if take < chunk.len() {
            break;
        }
    }
    Ok(buf)
}

/// As DUAS guardas, nesta ordem — respondem a perguntas diferentes: `validate_navigable_url`
/// pergunta se o ENDEREÇO é confiável (SSRF), `assert_network_allowed` pergunta se o USUÁRIO
/// permite sair. Roda pra CADA hop, não só pro primeiro.
/// Só os hosts da Wikipedia que `person_catalog::enrichment.rs` chama (`en.wikipedia.org` para o
/// resumo, `upload.wikimedia.org` para a thumbnail) — nunca um allowlist genérico.
#[cfg(any(test, feature = "e2e"))]
const E2E_WIKI_HOSTS: [&str; 2] = ["en.wikipedia.org", "upload.wikimedia.org"];

/// **Teste-only, dupla trava — nunca uma chave de `ConfigStore`.** Quando `MIKA_E2E=1` (o MESMO
/// gate de `e2e_hooks.rs`, já auditado como escape hatch de teste) E a env var
/// `MIKA_E2E_WIKI_BASE_URL` estão as DUAS setadas, uma URL cujo host é exatamente um dos
/// [`E2E_WIKI_HOSTS`] é reescrita para essa base local ANTES da validação SSRF — sem isto,
/// `memory-reaction.spec.ts` bateria na Wikipedia real toda vez que um E2E cataloga uma pessoa.
/// Existe porque `WIKI_SUMMARY_API`/o host de thumbnail em `enrichment.rs` são constantes fixas,
/// não configuráveis (de propósito: usuário não deve poder redirecionar a chamada "Wikipedia" pra
/// outro host via config). Path e query são preservados; só o esquema+host trocam.
#[cfg(any(test, feature = "e2e"))]
fn e2e_wiki_override(raw_url: &str) -> Option<String> {
    e2e_wiki_override_with(std::env::var("MIKA_E2E").ok().as_deref(), std::env::var("MIKA_E2E_WIKI_BASE_URL").ok().as_deref(), raw_url)
}

/// Núcleo puro de [`e2e_wiki_override`] — env lido pelo CHAMADOR, nunca aqui dentro, mesmo motivo
/// de `e2e_hooks.rs::e2e_env_says_enabled`: `cargo test` roda os testes de um binário no MESMO
/// processo, e mutar `std::env` de um teste vazaria pros outros rodando em paralelo.
#[cfg(any(test, feature = "e2e"))]
fn e2e_wiki_override_with(mika_e2e: Option<&str>, wiki_base: Option<&str>, raw_url: &str) -> Option<String> {
    if mika_e2e != Some("1") {
        return None;
    }
    let base = wiki_base.filter(|s| !s.trim().is_empty())?;
    let url = url::Url::parse(raw_url).ok()?;
    if !E2E_WIKI_HOSTS.contains(&url.host_str().unwrap_or_default()) {
        return None;
    }
    let base_url = url::Url::parse(base.trim_end_matches('/')).ok()?;
    let mut rewritten = base_url;
    rewritten.set_path(url.path());
    rewritten.set_query(url.query());
    Some(rewritten.to_string())
}

/// Binário distribuído (sem a feature `e2e`): o override nem existe — `MIKA_E2E`/`MIKA_E2E_WIKI_BASE_URL`
/// no ambiente do usuário não redirecionam a Wikipedia para um host local.
#[cfg(not(any(test, feature = "e2e")))]
fn e2e_wiki_override(_raw_url: &str) -> Option<String> {
    None
}

fn authorize(store: &ConfigStore, raw_url: &str) -> Result<String, GuardedFetchError> {
    // O override troca o URL ANTES da validação — o alvo reescrito (`MIKA_E2E_WIKI_BASE_URL`,
    // tipicamente `http://127.0.0.1:<porta>`) passa pela MESMA `validate_navigable_url` normal.
    // Loopback só é aceito aqui quando as DUAS env vars acima estão setadas — fora de teste, esta
    // função nunca troca a URL, e `validate_navigable_url` continua rejeitando loopback como
    // sempre (ver `rejects_loopback_and_private_ipv4_literals`).
    let target = e2e_wiki_override(raw_url);
    let effective_url = target.as_deref().unwrap_or(raw_url);
    let url = if target.is_some() {
        url::Url::parse(effective_url).map_err(|e| GuardedFetchError::Ssrf(e.to_string()))?
    } else {
        validate_navigable_url(effective_url).map_err(GuardedFetchError::Ssrf)?
    };
    privacy::assert_network_allowed(store, url.as_str(), "browser").map_err(GuardedFetchError::Offline)?;
    Ok(url.to_string())
}

fn resolve_redirect(base: &str, location: &str) -> Result<String, GuardedFetchError> {
    let base_url = url::Url::parse(base).map_err(|_| GuardedFetchError::Ssrf("redirect: url base invalida".to_string()))?;
    let joined = base_url.join(location).map_err(|_| GuardedFetchError::Ssrf("redirect: location invalida".to_string()))?;
    Ok(joined.to_string())
}

/// Segue redirects manualmente, revalidando CADA hop antes do próximo fetch — o mesmo espírito de
/// `forwardValidatedRequest` em `navigation-guard.ts`. Diferença deliberada: se ainda estiver
/// redirecionando depois de [`MAX_REDIRECT_HOPS`], isto é ERRO (`TooManyRedirects`). O TS original
/// `fulfill`a a última resposta 3xx sem revalidar quando estoura o teto — o browser a segue por
/// fora da guarda nesse caso raro. Mais seguro nunca despachar esse hop.
pub(super) async fn guarded_get<F: HopFetcher>(
    store: &ConfigStore,
    fetcher: &F,
    raw_url: &str,
) -> Result<GuardedResponse, GuardedFetchError> {
    let mut current = authorize(store, raw_url)?;
    let mut response = fetcher.fetch(&current).await.map_err(GuardedFetchError::Network)?;

    for _ in 0..MAX_REDIRECT_HOPS {
        if !(300..400).contains(&response.status) {
            break;
        }
        let Some(location) = response.location.clone() else { break };
        current = authorize(store, &resolve_redirect(&current, &location)?)?;
        response = fetcher.fetch(&current).await.map_err(GuardedFetchError::Network)?;
    }

    if (300..400).contains(&response.status) {
        return Err(GuardedFetchError::TooManyRedirects);
    }
    if !(200..300).contains(&response.status) {
        return Err(GuardedFetchError::BadStatus(response.status));
    }
    Ok(GuardedResponse { body: response.body, status: response.status, content_type: response.content_type })
}

#[cfg(test)]
mod tests;

