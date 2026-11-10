//! Mecanismo do barramento: `tokio::sync::broadcast` por baixo de [`EventBus`]. Ver o cabeçalho de
//! `events/mod.rs` para o que este módulo é, e o de `events/keys.rs` para o vocabulário que ele
//! transporta e a tolerância a perda de CADA evento — aqui só a primitiva de fan-out em si e o que
//! acontece, mecanicamente, quando um assinante fica para trás.
//!
//! # LEIA ANTES DE ESCREVER UM TESTE QUE ASSINA [`bus()`]
//!
//! [`bus()`] é **singleton de processo** ([`GLOBAL`], um `OnceLock`), e `cargo test` roda os testes
//! de um binário **em paralelo, no mesmo processo**. Um teste que assina `bus()` recebe TODO evento
//! publicado por QUALQUER outro teste rodando ao mesmo tempo — não só os seus. Já quebrou duas vezes
//! (achado pela frente de `features`, consertado caso a caso em `modules/features/mood/tool.rs` e
//! `modules/features/character/tool.rs`) de dois jeitos diferentes:
//!
//! - **Teste positivo que assume "a primeira mensagem é a minha"** (`sub.recv().await` cru, seguido
//!   de `assert_eq!` sobre o payload) — se outro teste publicar um evento ANTES do seu, a asserção
//!   compara contra o evento ERRADO e falha sem ninguém ter quebrado nada.
//! - **Teste negativo que assume "nada chegou == nada foi publicado"** — se outro teste publicar
//!   QUALQUER coisa durante a janela de espera, o teste vê uma mensagem e conclui (errado) que a
//!   sua ação publicou algo.
//!
//! Os dois se manifestam como flake intermitente — o pior tipo de vermelho, porque as pessoas
//! re-rodam até passar em vez de investigar.
//!
//! **O padrão correto, em ordem de preferência:**
//!
//! 1. **Prefira uma instância isolada.** `EventBus::new()`/`with_capacity` não tem esse problema —
//!   nenhum outro teste enxerga suas mensagens. Use isto sempre que o código sob teste ACEITA um
//!   `&EventBus` injetado (como `modules/voice/vad_bridge.rs::VadStream::process`); é o que todo
//!   teste deste próprio arquivo já faz. Só recorra a `bus()` quando o código sob teste chama o
//!   singleton por dentro (não é injetável) e você precisa provar a fiação REAL, não uma cópia.
//! 2. **Contra `bus()`, filtre pela [`EventKey`] específica com prazo — nunca pela ORDEM de
//!   chegada.** `recv_matching` (`events/test_support.rs`) faz isso: ignora qualquer envelope de
//!   outra chave e só devolve quando a sua aparecer (ou o prazo esgotar). É o mesmo laço que
//!   `mood/tool.rs` e `character/tool.rs` já reimplementaram à mão — use a função em vez de copiar
//!   o laço nesta rodada de novo.
//! 3. **Contra `bus()`, uma afirmação negativa só pode ser ESCOPADA a uma chave, nunca ao
//!   silêncio inteiro.** Não existe como provar "esta chamada não publicou nada" contra um
//!   singleton compartilhado por centenas de testes paralelos — outro teste pode publicar QUALQUER
//!   coisa a qualquer momento, legitimamente. O que dá para provar é "esta chamada não publicou
//!   ESTA chave específica", drenando a janela inteira e tolerando qualquer evento alheio —
//!   `saw_within` (`events/test_support.rs`) faz exatamente isso. Se a afirmação que você precisa é
//!   mesmo "nada foi publicado, ponto", volte à opção 1: só uma instância isolada responde essa
//!   pergunta.
//!
//! Nenhum helper resolve a opção 3 sem uma chave-alvo — isso é arquitetural (singleton de processo
//! + testes paralelos), não um bug para corrigir aqui. Os dois helpers moram em arquivo próprio
//! (`events/test_support.rs`) só por causa do limite de 300 linhas do AGENTS.md — leia-os como
//! parte deste mesmo aviso, não como um utilitário à parte.
//!
//! ## Primitiva escolhida: `tokio::sync::broadcast`
//!
//! - **Publicar nunca bloqueia.** `broadcast::Sender::send` é síncrono e retorna na hora — nunca um
//!   `.await` esperando alguém consumir. Requisito duro: o publicador real de VAD roda dentro de
//!   `tokio::task::spawn_blocking` (`inference/vad.rs`), e travar ali travaria o worker que também
//!   resolve `mika_invoke`.
//! - **Seguro entre threads.** `Sender`/`Receiver` são `Send + Sync`; o barramento do processo mora
//!   num `OnceLock` estático ([`bus`]), sem lock próprio no chamador — o `broadcast` já serializa
//!   por dentro.
//! - **Descartados como alternativa:** `mpsc` só tem UM consumidor (cross-módulo é fan-out N:N);
//!   `watch` guarda só o valor MAIS RECENTE (perde toda amostra intermediária — pior que
//!   `broadcast` para uma série temporal como VAD); um `Mutex<Vec<Box<dyn Fn>>>` (o desenho do
//!   `EventBusService` TS) reintroduziria o mesmo travamento que `spawn_blocking` existe para
//!   evitar, e listeners não tipados são exatamente o "nada de `String` solta" que [`EventKey`]
//!   evita.
//!
//! ## O que acontece com um assinante lento
//!
//! `broadcast` DESCARTA, nunca bloqueia nem enfileira sem limite. Cada assinante tem seu próprio
//! cursor sobre um anel de `capacity` posições; quem não drena a tempo recebe
//! `RecvError::Lagged(n)` na próxima leitura ([`EventSubscription::recv`] loga e segue — nunca
//! propaga como erro fatal) e perde as `n` mensagens mais antigas do anel; as mais novas continuam
//! chegando normalmente. Se essa perda é aceitável para um evento específico é decisão POR CHAVE,
//! não deste arquivo — ver a tabela em `events/keys.rs`.

