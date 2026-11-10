//! Pipeline pós-turno — o que roda DEPOIS que `chat::send` fecha um turno (`ThinkingStop`).
//! Porte parcial de `PostTurnPipeline` (`apps/desktop/src/main/services/brain/chat/pipeline.ts`);
//! ver o cabeçalho de [`pipeline`] para o que roda e o porquê explícito do que ficou de fora.
//!
//! Diretório, não arquivo único, pela mesma regra de 300 linhas que já divide `brain` em módulos:
//! - [`assembler`] — reconstrói o par usuário/assistente a partir dos eventos que `chat::send` já
//!   publica no barramento (não temos um evento pronto com o par completo — ver o cabeçalho dele).
//! - [`pipeline`] — orquestra o que roda para cada par fechado, na ordem do TS: memória episódica,
//!   fatos/ingestão social, e consolidação (só em turno pesado).
//! - [`cadence`] — `HeavyCadence`, o "é turno pesado?" (`shouldRunHeavyExtraction`).
//! - [`facts`] — extração de fatos via LLM (`fact-extractor.ts`), amarrada a [`social_ingest`].
//! - [`social_ingest`] — a metade REGEX de `ingestUserMessage` (nome/relação, sem `addEvent`).
//! - [`consolidation`] — dispara `EpisodicMemoryService::consolidate_old` (async desde esta rodada)
//!   com o prompt `memory/consolidation.md`.
//! - [`listener`] — assina o barramento e liga tudo acima à reflexão
//!   ([`super::reflection::run_reflection`]), que corre no MESMO gatilho (`ThinkingStop`) — daí os
//!   dois se encontrarem aqui, não em dois listeners separados competindo pelo mesmo evento.
//! - [`session_accumulator`] — porte de `engine/session-memory.ts::SessionMemoryAccumulator` +
//!   `chat/pipeline.ts::localExtract` (fatia REGEX), alimentado por [`listener`] a cada turno
//!   fechado; a injeção no prompt mora em `turn::build_raw_system_prompt`, fora deste diretório (ver
//!   o cabeçalho do arquivo). `flush`/`clear` ganharam chamador nesta rodada — ver o cabeçalho do
//!   próprio arquivo para onde.
//! - [`aging_sweep`] — consolidação/promoção por IDADE do tier curto da memória episódica,
//!   independente do teto de VOLUME que `consolidation::maybe_consolidate` exige (defects.md
//!   "memory-relationship" P1). Timer próprio (boot + a cada poucas horas), não turno-a-turno — daí
//!   viver ao lado de, e não dentro de, `listener`.
//!
//! `pub use listener::spawn_post_turn_listener`/`aging_sweep::spawn_aging_sweep` são os dois pontos
//! de entrada — ambos chamados uma vez no boot, mesmo ponto que já chama `features::mood::register`
//! (`modules/mod.rs::register_config_bound_tools`).

mod aging_sweep;
mod assembler;
mod cadence;
mod consolidation;
mod facts;
mod listener;
mod pipeline;
pub mod session_accumulator;
mod social_ingest;

pub use aging_sweep::spawn_aging_sweep;
pub use listener::spawn_post_turn_listener;
/// `origin` of bus turn events produced by a proactive speech (`chat/proactive.rs`).
pub const BACKGROUND_ORIGIN: &str = "background";

/// Proactive turns have no user request to pair with, so memory and turn-in-flight skip them.
pub fn is_background_turn(payload: &serde_json::Value) -> bool {
    payload.get("origin").and_then(serde_json::Value::as_str) == Some(BACKGROUND_ORIGIN)
}
