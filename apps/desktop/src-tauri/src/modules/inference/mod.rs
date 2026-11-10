//! Módulo `inference` em Rust — os modelos ONNX que hoje rodam num `utilityProcess` isolado do
//! Electron (`main/services/inference/host.ts`).
//!
//! `vad.rs` e `whisper.rs` já portam os dois motores de verdade — sessão Silero e pipeline Whisper
//! completo sobre `ort`, ambos testados contra áudio real (ver o cabeçalho de cada arquivo).
//! `modules()` continua `vec![]` porque este módulo não é dono de canal IPC nenhum: é biblioteca
//! (`whisper::transcribe()`, `vad::SileroVad`), não um `NativeModule`. Reivindicar
//! `mika:voice-transcribe`/`voice:vad-audio` é decisão de `modules/voice.rs`, que documenta os
//! dois bloqueios reais que sobram mesmo com os motores prontos: `tokenizer.json` do Whisper não é
//! baixado em produção, e não existe barramento de eventos cross-módulo em Rust para os
//! consumidores reais do VAD (`interruption-detector.ts`/`barge-in-recorder.ts`).
//!
//! **Isolamento de inferência: implementado, não mais recomendação.** `vad.rs` e `whisper.rs` já
//! rodam toda chamada síncrona de `ort` (`Session::run`) dentro de `tokio::task::spawn_blocking` —
//! mesma garantia que o `utilityProcess` do Node dava, sem processo separado. O risco real
//! continua sendo travar o worker que também resolve `mika_invoke`, não o processo inteiro.
//!
//! `ort` é a ÚNICA opção de ONNX aqui. `tract-onnx` está reprovado e documentado: não roda nem o
//! Silero VAD (LSTM com mistura de I64 e F32). Não reabrir.

pub mod accel;
pub mod embedder;
pub mod nemotron;
pub mod session;
pub mod vad;
pub mod whisper;
#[cfg(test)]
mod tokenizer_regex_tests;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::native::NativeModule;

fn uptime_ms() -> u64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Último uso de um cache de modelo (Whisper, Nemotron, e5, Supertonic). Os pesos ficam em RAM (centenas de MB
/// cada) o app inteiro; com a Mika ao lado de um jogo, modelo parado há minutos é memória roubada
/// do jogo. [`evict_idle`] solta o que o governador de energia manda; a próxima chamada recarrega.
pub struct Lease(AtomicU64);

impl Lease {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn touch(&self) {
        self.0.store(uptime_ms(), Ordering::Relaxed);
    }

    fn idle_for(&self, max_idle: Duration, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.0.load(Ordering::Relaxed)) >= max_idle.as_millis() as u64
    }
}

/// Esvazia `cache` se ele está parado há pelo menos `max_idle` em `now_ms`. `try_lock`: um modelo
/// em uso (lock tomado) nunca é esperado nem descarregado.
fn evict_if_idle<T>(cache: &Mutex<Option<T>>, lease: &Lease, max_idle: Duration, now_ms: u64) -> bool {
    if !lease.idle_for(max_idle, now_ms) {
        return false;
    }
    let Ok(mut guard) = cache.try_lock() else { return false };
    guard.take().is_some()
}

/// [`evict_if_idle`] for a cache behind a `tokio` mutex (Supertonic: its async lock collapses
/// concurrent loads into one). Same rule: in use (locked) is never waited for nor dropped.
pub(crate) fn evict_async_if_idle<T>(cache: &tokio::sync::Mutex<Option<T>>, lease: &Lease, max_idle: Duration, now_ms: u64) -> bool {
    if !lease.idle_for(max_idle, now_ms) {
        return false;
    }
    let Ok(mut guard) = cache.try_lock() else { return false };
    guard.take().is_some()
}

/// Descarrega todo modelo ocioso — chamado pelo governador (`modules::system::governor`). Bloqueante
/// (soltar sessão ONNX libera memória de driver): rode fora do reator.
pub fn evict_idle(max_idle: Duration) {
    let now = uptime_ms();
    for (name, evicted) in [
        ("Whisper", whisper::evict_if_idle(max_idle, now)),
        ("Nemotron", nemotron::evict_if_idle(max_idle, now)),
        ("e5", embedder::evict_if_idle(max_idle, now)),
        ("Supertonic", crate::modules::voice::tts::providers::supertonic::evict_if_idle(max_idle, now)),
    ] {
        if evicted {
            log::info!("modelo {name} descarregado após {} min sem uso", max_idle.as_secs() / 60);
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![]
}

#[cfg(test)]
mod lease_tests {
    use super::*;

    fn lease_at(ms: u64) -> Lease {
        Lease(AtomicU64::new(ms))
    }

    #[test]
    fn a_recently_used_model_stays_loaded() {
        let cache = Mutex::new(Some(1));
        assert!(!evict_if_idle(&cache, &lease_at(60_000), Duration::from_secs(300), 120_000));
        assert!(cache.lock().unwrap().is_some());
    }

    #[test]
    fn an_idle_model_is_dropped() {
        let cache = Mutex::new(Some(1));
        assert!(evict_if_idle(&cache, &lease_at(0), Duration::from_secs(300), 300_000));
        assert!(cache.lock().unwrap().is_none());
    }

    /// Supertonic's sessions live behind a `tokio` mutex (a load collapses concurrent callers), and
    /// they stayed in RAM/VRAM for the app's whole life while the other models were unloaded.
    #[test]
    fn an_idle_model_behind_an_async_lock_is_dropped_too_unless_in_use() {
        let cache = tokio::sync::Mutex::new(Some(1));
        assert!(!evict_async_if_idle(&cache, &lease_at(60_000), Duration::from_secs(300), 120_000));
        {
            let _in_use = cache.try_lock().unwrap();
            assert!(!evict_async_if_idle(&cache, &lease_at(0), Duration::from_secs(1), 3_600_000));
        }
        assert!(evict_async_if_idle(&cache, &lease_at(0), Duration::from_secs(300), 300_000));
        assert!(cache.try_lock().unwrap().is_none());
    }

    #[test]
    fn a_model_in_use_is_neither_waited_for_nor_dropped() {
        let cache = Mutex::new(Some(1));
        let _in_use = cache.lock().unwrap();
        assert!(!evict_if_idle(&cache, &lease_at(0), Duration::from_secs(1), 3_600_000));
    }
}