#![allow(dead_code)]
// Sem chamador em produção ainda (mesma categoria de `brain/turn.rs::TurnResult::usage`): o
// barramento em si é a entrega desta tarefa. `cfg(test)` já exercita tudo.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tokio::sync::broadcast;

use super::keys::EventKey;

/// Equivalente a `EventRecord` (`packages/sdk/src/svc/event-bus.ts`): a chave, o payload e o
/// instante da publicação.
#[derive(Debug, Clone)]
pub struct EventEnvelope {
    pub key: EventKey,
    pub payload: Value,
    /// Milissegundos desde o Unix epoch, espelhando `EventRecord.at`.
    pub at_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Quantas mensagens cada assinante consegue ficar atrás antes de perder a mais antiga. Folga
/// generosa acima da maior taxa conhecida hoje (VAD a ~31 Hz por janela de 32 ms —
/// `inference/vad::FRAME_SAMPLES`); um assinante ainda pode enfileirar ~8s de amostras antes de
/// entrar em `Lagged`. Todo evento adicionado depois de `VadAudio`/`VadSample` dispara com
/// frequência muito menor (ver `events/keys.rs`), então esta capacidade continua dimensionada pelo
/// pior caso real, não pelo número de variantes.
const DEFAULT_CAPACITY: usize = 256;

/// O barramento em si: um `broadcast::Sender` compartilhado. Construível diretamente (testes e
/// futuros chamadores injetam a própria instância, em vez de amarrar tudo ao singleton — mesmo
/// padrão de `TurnObserver` injetado em `brain/turn.rs::run_turn`); [`bus`] expõe a instância única
/// do processo para quem publica/assina de verdade.
pub struct EventBus {
    sender: broadcast::Sender<EventEnvelope>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Nunca bloqueia. `send` só falha quando não há NENHUM assinante no instante da chamada — não
    /// é erro do publicador, é ausência de interessado; por isso o resultado é descartado.
    pub fn publish(&self, key: EventKey, payload: Value) {
        let _ = self.sender.send(EventEnvelope { key, payload, at_ms: now_ms() });
    }

    /// Nova assinatura. Cancelar é dropar o [`EventSubscription`] devolvido — sem método
    /// `unsubscribe` explícito porque o `Drop` do `broadcast::Receiver` já faz exatamente isso.
    /// `#[track_caller]`: o aviso de lag diz QUEM atrasou — sem isso, 3 assinantes lentos davam 3
    /// linhas idênticas e nenhuma pista de qual laço bloqueava.
    #[track_caller]
    pub fn subscribe(&self) -> EventSubscription {
        EventSubscription { receiver: self.sender.subscribe(), origin: std::panic::Location::caller() }
    }

    /// `false` permite pular a montagem de payloads caros (ex. PCM do VAD) que ninguém ouviria.
    pub fn has_subscribers(&self) -> bool {
        self.sender.receiver_count() > 0
    }

