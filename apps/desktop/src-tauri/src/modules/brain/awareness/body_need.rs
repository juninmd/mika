//! Candidato proativo do corpo da Mika (`features::mood::needs`): sede, fome ou sono FORTES viram
//! uma deixa de baixa prioridade — a cadência normal decide se ela fala, e o horário silencioso a
//! barra (a madrugada é do `bedtime.rs`, prioridade 9). Um aviso por necessidade a cada
//! [`BODY_RENUDGE_MS`]; o texto é do LLM (`awareness/body-need.md`), nunca pronto.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::prompts;
use super::types::{ProactiveCandidate, ProactiveProvider};
use crate::config::store::ConfigStore;
use crate::modules::features::mood::needs::{runtime::current_needs, Needs, STRONG};

pub const BODY_RENUDGE_MS: u64 = 90 * 60 * 1000;
const BODY_PRIORITY: u8 = 2;

/// A necessidade forte mais urgente que ainda não foi avisada na janela — pura.
pub fn body_need_due(needs: &Needs, last_by_need: &HashMap<&'static str, u64>, now_ms: u64) -> Option<&'static str> {
    let mut felt = [("sleepy", needs.sleepiness), ("thirsty", needs.thirst), ("hungry", needs.hunger)];
    felt.sort_by(|a, b| b.1.total_cmp(&a.1));
    felt.into_iter()
        .filter(|(_, v)| *v >= STRONG)
        .map(|(need, _)| need)
        .find(|need| last_by_need.get(need).is_none_or(|last| now_ms.saturating_sub(*last) >= BODY_RENUDGE_MS))
}

pub struct BodyNeedProvider {
    nickname: String,
    store: Arc<ConfigStore>,
    last_by_need: Mutex<HashMap<&'static str, u64>>,
}

impl BodyNeedProvider {
    pub fn new(nickname: String, store: Arc<ConfigStore>) -> Self {
        Self { nickname, store, last_by_need: Mutex::default() }
    }
}

impl ProactiveProvider for BodyNeedProvider {
    fn id(&self) -> &'static str {
        "body-need"
    }

    fn due(&self, now_ms: u64) -> Option<ProactiveCandidate> {
        let needs = current_needs(&self.store)?;
        let mut last = self.last_by_need.lock().unwrap_or_else(|e| e.into_inner());
        let need = body_need_due(&needs, &last, now_ms)?;
        last.insert(need, now_ms);
        let prompt = prompts::body_need(&self.nickname, need)
            .inspect_err(|error| log::error!("provider proativo 'body-need': prompt não carregou: {error}"))
            .ok()?;
        Some(ProactiveCandidate { key: format!("body:{need}"), priority: BODY_PRIORITY, label: "body need".into(), prompt })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 10 * 60 * 60 * 1000;

    #[test]
    fn only_a_strong_need_is_offered_the_most_urgent_first() {
        let needs = Needs { sleepiness: 0.8, thirst: 0.9, hunger: 0.6, boredom: 1.0 };
        assert_eq!(body_need_due(&needs, &HashMap::new(), NOW), Some("thirsty"));
        assert_eq!(body_need_due(&Needs { thirst: 0.7, ..Needs::default() }, &HashMap::new(), NOW), None);
    }

    #[test]
    fn a_need_already_mentioned_waits_its_window_and_the_next_one_goes_instead() {
        let needs = Needs { sleepiness: 0.8, thirst: 0.9, ..Needs::default() };
        let last = HashMap::from([("thirsty", NOW - BODY_RENUDGE_MS + 1)]);
        assert_eq!(body_need_due(&needs, &last, NOW), Some("sleepy"));
        let both = HashMap::from([("thirsty", NOW - 1), ("sleepy", NOW - 1)]);
        assert_eq!(body_need_due(&needs, &both, NOW), None);
    }

    /// Baixa prioridade: o horário silencioso barra (a madrugada fica com `bedtime`).
    #[test]
    fn quiet_hours_silence_it() {
        let empty = HashMap::new();
        let state = super::super::gates::CadenceState { last_spoke_at: 0, last_for_key: &empty, hourly_speak_timestamps: &[], base_interval_ms: 120_000 };
        let candidate = ProactiveCandidate { key: "body:thirsty".into(), priority: BODY_PRIORITY, label: String::new(), prompt: String::new() };
        assert!(!super::super::gates::passes_cadence(&state, &candidate, NOW, true, 50.0));
        assert!(super::super::gates::passes_cadence(&state, &candidate, NOW, false, 50.0));
    }

    #[test]
    fn the_prompt_names_the_user_and_the_need() {
        let prompt = prompts::body_need("Ana", "thirsty").expect("awareness/body-need.md");
        assert!(prompt.contains("Ana") && prompt.contains("thirsty"), "{prompt}");
    }
}
