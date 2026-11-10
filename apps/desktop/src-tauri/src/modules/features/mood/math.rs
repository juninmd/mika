//! Funções puras de humor/afinidade — port de `MoodService`/`AffinityService`
//! (`packages/mood/src/services/*.service.ts`). Sem mutex, sem relógio, sem `Value`: só números e
//! rótulos, testáveis em isolamento. `engine.rs` é quem dá estado a isso.

/// Espelha `LABEL_THRESHOLDS` (`mood.service.ts`): ordem importa, o primeiro teste que casa vence.
pub(super) fn label_for(valence: f64, arousal: f64) -> &'static str {
    if valence > 0.3 && arousal > 0.3 {
        return "excited";
    }
    if valence > 0.3 && arousal < -0.3 {
        return "content";
    }
    if valence > 0.3 {
        return "happy";
    }
    if arousal > 0.3 && valence < -0.3 {
        return "tense";
    }
    if arousal < -0.3 && valence >= -0.3 {
        return "bored";
    }
    if valence < -0.3 {
        return "sad";
    }
    "neutral"
}

pub(super) fn clamp_unit(value: f64) -> f64 {
    value.clamp(-1.0, 1.0)
}

pub(super) fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

/// Port puro de `MoodService.tick` — chamada por [`super::engine::MoodEngine::tick`], que
/// [`super::boot::spawn_decay_timer`] agenda a cada 60s (ver item 4 do cabeçalho de `mod.rs`:
/// "RESOLVIDO nesta rodada — o timer de decaimento roda").
pub(super) fn tick(valence: f64, arousal: f64, decay_rate: f64, seconds_elapsed: f64) -> (f64, f64) {
    let decay_factor = decay_rate * (seconds_elapsed / 60.0);

    let arousal = if arousal > 0.0 {
        (arousal - decay_factor).max(0.0)
    } else if arousal < 0.0 {
        (arousal + decay_factor).min(0.0)
    } else {
        arousal
    };

    let valence_decay = decay_factor * 0.5;
    let valence = if valence > 0.0 {
        (valence - valence_decay).max(0.0)
    } else if valence < 0.0 {
        (valence + valence_decay).min(0.0)
    } else {
        valence
    };

    (valence, arousal)
}

/// Espelha `decayRate` default de `createMood` (`mood.runtime.ts`). Ver o cabeçalho de `mod.rs`
/// para o porquê deste valor não vir de config.
pub(super) const DEFAULT_DECAY_RATE: f64 = 0.02;
/// Espelha `gainRate` default.
pub(super) const DEFAULT_GAIN_RATE: f64 = 1.0;

/// Espelha `LEVELS` (`affinity.service.ts`): limiar mínimo de pontuação por nível, crescente.
pub(super) const AFFINITY_LEVELS: [(f64, &str); 5] =
    [(0.0, "new"), (20.0, "known"), (40.0, "friend"), (60.0, "fan"), (80.0, "bestie")];

pub(super) fn affinity_level_for(score: f64) -> usize {
    for (index, (threshold, _)) in AFFINITY_LEVELS.iter().enumerate().rev() {
        if score >= *threshold {
            return index + 1;
        }
    }
    1
}

pub(super) fn affinity_label_for(score: f64) -> &'static str {
    for (threshold, label) in AFFINITY_LEVELS.iter().rev() {
        if score >= *threshold {
            return label;
        }
    }
    "new"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_thresholds_match_the_typescript_order() {
        assert_eq!(label_for(0.5, 0.5), "excited");
        assert_eq!(label_for(0.5, -0.5), "content");
        assert_eq!(label_for(0.5, 0.0), "happy");
        assert_eq!(label_for(-0.5, 0.5), "tense");
        assert_eq!(label_for(0.0, -0.5), "bored");
        assert_eq!(label_for(-0.5, 0.0), "sad");
        assert_eq!(label_for(0.0, 0.0), "neutral");
    }

    #[test]
    fn tick_decays_positive_arousal_and_valence_toward_zero() {
        let (valence, arousal) = tick(0.5, 0.8, 0.02, 60.0);
        assert!((arousal - 0.78).abs() < 1e-9, "arousal={arousal}");
        assert!((valence - 0.49).abs() < 1e-9, "valence={valence}");
    }

    #[test]
    fn tick_decays_negative_arousal_and_valence_toward_zero() {
        let (valence, arousal) = tick(-0.5, -0.8, 0.02, 60.0);
        assert!((arousal - -0.78).abs() < 1e-9, "arousal={arousal}");
        assert!((valence - -0.49).abs() < 1e-9, "valence={valence}");
    }

    /// NEGATIVO — decaimento nunca cruza zero e vira o sinal oposto.
    #[test]
    fn tick_never_overshoots_past_zero() {
        let (valence, arousal) = tick(0.01, 0.01, 0.02, 6000.0);
        assert_eq!(valence, 0.0);
        assert_eq!(arousal, 0.0);
    }

    #[test]
    fn affinity_level_boundaries_match_the_typescript_table() {
        assert_eq!(affinity_level_for(0.0), 1);
        assert_eq!(affinity_level_for(19.9), 1);
        assert_eq!(affinity_level_for(20.0), 2);
        assert_eq!(affinity_level_for(99.9), 5);
        assert_eq!(affinity_level_for(100.0), 5);
        assert_eq!(affinity_label_for(20.0), "known");
        assert_eq!(affinity_label_for(80.0), "bestie");
    }

    #[test]
    fn every_affinity_label_has_a_card_label_in_the_renderer() {
        let card = include_str!("../../../../../src/features/mood/renderer/live-mood-card.tsx");
        for (_, label) in AFFINITY_LEVELS {
            assert!(card.contains(&format!("{label}:")), "live-mood-card.tsx não rotula o nível '{label}'");
        }
    }
}
