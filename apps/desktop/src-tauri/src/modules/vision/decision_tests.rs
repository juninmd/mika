use super::*;

/// `/v1/systemone` answers in the shape llama-server and the reference model return.
fn answers(game: f64, moment: (&str, f64), danger: f64, notable: f64) -> Value {
    json!({
        "game": {"type": "noul", "noul": game},
        "moment": {"type": "choice", "choice": moment.0, "confidence": moment.1},
        "danger": {"type": "noul", "noul": danger},
        "notable": {"type": "noul", "noul": notable},
    })
}

/// Real d1-2B readings from the smoke run (Xonotic with a rocket incoming, a VS Code window the
/// model labelled "victory", a calm 0 A.D. map): only the in-game threat is urgent.
#[test]
fn moment_and_danger_only_count_inside_a_game() {
    let rocket = verdict_from(&answers(1.0, ("none", 0.44), 0.87, 0.59), 0.5).unwrap();
    assert_eq!(rocket, Verdict { urgent: true, notable: true });

    let editor = verdict_from(&answers(0.03, ("victory", 0.52), 0.01, 0.06), 0.5).unwrap();
    assert_eq!(editor, Verdict { urgent: false, notable: false });

    let calm_map = verdict_from(&answers(0.96, ("none", 0.62), 0.15, 0.12), 0.5).unwrap();
    assert_eq!(calm_map, Verdict { urgent: false, notable: false });
}

#[test]
fn a_confident_game_moment_is_urgent() {
    let boss = verdict_from(&answers(0.9, ("boss", 0.8), 0.1, 0.2), 0.5).unwrap();
    assert!(boss.urgent && boss.notable);
}

/// A server that drops a question must not read as "nothing happening" and silence the watcher.
#[test]
fn a_missing_answer_is_no_verdict() {
    let mut partial = answers(0.9, ("none", 0.9), 0.1, 0.1);
    partial.as_object_mut().unwrap().remove("danger");
    assert_eq!(verdict_from(&partial, 0.5), None);
    assert_eq!(verdict_from(&Value::Null, 0.5), None);
}

#[test]
fn gate_keeps_the_plain_cadence_without_a_verdict() {
    let urgent = Some(Verdict { urgent: true, notable: true });
    let notable = Some(Verdict { urgent: false, notable: true });
    let boring = Some(Verdict { urgent: false, notable: false });
    assert_eq!(gate(urgent, false), Gate::Analyze, "urgent skips the cadence");
    assert_eq!(gate(notable, false), Gate::NotDue, "notable waits for the cadence, frame stays unseen");
    assert_eq!(gate(notable, true), Gate::Analyze);
    assert_eq!(gate(boring, true), Gate::Boring, "a boring frame never pays for the LLM");
    assert_eq!(gate(None, true), Gate::Analyze, "decision server down: old behaviour");
    assert_eq!(gate(None, false), Gate::NotDue);
}

#[test]
fn raw_base64_becomes_a_jpeg_data_url() {
    assert_eq!(as_data_url("QUJD"), "data:image/jpeg;base64,QUJD");
    assert_eq!(as_data_url("data:image/png;base64,QUJD"), "data:image/png;base64,QUJD");
}

struct TempDir(std::path::PathBuf);

/// One-shot loopback HTTP server: answers the first request with `body` and hands back the
/// request body it received.
fn serve_once(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = v.trim().parse().unwrap();
            }
        }
        let mut request = vec![0; length];
        reader.read_exact(&mut request).unwrap();
        let mut stream = stream;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, handle)
}

fn store_for(url: &str, tag: &str) -> (ConfigStore, TempDir) {
    let dir = TempDir(std::env::temp_dir().join(format!("mika-decision-{tag}-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    store.set("vision.decisionBaseUrl", json!(url)).unwrap();
    (store, dir)
}

/// The real HTTP path: every frame goes out as a data URL with the four questions, and the
/// server's answers come back as a verdict.
#[tokio::test(flavor = "multi_thread")]
async fn decide_sends_the_frames_and_reads_the_answers() {
    let (url, server) = serve_once(r#"{"answers":{"game":{"type":"noul","noul":1.0},"moment":{"type":"choice","choice":"none","confidence":0.4},"danger":{"type":"noul","noul":0.9},"notable":{"type":"noul","noul":0.6}},"usage":{"input_tokens":9,"output_tokens":0}}"#);
    let (store, _dir) = store_for(&url, "ok");
    let verdict = decide(&store, &["QUJD".to_string(), "data:image/png;base64,REVG".to_string()]).await;
    assert_eq!(verdict, Some(Verdict { urgent: true, notable: true }));
    let request: Value = serde_json::from_str(&server.join().unwrap()).unwrap();
    assert_eq!(request["images"], json!(["data:image/jpeg;base64,QUJD", "data:image/png;base64,REVG"]));
    assert_eq!(request["state"], Value::Null);
    assert_eq!(request["questions"], questions());
}

/// A server that answers without `answers` (wrong model, old llama-server) is no verdict and is
/// recorded in the provider-health banner, never read as "nothing happening".
#[tokio::test(flavor = "multi_thread")]
async fn a_response_without_answers_is_no_verdict_and_a_recorded_failure() {
    let (url, server) = serve_once(r#"{"usage":{"input_tokens":9,"output_tokens":0}}"#);
    let (store, _dir) = store_for(&url, "missing");
    assert_eq!(decide(&store, &["QUJD".to_string()]).await, None);
    server.join().unwrap();
    let entry = provider_health::get(PROVIDER, &url).expect("failure recorded");
    assert_eq!(entry.to_json()["failureCount"], json!(1));
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Privacy: the screen frame only goes to a decision server on this machine. A LAN or internet
/// address yields no verdict (plain cadence) and never reaches the network.
#[tokio::test]
async fn a_non_loopback_server_never_receives_the_frame() {
    let dir = TempDir(std::env::temp_dir().join(format!("mika-decision-remote-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    store.set("vision.decisionBaseUrl", json!("http://192.0.2.10:8080")).unwrap();
    assert_eq!(decide(&store, &["QUJD".to_string()]).await, None);
    assert!(provider_health::get(PROVIDER, "http://192.0.2.10:8080").is_none(), "no request, no failure recorded");
}

/// Real decision server, real screenshots: `urgent-*` frames must come back urgent and every other
/// frame must not. Prints one JSON row per frame (verdict + latency).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a /v1/systemone server in MIKA_DECISION_SMOKE_URL and frames in MIKA_DECISION_SMOKE_DIR"]
async fn decision_smoke_report() {
    use base64::Engine;
    let url = std::env::var("MIKA_DECISION_SMOKE_URL").expect("MIKA_DECISION_SMOKE_URL");
    let frames = std::path::PathBuf::from(std::env::var("MIKA_DECISION_SMOKE_DIR").expect("MIKA_DECISION_SMOKE_DIR"));
    let dir = TempDir(std::env::temp_dir().join(format!("mika-decision-smoke-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
    store.set("vision.decisionBaseUrl", json!(url)).unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(&frames).unwrap().map(|e| e.unwrap().path()).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let mime = if name.ends_with(".png") { "image/png" } else { "image/jpeg" };
        let image = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(std::fs::read(&path).unwrap()));
        let started = std::time::Instant::now();
        let verdict = decide(&store, &[image]).await.unwrap_or_else(|| panic!("{name}: no verdict"));
        println!("{}", json!({ "frame": name, "urgent": verdict.urgent, "notable": verdict.notable, "ms": started.elapsed().as_millis() as u64 }));
        assert_eq!(verdict.urgent, name.starts_with("urgent-"), "{name}");
    }
}
