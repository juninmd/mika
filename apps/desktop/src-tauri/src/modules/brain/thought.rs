//! `ThoughtService` — port funcional de `services/brain/services/thought.service.ts`, recortado ao
//! braço que o resto do backend Rust ainda não tinha: `handleProactiveEvent`, o consumidor de
//! `Events.BRAIN_PROACTIVE_THOUGHT` (`EventKey::BrainProactiveThought` desde 2026-09-22).
//!
//! **Só este braço.** O TS também assina `Events.BRAIN_GAMING_TICK` (comentário de gaming tick) e
//! registra um `ProactiveProvider` de "pensamento autônomo" periódico (`buildThoughtPrompt`,
//! ciclos de sonho/inovação) — nenhum dos dois é portado aqui: `EventKey::BrainGamingTick` não
//! existe em `events/keys.rs` e o provider de pensamento periódico precisa de
//! `BrainStateMachine`/config de intervalo que não fazem parte desta rodada. Ambos ficam nomeados,
//! não escondidos, para quem for portá-los depois.
//!
//! **`isProactivityAllowed` (o gate de `BrainStateMachine.state`: `meeting`/`focus-ide`/`away`
//! suprimem toda proatividade) também não é portado.** Não existe `BrainStateMachine` equivalente
//! no lado Rust ainda — a única cadência que filtra este caminho aqui é a do próprio
//! `ProactiveEngine::request` (`awareness/gates.rs`), a mesma que o TS já aplica por baixo do gate
//! de estado. Consumidor deste evento é o único, e não deve reimplementar o resto da máquina de
//! estados só para este braço.
//!
//! Mesmo contrato de `super::post_turn::listener::spawn_post_turn_listener`: assina o barramento
//! ANTES de `spawn` (broadcast não entrega retroativamente), e roda em
//! `tauri::async_runtime::spawn`, nunca `tokio::spawn`. O `ProactiveEngine` é INJETADO (o mesmo
//! `Arc` que `awareness::spawn_proactive_engine` devolve), nunca reconstruído aqui — duas
//! instâncias duplicariam o estado de cadência/dedup que a chave `proactive-event:{source}` conta
//! com estar compartilhado entre o tick periódico e este consumidor.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::awareness::engine::ProactiveEngine;
use super::awareness::types::ProactiveCandidate;
use crate::events::{bus, EventKey};

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// `buildProactiveEventPrompt` (`thought-prompts.ts`): o mesmo `.md` de reação a evento
/// (`brain/awareness-reactions.md`, namespace `brain` — não confundir com o namespace `awareness`
/// que `awareness/prompts.rs` usa para os candidatos do tick) com o contexto do evento anexado.
fn build_proactive_event_prompt(context: &str) -> Result<String, String> {
    let base = crate::modules::chat::load_prompt_no_handle("brain", "awareness-reactions", &std::collections::HashMap::new())?;
    Ok(format!("{base}\n\nEvent: {context}"))
}

/// Porte puro de `handleProactiveEvent`'s leitura de payload — extraído para ser testável sem
/// `ProactiveEngine`/prompt real. `source` cai em `"unknown"` e `priority` em `2` quando ausentes,
/// mesmos padrões defensivos que o resto do crate usa para payload de barramento não tipado.
fn payload_fields(payload: &Value) -> (&str, &str, u8) {
    let source = payload.get("source").and_then(Value::as_str).unwrap_or("unknown");
    let context = payload.get("context").and_then(Value::as_str).unwrap_or_default();
    let priority = payload.get("priority").and_then(Value::as_u64).unwrap_or(2).min(u8::MAX as u64) as u8;
    (source, context, priority)
}

/// Port de `ThoughtService::handleProactiveEvent`: monta o candidato e entrega ao motor — a MESMA
/// cadência (`ProactiveEngine::request`) que o tick periódico usa, nunca uma fala fora dela.
async fn handle_proactive_event(engine: &ProactiveEngine, payload: &Value, now_ms: u64) {
    let (source, context, priority) = payload_fields(payload);
    let prompt = match build_proactive_event_prompt(context) {
        Ok(p) => p,
        Err(error) => {
            log::error!("pensamento proativo sem prompt (`brain/awareness-reactions.md`): {error}");
            return;
        }
    };
    log::info!("Mika reagiu a um pensamento proativo (source={source}, priority={priority})");
    engine
        .request(
            ProactiveCandidate { key: format!("proactive-event:{source}"), priority, label: format!("proactive event: {source}"), prompt },
            now_ms,
        )
        .await;
}