    #[cfg(test)]
    fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Lado de quem assina. Dropar a instância cancela a assinatura (mesmo padrão do retorno de
/// `IEventBus.on` no lado TS, que devolve a própria função de `off`).
pub struct EventSubscription {
    receiver: broadcast::Receiver<EventEnvelope>,
    origin: &'static std::panic::Location<'static>,
}

impl EventSubscription {
    /// Espera o próximo evento. `Lagged` é absorvido aqui — ver o cabeçalho do arquivo para o que
    /// isso significa mecanicamente, e `events/keys.rs` para o que isso significa POR EVENTO — e
    /// nunca chega ao chamador como erro; só `Closed` (o publicador em si não existe mais) devolve
    /// `None`.
    pub async fn recv(&mut self) -> Option<EventEnvelope> {
        loop {
            match self.receiver.recv().await {
                Ok(envelope) => return Some(envelope),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    LAGGED_TOTAL.fetch_add(skipped, Ordering::Relaxed);
                    log::warn!(
                        "assinante do barramento de eventos ({}) ficou {skipped} mensagem(ns) atrás — \
                         descartadas, não entregues (ver crate::events::keys para a tolerância por evento)",
                        self.origin
                    );
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

/// Contador de mensagens perdidas por lag, processo inteiro — observabilidade mínima para quem
/// investigar "barge-in perdeu áudio" no futuro sem precisar instrumentar de novo.
static LAGGED_TOTAL: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn lagged_total() -> u64 {
    LAGGED_TOTAL.load(Ordering::Relaxed)
}

static GLOBAL: OnceLock<EventBus> = OnceLock::new();

/// O barramento único do processo. Todo publicador/assinante cross-módulo de produção passa por
/// aqui; testes preferem `EventBus::new()`/`with_capacity` isolados, para não interferir uns com
/// os outros quando `cargo test` roda em paralelo dentro do mesmo processo — ver o aviso no topo
/// deste arquivo antes de assinar isto num teste novo.
pub fn bus() -> &'static EventBus {
    GLOBAL.get_or_init(EventBus::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn publish_without_subscribers_does_not_panic_or_block() {
        let bus = EventBus::new();
        bus.publish(EventKey::VadSample, json!({ "speechProbability": 0.1 }));
    }

    #[tokio::test]
    async fn a_subscriber_receives_the_published_event() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();

        bus.publish(EventKey::VadAudio, json!({ "samples": [1, 2, 3] }));

        let envelope = sub.recv().await.expect("assinante devia receber o evento publicado");
        assert_eq!(envelope.key, EventKey::VadAudio);
        assert_eq!(envelope.payload["samples"], json!([1, 2, 3]));
    }

    #[tokio::test]
    async fn every_subscriber_gets_its_own_copy_fan_out() {
        let bus = EventBus::new();
        let mut a = bus.subscribe();
        let mut b = bus.subscribe();

        bus.publish(EventKey::VadSample, json!({ "speechProbability": 0.9 }));

        assert_eq!(a.recv().await.unwrap().key, EventKey::VadSample);
        assert_eq!(b.recv().await.unwrap().key, EventKey::VadSample);
    }

    #[tokio::test]
    async fn dropping_a_subscription_unsubscribes_it() {
        let bus = EventBus::new();
        let sub = bus.subscribe();
        assert_eq!(bus.receiver_count(), 1);

        drop(sub);
        assert_eq!(bus.receiver_count(), 0);
    }

    /// O ponto central do desenho: um assinante que não drena a tempo PERDE mensagens antigas em
    /// vez de travar o publicador ou crescer sem limite. Capacidade 2 força o lag com poucas
    /// mensagens.
    #[tokio::test]
    async fn a_lagging_subscriber_is_notified_with_lagged_instead_of_blocking_the_publisher() {
        let bus = EventBus::with_capacity(2);
        let mut sub = bus.subscribe();
        let before = lagged_total();

        // Publica mais do que a capacidade SEM o assinante drenar — se `publish` bloqueasse
        // esperando espaço no anel, esta chamada nunca voltaria.
        for i in 0..5 {
            bus.publish(EventKey::VadSample, json!({ "speechProbability": i }));
        }

        // `recv` absorve o `Lagged` internamente e devolve o que sobrou no anel — as 3 mensagens
        // mais antigas (0, 1, 2) já foram descartadas pelo `broadcast`.
        let envelope = sub.recv().await.expect("assinante atrasado ainda recebe o que restou no anel");
        assert_eq!(envelope.payload["speechProbability"], json!(3));
        assert!(lagged_total() > before, "o lag não foi contabilizado");
    }

    #[tokio::test]
    async fn concurrent_publishers_from_multiple_tasks_never_panic() {
        let bus = Arc::new(EventBus::new());
        let mut sub = bus.subscribe();

        let mut tasks = Vec::new();
        for i in 0..8 {
            let bus = Arc::clone(&bus);
            tasks.push(tokio::spawn(async move {
                bus.publish(EventKey::VadAudio, json!({ "task": i }));
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }

        // Só confirma que pelo menos uma mensagem chegou sã — a ordem entre tasks concorrentes não
        // é garantia deste barramento.
        assert!(sub.recv().await.is_some());
    }

    #[test]
    fn bus_returns_the_same_process_wide_instance() {
        assert!(std::ptr::eq(bus(), bus()));
    }
}
