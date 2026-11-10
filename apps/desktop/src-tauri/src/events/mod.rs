//! Barramento de eventos cross-módulo em Rust — equivalente ao par `Events`
//! (`packages/sdk/src/svc/events.ts`) + `EventBusService` (`packages/sdk/src/services/event-bus.ts`)
//! do lado TypeScript. Dividido em dois arquivos pela regra de 300 linhas do AGENTS.md, cada um com
//! uma responsabilidade:
//!
//! - [`keys`] — o VOCABULÁRIO: quais nomes de evento existem, por que cada um entrou nesta rodada e
//!   (para cada um) se ele tolera perda no barramento. Leia o cabeçalho de lá antes de adicionar uma
//!   variante nova.
//! - [`bus`] — o MECANISMO: como publicar/assinar sem bloquear, o que acontece com um assinante
//!   lento, e o AVISO DE TOPO sobre testar contra o singleton `bus()` em paralelo com outros
//!   testes — leia isso antes de escrever um teste que assina.
//! - `test_support` — os dois helpers que aquele aviso recomenda (`recv_matching`/`saw_within`),
//!   em arquivo próprio só pelo limite de 300 linhas; leia como continuação do aviso de `bus.rs`.
//!
//! `crate::events::{EventBus, EventKey, EventEnvelope, EventSubscription, bus}` continuam
//! resolvendo por este caminho — nenhum chamador existente precisa mudar o próprio `use` só porque
//! o arquivo virou diretório.
//!
//! **Por que este módulo existe.** Três módulos já portados documentaram a MESMA lacuna, cada um do
//! seu jeito: `brain/turn.rs` só tinha um `TurnObserver` LOCAL (o HUD nunca escutava de verdade fora
//! do processo que gera o turno); `vision.rs` citava a falta no próprio cabeçalho; e `voice/mod.rs`
//! recusava `voice:vad-audio` porque não existia onde publicar a amostra para os dois consumidores
//! reais. Este módulo resolve a lacuna em si — ligar cada publicador/assinante real ao canal IPC
//! continua sendo trabalho de quem tiver o módulo específico e o lado TS no escopo.

mod bus;
mod keys;
#[cfg(test)]
mod test_support;

// `EventEnvelope`/`EventSubscription`/`bus()` ainda não têm chamador fora deste módulo e de seus
// próprios testes — mesma categoria de `#![allow(dead_code)]` que `events/bus.rs` já carrega, só
// que para o re-export em vez do item em si. `EventBus`/`EventKey` já são consumidos por
// `modules/voice/vad_bridge.rs`.
#[allow(unused_imports)]
pub use bus::{bus, EventBus, EventEnvelope, EventSubscription};
pub use keys::EventKey;

// Observabilidade mínima (ver o cabeçalho de `events/bus.rs`) sem chamador fora dos próprios testes
// ainda.
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use bus::lagged_total;

// Helpers de teste contra o singleton `bus()` — ver o aviso no topo de `events/bus.rs` antes de
// escrever um teste novo que assina `bus()` em vez de usar uma instância isolada. Sem chamador FORA
// deste módulo ainda (outros arquivos de teste reimplementam o laço à mão hoje) — mesma categoria
// de `#[allow(unused_imports)]` já usada acima.
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use test_support::{recv_matching, saw_within};
