//! Jogada da fala proativa — O QUE ela faz ao puxar assunto (piada, pergunta, discordar...),
//! sorteada em código com peso e cooldown por jogada. Os prompts já pediam "varie
//! obrigatoriamente", mas o modelo não lembra qual abordagem usou no turno anterior e repete a
//! mesma forma; a escolha é roteamento, então é código, não prompt.

use std::collections::HashMap;

use super::prompts;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Move {
    Comment,
    Question,
    Tease,
    Disagree,
    Complain,
    Ask,
    Confide,
}

struct Spec {
    mv: Move,
    key: &'static str,
    weight: u32,
    cooldown_ms: u64,
}

const MIN: u64 = 60_000;

/// `Comment` sem cooldown garante que um baralho novo sempre tem jogada; as raras (discordar,
/// confidência) têm peso baixo e cooldown longo para não virarem tique.
const DECK: [Spec; 7] = [
    Spec { mv: Move::Comment, key: "comment", weight: 4, cooldown_ms: 0 },
    Spec { mv: Move::Question, key: "question", weight: 3, cooldown_ms: 10 * MIN },
    Spec { mv: Move::Tease, key: "tease", weight: 3, cooldown_ms: 10 * MIN },
    Spec { mv: Move::Disagree, key: "disagree", weight: 1, cooldown_ms: 45 * MIN },
    Spec { mv: Move::Complain, key: "complain", weight: 1, cooldown_ms: 30 * MIN },
    Spec { mv: Move::Ask, key: "ask", weight: 1, cooldown_ms: 60 * MIN },
    Spec { mv: Move::Confide, key: "confide", weight: 1, cooldown_ms: 120 * MIN },
];

/// Só candidato de "puxar assunto" livre ganha jogada — reação a um fato (bateria, tela, evento)
/// tem conteúdo próprio; discordar de um aviso de bateria não faz sentido.
pub fn takes_a_move(candidate_key: &str) -> bool {
    matches!(candidate_key, "idle" | "game-sustained")
}

#[derive(Default)]
pub struct MoveDeck {
    last_used: HashMap<Move, u64>,
    last: Option<Move>,
}

impl MoveDeck {
    /// `roll` em `[0, 1)`. `None` quando toda jogada está em cooldown ou é a anterior.
    pub fn draw(&self, now_ms: u64, roll: f64) -> Option<Move> {
        let open: Vec<&Spec> = DECK.iter().filter(|spec| self.is_open(spec, now_ms)).collect();
        let total: u32 = open.iter().map(|spec| spec.weight).sum();
        let mut target = (roll.clamp(0.0, 1.0 - f64::EPSILON) * f64::from(total)) as u32;
        for spec in open {
            if target < spec.weight {
                return Some(spec.mv);
            }
            target -= spec.weight;
        }
        None
    }

    fn is_open(&self, spec: &Spec, now_ms: u64) -> bool {
        self.last != Some(spec.mv)
            && self.last_used.get(&spec.mv).is_none_or(|&at| now_ms.saturating_sub(at) >= spec.cooldown_ms)
    }

    /// Chamado só depois de uma fala que deu certo — falha não gasta a jogada.
    pub fn mark(&mut self, mv: Move, now_ms: u64) {
        self.last_used.insert(mv, now_ms);
        self.last = Some(mv);
    }
}

/// Anexa a instrução da jogada (`awareness/move-<key>.md`) ao prompt do candidato.
pub fn with_move(prompt: &str, mv: Move) -> Result<String, String> {
    let key = DECK.iter().find(|spec| spec.mv == mv).expect("todo Move tem Spec no DECK").key;
    Ok(format!("{prompt}\n\n{}", prompts::proactive_move(key)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draws(deck: &MoveDeck, now_ms: u64) -> Vec<Move> {
        (0..100).filter_map(|i| deck.draw(now_ms, f64::from(i) / 100.0)).collect()
    }

    #[test]
    fn a_fresh_deck_can_draw_every_move_in_proportion_to_its_weight() {
        let all = draws(&MoveDeck::default(), 0);
        assert_eq!(all.len(), 100, "baralho novo sempre tem jogada");
        let count = |mv| all.iter().filter(|&&m| m == mv).count();
        assert!(count(Move::Comment) > count(Move::Disagree), "peso maior sai mais");
        for spec in &DECK {
            assert!(count(spec.mv) > 0, "{} nunca sai", spec.key);
        }
    }

    #[test]
    fn the_previous_move_is_never_drawn_twice_in_a_row() {
        let mut deck = MoveDeck::default();
        deck.mark(Move::Comment, 0);
        assert!(!draws(&deck, 10 * 24 * 60 * MIN).contains(&Move::Comment));
    }

    #[test]
    fn a_used_move_waits_out_its_own_cooldown() {
        let mut deck = MoveDeck::default();
        deck.mark(Move::Disagree, 0);
        deck.mark(Move::Comment, 1); // tira `Disagree` da regra de "não repetir a anterior"
        assert!(!draws(&deck, 45 * MIN - 1).contains(&Move::Disagree));
        assert!(draws(&deck, 45 * MIN).contains(&Move::Disagree));
    }

    #[test]
    fn only_free_conversation_candidates_take_a_move() {
        assert!(takes_a_move("idle"));
        assert!(takes_a_move("game-sustained"));
        assert!(!takes_a_move("hardware"));
        assert!(!takes_a_move("screen"));
    }

    #[test]
    fn every_move_has_a_prompt_that_keeps_the_candidate_prompt() {
        for spec in &DECK {
            let composed = with_move("BASE", spec.mv).unwrap_or_else(|e| panic!("{}: {e}", spec.key));
            assert!(composed.starts_with("BASE"), "{}", spec.key);
            assert!(composed.contains("[THIS TURN'S MOVE]"), "{}", spec.key);
        }
    }
}
