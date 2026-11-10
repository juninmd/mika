//! "Pausar mic e tela": o interruptor único dos sentidos da Mika, ligado pela bandeja
//! (`tray.rs`). Pausado, o áudio que chega do renderer NÃO entra no VAD nem no STT
//! (`voice::vad_audio`, `voice::transcribe`) e `is_screen_capture_allowed` nega — o chokepoint de
//! captura de tela que o watcher periódico, as tools de visão e o contexto de tela consultam.
//!
//! Estado de PROCESSO, de propósito, e não config: pausa é "agora não", e reiniciar o app volta ao
//! que o usuário configurou em vez de deixá-la surda para sempre sem ninguém lembrar. O que NÃO faz:
//! o microfone do SO continua aberto no renderer (o indicador do sistema segue aceso); o que muda é
//! que nada do que ele capta é processado nem sai do computador.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::events::{EventBus, EventKey};

pub struct Senses(AtomicBool);

impl Senses {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub fn paused(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// Alterna e devolve o estado NOVO.
    pub fn toggle(&self) -> bool {
        !self.0.fetch_xor(true, Ordering::SeqCst)
    }
}

impl Default for Senses {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL: Senses = Senses::new();

pub fn paused() -> bool {
    GLOBAL.paused()
}

pub fn toggle() -> bool {
    GLOBAL.toggle()
}

/// Publica o estado dos sentidos para o renderer (`hud:senses-paused`): sem isto o HUD e o
/// `SensesMonitor` seguiam mostrando "ouvindo"/"vendo" com tudo pausado pela bandeja.
fn announce(bus: &EventBus, paused: bool) {
    bus.publish(EventKey::SensesPaused, serde_json::json!({ "paused": paused }));
}

/// [`toggle`] + aviso ao renderer; devolve o estado NOVO. O chamador é a bandeja.
pub fn toggle_and_announce() -> bool {
    let paused = toggle();
    announce(crate::events::bus(), paused);
    paused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn senses_start_on_and_the_toggle_reports_the_new_state() {
        let senses = Senses::new();
        assert!(!senses.paused(), "ninguém nasce pausado");
        assert!(senses.toggle());
        assert!(senses.paused());
        assert!(!senses.toggle());
        assert!(!senses.paused());
    }

    /// B3: pausar pela bandeja só mudava o estado do Rust — HUD e SensesMonitor seguiam "ouvindo".
    /// O novo estado vai ao barramento (e dali ao renderer, `hud_bridge`).
    #[tokio::test]
    async fn announcing_the_senses_state_publishes_it_for_the_renderer() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe();
        for paused in [true, false] {
            announce(&bus, paused);
            let envelope = sub.recv().await.expect("evento publicado");
            assert_eq!(envelope.key, EventKey::SensesPaused);
            assert_eq!(envelope.payload, serde_json::json!({ "paused": paused }));
        }
    }
}
