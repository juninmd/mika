use super::*;

struct FakeGenerator(Result<String, String>);
#[async_trait]
impl CommentGenerator for FakeGenerator {
    async fn generate(&self, _prompt: &str) -> Result<String, String> {
        self.0.clone()
    }
}

fn store() -> ConfigStore {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mika-gamer-moment-commentary-test-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
    store.set("gamer-companion.backseatGamingEnabled", serde_json::json!(true)).unwrap();
    store
}

/// Approves every line: these tests are about the cooldown gates, and the process-wide repetition
/// history would otherwise make each "comentário" depend on which test ran before.
fn state() -> CommentaryState {
    CommentaryState::with_approval(Box::new(|_| true))
}

fn payload(game: &str, moment_type: &str) -> Value {
    serde_json::json!({ "game": game, "type": moment_type, "description": "descrição do momento" })
}

#[test]
fn is_competitive_game_matches_the_regex_hint() {
    assert!(is_competitive_game("Valorant"));
    assert!(is_competitive_game("Counter-Strike 2"));
    assert!(!is_competitive_game("Stardew Valley"));
}

#[tokio::test]
async fn backseat_disabled_by_default_blocks_every_comment() {
    let store = ConfigStore::open(
        &std::env::temp_dir().join(format!("mika-gamer-moment-commentary-default-{}", std::process::id())),
        crate::config::schemas::registry(),
    )
    .unwrap();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Ok("comentário".into()));

    let sent = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000).await;
    assert!(!sent, "backseatGamingEnabled default false deveria bloquear");
}

/// `gamer-companion.enabled=false` (toggle mestre) tem de bloquear o comentário mesmo com
/// `backseatGamingEnabled=true` — antes desta rodada só o sub-toggle de backseat era lido.
#[tokio::test]
async fn master_toggle_off_blocks_every_comment_even_with_backseat_gaming_enabled() {
    let store = store();
    store.set("gamer-companion.enabled", serde_json::json!(false)).unwrap();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Ok("comentário".into()));

    let sent = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000).await;
    assert!(!sent, "gamer-companion.enabled=false deveria bloquear mesmo com backseatGamingEnabled=true");
}

#[tokio::test]
async fn a_tense_moment_blocks_the_comment() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    tension.apply_moment("boss", false, None, 1_000_000);
    let generator = FakeGenerator(Ok("comentário".into()));

    let sent = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000).await;
    assert!(!sent, "momento tenso deveria bloquear o comentário");
}

#[tokio::test]
async fn a_competitive_game_only_comments_on_victory_or_defeat() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Ok("comentário".into()));

    let blocked = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Valorant", "death"), 1_000_000).await;
    assert!(!blocked, "morte num jogo competitivo não deveria comentar");

    let allowed = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Valorant", "victory"), 1_000_000).await;
    assert!(allowed, "vitória num jogo competitivo deveria passar o gate de tipo");
}

#[tokio::test]
async fn the_hard_floor_blocks_a_second_comment_too_soon_regardless_of_type() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Ok("comentário".into()));

    assert!(maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000).await);
    // Tipo diferente, mas dentro do piso de 15s desde o último comentário (qualquer tipo) —
    // ainda bloqueado.
    assert!(!maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "death"), 1_000_000 + 10_000).await);
}

#[tokio::test]
async fn the_per_type_cooldown_blocks_the_same_type_repeating() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Ok("comentário".into()));

    assert!(maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000).await);
    assert!(!maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000 + 20_000).await, "cooldown de victory é 60s");
    assert!(maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000 + 61_000).await);
}

#[tokio::test]
async fn a_failed_generation_rolls_back_the_reservation() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Err("provider indisponível".into()));

    let sent = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000).await;
    assert!(!sent);

    // Reserva desfeita: o mesmo tipo pode tentar de novo imediatamente (sem esperar 60s).
    let generator_ok = FakeGenerator(Ok("comentário".into()));
    let retried = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator_ok, &payload("Hades", "victory"), 1_000_010).await;
    assert!(retried, "reserva deveria ter sido desfeita após falha de geração");
}

#[tokio::test]
async fn an_empty_generated_comment_never_publishes_speech_request() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Ok("   ".into()));

    let sent = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000).await;
    assert!(!sent);
}

#[tokio::test]
async fn an_uncommented_moment_type_is_silently_skipped() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Ok("comentário".into()));

    let sent = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "boss"), 1_000_000).await;
    assert!(!sent);
}

#[tokio::test]
async fn publishes_speech_request_on_the_global_bus_for_an_allowed_comment() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    let generator = FakeGenerator(Ok("Boa, você venceu!".into()));

    let mut sub = crate::events::bus().subscribe();
    let sent = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "victory"), 1_000_000).await;
    assert!(sent);

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match tokio::time::timeout_at(deadline, sub.recv()).await {
            Ok(Some(envelope)) if envelope.key == crate::events::EventKey::SpeechRequest => {
                if envelope.payload["text"] == serde_json::json!("Boa, você venceu!") {
                    return;
                }
            }
            Ok(Some(_)) => continue,
            _ => panic!("SpeechRequest não publicado dentro do prazo"),
        }
    }
}

