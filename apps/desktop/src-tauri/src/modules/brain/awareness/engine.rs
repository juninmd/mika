//! `ProactiveEngine` — port de `proactive-engine.ts`: o laço que pergunta a cada provider "tem algo
//! a dizer?", escolhe o vencedor (`tick.rs`), reserva a cadência ANTES de chamar o LLM e desfaz a
//! reserva se a fala falhar — para um turno proativo que dá errado não custar o cooldown de um que
//! deu certo.
//!
//! Estado em uma struct própria (nunca `static`/global), injetado no `spawn`, testável sem Tauri —
//! ver AGENTS.md.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;

use super::gates::{cooldown_key, is_quiet_hours, passes_cadence, stretched_interval, CadenceState};
use super::interruption::interruption_prompt_from_payload;
use super::moves::{takes_a_move, with_move, Move, MoveDeck};
use super::tick::pick_best;
use super::types::{ProactiveCandidate, ProactiveProvider};
use crate::modules::gamer::gaming_state::is_game_candidate_key;

/// Fallback para `on_interruption` quando o payload não carrega `window_label` (versão antiga do
/// publicador, ou payload sintético de teste) — mesmo literal que `is_owner_surface` trata como a
/// janela principal do app.
const DEFAULT_INTERRUPTION_WINDOW: &str = "main";

/// A sender's reply meaning "chose not to speak" (a repeat, or nothing new): unlike an empty reply
/// (failure, slot released), the cooldown stays reserved so she waits for something different.
pub const HELD_BACK: &str = "\u{0}held-back";

/// Quem efetivamente fala — em produção, um turno real (`modules/chat/proactive.rs::send_awareness`);
/// em teste, um dublê que nunca toca um provedor de LLM de verdade.
#[async_trait]
pub trait ProactiveSender: Send + Sync {
    async fn send(&self, prompt: &str) -> String;

    /// Mesmo contrato de [`Self::send`], mas escopado à janela cujo barge-in disparou esta resposta
    /// — para que ela registre seu handle de abort sob essa janela (`chat::turns::register_turn`) e
    /// um SEGUNDO barge-in aborte a chamada de LLM em curso, não só o áudio (defects.md "barge-in",
    /// item 2). `barge_in_at_ms` (epoch ms do instante em que o barge-in foi DETECTADO, code review
    /// major) deixa o chamador real descartar a resposta se um turno de chat de verdade já tiver
    /// TERMINADO desde então — ver `chat::proactive::run_proactive_turn`. Default: cai para
    /// [`Self::send`] sem escopo — os dublês de teste e o caminho de tick periódico (que não tem
    /// janela chamadora nenhuma) continuam funcionando sem mudança.
    async fn send_for_window(&self, prompt: &str, _window_label: &str, _barge_in_at_ms: Option<u64>) -> String {
        self.send(prompt).await
    }
}

struct EngineState {
    last_spoke_at: u64,
    last_for_key: HashMap<String, u64>,
    hourly_speak_timestamps: Vec<u64>,
    moves: MoveDeck,
    /// Proactive lines spoken since the user last started a turn — see [`gates::stretched_interval`].
    unanswered: u32,
}

pub struct ProactiveEngine {
    providers: Vec<Box<dyn ProactiveProvider>>,
    sender: Box<dyn ProactiveSender>,
    state: Mutex<EngineState>,
    user_turn_in_flight: AtomicBool,
    speaking: AtomicBool,
    bypass_dedup: bool,
    base_interval_ms: u64,
    quiet_hours: Box<dyn Fn() -> (u32, u32) + Send + Sync>,
    proactiveness: Box<dyn Fn() -> f64 + Send + Sync>,
    current_hour: Box<dyn Fn() -> u32 + Send + Sync>,
    /// Port de `isGameTense()` (`proactive-gates.ts`) — checado ANTES de qualquer outra regra em
    /// `should_speak`, mesma ordem do TS: um momento de tensão suprime TODO candidato, mesmo
    /// prioridade 9. `None` (motor de teste que não injeta relógio de tensão) equivale a "nunca
    /// tenso" — mesmo default do TS antes de `game-tension.ts` publicar algo.
    game_tense: Box<dyn Fn() -> bool + Send + Sync>,
    /// specs/08-gamer-companion.md ("Estado de comportamento") — `MUST`: proatividade reduzida para
    /// candidatos que NÃO são de jogo enquanto uma sessão está aberta. `None` (motor sem opinião
    /// sobre o estado de gaming, mesmo default de `game_tense`) equivale a "nunca jogando".
    is_gaming: Box<dyn Fn() -> bool + Send + Sync>,
    /// defects.md "barge-in", item 3: a Mika não pode COMEÇAR uma fala proativa enquanto o usuário
    /// fala, nem logo depois que ele para (`voice::user_speech_activity`, mesmo shape de
    /// `game_tense`/`is_gaming` — `false` por padrão para um motor de teste sem opinião sobre isso).
    /// Só afeta `should_speak` (tick/`request`); `on_interruption` responde à própria fala que
    /// interrompeu, então nunca passa por aqui.
    user_recently_spoke: Box<dyn Fn() -> bool + Send + Sync>,
    speak_timeout: std::time::Duration,
}

