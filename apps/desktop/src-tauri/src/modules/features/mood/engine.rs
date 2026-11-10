//! Motor de humor e afinidade — estado de processo sobre as funções puras de `math.rs`, port de
//! `MoodService`/`AffinityService` (`packages/mood/src/services/*.service.ts`). Ver o cabeçalho de
//! `mod.rs` para o que ainda diverge do TS. [`MoodEngine::tick`] é o que
//! [`super::boot::spawn_decay_timer`] chama a cada 60s — a mesma matemática de [`super::math::tick`],
//! com o `Mutex` que dá estado a ela.

use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use super::math::{affinity_level_for, affinity_label_for, clamp_percent, clamp_unit, label_for, DEFAULT_GAIN_RATE};

struct MoodInner {
    valence: f64,
    arousal: f64,
}

impl MoodInner {
    fn new() -> Self {
        Self { valence: 0.0, arousal: 0.0 }
    }

    fn snapshot(&self) -> Value {
        json!({ "valence": self.valence, "arousal": self.arousal, "label": label_for(self.valence, self.arousal) })
    }
}

struct AffinityInner {
    score: f64,
    gain_rate: f64,
}

impl AffinityInner {
    fn new() -> Self {
        Self { score: 0.0, gain_rate: DEFAULT_GAIN_RATE }
    }

    fn snapshot(&self) -> Value {
        json!({
            "level": affinity_level_for(self.score),
            "label": affinity_label_for(self.score),
            "score": self.score,
        })
    }

    /// Espelha `addInteraction`. Devolve `(snapshot, level_up)`.
    fn add_interaction(&mut self, weight: f64) -> (Value, bool) {
        let previous_level = affinity_level_for(self.score);
        let gain = self.gain_rate * weight;
        self.score = clamp_percent(self.score + gain);
        let level_up = affinity_level_for(self.score) > previous_level;
        (self.snapshot(), level_up)
    }
}

/// Um por processo, como `crate::modules::tools::registry()` — ver item 1 do cabeçalho de
/// `mod.rs` para o porquê de não ser por ativação.
pub(super) struct MoodEngine {
    mood: Mutex<MoodInner>,
    affinity: Mutex<AffinityInner>,
}

impl MoodEngine {
    fn new() -> Self {
        Self { mood: Mutex::new(MoodInner::new()), affinity: Mutex::new(AffinityInner::new()) }
    }

    pub(super) fn get(&self) -> Value {
        let mood = self.mood.lock().unwrap_or_else(|e| e.into_inner());
        let affinity = self.affinity.lock().unwrap_or_else(|e| e.into_inner());
        json!({
            "success": true,
            "data": { "mood": mood.snapshot(), "affinity": affinity.snapshot() },
            "affinityPersisted": false,
        })
    }

    /// Só a fatia de humor (valência/excitação) — usada por [`super::persistence`], que substitui a
    /// afinidade em memória por `character.affinityScore` mas continua sobre ESTE motor para
    /// `update`/`decay` (item 1 do cabeçalho de `mod.rs`: sem `AppHandle`/timer, o humor
    /// valência/excitação nunca foi persistido nem pelo TS — só a afinidade é).
    pub(super) fn mood_snapshot(&self) -> Value {
        let mood = self.mood.lock().unwrap_or_else(|e| e.into_inner());
        mood.snapshot()
    }

    pub(super) fn update(&self, valence: f64, arousal: f64) -> Value {
        let mut mood = self.mood.lock().unwrap_or_else(|e| e.into_inner());
        mood.valence = clamp_unit(valence);
        mood.arousal = clamp_unit(arousal);
        json!({ "success": true, "data": mood.snapshot() })
    }

    /// Espelha `MoodService.tick` — usado pelo timer de boot ([`super::boot::spawn_decay_timer`]),
    /// não pela tool `mood` (que só tem `action=decay`, um fator explícito). Decai `valence`/
    /// `arousal` pelo tempo REAL passado, à taxa `decay_rate` — mesma matemática de [`super::math::tick`].
    pub(super) fn tick(&self, decay_rate: f64, seconds_elapsed: f64) -> Value {
        let mut mood = self.mood.lock().unwrap_or_else(|e| e.into_inner());
        let (valence, arousal) = super::math::tick(mood.valence, mood.arousal, decay_rate, seconds_elapsed);
        mood.valence = valence;
        mood.arousal = arousal;
        mood.snapshot()
    }

