//! Host HTTP do E2E fora do Windows — só existe com `--features e2e` e `MIKA_E2E_HOST_PORT`.
//!
//! O E2E dirige o app por CDP, e CDP só existe no WebView2 (Windows). WebKitGTK (Linux) e
//! WKWebView (macOS) não o expõem, então o Playwright não consegue conectar. Em vez de trocar de
//! ferramenta, este módulo deixa o Playwright abrir um Chromium nas MESMAS páginas do app e liga
//! essas páginas ao backend Rust REAL por loopback:
//!
//! - `GET /<arquivo>` serve o bundle embarcado (`asset_resolver`), com o CSP do `tauri.conf.json`;
//! - `POST /__e2e/invoke` e `/__e2e/pcm` chamam `bridge::dispatch` — o corpo de `mika_invoke`/
//!   `mika_send` —, com o contexto derivado do label da janela, como em produção;
//! - `POST /__e2e/listen` assina o evento na janela NATIVA de mesmo label (semântica idêntica à do
//!   `listen` do Tauri, inclusive `emit_to`) e o entrega por `GET /__e2e/stream` (SSE);
//! - `GET /__e2e/windows` diz que janelas o backend criou, para o Playwright abrir uma página por
//!   janela. As janelas nativas continuam existindo, mas carregam `about:blank`: só a página do
//!   Chromium roda a UI, sem efeitos duplicados.
//!
//! **Superfície de ataque, por isso as travas:** isto abre o IPC inteiro por TCP. (1) o módulo não
//! é compilado sem a feature `e2e` (`lib.rs`); (2) só liga com `MIKA_E2E=1` + `MIKA_E2E_HOST_PORT`;
//! (3) escuta só em 127.0.0.1; (4) toda rota `/__e2e/*` exige o `x-mika-e2e-token` de
//! `MIKA_E2E_HOST_TOKEN`, que só o processo do Playwright conhece.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};

use bytes::Bytes;
use futures::stream::{self, StreamExt};
use http_body_util::{combinators::BoxBody, BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use tauri::{AppHandle, EventId, Listener, Manager, WebviewUrl};
use tokio::sync::mpsc;

type Body = BoxBody<Bytes, Infallible>;

struct Config {
    port: u16,
    token: String,
}

fn config() -> Option<&'static Config> {
    static CONFIG: OnceLock<Option<Config>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let on = std::env::var("MIKA_E2E").is_ok_and(|v| v == "1");
            let port = std::env::var("MIKA_E2E_HOST_PORT").ok()?.parse().ok()?;
            let token = std::env::var("MIKA_E2E_HOST_TOKEN").ok().filter(|t| !t.is_empty())?;
            on.then_some(Config { port, token })
        })
        .as_ref()
}

/// `true` quando o host está ligado: a UI roda no Chromium do Playwright, não nas janelas nativas.
pub fn active() -> bool {
    config().is_some()
}

fn pages() -> &'static Mutex<HashMap<String, String>> {
    static PAGES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    PAGES.get_or_init(Default::default)
}

/// URL que a janela `label` carregaria no app; a janela nativa recebe `about:blank` no lugar.
pub fn page(label: &str, path: &str) -> WebviewUrl {
    pages().lock().expect("registro de páginas").insert(label.to_string(), path.to_string());
    WebviewUrl::External("about:blank".parse().expect("url fixa"))
}

struct Stream {
    tx: mpsc::UnboundedSender<String>,
    listeners: Vec<(String, EventId)>,
}

struct Host {
    app: AppHandle,
    token: &'static str,
    streams: Mutex<HashMap<String, Stream>>,
}

/// Sobe o servidor. Falha alto (panic no boot de teste) em vez de deixar o Playwright esperando.
pub fn start(app: &AppHandle) {
    let Some(cfg) = config() else { return };
    let listener = std::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], cfg.port)))
        .unwrap_or_else(|e| panic!("e2e_host: não consegui escutar em 127.0.0.1:{}: {e}", cfg.port));
    listener.set_nonblocking(true).expect("e2e_host: socket não bloqueante");
    let host = Arc::new(Host { app: app.clone(), token: &cfg.token, streams: Default::default() });
    tauri::async_runtime::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).expect("e2e_host: listener tokio");
        loop {
            let Ok((socket, _)) = listener.accept().await else { continue };
            let host = Arc::clone(&host);
            tauri::async_runtime::spawn(async move {
                let service = service_fn(move |req| route(Arc::clone(&host), req));
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(socket), service).await;
            });
        }
    });
    log::info!("e2e_host: http://127.0.0.1:{}", cfg.port);
}