impl ProactiveEngine {
    /// Construtor completo, com o gate de fala do usuário injetável (defects.md "barge-in", item 3).
    /// `new`/`with_game_tense`/`with_gaming` (só de teste, no fim do arquivo) fixam em `false` os
    /// sinais sobre os quais quem chama não tem opinião. Único construtor que a produção usa hoje
    /// (`awareness/mod.rs::spawn_proactive_engine`).
    #[allow(clippy::too_many_arguments)]
    pub fn with_user_speech_gate(
        providers: Vec<Box<dyn ProactiveProvider>>,
        sender: Box<dyn ProactiveSender>,
        bypass_dedup: bool,
        base_interval_ms: u64,
        quiet_hours: Box<dyn Fn() -> (u32, u32) + Send + Sync>,
        proactiveness: Box<dyn Fn() -> f64 + Send + Sync>,
        current_hour: Box<dyn Fn() -> u32 + Send + Sync>,
        game_tense: Box<dyn Fn() -> bool + Send + Sync>,
        is_gaming: Box<dyn Fn() -> bool + Send + Sync>,
        user_recently_spoke: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self {
            providers,
            sender,
            state: Mutex::new(EngineState {
                last_spoke_at: 0,
                last_for_key: HashMap::new(),
                hourly_speak_timestamps: Vec::new(),
                moves: MoveDeck::default(),
                unanswered: 0,
            }),
            user_turn_in_flight: AtomicBool::new(false),
            speaking: AtomicBool::new(false),
            speak_timeout: std::time::Duration::from_secs(super::constants::PROACTIVE_TIMEOUT_SECS),
            bypass_dedup,
            base_interval_ms,
            quiet_hours,
            proactiveness,
            current_hour,
            game_tense,
            is_gaming,
            user_recently_spoke,
        }
    }

    /// MUST da spec ("entrar e sair do estado reinicia os timers dependentes"): limpa o cooldown
    /// POR CHAVE de todo candidato — sem isto, um assunto que já estava de castigo antes da transição
    /// continuaria contando o relógio antigo depois dela. `last_spoke_at`/o teto por hora NÃO são
    /// tocados: são o ritmo GERAL de fala, não "timers dependentes" do estado de gaming.
    pub fn on_gaming_transition(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.last_for_key.clear();
    }

    pub fn on_thinking_start(&self) {
        self.user_turn_in_flight.store(true, Ordering::SeqCst);
        self.state.lock().unwrap_or_else(|e| e.into_inner()).unanswered = 0;
    }

