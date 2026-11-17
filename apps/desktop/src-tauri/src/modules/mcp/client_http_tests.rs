//! Servidor MCP remoto (`"url"` no `mcp.json`): um `StreamableHttpService` de verdade em loopback,
//! atrás de um porteiro que exige `Authorization` — prova handshake, descoberta e o cabeçalho
//! chegando ao servidor sem rede externa.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tower_service::Service;

use super::tests::FakeMcpServer;
use super::*;

const TOKEN: &str = "Bearer t0k";

/// Aceita conexões até o teste acabar (o runtime do `#[tokio::test]` derruba a task junto).
async fn serve(listener: tokio::net::TcpListener, service: StreamableHttpService<FakeMcpServer, LocalSessionManager>) {
    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let service = service.clone();
        let gate = hyper::service::service_fn(move |request: hyper::Request<hyper::body::Incoming>| {
            let mut service = service.clone();
            async move {
                if request.headers().get("authorization").and_then(|v| v.to_str().ok()) != Some(TOKEN) {
                    let body = Full::new(Bytes::from_static(b"unauthorized")).map_err(|never: Infallible| match never {}).boxed();
                    return Ok::<_, Infallible>(hyper::Response::builder().status(401).body(body).unwrap());
                }
                service.call(request).await
            }
        });
        tokio::spawn(hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), gate));
    }
}

/// Sobe o servidor e devolve a URL do endpoint `/mcp`. Sem o `Authorization` esperado: 401.
async fn spawn_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = StreamableHttpService::new(|| Ok(FakeMcpServer), Arc::new(LocalSessionManager::default()), StreamableHttpServerConfig::default());
    tokio::spawn(serve(listener, service));
    format!("http://{addr}/mcp")
}

fn remote(url: &str, token: Option<&str>) -> ServerSpec {
    let headers: BTreeMap<String, String> = token.map(|t| ("Authorization".to_string(), t.to_string())).into_iter().collect();
    ServerSpec { url: Some(url.to_string()), headers, ..Default::default() }
}

#[tokio::test]
async fn a_remote_server_connects_over_http_with_its_auth_header_and_exposes_prefixed_tools() {
    let url = spawn_server().await;

    let connected = connect_one("web", &remote(&url, Some(TOKEN)), Duration::from_secs(10)).await.expect("remote handshake");

    assert_eq!(connected.tools.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(), ["mcp_web_echo"]);
    close_all(vec![connected]).await;
}

#[tokio::test]
async fn a_remote_server_without_the_header_it_requires_fails_instead_of_hanging() {
    let url = spawn_server().await;

    let error = connect_one("web", &remote(&url, None), Duration::from_secs(10)).await.unwrap_err();

    assert!(!error.contains("no answer"), "should fail on the 401, not time out: {error}");
}

#[tokio::test]
async fn only_http_urls_and_valid_headers_reach_the_network() {
    for url in ["file:///etc/passwd", "ftp://host/mcp", "not a url"] {
        assert!(connect_one("web", &remote(url, None), Duration::from_secs(1)).await.is_err(), "{url}");
    }
    let mut spec = remote("http://127.0.0.1:1/mcp", None);
    spec.headers.insert("X-Bad".into(), "a\r\nInjected: 1".into());
    let error = connect_one("web", &spec, Duration::from_secs(1)).await.unwrap_err();
    assert!(error.contains("X-Bad") && !error.contains("Injected"), "{error}");
}
