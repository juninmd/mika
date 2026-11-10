//! Infra HTTP compartilhada pelos 4 provedores remotos de TTS.
//!
//! Três responsabilidades que os quatro arquivos-irmãos (`elevenlabs.rs`/`openai.rs`/`azure.rs`/
//! `google.rs`) NÃO devem duplicar cada um a sua maneira:
//!
//! 1. [`guard`] — o chokepoint de privacidade (`assert_network_allowed`) ANTES de qualquer
//!    requisição, mesmo ponto único que `providers/supertonic/assets.rs` já usa para download de
//!    modelo. Modo offline bloqueia os 4 provedores aqui, uma vez só.
//! 2. [`race_cancel`] — corrida entre a requisição e o `CancelHandle` do chamador (`speaker.rs`),
//!    porque um provedor remoto (ao contrário do Supertonic em `spawn_blocking`) PODE ser abortado
//!    no meio: a requisição HTTP é só um future, então correr contra um `sleep` curto e checar o
//!    cancelamento a cada rodada aborta sem esperar o corpo inteiro chegar.
//! 3. [`redact`] — nenhum segredo pode sobreviver numa mensagem de erro que vira texto de tela. A
//!    chave nunca aparece na URL dos 3 primeiros provedores (vai em header), mas o Google manda a
//!    dela via query string (`?key=...`), e o `Display` de `reqwest::Error` embute a URL da
//!    requisição — inclusive a query. `modules/settings/test_redaction.rs::strip_url_queries` já
//!    resolve exatamente isto, mas é `pub(super)` do módulo `settings`, inalcançável daqui;
//!    reimplementado aqui porque o pedido desta tarefa preferiu isso a alargar aquela visibilidade.

use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

use crate::config::store::ConfigStore;
use crate::modules::privacy::assert_network_allowed;
use crate::modules::voice::tts::abort_registry::CancelHandle;

/// Pool HTTP único dos 4 provedores: TTS fala frase a frase, e um `Client` novo por frase pagava
/// TCP + handshake TLS de novo a cada uma — latência somada a todo turno falado. O timeout continua
/// por-provedor (`elevenLabsTimeoutMs` etc.) porque vai na REQUISIÇÃO (`RequestBuilder::timeout`),
/// não no pool.
static POOL: OnceLock<reqwest::Client> = OnceLock::new();

/// O pool com o prazo de UM provedor; `post` aplica o prazo a cada requisição que sai dele.
pub(super) struct TimedClient {
    timeout: Duration,
}

impl TimedClient {
    pub(super) fn post(&self, url: &str) -> reqwest::RequestBuilder {
        pool().post(url).timeout(self.timeout)
    }
}

fn pool() -> &'static reqwest::Client {
    POOL.get_or_init(|| {
        reqwest::Client::builder().connect_timeout(Duration::from_secs(10)).build().expect("cliente http de tts")
    })
}

pub(super) fn request_client(timeout_ms: u32) -> TimedClient {
    TimedClient { timeout: Duration::from_millis(timeout_ms.max(1) as u64) }
}

/// Ver item 1 do cabeçalho. `provider_tag` é só o rótulo logado (nunca a URL/query) — mesma regra
/// de `assert_network_allowed`.
pub(super) fn guard(store: &ConfigStore, url: &str, provider_tag: &str) -> Result<(), String> {
    assert_network_allowed(store, url, provider_tag)
}

/// Ver item 2 do cabeçalho. `None` quando `cancel` disparou antes de `fut` terminar. A checagem a
/// cada 100ms é fina o bastante para o barge-in parecer instantâneo sem apertar CPU num loop.
pub(super) async fn race_cancel<T>(fut: impl Future<Output = T>, cancel: &CancelHandle) -> Option<T> {
    tokio::pin!(fut);
    loop {
        tokio::select! {
            biased;
            result = &mut fut => return Some(result),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if cancel.is_cancelled() {
                    return None;
                }
            }
        }
    }
}

/// Ver item 3 do cabeçalho. Mesmo algoritmo de `strip_url_queries`: acha `http(s)://`, corta no
/// primeiro `?` daquela URL, preserva o resto da mensagem intacto.
pub(super) fn redact(message: impl Into<String>) -> String {
    let message = message.into();
    let mut out = String::with_capacity(message.len());
    let mut rest = message.as_str();

    while let Some(start) = [rest.find("http://"), rest.find("https://")].into_iter().flatten().min() {
        let (before, from_url) = rest.split_at(start);
        out.push_str(before);

        let url_end = from_url.find([' ', '\t', '\n', '"', '\'', ')', '>']).unwrap_or(from_url.len());
        let (url, tail) = from_url.split_at(url_end);
        match url.find('?') {
            Some(query) => {
                out.push_str(&url[..query]);
                out.push_str("?[REDACTED]");
            }
            None => out.push_str(url),
        }
        rest = tail;
    }

    out.push_str(rest);
    out
}

