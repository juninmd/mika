//! `run_expression` — o que a ação `expression` da tool `avatar` despacha
//! (`tool.rs::AvatarToolHandler::call`), extraído para arquivo próprio pelo teto de 300 linhas do
//! AGENTS.md. Espelha `CharacterService.setExpression`/`scheduleRevert`
//! (`apps/desktop/src/features/character/services/character.service.ts`): publica a pose
//! imediatamente e agenda o retorno a `neutral` depois de `durationMs`.
//!
//! **Barramento injetado, não `crate::events::bus()` direto** — mesmo padrão de
//! `voice/tts/bus_bridge.rs` (ver o cabeçalho de lá): produção passa `crate::events::bus()`
//! (`tool.rs`), os testes deste arquivo passam uma instância ISOLADA
//! (`Box::leak(Box::new(EventBus::new()))`), fora da disputa por singleton de processo que
//! `events/bus.rs` documenta (`cargo test` roda o binário inteiro em paralelo).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::events::{EventBus, EventKey};

/// `durationMs` já vem limitado a 500..30000 pelo schema de entrada do lado TS
/// (`z.number().min(500).max(30_000)`); o clamp aqui é defensivo porque
/// `modules/tools/validation.rs` não checa `minimum`/`maximum` de JSON Schema (só
/// `type`/`enum`/`required`) — mesmo motivo de `clamp_affinity` em `tool.rs` existir.
///
/// **Cancelamento por geração, não `JoinHandle`.** O TS usa um slot único (`this.timers`,
/// `EXPRESSION_REVERT_SLOT`) e `clearTimeout` na chamada seguinte; aqui, cada chamada incrementa
/// `generation` e sua própria task só publica `neutral` se `generation` ainda for a SUA, no
/// momento em que acorda — uma chamada mais nova invalida qualquer reversão pendente de uma mais
/// antiga, mesmo efeito do `clearTimeout`, sem guardar o `JoinHandle` no handler.
pub fn run_expression(bus: &'static EventBus, generation: &Arc<AtomicU64>, expression: &str, duration_ms: f64) -> Value {
    let duration_ms = duration_ms.clamp(500.0, 30_000.0);
    bus.publish(EventKey::Expression, json!({ "expression": expression }));

    let my_generation = generation.fetch_add(1, Ordering::SeqCst) + 1;
    let generation = Arc::clone(generation);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(duration_ms as u64)).await;
        if revert_is_still_current(my_generation, generation.load(Ordering::SeqCst)) {
            bus.publish(EventKey::Expression, json!({ "expression": "neutral" }));
        }
    });

    json!({ "success": true, "data": { "expression": expression, "durationMs": duration_ms } })
}

/// A regra de cancelamento em si, separada para ser testável sem depender de sleep real — mesmo
/// motivo de `expression_bridge::select_targets` ser testado como função pura.
fn revert_is_still_current(scheduled_generation: u64, current_generation: u64) -> bool {
    scheduled_generation == current_generation
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaked_bus() -> &'static EventBus {
        Box::leak(Box::new(EventBus::new()))
    }

    #[tokio::test]
    async fn publishes_the_pose_immediately_and_returns_the_ts_shape() {
        let bus = leaked_bus();
        let mut sub = bus.subscribe();
        let generation = Arc::new(AtomicU64::new(0));

        let result = run_expression(bus, &generation, "sitting", 5000.0);
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["data"]["expression"], json!("sitting"));
        assert_eq!(result["data"]["durationMs"], json!(5000.0));

        let envelope = sub.recv().await.expect("devia publicar imediatamente");
        assert_eq!(envelope.key, EventKey::Expression);
        assert_eq!(envelope.payload, json!({ "expression": "sitting" }));
    }

    #[tokio::test]
    async fn duration_is_clamped_to_the_declared_range() {
        let bus = leaked_bus();
        let generation = Arc::new(AtomicU64::new(0));

        let too_short = run_expression(bus, &generation, "happy", 10.0);
        assert_eq!(too_short["data"]["durationMs"], json!(500.0));

        let too_long = run_expression(bus, &generation, "happy", 999_999.0);
        assert_eq!(too_long["data"]["durationMs"], json!(30000.0));
    }

    /// Prova de ponta a ponta do agendamento: a pose some e `neutral` chega depois de
    /// `durationMs` (mínimo aceito, 500ms) — mesmo comportamento de
    /// `CharacterService.scheduleRevert`.
    #[tokio::test]
    async fn reverts_to_neutral_after_the_duration_elapses() {
        let bus = leaked_bus();
        let mut sub = bus.subscribe();
        let generation = Arc::new(AtomicU64::new(0));

        run_expression(bus, &generation, "reading", 500.0);

        let first = sub.recv().await.expect("pose imediata não publicada");
        assert_eq!(first.payload, json!({ "expression": "reading" }));

        let reverted = tokio::time::timeout(Duration::from_millis(1500), sub.recv())
            .await
            .expect("reversão para neutral não chegou a tempo")
            .expect("bus fechou antes da reversão");
        assert_eq!(reverted.payload, json!({ "expression": "neutral" }));
    }

    /// NEGATIVO — uma chamada mais nova invalida a reversão pendente da mais antiga, mesmo efeito
    /// do `clearTimeout` de slot único do TS. O barramento é ISOLADO (`leaked_bus`), então "exatamente
    /// um `neutral`" é uma afirmação segura aqui — nenhum outro teste pode publicar nele.
    #[tokio::test]
    async fn a_newer_call_cancels_an_older_pending_revert() {
        let bus = leaked_bus();
        let mut sub = bus.subscribe();
        let generation = Arc::new(AtomicU64::new(0));

        run_expression(bus, &generation, "happy", 500.0);
        run_expression(bus, &generation, "sad", 500.0);

        let mut neutral_count = 0;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(1500);
        loop {
            let Ok(Some(envelope)) = tokio::time::timeout_at(deadline, sub.recv()).await else { break };
            if envelope.payload == json!({ "expression": "neutral" }) {
                neutral_count += 1;
            }
        }
        assert_eq!(neutral_count, 1, "só a chamada mais recente devia reverter — igual ao slot único do TS");
    }

    #[test]
    fn the_pure_cancellation_rule_matches_generation_numbers() {
        assert!(revert_is_still_current(1, 1));
        assert!(!revert_is_still_current(1, 2), "uma chamada mais nova devia invalidar a reversão pendente da mais antiga");
    }
}