fn reply(status: StatusCode, content_type: &str, body: impl Into<Bytes>) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("content-type", content_type)
        .header("cache-control", "no-store")
        .body(Full::new(body.into()).boxed())
        .expect("resposta válida")
}

fn json_reply(value: Value) -> Response<Body> {
    reply(StatusCode::OK, "application/json", value.to_string())
}

fn query(req: &Request<Incoming>, key: &str) -> Option<String> {
    let q = req.uri().query()?;
    q.split('&').find_map(|pair| pair.strip_prefix(key)?.strip_prefix('=').map(str::to_string))
}

async fn route(host: Arc<Host>, req: Request<Incoming>) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path().to_string();
    if !path.starts_with("/__e2e/") {
        return Ok(asset(&host.app, &path));
    }
    // `<img>`/`fetch` do Live2D não carregam cabeçalho: o token vai na query, como no SSE.
    let authorized = req.headers().get("x-mika-e2e-token").and_then(|v| v.to_str().ok()) == Some(host.token)
        || query(&req, "token").as_deref() == Some(host.token);
    if !authorized {
        return Ok(reply(StatusCode::FORBIDDEN, "text/plain", "token E2E ausente ou errado"));
    }
    Ok(match (req.method().clone(), path.as_str()) {
        (Method::GET, "/__e2e/windows") => json_reply(windows(&host.app)),
        (Method::GET, "/__e2e/stream") => stream_response(&host, query(&req, "id").unwrap_or_default()),
        (Method::POST, "/__e2e/invoke") => json_reply(invoke(&host, body_json(req).await).await),
        (Method::POST, "/__e2e/listen") => json_reply(listen(&host, body_json(req).await)),
        (Method::POST, "/__e2e/unlisten") => json_reply(unlisten(&host, body_json(req).await)),
        (Method::GET, p) if p.starts_with("/__e2e/live2d/") => live2d(&host.app, &p["/__e2e/live2d".len()..]),
        (Method::POST, "/__e2e/close") => json_reply(close(&host, body_json(req).await)),
        (Method::POST, "/__e2e/pcm") => json_reply(pcm(&host, req).await),
        _ => reply(StatusCode::NOT_FOUND, "text/plain", "rota E2E desconhecida"),
    })
}

async fn body_bytes(req: Request<Incoming>) -> Bytes {
    req.into_body().collect().await.map(|b| b.to_bytes()).unwrap_or_default()
}

async fn body_json(req: Request<Incoming>) -> Value {
    serde_json::from_slice(&body_bytes(req).await).unwrap_or(Value::Null)
}

fn asset(app: &AppHandle, path: &str) -> Response<Body> {
    let key = if path == "/" { "/index.html" } else { path };
    match app.asset_resolver().get(key.trim_start_matches('/').to_string()) {
        Some(asset) => {
            let mut response = reply(StatusCode::OK, &asset.mime_type, asset.bytes);
            if let Some(csp) = asset.csp_header {
                response.headers_mut().insert("content-security-policy", csp.parse().expect("csp válido"));
            }
            response
        }
        None => reply(StatusCode::NOT_FOUND, "text/plain", format!("asset ausente: {key}")),
    }
}

/// Modelos Live2D do perfil: o Chromium do Playwright não conhece `mika-live2d:`, o shim reescreve.
fn live2d(app: &AppHandle, path: &str) -> Response<Body> {
    match crate::live2d_protocol::read_asset(app, path) {
        Some((content_type, bytes)) => reply(StatusCode::OK, content_type, bytes),
        None => reply(StatusCode::NOT_FOUND, "text/plain", "not found"),
    }
}

fn windows(app: &AppHandle) -> Value {
    let pages = pages().lock().expect("registro de páginas");
    let list: Vec<Value> = app
        .webview_windows()
        .into_iter()
        .map(|(label, window)| {
            let scale = window.scale_factor().unwrap_or(1.0);
            let size = window.inner_size().map(|s| s.to_logical::<f64>(scale)).ok();
            json!({
                "label": label,
                "path": pages.get(&label),
                "width": size.map(|s| s.width),
                "height": size.map(|s| s.height),
                "visible": window.is_visible().unwrap_or(false),
            })
        })
        .collect();
    Value::Array(list)
}

async fn invoke(host: &Host, body: Value) -> Value {
    let label = body["label"].as_str().unwrap_or_default();
    let channel = body["channel"].as_str().unwrap_or_default();
    let payload = body.get("payload").filter(|p| !p.is_null()).cloned();
    let ctx = crate::bridge::context_from_label(label);
    match crate::bridge::dispatch(&host.app, &ctx, channel, payload).await {
        Ok(value) => json!({ "ok": value }),
        Err(error) => json!({ "err": error }),
    }
}

