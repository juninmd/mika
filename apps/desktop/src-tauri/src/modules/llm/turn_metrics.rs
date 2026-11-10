//! Latência dos últimos turnos de chat (tempo até o primeiro token, até o primeiro áudio e total),
//! num anel de [`CAPACITY`] amostras que `llm:getStatus` devolve e `llm:status` empurra à tela.
//!
//! Antes só existia a LINHA de log (`brain::turn` e `voice::auto_speak::turn_timing`): o número
//! nascia e morria no buffer de log, e a aba LLM não tinha como dizer "a Mika está lenta hoje".
//! As três marcas datam do mesmo instante (`chat:thinking-start`), então conversam entre si.
//!
//! Estado de processo (`static`), como `provider_health`: um turno é um turno do app inteiro, e os
//! três pontos que marcam (`brain::turn` e o worker de fala) não compartilham dono.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::{json, Value};

pub const CAPACITY: usize = 20;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct TurnSample {
    pub at_ms: u64,
    pub first_token_ms: Option<u64>,
    pub first_audio_ms: Option<u64>,
    pub total_ms: Option<u64>,
}

#[derive(Default)]
pub struct Ring {
    samples: VecDeque<TurnSample>,
}

#[derive(Clone, Copy)]
pub enum Mark {
    FirstToken,
    FirstAudio,
    Total,
}

impl Ring {
    pub fn begin(&mut self, at_ms: u64) {
        if self.samples.len() == CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(TurnSample { at_ms, ..TurnSample::default() });
    }

    /// Só a primeira marca de cada tipo vale, e só no turno aberto — uma marca sem turno nenhum
    /// (fala solta, fora de chat) não inventa amostra. Devolve se algo mudou.
    pub fn mark(&mut self, mark: Mark, ms: u64) -> bool {
        let Some(last) = self.samples.back_mut() else { return false };
        let slot = match mark {
            Mark::FirstToken => &mut last.first_token_ms,
            Mark::FirstAudio => &mut last.first_audio_ms,
            Mark::Total => &mut last.total_ms,
        };
        if slot.is_some() {
            return false;
        }
        *slot = Some(ms);
        true
    }

    pub fn to_json(&self) -> Value {
        let series = |pick: fn(&TurnSample) -> Option<u64>| {
            let values: Vec<u64> = self.samples.iter().filter_map(pick).collect();
            json!({ "last": values.last(), "median": median(&values), "count": values.len() })
        };
        let samples: Vec<Value> = self
            .samples
            .iter()
            .map(|s| json!({ "atMs": s.at_ms, "firstTokenMs": s.first_token_ms, "firstAudioMs": s.first_audio_ms, "totalMs": s.total_ms }))
            .collect();
        json!({
            "samples": samples,
            "firstToken": series(|s| s.first_token_ms),
            "firstAudio": series(|s| s.first_audio_ms),
            "total": series(|s| s.total_ms),
        })
    }
}

pub fn median(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 0 { (sorted[mid - 1] + sorted[mid]) / 2 } else { sorted[mid] })
}

static RING: Mutex<Ring> = Mutex::new(Ring { samples: VecDeque::new() });

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// `chat:thinking-start` do lado de `brain::turn`: abre a amostra do turno.
pub fn begin_turn() {
    if let Ok(mut ring) = RING.lock() {
        ring.begin(now_ms());
    }
}

pub fn record(mark: Mark, ms: u64) {
    let changed = RING.lock().map(|mut ring| ring.mark(mark, ms)).unwrap_or(false);
    if changed {
        super::status::notify();
    }
}

pub fn snapshot() -> Value {
    RING.lock().map(|ring| ring.to_json()).unwrap_or_else(|_| Ring::default().to_json())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_odd_even_and_empty() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[300, 100, 200]), Some(200));
        assert_eq!(median(&[100, 400, 200, 300]), Some(250));
    }

    #[test]
    fn a_mark_without_an_open_turn_invents_nothing() {
        let mut ring = Ring::default();
        assert!(!ring.mark(Mark::FirstAudio, 120));
        assert_eq!(ring.to_json()["samples"], json!([]));
    }

    #[test]
    fn only_the_first_mark_of_each_kind_counts_per_turn() {
        let mut ring = Ring::default();
        ring.begin(1);
        assert!(ring.mark(Mark::FirstToken, 300));
        assert!(!ring.mark(Mark::FirstToken, 900), "segundo token do mesmo turno não é o primeiro");
        assert!(ring.mark(Mark::Total, 2_000));
        assert!(ring.mark(Mark::FirstAudio, 700), "o áudio pode chegar depois do fim do texto");
        let out = ring.to_json();
        assert_eq!(out["firstToken"], json!({ "last": 300, "median": 300, "count": 1 }));
        assert_eq!(out["firstAudio"]["last"], 700);
        assert_eq!(out["total"]["last"], 2_000);
    }

    #[test]
    fn the_ring_keeps_only_the_last_capacity_turns_and_summarizes_them() {
        let mut ring = Ring::default();
        for i in 0..(CAPACITY as u64 + 5) {
            ring.begin(i);
            ring.mark(Mark::FirstToken, i * 10);
        }
        let out = ring.to_json();
        assert_eq!(out["samples"].as_array().unwrap().len(), CAPACITY);
        assert_eq!(out["samples"][0]["atMs"], 5, "as 5 mais antigas saíram");
        assert_eq!(out["firstToken"]["last"], (CAPACITY as u64 + 4) * 10);
        assert_eq!(out["firstToken"]["count"], CAPACITY);
    }
}
