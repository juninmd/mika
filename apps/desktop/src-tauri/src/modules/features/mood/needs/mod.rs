//! Necessidades do corpo simulado da Mika — sono, sede, fome e tédio (0..1).
//!
//! **Sem relógio de fundo para os valores.** Só quatro instantes são guardados ([`BodyClock`]:
//! acordou, bebeu, comeu, conversou) e cada necessidade é calculada do tempo decorrido na hora em
//! que alguém pergunta ([`needs_at`]) — reiniciar o app não perde nada e nada precisa "tickar".
//! O único batimento é [`BodyClock::observe`] (boot e a cada poucos minutos, em [`runtime`]): um
//! buraco longo desde a última vez que o app estava aberto, ou o usuário longe do PC por horas, conta
//! como ela ter dormido.
//!
//! O efeito é INFORMAÇÃO, nunca fala pronta: uma linha curta no prompt ([`prompt_line`], texto em
//! `prompts/brain/body-state.md`), um bocejo no avatar pela tool `avatar` já existente, e um
//! candidato proativo de baixa prioridade (`brain::awareness::body_need`). O que ela diz sobre
//! isso é do LLM (AGENTS.md §2.6).

pub(crate) mod runtime;
mod tool;

pub(crate) use tool::register;

use serde::{Deserialize, Serialize};

pub const HOUR_MS: u64 = 3_600_000;
const MINUTE_MS: u64 = 60_000;
/// App fechado (ou usuário longe do PC) por este tempo = ela dormiu.
pub const SLEEP_GAP_MS: u64 = 4 * HOUR_MS;
/// Horas acordada até o sono de cansaço chegar a 1.
const AWAKE_FULL_H: f64 = 18.0;
const THIRST_FULL_H: f64 = 5.0;
const HUNGER_FULL_H: f64 = 7.0;
/// Tédio começa depois deste silêncio e satura em `BOREDOM_FULL_MIN` a mais.
const BOREDOM_START_MIN: f64 = 10.0;
const BOREDOM_FULL_MIN: f64 = 90.0;
/// Ao acordar ela já tem um pouco de sede e fome (≈0.4), não zero nem cheia.
const WAKE_THIRST_MS: u64 = 2 * HOUR_MS;
const WAKE_HUNGER_MS: u64 = 3 * HOUR_MS;

/// Os quatro instantes que definem o corpo — o que é persistido.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyClock {
    pub woke_at_ms: u64,
    pub drank_at_ms: u64,
    pub ate_at_ms: u64,
    pub talked_at_ms: u64,
    pub last_seen_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Sleep,
    Drink,
    Eat,
    Play,
}