    /// A line spoken outside the engine (backseat game comment) still counts for the minimum gap:
    /// two pipelines talking back to back is "fala sem parar" all the same.
    pub fn note_spoken_elsewhere(&self, now_ms: u64) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.last_spoke_at = state.last_spoke_at.max(now_ms);
    }

    pub fn on_thinking_stop(&self) {
        self.user_turn_in_flight.store(false, Ordering::SeqCst);
    }

    /// Reage a `EventKey::Interruption` — sem gate de cadência, ver o cabeçalho de `interruption.rs`.
    /// Descartada se a Mika já estiver falando de novo (mesma seção crítica de `speak`, evita duas
    /// falas simultâneas). `window_label` (a janela que sofreu o barge-in, publicada por
    /// `vad_audio.rs::interruption_event_payload`) escopa o handle de abort — defects.md "barge-in",
    /// item 2. Ausente (payload de outra origem/versão) degrada para `DEFAULT_INTERRUPTION_WINDOW`
    /// em vez de recusar a resposta: melhor um handle sob a janela padrão do que nenhuma fala.
    pub async fn on_interruption(&self, payload: &Value) {
        let Some(prompt) = interruption_prompt_from_payload(payload) else { return };
        let window_label = payload.get("window_label").and_then(Value::as_str).unwrap_or(DEFAULT_INTERRUPTION_WINDOW);
        // `vad_audio.rs::interruption_event_payload` — instante em que o barge-in foi DETECTADO
        // (code review, major), não quando a transcrição terminou (`timestamp`, mais tarde).
        let barge_in_at_ms = payload.get("bargeInAtMs").and_then(Value::as_u64);
        if self.speaking.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = tokio::time::timeout(self.speak_timeout, self.sender.send_for_window(&prompt, window_label, barge_in_at_ms)).await;
        self.speaking.store(false, Ordering::SeqCst);
    }

    fn should_speak(&self, candidate: &ProactiveCandidate, now_ms: u64) -> bool {
        // Mesma ordem do TS (`shouldSpeak` em `proactive-engine.ts`): tensão de jogo é a PRIMEIRA
        // checagem, antes até de `userTurnInFlight` — um momento tenso suprime qualquer candidato.
        if (self.game_tense)() {
            return false;
        }
        if self.user_turn_in_flight.load(Ordering::SeqCst) {
            return false;
        }
        if self.speaking.load(Ordering::SeqCst) {
            return false;
        }
        // defects.md "barge-in", item 3: guarda de segurança, não de cadência — mesmo lugar que os
        // três gates acima, ANTES de `bypass_dedup` (que só pula cadência/horário/proatividade, não
        // "não fale por cima do usuário"). `should_speak` nunca reserva a cadência quando devolve
        // `false` (só `speak()` reserva), então isto ADIA o candidato para o próximo tick — nunca o
        // descarta.
        if (self.user_recently_spoke)() {
            return false;
        }
        if self.bypass_dedup {
            return true;
        }

        let (start, end) = (self.quiet_hours)();
        let quiet = is_quiet_hours((self.current_hour)(), start, end);
        let mut proactiveness = (self.proactiveness)();
        // specs/08-gamer-companion.md ("Estado de comportamento"): jogando, todo assunto que NÃO é o
        // próprio jogo compete por metade da proatividade normal (cooldown mais longo, ver
        // `gates::cooldown_for`) — o jogo continua tendo prioridade cheia.
        if (self.is_gaming)() && !is_game_candidate_key(&candidate.key) {
            proactiveness = (proactiveness / 2.0).max(0.0);
        }

        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let cadence = CadenceState {
            last_spoke_at: state.last_spoke_at,
            last_for_key: &state.last_for_key,
            hourly_speak_timestamps: &state.hourly_speak_timestamps,
            base_interval_ms: stretched_interval(self.base_interval_ms, state.unanswered),
        };
        passes_cadence(&cadence, candidate, now_ms, quiet, proactiveness)
    }

    /// Port de `ProactiveEngine.request` — porta de entrada para um candidato PONTUAL disparado por
    /// evento (`event_reactions.rs`), fora do tick periódico dos providers. Passa pela MESMA cadência
    /// (`should_speak`) que o tick usa — nunca fala fora dela, mesmo para um evento "importante".
    pub async fn request(&self, candidate: ProactiveCandidate, now_ms: u64) {
        if !self.should_speak(&candidate, now_ms) {
            return;
        }
        self.speak(candidate, now_ms).await;
    }

    /// Port de `runProactiveTick` + `shouldSpeak` + `speak`: pergunta a cada provider, filtra pelos
    /// que passam a cadência, fala o de maior prioridade.
    pub async fn tick(&self, now_ms: u64) {
        let candidates: Vec<ProactiveCandidate> =
            self.providers.iter().filter_map(|p| p.due(now_ms)).filter(|c| self.should_speak(c, now_ms)).collect();

        if let Some(winner) = pick_best(candidates) {
            self.speak(winner, now_ms).await;
        }
    }

    /// Reserva a cadência ANTES de chamar o sender; desfaz a reserva (`release`) se a fala falhar ou
    /// vier vazia — mesmo contrato de `speak()` no TS (`releaseReservation`).
    async fn speak(&self, candidate: ProactiveCandidate, now_ms: u64) {
        if self.speaking.swap(true, Ordering::SeqCst) {
            return;
        }

        let (previous, drawn) = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let previous = (state.last_spoke_at, state.last_for_key.get(cooldown_key(&candidate.key)).copied());
            state.last_spoke_at = now_ms;
            state.last_for_key.insert(cooldown_key(&candidate.key).to_string(), now_ms);
            state.hourly_speak_timestamps.push(now_ms);
            let drawn = takes_a_move(&candidate.key).then(|| state.moves.draw(now_ms, rand::random())).flatten();
            (previous, drawn)
        };
        let prompt = compose_prompt(&candidate.prompt, drawn);

        // Sem teto, um provedor travado deixaria `speaking` preso em `true` e a Mika calada para
        // sempre — o timeout do TS existia por isso. Estourar conta como falha: devolve o slot.
        let text = tokio::time::timeout(self.speak_timeout, self.sender.send(&prompt)).await.unwrap_or_default();

        if text == HELD_BACK {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(pos) = state.hourly_speak_timestamps.iter().rposition(|&at| at == now_ms) {
                state.hourly_speak_timestamps.remove(pos);
            }
        } else if text.trim().is_empty() {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.last_spoke_at = previous.0;
            match previous.1 {
                Some(at) => {
                    state.last_for_key.insert(cooldown_key(&candidate.key).to_string(), at);
                }
                None => {
                    state.last_for_key.remove(cooldown_key(&candidate.key));
                }
            }
            if let Some(pos) = state.hourly_speak_timestamps.iter().rposition(|&at| at == now_ms) {
                state.hourly_speak_timestamps.remove(pos);
            }
        } else {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.unanswered += 1;
            if let Some(mv) = drawn {
                state.moves.mark(mv, now_ms);
            }
        }

        self.speaking.store(false, Ordering::SeqCst);
    }
}

