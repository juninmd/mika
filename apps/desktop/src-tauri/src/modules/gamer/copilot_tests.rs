use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use super::*;
use crate::modules::gamer::test_support::ScratchDir;

/// Conta as chamadas: "não interrompeu o jogo" tem de valer também para o custo do LLM.
struct FakeGenerator {
    reply: Result<String, String>,
    calls: AtomicUsize,
}

impl FakeGenerator {
    fn new(reply: Result<&str, &str>) -> Self {
        Self { reply: reply.map(str::to_string).map_err(str::to_string), calls: AtomicUsize::new(0) }
    }
}

#[async_trait]
impl CommentGenerator for FakeGenerator {
    async fn generate(&self, _prompt: &str) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.reply.clone()
    }
}

/// Store com o schema REAL (o default de `copilotEnabled` vem dele, não da fixture).
fn store(copilot: Option<bool>) -> (ConfigStore, ScratchDir) {
    let dir = ScratchDir::new("copilot");
    let store = ConfigStore::open(dir.path(), crate::config::schemas::registry()).unwrap();
    if let Some(on) = copilot {
        store.set("gamer-companion.copilotEnabled", serde_json::json!(on)).unwrap();
    }
    (store, dir)
}

fn alert(kind: &str, detail: &str) -> Value {
    serde_json::json!({ "game": "Elden Ring", "kind": kind, "detail": detail })
}

const T0: u64 = 5_000_000;

/// `decide`, não `maybe_alert`: o barramento global descarta para assinante atrasado sob a suíte
/// inteira, e um teste que depende dele é flaky. A forma do `SpeechRequest` tem teste próprio.
async fn fire(state: &CopilotState, store: &ConfigStore, tension: &GameTensionState, generator: &FakeGenerator, payload: Value, now: u64) -> bool {
    decide(state, store, tension, generator, &payload, now).await.is_some()
}

#[tokio::test]
async fn copilot_is_off_by_default_and_never_calls_the_llm() {
    let (store, _dir) = store(None);
    let generator = FakeGenerator::new(Ok("<|PANIC|> Inimigo atrás de você!"));
    let sent = fire(&CopilotState::default(), &store, &GameTensionState::new(), &generator, alert("danger", "enemy behind"), T0).await;
    assert!(!sent);
    assert_eq!(generator.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_feature_master_toggle_silences_the_copilot_too() {
    let (store, _dir) = store(Some(true));
    store.set("gamer-companion.enabled", serde_json::json!(false)).unwrap();
    let generator = FakeGenerator::new(Ok("<|PANIC|> Cuidado!"));
    assert!(!fire(&CopilotState::default(), &store, &GameTensionState::new(), &generator, alert("danger", "enemy behind"), T0).await);
}

#[tokio::test]
async fn a_danger_alert_is_spoken_as_a_voice_only_copilot_line() {
    let (store, _dir) = store(Some(true));
    let generator = FakeGenerator::new(Ok("<|PANIC|> Inimigo vindo pela esquerda!"));
    let line = decide(&CopilotState::default(), &store, &GameTensionState::new(), &generator, &alert("danger", "enemy left"), T0).await;
    assert_eq!(line.as_deref(), Some("<|PANIC|> Inimigo vindo pela esquerda!"));
    assert_eq!(speech_payload("x"), serde_json::json!({ "text": "x", "origin": "game-copilot" }));
}

#[tokio::test]
async fn unknown_kinds_and_empty_details_are_dropped_at_the_border() {
    let (store, _dir) = store(Some(true));
    let (state, tension) = (CopilotState::default(), GameTensionState::new());
    let generator = FakeGenerator::new(Ok("<|PANIC|> x"));
    assert!(!fire(&state, &store, &tension, &generator, alert("boss", "big guy"), T0).await);
    assert!(!fire(&state, &store, &tension, &generator, alert("danger", "  "), T0).await);
    assert_eq!(generator.calls.load(Ordering::SeqCst), 0);
}

/// "Sem atrapalhar": o mesmo aviso não volta por 3 min, mesmo depois do cooldown do tipo, e dois
/// avisos de tipos diferentes não saem colados.
#[tokio::test]
async fn the_same_alert_never_repeats_and_alerts_never_come_back_to_back() {
    let (store, _dir) = store(Some(true));
    let (state, tension) = (CopilotState::default(), GameTensionState::new());
    let generator = FakeGenerator::new(Ok("<|PANIC|> Cuidado!"));
    assert!(fire(&state, &store, &tension, &generator, alert("danger", "Archer on the  roof"), T0).await);
    assert!(!fire(&state, &store, &tension, &generator, alert("missed_item", "chest by the door"), T0 + 3_000).await, "piso entre avisos");
    assert!(!fire(&state, &store, &tension, &generator, alert("danger", "archer on the roof"), T0 + 60_000).await, "mesmo aviso");
    assert!(!fire(&state, &store, &tension, &generator, alert("danger", "archer on the right rooftop"), T0 + 30_000).await, "paráfrase dentro do cooldown");
    assert!(fire(&state, &store, &tension, &generator, alert("danger", "wolf pack ahead"), T0 + 60_000).await, "aviso novo depois do cooldown");
    assert!(fire(&state, &store, &tension, &generator, alert("danger", "archer on the roof"), T0 + 200_000).await, "passada a janela");
}

#[tokio::test]
async fn a_missed_item_waits_out_a_tense_moment_but_a_danger_does_not() {
    let (store, _dir) = store(Some(true));
    let tension = GameTensionState::new();
    tension.apply_moment("boss", true, Some(60_000), T0);
    let generator = FakeGenerator::new(Ok("<|SURPRISED|> Ficou um baú ali!"));
    assert!(!fire(&CopilotState::default(), &store, &tension, &generator, alert("missed_item", "chest"), T0 + 1_000).await);
    assert!(fire(&CopilotState::default(), &store, &tension, &generator, alert("danger", "boss charging"), T0 + 1_000).await);
    let mut same_tick = alert("missed_item", "chest");
    same_tick["tense"] = serde_json::json!(true);
    let calm = GameTensionState::new();
    assert!(!fire(&CopilotState::default(), &store, &calm, &generator, same_tick, T0).await, "tensão do próprio tick");
}

#[tokio::test]
async fn a_failed_generation_frees_the_slot_and_a_pass_keeps_it() {
    let (store, _dir) = store(Some(true));
    let tension = GameTensionState::new();
    let state = CopilotState::default();
    assert!(!fire(&state, &store, &tension, &FakeGenerator::new(Err("offline")), alert("danger", "trap"), T0).await);
    assert!(!fire(&state, &store, &tension, &FakeGenerator::new(Ok("[PASS]")), alert("danger", "trap"), T0 + 1).await);
    let generator = FakeGenerator::new(Ok("<|PANIC|> Armadilha!"));
    assert!(!fire(&state, &store, &tension, &generator, alert("danger", "trap"), T0 + 30_000).await, "o [PASS] reservou o aviso");
    assert_eq!(generator.calls.load(Ordering::SeqCst), 0);
}