/// Fechar como o usuário pelo X da janela. Serve às janelas que não têm `window-close` (view de
/// plugin: `is_owner_surface` a recusa), cujo `window.close()` o Chromium ignora por não tê-la aberto.
fn close(host: &Host, body: Value) -> Value {
    let label = body["label"].as_str().unwrap_or_default();
    match host.app.get_webview_window(label).map(|w| w.close()) {
        Some(Ok(())) => json!({ "ok": null }),
        Some(Err(error)) => json!({ "err": error.to_string() }),
        None => json!({ "err": format!("janela '{label}' não existe") }),
    }
}

async fn pcm(host: &Host, req: Request<Incoming>) -> Value {
    let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
    let (Some(label), Some(amplitude)) = (header("x-mika-label"), header("x-mika-amplitude").and_then(|v| v.parse::<f32>().ok())) else {
        return json!({ "err": "x-mika-label/x-mika-amplitude ausentes" });
    };
    let playing = header("x-mika-playing").map(|v| v == "1");
    let bytes = body_bytes(req).await;
    let ctx = crate::bridge::context_from_label(&label);
    match crate::modules::voice::vad_audio::handle_binary(&host.app, &ctx, &bytes, amplitude, playing).await {
        Ok(()) => json!({ "ok": null }),
        Err(error) => json!({ "err": error }),
    }
}

/// Quando o cliente fecha o SSE, o corpo é largado e esta guarda solta as assinaturas nativas.
struct StreamGuard {
    host: Arc<Host>,
    id: String,
    rx: mpsc::UnboundedReceiver<String>,
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        let Some(stream) = self.host.streams.lock().expect("streams").remove(&self.id) else { return };
        for (label, id) in stream.listeners {
            if let Some(window) = self.host.app.get_webview_window(&label) {
                window.unlisten(id);
            }
        }
    }
}

fn stream_response(host: &Arc<Host>, id: String) -> Response<Body> {
    let (tx, rx) = mpsc::unbounded_channel();
    host.streams.lock().expect("streams").insert(id.clone(), Stream { tx, listeners: Vec::new() });
    let guard = StreamGuard { host: Arc::clone(host), id, rx };
    let frames = stream::unfold(guard, |mut guard| async move {
        let line = guard.rx.recv().await?;
        Some((Ok::<_, Infallible>(Frame::data(Bytes::from(format!("data: {line}\n\n")))), guard))
    });
    let preamble = stream::once(async { Ok::<_, Infallible>(Frame::data(Bytes::from(": ok\n\n"))) });
    Response::builder()
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-store")
        .body(BodyExt::boxed(StreamBody::new(preamble.chain(frames))))
        .expect("resposta SSE")
}

fn listen(host: &Host, body: Value) -> Value {
    let (stream_id, label, event) = (body["stream"].as_str().unwrap_or_default(), body["label"].as_str().unwrap_or_default(), body["event"].as_str().unwrap_or_default().to_string());
    let Some(window) = host.app.get_webview_window(label) else { return json!({ "err": format!("janela '{label}' não existe") }) };
    let mut streams = host.streams.lock().expect("streams");
    let Some(stream) = streams.get_mut(stream_id) else { return json!({ "err": "stream E2E não aberto" }) };
    let tx = stream.tx.clone();
    let name = event.clone();
    let handler = body["handler"].clone();
    let id = window.listen(event, move |ev| {
        let payload: Value = serde_json::from_str(ev.payload()).unwrap_or(Value::Null);
        let _ = tx.send(json!({ "event": name, "handler": handler, "eventId": ev.id(), "payload": payload }).to_string());
    });
    stream.listeners.push((label.to_string(), id));
    json!({ "ok": id })
}

fn unlisten(host: &Host, body: Value) -> Value {
    let (stream_id, event_id) = (body["stream"].as_str().unwrap_or_default(), body["eventId"].as_u64().unwrap_or_default() as EventId);
    let mut streams = host.streams.lock().expect("streams");
    if let Some(stream) = streams.get_mut(stream_id) {
        if let Some(at) = stream.listeners.iter().position(|(_, id)| *id == event_id) {
            let (label, id) = stream.listeners.remove(at);
            if let Some(window) = host.app.get_webview_window(&label) {
                window.unlisten(id);
            }
        }
    }
    json!({ "ok": null })
}