/// Prompt sem jogada é degradação, não falha: a fala sai como antes do sorteio existir.
fn compose_prompt(prompt: &str, drawn: Option<Move>) -> String {
    let Some(mv) = drawn else { return prompt.to_string() };
    with_move(prompt, mv).unwrap_or_else(|error| {
        log::error!("jogada proativa {mv:?}: prompt não carregou: {error}");
        prompt.to_string()
    })
}

#[cfg(test)]
impl ProactiveEngine {
    pub fn new(
        providers: Vec<Box<dyn ProactiveProvider>>,
        sender: Box<dyn ProactiveSender>,
        bypass_dedup: bool,
        base_interval_ms: u64,
        quiet_hours: Box<dyn Fn() -> (u32, u32) + Send + Sync>,
        proactiveness: Box<dyn Fn() -> f64 + Send + Sync>,
        current_hour: Box<dyn Fn() -> u32 + Send + Sync>,
    ) -> Self {
        Self::with_game_tense(providers, sender, bypass_dedup, base_interval_ms, quiet_hours, proactiveness, current_hour, Box::new(|| false))
    }

    /// Igual a [`Self::new`], mas com o relógio de tensão de jogo injetável — separado para não
    /// quebrar os chamadores/testes existentes que não têm opinião sobre tensão.
    #[allow(clippy::too_many_arguments)]
    pub fn with_game_tense(
        providers: Vec<Box<dyn ProactiveProvider>>,
        sender: Box<dyn ProactiveSender>,
        bypass_dedup: bool,
        base_interval_ms: u64,
        quiet_hours: Box<dyn Fn() -> (u32, u32) + Send + Sync>,
        proactiveness: Box<dyn Fn() -> f64 + Send + Sync>,
        current_hour: Box<dyn Fn() -> u32 + Send + Sync>,
        game_tense: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self::with_gaming(
            providers,
            sender,
            bypass_dedup,
            base_interval_ms,
            quiet_hours,
            proactiveness,
            current_hour,
            game_tense,
            Box::new(|| false),
        )
    }

    /// Igual a [`Self::with_game_tense`], mas com o estado de gaming injetável — separado pela mesma
    /// razão que aquele construtor existe: não quebrar os chamadores/testes que não têm opinião sobre
    /// o assunto.
    #[allow(clippy::too_many_arguments)]
    pub fn with_gaming(
        providers: Vec<Box<dyn ProactiveProvider>>,
        sender: Box<dyn ProactiveSender>,
        bypass_dedup: bool,
        base_interval_ms: u64,
        quiet_hours: Box<dyn Fn() -> (u32, u32) + Send + Sync>,
        proactiveness: Box<dyn Fn() -> f64 + Send + Sync>,
        current_hour: Box<dyn Fn() -> u32 + Send + Sync>,
        game_tense: Box<dyn Fn() -> bool + Send + Sync>,
        is_gaming: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self::with_user_speech_gate(
            providers,
            sender,
            bypass_dedup,
            base_interval_ms,
            quiet_hours,
            proactiveness,
            current_hour,
            game_tense,
            is_gaming,
            Box::new(|| false),
        )
    }

    /// Teto da chamada ao modelo — o padrão é `PROACTIVE_TIMEOUT_SECS`; teste encurta para provar
    /// a liberação sem esperar três minutos de relógio real.
    pub fn with_speak_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.speak_timeout = timeout;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::constants::PROACTIVE_BASE_INTERVAL_MS;
    use std::sync::Arc;

    struct FakeProvider {
        id: &'static str,
        candidate: Mutex<Option<ProactiveCandidate>>,
    }

