//! Escolha do vencedor de um tick — port de `runProactiveTick` (`proactive-tick.ts`), separado do
//! laço de I/O (`engine.rs`) para ficar puro e testável sem providers de verdade.

use super::types::ProactiveCandidate;

/// Entre os candidatos que PASSARAM pelo `shouldSpeak` do chamador, o de maior prioridade vence —
/// empate resolvido pelo primeiro da lista (ordem de registro dos providers), igual ao `for` do TS
/// (`if (candidate.priority > bestPriority)`, estritamente maior).
pub fn pick_best(candidates: Vec<ProactiveCandidate>) -> Option<ProactiveCandidate> {
    let mut best: Option<ProactiveCandidate> = None;
    for candidate in candidates {
        let wins = match &best {
            None => true,
            Some(current) => candidate.priority > current.priority,
        };
        if wins {
            best = Some(candidate);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(key: &str, priority: u8) -> ProactiveCandidate {
        ProactiveCandidate { key: key.into(), priority, label: key.into(), prompt: "oi".into() }
    }

    #[test]
    fn no_candidates_means_no_winner() {
        assert_eq!(pick_best(vec![]), None);
    }

    #[test]
    fn highest_priority_wins() {
        let winner = pick_best(vec![candidate("idle", 3), candidate("game", 7), candidate("hardware", 9)]);
        assert_eq!(winner.map(|c| c.key), Some("hardware".to_string()));
    }

    #[test]
    fn a_tie_keeps_the_first_registered_provider() {
        let winner = pick_best(vec![candidate("first", 5), candidate("second", 5)]);
        assert_eq!(winner.map(|c| c.key), Some("first".to_string()));
    }
}
