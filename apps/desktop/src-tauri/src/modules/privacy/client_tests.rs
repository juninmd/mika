//! Contrato do cliente HTTP compartilhado dos endpoints fixos (`fixed_endpoint_client`): servidor
//! mudo não prende a chamada para sempre e redirect em laço para depois de 3 saltos. Antes, esses
//! chamadores usavam `Client::new()` — sem timeout nenhum e com até 10 redirects.
//!
//! `#![cfg(test)]` interno pelo mesmo motivo de `tests.rs` (`state.rs::single_config_store_per_process`).
#![cfg(test)]

use super::build_fixed_client;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn spawn_server(respond: bool, hits: Arc<AtomicUsize>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            hits.fetch_add(1, Ordering::SeqCst);
            if !respond {
                std::thread::sleep(Duration::from_secs(20));
                continue;
            }
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    format!("http://{addr}/")
}

#[tokio::test]
async fn silent_server_times_out_instead_of_hanging() {
    let url = spawn_server(false, Arc::default());
    let client = build_fixed_client(Duration::from_secs(2), Duration::from_millis(300));
    let started = Instant::now();
    let error = client.get(url).send().await.unwrap_err();
    assert!(error.is_timeout(), "esperava timeout, veio: {error}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn redirect_loop_stops_after_three_hops() {
    let hits = Arc::new(AtomicUsize::new(0));
    let url = spawn_server(true, hits.clone());
    let client = build_fixed_client(Duration::from_secs(2), Duration::from_secs(2));
    let error = client.get(url).send().await.unwrap_err();
    assert!(error.is_redirect(), "esperava redirect excedido, veio: {error}");
    assert_eq!(hits.load(Ordering::SeqCst), 4, "1 requisição + 3 saltos");
}