/// Backseat comments skipped the repetition guard every other unprompted line goes through: the
/// same death line came back on every death (2026-09-27, "ela repete muitas coisas").
#[tokio::test]
async fn a_comment_the_repetition_guard_already_heard_is_not_spoken_again() {
    use crate::modules::brain::guards::RepetitionGuard;
    let guard = Mutex::new(RepetitionGuard::with_embed(0.75, 20, |_| None));
    let state = CommentaryState::with_approval(Box::new(move |line| guard.lock().unwrap().approve_proactive(line)));
    let store = store();
    let (tension, gaming) = (GameTensionState::new(), GamingState::new());
    let generator = FakeGenerator(Ok("Morreu pro boss de novo, hein? Respira e tenta outra vez.".into()));

    assert!(maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "death"), 1_000_000).await);
    let later = 1_000_000 + 120_000; // past the hard floor and the 45 s death cooldown
    assert!(!maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "death"), later).await);
    // Held back is a choice, not a failure: the slot stays taken, so it waits instead of retrying.
    assert!(!maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &FakeGenerator(Ok("Outra coisa totalmente nova".into())), &payload("Hades", "death"), later + 1_000).await);
}

// -- aterramento em visão (smoke-baseline.md item 8/2) ------------------------------------------

fn vision_payload(game: &str, moment_type: &str) -> Value {
    serde_json::json!({ "game": game, "type": moment_type, "description": "descrição do momento", "source": "vision" })
}

#[tokio::test]
async fn a_vision_sourced_moment_is_silent_without_a_recent_screen_confirmation() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new(); // nunca recebeu record_vision_signal
    let generator = FakeGenerator(Ok("comentário".into()));

    let sent =
        maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &vision_payload("Stray", "victory"), 1_000_000).await;
    assert!(!sent, "momento marcado source=vision sem confirmação de tela nunca deveria comentar");
}

#[tokio::test]
async fn a_vision_sourced_moment_speaks_once_the_screen_confirms_a_game() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new();
    gaming.record_vision_signal(true, 999_000);
    let generator = FakeGenerator(Ok("comentário".into()));

    let sent =
        maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &vision_payload("Hades", "victory"), 1_000_000).await;
    assert!(sent, "com a tela confirmando um jogo recentemente, o comentário deveria sair");
}

/// Um minigame local (`gamer::rpc::MiniGameResult`, `source: "minigame"`) NUNCA deveria precisar
/// de confirmação de tela — é a própria Mika jogando contra o usuário.
#[tokio::test]
async fn a_minigame_sourced_moment_never_needs_a_screen_confirmation() {
    let store = store();
    let state = state();
    let tension = GameTensionState::new();
    let gaming = GamingState::new(); // sem nenhum sinal de visão
    let generator = FakeGenerator(Ok("comentário".into()));

    let payload = serde_json::json!({
        "game": "Pedra, Papel ou Tesoura", "type": "victory", "description": "Você venceu.", "source": "minigame",
    });
    let sent = maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload, 1_000_000).await;
    assert!(sent, "minigame local não deveria ser bloqueado pelo aterramento de visão");
}

struct RecordingGenerator {
    reply: String,
    prompts: Mutex<Vec<String>>,
}
#[async_trait]
impl CommentGenerator for RecordingGenerator {
    async fn generate(&self, prompt: &str) -> Result<String, String> {
        self.prompts.lock().unwrap().push(prompt.to_string());
        Ok(self.reply.clone())
    }
}

/// Backseat comments ran blind: no recent lines, no way to pass, so the same death line came back
/// reworded (2026-09-27). Same novelty contract as the awareness turn.
#[tokio::test]
async fn the_comment_sees_what_she_said_lately_and_a_pass_is_never_spoken() {
    static APPROVED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let state = CommentaryState::with_history(
        Box::new(|_| {
            APPROVED.store(true, std::sync::atomic::Ordering::SeqCst);
            true
        }),
        Box::new(|| vec!["Morreu pro boss de novo, hein?".to_string()]),
    );
    let store = store();
    let (tension, gaming) = (GameTensionState::new(), GamingState::new());
    let generator = RecordingGenerator { reply: "<|NEUTRAL|> [PASS]".into(), prompts: Mutex::new(Vec::new()) };

    assert!(!maybe_comment_on_game_moment(&state, &store, &tension, &gaming, &generator, &payload("Hades", "death"), 1_000_000).await);
    let prompt = generator.prompts.lock().unwrap()[0].clone();
    assert!(prompt.contains("Morreu pro boss de novo, hein?"), "{prompt}");
    assert!(prompt.contains(crate::modules::chat::proactive_novelty::PASS_TOKEN));
    assert!(!APPROVED.load(std::sync::atomic::Ordering::SeqCst), "a pass is not a line: nothing to record");
}

/// "Game events: win → cheer, death → facepalm": every commented moment strikes a body pose, and
/// the pose is one the `avatar` tool vocabulary (and so both render backends) actually knows.
#[test]
fn every_commented_moment_strikes_a_pose_the_avatar_knows() {
    use crate::modules::features::character::model_inventory::EXPRESSIONS;
    for moment in COMMENTED_TYPES {
        let pose = moment_pose(moment).unwrap_or_else(|| panic!("{moment} sem pose"));
        assert!(EXPRESSIONS.contains(&pose), "{moment} → {pose}, fora de EXPRESSIONS");
    }
    assert_eq!(moment_pose("victory"), Some("celebrate"));
    assert_eq!(moment_pose("death"), Some("facepalm"));
    assert_eq!(moment_pose("cutscene"), None);
}