    /// Espelha `MoodService.decay` (via `MoodInstance.decay` em `mood.runtime.ts`): reduz o estado
    /// ATUAL por um fator explícito — não é o `tick()` periódico (ver item 3 do cabeçalho de
    /// `mod.rs`).
    pub(super) fn decay(&self, factor: f64) -> Value {
        let mut mood = self.mood.lock().unwrap_or_else(|e| e.into_inner());
        mood.valence = clamp_unit(mood.valence * (1.0 - factor));
        mood.arousal = clamp_unit(mood.arousal * (1.0 - factor));
        json!({ "success": true, "data": { "mood": mood.snapshot(), "decayApplied": true } })
    }

    pub(super) fn register_interaction(&self, weight: f64, reason: Option<&str>) -> Value {
        let mut affinity = self.affinity.lock().unwrap_or_else(|e| e.into_inner());
        let (snapshot, level_up) = affinity.add_interaction(weight);
        let mut data = snapshot;
        if let Some(obj) = data.as_object_mut() {
            if let Some(reason) = reason {
                obj.insert("reason".into(), json!(reason));
            }
            obj.insert("levelUp".into(), json!(level_up));
        }
        json!({ "success": true, "data": data, "affinityPersisted": false })
    }
}

static ENGINE: OnceLock<MoodEngine> = OnceLock::new();

/// Singleton de processo, consumido pela tool (`super::tool`) e pelos testes daqui.
pub(super) fn engine() -> &'static MoodEngine {
    ENGINE.get_or_init(MoodEngine::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_interaction_reports_level_up_only_when_it_crosses_a_boundary() {
        let mut affinity = AffinityInner { score: 19.0, gain_rate: 1.0 };
        let (_, level_up) = affinity.add_interaction(1.0);
        assert!(level_up, "19 + 1 = 20 cruza para 'known'");

        let (_, level_up) = affinity.add_interaction(0.5);
        assert!(!level_up, "20.5 continua em 'known'");
    }

    /// NEGATIVO — pontuação nunca sai de [0, 100] mesmo com ganho agressivo.
    #[test]
    fn affinity_score_is_clamped_to_percent_range() {
        let mut affinity = AffinityInner { score: 99.0, gain_rate: 50.0 };
        affinity.add_interaction(5.0);
        assert_eq!(affinity.score, 100.0);
    }

    #[test]
    fn engine_get_reports_affinity_as_not_persisted() {
        let engine = MoodEngine::new();
        let result = engine.get();
        assert_eq!(result["affinityPersisted"], json!(false));
        assert_eq!(result["data"]["mood"]["label"], json!("neutral"));
    }

    #[test]
    fn engine_update_clamps_to_the_unit_range() {
        let engine = MoodEngine::new();
        let result = engine.update(5.0, -5.0);
        assert_eq!(result["data"]["valence"], json!(1.0));
        assert_eq!(result["data"]["arousal"], json!(-1.0));
    }

    #[test]
    fn engine_tick_decays_by_the_configured_rate_and_elapsed_seconds() {
        let engine = MoodEngine::new();
        engine.update(0.5, 0.8);
        let result = engine.tick(0.02, 60.0);
        assert!((result["arousal"].as_f64().unwrap() - 0.78).abs() < 1e-9, "got: {result}");
        assert!((result["valence"].as_f64().unwrap() - 0.49).abs() < 1e-9, "got: {result}");
    }

    /// NEGATIVO — `tick()` nunca ultrapassa zero, mesmo com decaimento agressivo acumulado.
    #[test]
    fn engine_tick_never_overshoots_past_zero() {
        let engine = MoodEngine::new();
        engine.update(0.01, 0.01);
        let result = engine.tick(0.02, 6000.0);
        assert_eq!(result["valence"], json!(0.0));
        assert_eq!(result["arousal"], json!(0.0));
    }

    #[test]
    fn engine_decay_reduces_toward_neutral_by_the_given_factor() {
        let engine = MoodEngine::new();
        engine.update(1.0, 1.0);
        let result = engine.decay(0.5);
        assert_eq!(result["data"]["mood"]["valence"], json!(0.5));
        assert_eq!(result["data"]["decayApplied"], json!(true));
    }

    #[test]
    fn engine_register_interaction_carries_the_optional_reason() {
        let engine = MoodEngine::new();
        let result = engine.register_interaction(1.0, Some("milestone"));
        assert_eq!(result["data"]["reason"], json!("milestone"));
        assert_eq!(result["affinityPersisted"], json!(false));
    }
}
