//! Helpers de teste contra o singleton `bus()` — existem por causa do aviso no topo de
//! `events/bus.rs`: `bus()` é global ao processo e `cargo test` roda em paralelo, então um teste
//! que assina `bus()` vê eventos de QUALQUER outro teste concorrente. Este arquivo é o segundo
//! metade daquele aviso, separado só pelo limite de 300 linhas do AGENTS.md — leia o cabeçalho de
//! `bus.rs` primeiro para o porquê; aqui só a implementação dos dois padrões que ele recomenda.
//!
//! `mood/tool.rs` e `character/tool.rs` já reimplementaram este laço à mão, cada um do seu jeito,
//! antes deste arquivo existir — [`recv_matching`]/[`saw_within`] existem para que a próxima
//! chamadora não precise copiar o laço de novo.

use std::time::Duration;

use super::bus::{EventEnvelope, EventSubscription};
use super::keys::EventKey;

/// Espera até `key` aparecer no assinante, IGNORANDO qualquer outro evento — o padrão correto para
/// um teste POSITIVO contra `bus()` (ver o aviso no topo de `events/bus.rs`). Pára no primeiro
/// achado; não prova ausência de nada (um `None` só diz que `key` não chegou dentro do prazo).
pub(crate) async fn recv_matching(sub: &mut EventSubscription, key: EventKey, timeout: Duration) -> Option<EventEnvelope> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, sub.recv()).await {
            Ok(Some(envelope)) if envelope.key == key => return Some(envelope),
            Ok(Some(_)) => continue, // evento de OUTRO teste concorrente — ignora, continua esperando o nosso
            Ok(None) | Err(_) => return None, // publicador fechou, ou o prazo esgotou
        }
    }
}

/// Drena a janela `window` INTEIRA (não pára no primeiro achado) e diz se `key` apareceu em algum
/// momento — o padrão correto para um teste NEGATIVO escopado contra `bus()` (ver o aviso no topo
/// de `events/bus.rs`): "esta chamada não publicou `key`", tolerando qualquer evento alheio no meio.
pub(crate) async fn saw_within(sub: &mut EventSubscription, key: EventKey, window: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + window;
    loop {
        match tokio::time::timeout_at(deadline, sub.recv()).await {
            Ok(Some(envelope)) if envelope.key == key => return true,
            Ok(Some(_)) => continue,
            Ok(None) | Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::bus::EventBus;
    use serde_json::json;

    // Provados contra uma instância ISOLADA (`EventBus::new()`), não `bus()`, para controlar
    // exatamente qual "ruído de outro teste" simular — o singleton em si continua fora do escopo
    // destes testes, por ser exatamente o que estes helpers existem para tornar seguro de usar.

    /// O caso central de `recv_matching`: um evento de OUTRA chave chega primeiro (simulando outro
    /// teste concorrente publicando no mesmo `bus()`) e tem de ser ignorado, não interpretado como
    /// resposta.
    #[tokio::test]
    async fn recv_matching_skips_events_of_another_key_before_finding_its_own() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();

        bus.publish(EventKey::MoodChanged, json!({ "from": "outro teste" }));
        bus.publish(EventKey::VadSample, json!({ "speechProbability": 0.7 }));

        let envelope = recv_matching(&mut sub, EventKey::VadSample, Duration::from_millis(200))
            .await
            .expect("devia achar VadSample depois de pular o MoodChanged alheio");
        assert_eq!(envelope.payload["speechProbability"], json!(0.7));
    }

    #[tokio::test]
    async fn recv_matching_times_out_when_the_key_never_arrives() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();
        bus.publish(EventKey::MoodChanged, json!({}));

        let result = recv_matching(&mut sub, EventKey::VadAudio, Duration::from_millis(20)).await;
        assert!(result.is_none(), "VadAudio nunca foi publicado — não podia achar nada");
    }

    /// O caso central de `saw_within`: ruído de outra chave no meio da janela não derruba a
    /// detecção da chave certa mais adiante — diferente de `recv_matching`, aqui a janela é
    /// varrida inteira, não interrompida no primeiro achado.
    #[tokio::test]
    async fn saw_within_finds_the_key_even_with_unrelated_noise_around_it() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();

        bus.publish(EventKey::MoodChanged, json!({}));
        bus.publish(EventKey::MoodAffinityLevelUp, json!({}));

        assert!(saw_within(&mut sub, EventKey::MoodAffinityLevelUp, Duration::from_millis(200)).await);
    }

    /// A afirmação negativa escopada: só falso se a chave PROIBIDA nunca aparecer, mesmo com
    /// outros eventos passando pela janela — é o que substitui "nada foi publicado" contra `bus()`.
    #[tokio::test]
    async fn saw_within_returns_false_when_only_unrelated_events_arrive() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();
        bus.publish(EventKey::MoodChanged, json!({}));

        assert!(!saw_within(&mut sub, EventKey::MoodAffinityLevelUp, Duration::from_millis(20)).await);
    }
}