/// Servidor HTTP mínimo para os testes dos 4 provedores — sem crate de mock, sem rede real: só
/// loopback (`127.0.0.1:0`, porta dada pelo SO). Cada instância atende UMA requisição, o bastante
/// para os testes de `generate_at`, que fazem uma chamada por caso.
#[cfg(test)]
pub(super) mod test_support {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    pub(in crate::modules::voice::tts::providers::remote) struct MockServer {
        pub addr: std::net::SocketAddr,
        pub received: Arc<Mutex<Option<String>>>,
    }

    fn reason_phrase(status: u16) -> &'static str {
        match status {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            429 => "Too Many Requests",
            _ => "Error",
        }
    }

    /// Sobe o servidor, devolve o endereço já escutando e um handle para ler a requisição recebida
    /// (método, path, headers — usado para provar que o provedor manda o que o TS manda).
    pub(in crate::modules::voice::tts::providers::remote) fn spawn(
        status: u16,
        content_type: &'static str,
        body: Vec<u8>,
    ) -> MockServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind do mock em loopback");
        let addr = listener.local_addr().expect("endereço do mock");
        let received = Arc::new(Mutex::new(None));
        let captured = Arc::clone(&received);

        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let mut buf = [0u8; 16 * 1024];
            let n = stream.read(&mut buf).unwrap_or(0);
            *captured.lock().expect("lock do mock") = Some(String::from_utf8_lossy(&buf[..n]).to_string());

            let header = format!(
                "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                reason_phrase(status),
                body.len(),
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        });

        MockServer { addr, received }
    }

    impl MockServer {
        pub fn base_url(&self) -> String {
            format!("http://{}", self.addr)
        }

        pub fn request_text(&self) -> String {
            for _ in 0..50 {
                if let Some(text) = self.received.lock().expect("lock do mock").clone() {
                    return text;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_cuts_the_query_string_of_every_url_in_the_message() {
        let masked = redact("falhou: https://texttospeech.googleapis.com/v1/voices?key=segredo-123 (timeout)");
        assert!(!masked.contains("segredo-123"), "chave vazou: {masked}");
        assert!(masked.contains("https://texttospeech.googleapis.com/v1/voices"));
        assert!(masked.ends_with(" (timeout)"));
    }

    /// `find("http://").or_else(find("https://"))` olhava o `https://` só se NÃO houvesse `http://` em
    /// lugar nenhum: com um `http://` mais adiante na mensagem, a query (chave) do `https://` anterior passava.
    #[test]
    fn redact_cuts_an_https_query_that_comes_before_a_later_http_url() {
        let masked = redact("falhou https://api.exemplo.com/v1/x?key=segredo-123 via proxy http://127.0.0.1:8080");
        assert!(!masked.contains("segredo-123"), "chave vazou: {masked}");
        assert!(masked.contains("http://127.0.0.1:8080"));
    }

    #[test]
    fn redact_leaves_a_message_without_a_url_untouched() {
        assert_eq!(redact("erro qualquer"), "erro qualquer");
    }

    #[tokio::test]
    async fn race_cancel_returns_the_future_result_when_it_finishes_first() {
        let fut = async { 42 };
        let registry = crate::modules::voice::tts::abort_registry::AbortRegistry::new();
        let guard = registry.register_active();
        assert_eq!(race_cancel(fut, &guard.handle()).await, Some(42));
    }

    #[tokio::test]
    async fn race_cancel_returns_none_once_the_handle_is_cancelled_mid_flight() {
        let registry = crate::modules::voice::tts::abort_registry::AbortRegistry::new();
        let guard = registry.register_active();
        let handle = guard.handle();

        let cancel_soon = handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            cancel_soon.cancel();
        });

        let never_finishes = std::future::pending::<()>();
        assert_eq!(race_cancel(never_finishes, &handle).await, None);
    }

    #[test]
    fn pooled_client_still_applies_each_providers_own_timeout_per_request() {
        let eleven = request_client(1_500).post("https://api.elevenlabs.io/v1/x").build().unwrap();
        let azure = request_client(9_000).post("https://x.tts.speech.microsoft.com/y").build().unwrap();
        assert_eq!(eleven.timeout(), Some(&Duration::from_millis(1_500)));
        assert_eq!(azure.timeout(), Some(&Duration::from_millis(9_000)));
    }
}