impl Need {
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw.trim().to_lowercase().as_str() {
            "sleep" | "rest" | "nap" => Self::Sleep,
            "drink" | "water" | "thirst" => Self::Drink,
            "eat" | "food" | "snack" | "hunger" => Self::Eat,
            "play" | "fun" | "boredom" => Self::Play,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct Needs {
    pub sleepiness: f64,
    pub thirst: f64,
    pub hunger: f64,
    pub boredom: f64,
}

fn hours(ms: u64) -> f64 {
    ms as f64 / HOUR_MS as f64
}

impl BodyClock {
    pub fn fresh(now_ms: u64) -> Self {
        Self { woke_at_ms: now_ms, drank_at_ms: now_ms, ate_at_ms: now_ms, talked_at_ms: now_ms, last_seen_ms: now_ms }
    }

    fn wake(self, now_ms: u64) -> Self {
        Self {
            woke_at_ms: now_ms,
            drank_at_ms: self.drank_at_ms.max(now_ms.saturating_sub(WAKE_THIRST_MS)),
            ate_at_ms: self.ate_at_ms.max(now_ms.saturating_sub(WAKE_HUNGER_MS)),
            talked_at_ms: now_ms,
            last_seen_ms: now_ms,
        }
    }

    /// Batimento: devolve o relógio atualizado e se ela "acordou" agora (app fechado ou usuário
    /// longe por [`SLEEP_GAP_MS`]).
    pub fn observe(self, now_ms: u64, user_idle_ms: u64) -> (Self, bool) {
        let gap = now_ms.saturating_sub(self.last_seen_ms);
        if gap >= SLEEP_GAP_MS || user_idle_ms >= SLEEP_GAP_MS {
            return (self.wake(now_ms), true);
        }
        (Self { last_seen_ms: now_ms, ..self }, false)
    }

    pub fn satisfy(self, need: Need, now_ms: u64) -> Self {
        match need {
            Need::Sleep => Self { woke_at_ms: now_ms, ..self },
            Need::Drink => Self { drank_at_ms: now_ms, ..self },
            Need::Eat => Self { ate_at_ms: now_ms, ..self },
            Need::Play => Self { talked_at_ms: now_ms, ..self },
        }
    }

    pub fn talked(self, now_ms: u64) -> Self {
        Self { talked_at_ms: now_ms, last_seen_ms: now_ms, ..self }
    }

    pub fn awake_hours(&self, now_ms: u64) -> f64 {
        hours(now_ms.saturating_sub(self.woke_at_ms))
    }
}

/// Madrugada pesa (pico às 3h), a sesta pós-almoço um pouco; o resto do dia, nada.
fn circadian(hour: f64) -> f64 {
    let from_three = (hour - 3.0).rem_euclid(24.0).min((3.0 - hour).rem_euclid(24.0));
    let night = (0.45 * (1.0 - from_three / 4.0)).max(0.0);
    let siesta = if (13.0..15.0).contains(&hour) { 0.1 } else { 0.0 };
    night + siesta
}

fn unit(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// As quatro necessidades agora. `hour` é a hora local do usuário (fracionária), `intensity` o
/// multiplicador da Settings (0 = corpo desligado, 2 = dobro).
pub fn needs_at(clock: &BodyClock, now_ms: u64, hour: f64, intensity: f64) -> Needs {
    let since = |at: u64| now_ms.saturating_sub(at);
    let silent_min = since(clock.talked_at_ms) as f64 / MINUTE_MS as f64;
    let raw = Needs {
        sleepiness: clock.awake_hours(now_ms) / AWAKE_FULL_H + circadian(hour),
        thirst: hours(since(clock.drank_at_ms)) / THIRST_FULL_H,
        hunger: hours(since(clock.ate_at_ms)) / HUNGER_FULL_H,
        boredom: (silent_min - BOREDOM_START_MIN).max(0.0) / BOREDOM_FULL_MIN,
    };
    let k = intensity.max(0.0);
    Needs { sleepiness: unit(raw.sleepiness * k), thirst: unit(raw.thirst * k), hunger: unit(raw.hunger * k), boredom: unit(raw.boredom * k) }
}

/// Abaixo disso a necessidade não vale uma palavra no prompt.
pub const NOTICEABLE: f64 = 0.5;
pub const STRONG: f64 = 0.75;

/// `sleepy (strong), thirsty (mild)` — só o que passa de [`NOTICEABLE`], mais forte primeiro; vazio
/// quando nada pesa (o bloco some do prompt).
pub fn prompt_line(needs: &Needs) -> String {
    let mut felt: Vec<(&str, f64)> = [("sleepy", needs.sleepiness), ("thirsty", needs.thirst), ("hungry", needs.hunger), ("bored", needs.boredom)]
        .into_iter()
        .filter(|(_, v)| *v >= NOTICEABLE)
        .collect();
    felt.sort_by(|a, b| b.1.total_cmp(&a.1));
    felt.iter().map(|(word, v)| format!("{word} ({})", if *v >= STRONG { "strong" } else { "mild" })).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_000 * HOUR_MS;
    const NOON: f64 = 11.0;

    #[test]
    fn a_fresh_body_needs_nothing_at_midday() {
        let n = needs_at(&BodyClock::fresh(T0), T0, NOON, 1.0);
        assert_eq!(n, Needs::default());
        assert_eq!(prompt_line(&n), "");
    }

    #[test]
    fn thirst_and_hunger_rise_with_time_and_reset_when_satisfied() {
        let clock = BodyClock::fresh(T0);
        let later = T0 + 4 * HOUR_MS;
        let n = needs_at(&clock, later, NOON, 1.0);
        assert!((n.thirst - 0.8).abs() < 1e-9, "{n:?}");
        assert!(n.hunger > 0.5 && n.hunger < n.thirst, "fome sobe mais devagar que sede: {n:?}");

        let drank = clock.satisfy(Need::Drink, later);
        assert_eq!(needs_at(&drank, later, NOON, 1.0).thirst, 0.0);
        assert!(needs_at(&drank, later, NOON, 1.0).hunger > 0.5, "beber não mata a fome");
    }

    #[test]
    fn sleepiness_grows_with_time_awake_and_is_heavier_late_at_night() {
        let clock = BodyClock::fresh(T0);
        let day = needs_at(&clock, T0 + 9 * HOUR_MS, NOON, 1.0).sleepiness;
        let night = needs_at(&clock, T0 + 9 * HOUR_MS, 3.0, 1.0).sleepiness;
        assert!((day - 0.5).abs() < 1e-9);
        assert!(night > day + 0.4, "3h da manhã pesa: {night} vs {day}");
        assert!(needs_at(&clock, T0, 23.5, 1.0).sleepiness > 0.0 && needs_at(&clock, T0, 9.0, 1.0).sleepiness == 0.0);
    }

    #[test]
    fn the_app_closed_overnight_counts_as_sleep_and_she_wakes_a_bit_thirsty() {
        let tired = BodyClock::fresh(T0);
        let morning = T0 + 20 * HOUR_MS;
        let (woke, did_wake) = tired.observe(morning, 0);
        assert!(did_wake);
        let n = needs_at(&woke, morning, 9.0, 1.0);
        assert_eq!(n.sleepiness, 0.0);
        assert!((n.thirst - 0.4).abs() < 1e-9 && (n.hunger - 3.0 / 7.0).abs() < 1e-9, "{n:?}");
        assert_eq!(n.boredom, 0.0);
    }

    #[test]
    fn a_short_gap_is_not_sleep_but_a_long_user_absence_is() {
        let clock = BodyClock::fresh(T0);
        let (still, woke) = clock.observe(T0 + HOUR_MS, 0);
        assert!(!woke);
        assert_eq!(still.woke_at_ms, T0);
        assert_eq!(still.last_seen_ms, T0 + HOUR_MS);

        let (_, woke) = still.observe(T0 + 2 * HOUR_MS, SLEEP_GAP_MS);
        assert!(woke, "usuário longe do PC por horas: ela cochilou");
    }

    #[test]
    fn boredom_rises_after_a_quiet_stretch_and_a_chat_resets_it() {
        let clock = BodyClock::fresh(T0);
        let quiet = T0 + 100 * MINUTE_MS;
        assert!((needs_at(&clock, quiet, NOON, 1.0).boredom - 1.0).abs() < 1e-9);
        assert_eq!(needs_at(&clock.talked(quiet), quiet, NOON, 1.0).boredom, 0.0);
    }

    #[test]
    fn intensity_scales_every_need_and_zero_turns_the_body_off() {
        let clock = BodyClock::fresh(T0);
        let later = T0 + 2 * HOUR_MS;
        let normal = needs_at(&clock, later, NOON, 1.0);
        let double = needs_at(&clock, later, NOON, 2.0);
        assert!((double.thirst - 2.0 * normal.thirst).abs() < 1e-9);
        assert_eq!(needs_at(&clock, T0 + 30 * HOUR_MS, 3.0, 0.0), Needs::default());
    }

    #[test]
    fn the_prompt_line_names_only_what_is_felt_strongest_first() {
        let n = Needs { sleepiness: 0.6, thirst: 0.9, hunger: 0.2, boredom: 0.49 };
        assert_eq!(prompt_line(&n), "thirsty (strong), sleepy (mild)");
    }

    #[test]
    fn need_names_from_the_tool_and_the_ui_parse() {
        assert_eq!(Need::parse("Water"), Some(Need::Drink));
        assert_eq!(Need::parse("nap"), Some(Need::Sleep));
        assert_eq!(Need::parse("snack"), Some(Need::Eat));
        assert_eq!(Need::parse("fly"), None);
    }
}