    impl ProactiveProvider for FakeProvider {
        fn id(&self) -> &'static str {
            self.id
        }
        fn due(&self, _now_ms: u64) -> Option<ProactiveCandidate> {
            self.candidate.lock().unwrap().clone()
        }
    }

    fn candidate(key: &str, priority: u8) -> ProactiveCandidate {
        ProactiveCandidate { key: key.into(), priority, label: key.into(), prompt: "oi".into() }
    }

    struct RecordingSender {
        replies: Mutex<Vec<String>>,
        next_reply: Mutex<String>,
    }

    impl RecordingSender {
        fn new(next_reply: &str) -> Self {
            Self { replies: Mutex::new(Vec::new()), next_reply: Mutex::new(next_reply.to_string()) }
        }
        fn call_count(&self) -> usize {
            self.replies.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl ProactiveSender for RecordingSender {
        async fn send(&self, prompt: &str) -> String {
            self.replies.lock().unwrap().push(prompt.to_string());
            self.next_reply.lock().unwrap().clone()
        }
    }

    fn engine(sender: Arc<RecordingSender>, providers: Vec<Box<dyn ProactiveProvider>>, bypass: bool) -> ProactiveEngine {
        struct ArcSender(Arc<RecordingSender>);
        #[async_trait]
        impl ProactiveSender for ArcSender {
            async fn send(&self, prompt: &str) -> String {
                self.0.send(prompt).await
            }
        }
        ProactiveEngine::new(
            providers,
            Box::new(ArcSender(sender)),
            bypass,
            PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12), // meio-dia: nunca é horário silencioso nos testes
        )
    }

    #[tokio::test]
    async fn a_tick_with_no_provider_candidates_speaks_nothing() {
        let sender = Arc::new(RecordingSender::new("oi"));
        let e = engine(sender.clone(), vec![], true);
        e.tick(1_000_000).await;
        assert_eq!(sender.call_count(), 0);
    }

    #[tokio::test]
    async fn the_highest_priority_candidate_wins_and_speaks() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let low = Box::new(FakeProvider { id: "low", candidate: Mutex::new(Some(candidate("low", 3))) });
        let high = Box::new(FakeProvider { id: "high", candidate: Mutex::new(Some(candidate("high", 9))) });
        let e = engine(sender.clone(), vec![low, high], true);
        e.tick(1_000_000).await;
        assert_eq!(sender.replies.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn user_turn_in_flight_suppresses_every_candidate() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let provider = Box::new(FakeProvider { id: "p", candidate: Mutex::new(Some(candidate("p", 9))) });
        let e = engine(sender.clone(), vec![provider], true);
        e.on_thinking_start();
        e.tick(1_000_000).await;
        assert_eq!(sender.call_count(), 0);
        e.on_thinking_stop();
        e.tick(1_000_000).await;
        assert_eq!(sender.call_count(), 1);
    }

    #[tokio::test]
    async fn request_speaks_a_single_candidate_that_passes_cadence() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = engine(sender.clone(), vec![], false);
        e.request(candidate("game:zelda", 9), 10_000_000).await;
        assert_eq!(sender.call_count(), 1);
    }

    #[tokio::test]
    async fn request_is_silent_when_cadence_rejects_the_candidate() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = engine(sender.clone(), vec![], false);
        // Prioridade 3 (fraca) logo após uma fala recente ainda não libera o intervalo-base.
        e.request(candidate("first", 9), 10_000_000).await;
        e.request(candidate("second", 3), 10_000_100).await;
        assert_eq!(sender.call_count(), 1, "só a primeira deveria ter falado");
    }

    #[tokio::test]
    async fn a_failed_speech_rolls_back_the_cadence_reservation() {
        let sender = Arc::new(RecordingSender::new("")); // resposta vazia = falha
        let provider = Box::new(FakeProvider { id: "p", candidate: Mutex::new(Some(candidate("idle", 3))) });
        let e = engine(sender.clone(), vec![provider], false);

        e.tick(10_000_000).await;
        assert_eq!(sender.call_count(), 1);

        let state = e.state.lock().unwrap();
        assert_eq!(state.last_spoke_at, 0, "reserva deveria ter sido desfeita após resposta vazia");
        assert!(!state.last_for_key.contains_key("idle"));
        assert!(state.hourly_speak_timestamps.is_empty());
    }

    /// Smoke 2026-09-27: a rejected repeat released the slot, the next tick asked again and a
    /// paraphrase slipped through ("mudou pro Telegram agora?" 3× in a row). Holding back waits.
    #[tokio::test]
    async fn a_held_back_line_keeps_the_cooldown_so_the_next_tick_waits() {
        let sender = Arc::new(RecordingSender::new(HELD_BACK));
        let e = engine(sender.clone(), vec![], false);

        e.request(candidate("idle", 3), 10_000_000).await;
        e.request(candidate("idle", 3), 10_000_100).await;
        assert_eq!(sender.call_count(), 1, "held back must not be retried right away");
        let state = e.state.lock().unwrap();
        assert!(state.hourly_speak_timestamps.is_empty(), "nothing was said, so the hourly cap is untouched");
    }

    /// "Ela fala sem parar" (2026-09-27): lines nobody answers must space out, not keep the same
    /// 2-minute beat. Two unanswered lines double the wait; the user talking resets it.
    #[tokio::test]
    async fn unanswered_lines_stretch_the_interval_until_the_user_talks() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = engine(sender.clone(), vec![], false);
        let t0 = 10_000_000;
        e.request(candidate("a", 3), t0).await;
        e.request(candidate("b", 3), t0 + 130_000).await;
        assert_eq!(sender.call_count(), 2, "one unanswered line keeps the base interval");
        e.request(candidate("c", 3), t0 + 260_000).await;
        assert_eq!(sender.call_count(), 2, "two unanswered lines wait twice as long");
        e.on_thinking_start();
        e.on_thinking_stop();
        e.request(candidate("d", 3), t0 + 261_000).await;
        assert_eq!(sender.call_count(), 3, "the user answered: back to the base interval");
    }

    /// "Fala os mesmos assuntos": a window change and the screen observation right after it are
    /// the same subject (the app in front of the user), but had separate cooldowns, so she
    /// commented the same VS Code window twice in 2.5 min. They share one cooldown now.
    #[tokio::test]
    async fn a_window_change_and_a_screen_observation_share_one_cooldown() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = engine(sender.clone(), vec![], false);
        let t0 = 10_000_000;
        e.request(candidate("window:change", 6), t0).await;
        // Past the 1 min gap and the 2 min base interval, inside the 3 min priority-6 cooldown.
        e.request(candidate("screen-observation", 6), t0 + 150_000).await;
        assert_eq!(sender.call_count(), 1, "same subject, still cooling down");
        e.request(candidate("screen-observation", 6), t0 + 190_000).await;
        assert_eq!(sender.call_count(), 2, "the shared cooldown ended");
    }

    /// Backseat game comments speak outside the engine (`gamer::game_moment_commentary`); without
    /// counting them she followed a death comment with a new subject seconds later.
    #[tokio::test]
    async fn a_line_spoken_outside_the_engine_counts_for_the_minimum_gap() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = engine(sender.clone(), vec![], false);
        let t0 = 10_000_000;
        e.note_spoken_elsewhere(t0);
        e.request(candidate("game:zelda", 9), t0 + 5_000).await;
        assert_eq!(sender.call_count(), 0, "she just spoke");
        e.request(candidate("game:zelda", 9), t0 + 61_000).await;
        assert_eq!(sender.call_count(), 1);
    }

    /// Provedor travado não pode calar a Mika para sempre: estourou o teto, o slot volta e a
    /// próxima fala passa.
    #[tokio::test]
    async fn a_hung_provider_times_out_and_releases_the_speech_slot() {
        struct HungSender;
        #[async_trait]
        impl ProactiveSender for HungSender {
            async fn send(&self, _prompt: &str) -> String {
                std::future::pending::<String>().await
            }
        }
        let e = ProactiveEngine::new(
            vec![],
            Box::new(HungSender),
            true,
            PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
        )
        .with_speak_timeout(std::time::Duration::from_millis(20));
        e.request(candidate("a", 9), 1_000).await;
        assert!(!e.speaking.load(Ordering::SeqCst), "o slot de fala ficou preso depois do timeout");
    }

    #[tokio::test]
    async fn a_successful_speech_keeps_the_cadence_reservation() {
        let sender = Arc::new(RecordingSender::new("oi, tudo bem?"));
        let provider = Box::new(FakeProvider { id: "p", candidate: Mutex::new(Some(candidate("idle", 3))) });
        let e = engine(sender.clone(), vec![provider], false);

        e.tick(10_000_000).await;
        let state = e.state.lock().unwrap();
        assert_eq!(state.last_spoke_at, 10_000_000);
        assert_eq!(state.last_for_key.get("idle"), Some(&10_000_000));
    }

    /// Jogada sorteada entra só no prompt de "puxar assunto"; reação a um fato segue como veio.
    #[tokio::test]
    async fn a_free_conversation_candidate_carries_a_drawn_move_and_a_fact_reaction_does_not() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = engine(sender.clone(), vec![], true);
        e.request(candidate("idle", 3), 1_000_000).await;
        e.request(candidate("hardware", 9), 2_000_000).await;
        let sent = sender.replies.lock().unwrap();
        assert!(sent[0].starts_with("oi") && sent[0].contains("[THIS TURN'S MOVE]"), "{}", sent[0]);
        assert_eq!(sent[1], "oi");
    }

    #[tokio::test]
    async fn a_tense_game_moment_suppresses_every_candidate_even_priority_nine() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let provider = Box::new(FakeProvider { id: "p", candidate: Mutex::new(Some(candidate("urgent", 9))) });
        let e = ProactiveEngine::with_game_tense(
            vec![provider],
            Box::new({
                struct ArcSender(Arc<RecordingSender>);
                #[async_trait]
                impl ProactiveSender for ArcSender {
                    async fn send(&self, prompt: &str) -> String {
                        self.0.send(prompt).await
                    }
                }
                ArcSender(sender.clone())
            }),
            true,
            PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
            Box::new(|| true),
        );
        e.tick(1_000_000).await;
        assert_eq!(sender.call_count(), 0, "momento tenso deveria suprimir mesmo o bypass de dedup");
    }

    #[tokio::test]
    async fn interruption_replies_regardless_of_cadence_but_never_overlaps_a_speech_in_flight() {
        let sender = Arc::new(RecordingSender::new("desculpa"));
        let e = engine(sender.clone(), vec![], false);
        e.on_interruption(&serde_json::json!({ "transcript": "espera", "is_interruption": true })).await;
        assert_eq!(sender.call_count(), 1);
    }

    fn gaming_engine(sender: Arc<RecordingSender>, is_gaming: bool) -> ProactiveEngine {
        struct ArcSender(Arc<RecordingSender>);
        #[async_trait]
        impl ProactiveSender for ArcSender {
            async fn send(&self, prompt: &str) -> String {
                self.0.send(prompt).await
            }
        }
        ProactiveEngine::with_gaming(
            vec![],
            Box::new(ArcSender(sender)),
            false,
            PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
            Box::new(|| false),
            Box::new(move || is_gaming),
        )
    }

    /// MUST da spec ("Estado de comportamento"): proatividade reduzida para candidatos que NÃO são
    /// de jogo enquanto ela joga — o cooldown de 300s (prioridade 3, proatividade 50) vira 375s
    /// (proatividade efetiva cai pela metade, 25).
    #[tokio::test]
    async fn gaming_state_stretches_the_cooldown_for_a_non_game_candidate() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = gaming_engine(sender.clone(), true);
        e.request(candidate("idle", 3), 400_000).await;
        e.request(candidate("idle", 3), 720_000).await; // 320s depois: passaria os 300s normais, não os 375s de gaming
        assert_eq!(sender.call_count(), 1, "cooldown deveria continuar bloqueando aos 320s enquanto joga");
    }

    #[tokio::test]
    async fn gaming_state_does_not_stretch_the_cooldown_for_a_game_candidate() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = gaming_engine(sender.clone(), true);
        e.request(candidate("game-sustained", 3), 400_000).await;
        e.request(candidate("game-sustained", 3), 720_000).await;
        assert_eq!(sender.call_count(), 2, "o jogo em si não perde proatividade enquanto ela joga");
    }

    /// MUST da spec ("Estado de comportamento"): entrar/sair de gaming reinicia os timers
    /// dependentes — sem `on_gaming_transition`, o cooldown por chave de antes da transição continua
    /// contando o relógio antigo.
    #[tokio::test]
    async fn on_gaming_transition_clears_the_per_key_cooldown_so_a_stale_timer_does_not_linger() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = gaming_engine(sender.clone(), false);
        e.request(candidate("idle", 3), 9_900_000).await;
        e.request(candidate("idle", 3), 10_030_000).await; // só 130s depois: ainda bloqueado
        assert_eq!(sender.call_count(), 1, "baseline: cooldown de 300s ainda não passou");

        e.on_gaming_transition();
        e.request(candidate("idle", 3), 10_030_000).await; // mesmo instante, mas o timer foi zerado
        assert_eq!(sender.call_count(), 2, "on_gaming_transition deveria ter liberado o candidato");
    }

    // --- item 2 (defects.md "barge-in"): on_interruption escopa a resposta pela janela -------

    struct WindowCapturingSender(Arc<Mutex<Option<String>>>, Arc<Mutex<Option<u64>>>);
    #[async_trait]
    impl ProactiveSender for WindowCapturingSender {
        async fn send(&self, _prompt: &str) -> String {
            panic!("on_interruption deve chamar send_for_window, nunca send")
        }
        async fn send_for_window(&self, prompt: &str, window_label: &str, barge_in_at_ms: Option<u64>) -> String {
            *self.0.lock().unwrap() = Some(window_label.to_string());
            *self.1.lock().unwrap() = barge_in_at_ms;
            prompt.to_string()
        }
    }

    fn window_capturing_engine() -> (ProactiveEngine, Arc<Mutex<Option<String>>>, Arc<Mutex<Option<u64>>>) {
        let seen_window = Arc::new(Mutex::new(None));
        let seen_barge_in_at_ms = Arc::new(Mutex::new(None));
        let e = ProactiveEngine::new(
            vec![],
            Box::new(WindowCapturingSender(Arc::clone(&seen_window), Arc::clone(&seen_barge_in_at_ms))),
            false,
            PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
        );
        (e, seen_window, seen_barge_in_at_ms)
    }

    #[tokio::test]
    async fn on_interruption_scopes_the_reply_to_the_window_label_carried_by_the_payload() {
        let (e, seen_window, _) = window_capturing_engine();
        e.on_interruption(&serde_json::json!({
            "transcript": "espera",
            "is_interruption": true,
            "window_label": "settings",
        }))
        .await;
        assert_eq!(seen_window.lock().unwrap().as_deref(), Some("settings"));
    }

    /// Payload sem `window_label` (publicador antigo, ou sintético) degrada para a janela padrão em
    /// vez de recusar a resposta — ver o comentário de `DEFAULT_INTERRUPTION_WINDOW`.
    #[tokio::test]
    async fn on_interruption_falls_back_to_the_default_window_when_the_payload_carries_none() {
        let (e, seen_window, _) = window_capturing_engine();
        e.on_interruption(&serde_json::json!({ "transcript": "espera", "is_interruption": true })).await;
        assert_eq!(seen_window.lock().unwrap().as_deref(), Some(DEFAULT_INTERRUPTION_WINDOW));
    }

    /// Code review, major: `bargeInAtMs` do payload (instante da DETECÇÃO, não da transcrição) tem
    /// de chegar a `send_for_window` — é o que permite `chat::proactive::run_proactive_turn` provar
    /// que um turno real já tinha TERMINADO antes desta resposta sequer começar a esperar o LLM.
    #[tokio::test]
    async fn on_interruption_forwards_the_barge_in_detection_timestamp_from_the_payload() {
        let (e, _, seen_barge_in_at_ms) = window_capturing_engine();
        e.on_interruption(&serde_json::json!({
            "transcript": "espera",
            "is_interruption": true,
            "bargeInAtMs": 12_345_u64,
        }))
        .await;
        assert_eq!(*seen_barge_in_at_ms.lock().unwrap(), Some(12_345));
    }

    /// Payload sem `bargeInAtMs` (publicador antigo) degrada para `None` — a checagem extra vira
    /// no-op, não um bloqueio.
    #[tokio::test]
    async fn on_interruption_forwards_none_when_the_payload_carries_no_detection_timestamp() {
        let (e, _, seen_barge_in_at_ms) = window_capturing_engine();
        e.on_interruption(&serde_json::json!({ "transcript": "espera", "is_interruption": true })).await;
        assert_eq!(*seen_barge_in_at_ms.lock().unwrap(), None);
    }

    // --- item 3 (defects.md "barge-in"): gate de fala do usuário na proatividade --------------

    fn user_speech_gated_engine(sender: Arc<RecordingSender>, user_recently_spoke: bool) -> ProactiveEngine {
        struct ArcSender(Arc<RecordingSender>);
        #[async_trait]
        impl ProactiveSender for ArcSender {
            async fn send(&self, prompt: &str) -> String {
                self.0.send(prompt).await
            }
        }
        ProactiveEngine::with_user_speech_gate(
            vec![],
            Box::new(ArcSender(sender)),
            false,
            PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
            Box::new(|| false),
            Box::new(|| false),
            Box::new(move || user_recently_spoke),
        )
    }

    #[tokio::test]
    async fn a_recent_user_utterance_defers_a_candidate_instead_of_speaking_over_them() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let e = user_speech_gated_engine(sender.clone(), true);
        e.request(candidate("idle", 3), 1_000_000).await;
        assert_eq!(sender.call_count(), 0, "não pode falar por cima do usuário");
    }

    /// A prova de "adia, não descarta": `should_speak` nunca reserva cadência quando devolve
    /// `false`, então o MESMO candidato ainda fala assim que o gate libera — sem esperar o cooldown
    /// normal de novo.
    #[tokio::test]
    async fn a_deferred_candidate_still_speaks_once_the_user_speech_gate_clears() {
        let sender = Arc::new(RecordingSender::new("falei"));
        let gate = Arc::new(AtomicBool::new(true));
        let gate_for_closure = Arc::clone(&gate);
        struct ArcSender(Arc<RecordingSender>);
        #[async_trait]
        impl ProactiveSender for ArcSender {
            async fn send(&self, prompt: &str) -> String {
                self.0.send(prompt).await
            }
        }
        let e = ProactiveEngine::with_user_speech_gate(
            vec![],
            Box::new(ArcSender(sender.clone())),
            false,
            PROACTIVE_BASE_INTERVAL_MS,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
            Box::new(|| false),
            Box::new(|| false),
            Box::new(move || gate_for_closure.load(Ordering::SeqCst)),
        );

        e.request(candidate("idle", 3), 1_000_000).await;
        assert_eq!(sender.call_count(), 0, "adiado enquanto o gate está aberto");

        gate.store(false, Ordering::SeqCst);
        e.request(candidate("idle", 3), 1_000_000).await;
        assert_eq!(sender.call_count(), 1, "o MESMO candidato fala assim que o gate libera — nunca foi descartado");
    }
}
