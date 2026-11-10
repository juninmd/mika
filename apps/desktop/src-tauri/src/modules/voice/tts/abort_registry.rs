//! Porte fiel de `apps/desktop/src/main/services/voice/audio/tts-abort-registry.ts` — o registro
//! de sínteses em voo que `Events.FORCE_STOP_TTS` aciona no lado TS. `vad_pipeline.rs` cita este
//! mesmo mecanismo como a peça que falta para o barge-in agir (ver o cabeçalho de lá,
//! "Sobra da atuação original"): [`force_stop_tts`] é o equivalente Rust de `abortActiveTTS`, a
//! função pública que fecha essa lacuna — falta só um chamador com o sinal de barge-in em mãos,
//! fora do escopo deste diretório (ver `mod.rs`).
//!
//! **Instância injetável, não só um `Set` module-level.** `tokio_util::sync::CancellationToken`
//! não está no grafo do crate; um `AbortController` vira aqui um `Arc<AtomicBool>` por síntese.
//! [`AbortRegistry`] é uma struct construível (`new()`), com [`global`] expondo a instância única
//! do processo — mesmo desenho de `events.rs::EventBus`/`bus()`, e pelo mesmo motivo: testes usam
//! instâncias isoladas para não se atropelarem quando `cargo test` roda em paralelo no mesmo
//! processo, e só um teste dedicado ancora a identidade do singleton.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Handle de cancelamento de UMA síntese. `Clone` porque o registro guarda uma cópia e o
/// chamador (`speaker.rs`) guarda outra — cancelar por qualquer uma das duas pontas é visível na
/// outra, mesma semântica do `AbortController` compartilhado do TS.
#[derive(Clone)]
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

pub struct AbortRegistry {
    active: Mutex<Vec<CancelHandle>>,
}

impl AbortRegistry {
    pub fn new() -> Self {
        Self { active: Mutex::new(Vec::new()) }
    }

    /// Registra uma nova síntese em voo. Chamado uma vez por chamada a `speaker::speak`.
    pub fn register_active(&self) -> ActiveGuard<'_> {
        let handle = CancelHandle::new();
        if let Ok(mut list) = self.active.lock() {
            list.push(handle.clone());
        }
        ActiveGuard { registry: self, handle }
    }

    /// Porte de `abortActiveTTS`: cancela toda síntese em voo. Devolve `true` se havia alguma —
    /// mesmo contrato do TS (o chamador usa isso para decidir se loga "nada para interromper").
    pub fn force_stop(&self) -> bool {
        let Ok(mut list) = self.active.lock() else { return false };
        if list.is_empty() {
            return false;
        }
        for handle in list.iter() {
            handle.cancel();
        }
        list.clear();
        true
    }

    pub fn has_active(&self) -> bool {
        self.active.lock().map(|list| !list.is_empty()).unwrap_or(false)
    }

    fn unregister(&self, handle: &CancelHandle) {
        if let Ok(mut list) = self.active.lock() {
            list.retain(|h| !Arc::ptr_eq(&h.0, &handle.0));
        }
    }
}

impl Default for AbortRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Guarda de RAII: o registro some quando a síntese termina (sucesso, erro ou cancelamento) —
/// mesmo papel do `unregister` devolvido por `registerActiveTTSController` no TS. `#[must_use]`
/// porque um registro dropado na hora é equivalente a nunca ter registrado.
#[must_use]
pub struct ActiveGuard<'a> {
    registry: &'a AbortRegistry,
    handle: CancelHandle,
}

impl ActiveGuard<'_> {
    pub fn handle(&self) -> CancelHandle {
        self.handle.clone()
    }
}

impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        self.registry.unregister(&self.handle);
    }
}

static GLOBAL: OnceLock<AbortRegistry> = OnceLock::new();

/// A instância única do processo — todo chamador de produção (o orquestrador em `speaker.rs`, e
/// quem futuramente ligar `voice:force-stop-tts` a este módulo) passa por aqui.
pub fn global() -> &'static AbortRegistry {
    GLOBAL.get_or_init(AbortRegistry::new)
}

/// Porte de `abortActiveTTS` sobre o registro global do processo — a função pública que
/// `vad_pipeline.rs` está esperando (ver o cabeçalho do arquivo).
pub fn force_stop_tts() -> bool {
    global().force_stop()
}

/// Sem chamador de produção: quem hoje pergunta "a Mika está falando?" é o `InterruptionDetector`,
/// e ele responde pelo par `SpeechStarted`/`SpeechFinished` do barramento, não consultando o
/// registro. Fica porque o TS expõe o equivalente (`hasActiveTTS`) e o `autoMuteDuringTts` vai
/// precisar dele quando for portado — mas o allow é por ITEM, para o resto do diretório continuar
/// sob vigilância de dead code.
#[allow(dead_code)]
pub fn has_active_tts() -> bool {
    global().has_active()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_stop_returns_false_when_nothing_is_active() {
        let registry = AbortRegistry::new();
        assert!(!registry.force_stop());
    }

    #[test]
    fn force_stop_cancels_every_registered_handle_and_clears_the_registry() {
        let registry = AbortRegistry::new();
        let guard_a = registry.register_active();
        let guard_b = registry.register_active();
        let handle_a = guard_a.handle();
        let handle_b = guard_b.handle();

        assert!(registry.has_active());
        assert!(registry.force_stop());

        assert!(handle_a.is_cancelled());
        assert!(handle_b.is_cancelled());
        assert!(!registry.has_active());
    }

    #[test]
    fn dropping_the_guard_unregisters_without_cancelling() {
        let registry = AbortRegistry::new();
        let guard = registry.register_active();
        let handle = guard.handle();
        drop(guard);

        assert!(!registry.has_active());
        assert!(!handle.is_cancelled(), "dropar não é o mesmo que abortar — a síntese terminou sozinha");
    }

    #[test]
    fn global_returns_the_same_process_wide_instance() {
        assert!(std::ptr::eq(global(), global()));
    }

    /// Prova ponta a ponta pela API de nível de módulo que `vad_pipeline.rs`/`speaker.rs` vão
    /// chamar de verdade — não só pela struct interna.
    #[test]
    fn module_level_force_stop_tts_reaches_the_global_registry() {
        let guard = global().register_active();
        let handle = guard.handle();

        assert!(force_stop_tts());
        assert!(handle.is_cancelled());
    }
}