/// Chamado uma vez no boot, logo após `awareness::spawn_proactive_engine` (que devolve o `Arc`
/// injetado aqui) — ver o cabeçalho do módulo `brain` para a linha exata de
/// `modules/mod.rs::register_config_bound_tools`.
///
/// `handle_proactive_event` -> `ProactiveEngine::request` -> `speak` pode ficar até
/// `PROACTIVE_TIMEOUT_SECS` (180s, `awareness/constants.rs`) esperando um turno de LLM real
/// (`ChatSender::send`). Await-lo DENTRO deste laço travaria `sub.recv()` pelo mesmo tempo — mesma
/// classe de defeito documentada em `gamer::game_learning::spawn` — então cada evento vira task
/// própria; `ProactiveEngine` já garante não sobrepor duas falas (`speaking` atômico em `engine.rs`),
/// então disparar várias concorrentemente é seguro.
pub fn spawn_thought_service(engine: Arc<ProactiveEngine>) -> tauri::async_runtime::JoinHandle<()> {
    // Assina ANTES do spawn — mesmo aviso de `post_turn::listener`/`awareness::spawn_proactive_engine`.
    let mut sub = bus().subscribe();
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if let EventKey::BrainProactiveThought = envelope.key {
                let engine = Arc::clone(&engine);
                let payload = envelope.payload.clone();
                let now = now_ms();
                tauri::async_runtime::spawn(async move {
                    handle_proactive_event(&engine, &payload, now).await;
                });
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- payload_fields: leitura defensiva, mesmos defaults do TS implícito -------------------------

    #[test]
    fn payload_fields_reads_source_context_and_priority() {
        let payload = json!({ "source": "clipboard-image", "context": "tem um gato na tela", "priority": 4 });
        assert_eq!(payload_fields(&payload), ("clipboard-image", "tem um gato na tela", 4));
    }

    #[test]
    fn payload_fields_falls_back_to_unknown_source_and_priority_two_when_missing() {
        assert_eq!(payload_fields(&json!({})), ("unknown", "", 2));
    }

    #[test]
    fn payload_fields_clamps_an_out_of_range_priority_instead_of_panicking() {
        // Barramento não tipado: um publicador poderia mandar um número absurdo. Nunca panica.
        let (_, _, priority) = payload_fields(&json!({ "priority": 999_999 }));
        assert_eq!(priority, u8::MAX);
    }

    // -- build_proactive_event_prompt: o mesmo `.md` real, via fallback de dev ----------------------

    #[test]
    fn build_proactive_event_prompt_wraps_the_real_awareness_reactions_md_with_the_event_context() {
        let prompt = build_proactive_event_prompt("o usuário copiou uma imagem").expect("prompt real deveria existir em dev");
        assert!(prompt.contains("Event: o usuário copiou uma imagem"));
        // Mesmo `.md` que `awareness/prompts.rs` usa para `buildProactiveEventPrompt` no TS —
        // não um placeholder inventado aqui.
        assert!(prompt.contains("You are Mika"));
    }

    // -- laço do assinante: um evento lento não pode travar o recv do próximo --------------------

    struct HungSender;
    #[async_trait::async_trait]
    impl crate::modules::brain::awareness::engine::ProactiveSender for HungSender {
        async fn send(&self, _prompt: &str) -> String {
            std::future::pending::<String>().await
        }
    }

    /// Mesma prova de `gamer::game_learning::a_slow_game_changed_handler_does_not_block_the_next_bus_receive`:
    /// `handle_proactive_event` -> `ProactiveEngine::request` pode ficar presa esperando o LLM
    /// (`speak_timeout`, até 180s em produção). Disparando-a como task própria em vez de
    /// `.await`ada inline, o assinante segue livre para receber o PRÓXIMO evento imediatamente.
    #[tokio::test]
    async fn a_slow_proactive_event_does_not_block_the_next_bus_receive() {
        let bus = crate::events::EventBus::with_capacity(4);
        let mut sub = bus.subscribe();
        bus.publish(EventKey::BrainProactiveThought, json!({ "source": "clipboard-image", "context": "gato" }));
        bus.publish(EventKey::VadSample, json!({ "speechProbability": 0.5 }));

        let first = sub.recv().await.unwrap();
        assert_eq!(first.key, EventKey::BrainProactiveThought);
        let engine = Arc::new(ProactiveEngine::new(
            vec![],
            Box::new(HungSender),
            false,
            120_000,
            Box::new(|| (23, 7)),
            Box::new(|| 50.0),
            Box::new(|| 12),
        ));
        let (engine, payload, now) = (Arc::clone(&engine), first.payload.clone(), now_ms());
        tokio::spawn(async move {
            handle_proactive_event(&engine, &payload, now).await;
        });

        // Se `handle_proactive_event` tivesse sido `.await`ada inline no laço, este `recv()` nunca
        // chegaria a rodar dentro do timeout do teste (o `HungSender` nunca resolve).
        let second = tokio::time::timeout(std::time::Duration::from_millis(500), sub.recv())
            .await
            .expect("o segundo evento devia chegar sem esperar o pensamento proativo terminar")
            .unwrap();
        assert_eq!(second.key, EventKey::VadSample);
    }
}
