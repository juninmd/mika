//! Baralho, ranking e força de carta de truco — porte de `handlers/truco-deck.ts`. Puro, sem I/O.
//!
//! `Suit`/`Rank` ficam como `String`/`&'static str` em vez de `enum`: os valores do TS já são só
//! rótulos de exibição (`'ouros'`, `'4'`, `'Q'`...) sem comportamento associado, e um `enum` Rust não
//! aceitaria `'4'`/`'2'`/`'3'` como nome de variante — a representação mais simples que ainda casa
//! byte a byte com o JSON que o TS produz é a string crua.

use serde::Serialize;
use uuid::Uuid;

pub const RANK_ORDER: [&str; 10] = ["4", "5", "6", "7", "Q", "J", "K", "A", "2", "3"];
const SUITS: [&str; 4] = ["ouros", "espadas", "copas", "paus"];

fn suit_manilha_power(suit: &str) -> i32 {
    match suit {
        "ouros" => 11,
        "espadas" => 12,
        "copas" => 13,
        "paus" => 14,
        _ => 0,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrucoCard {
    pub id: String,
    pub rank: String,
    pub suit: String,
    pub name: String,
    pub power: i32,
    pub is_manilha: bool,
}

/// Porte de `getManilhaRank`: o rank seguinte ao da vira, cíclico.
pub fn get_manilha_rank(vira_rank: &str) -> String {
    let idx = RANK_ORDER.iter().position(|r| *r == vira_rank).unwrap_or(0);
    RANK_ORDER[(idx + 1) % RANK_ORDER.len()].to_string()
}

/// Porte de `computeCardPower`.
pub fn compute_card_power(rank: &str, suit: &str, manilha_rank: &str) -> (i32, bool) {
    if rank == manilha_rank {
        return (suit_manilha_power(suit), true);
    }
    let base = RANK_ORDER.iter().position(|r| *r == rank).map(|i| i as i32 + 1).unwrap_or(0);
    (base, false)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Porte de `createDeck`: as 40 cartas (10 ranks x 4 naipes), com `power`/`isManilha` já computados
/// contra `manilha_rank`. `id` usa um sufixo aleatório curto (via `uuid`, já dependência do crate) em
/// vez do `Math.random().toString(36)` do TS — o formato exato do sufixo não é consumido por ninguém
/// fora deste módulo, só precisa ser único dentro do baralho.
pub fn create_deck(manilha_rank: &str) -> Vec<TrucoCard> {
    let mut deck = Vec::with_capacity(RANK_ORDER.len() * SUITS.len());
    for rank in RANK_ORDER {
        for suit in SUITS {
            let (power, is_manilha) = compute_card_power(rank, suit, manilha_rank);
            let suffix = Uuid::new_v4().simple().to_string();
            deck.push(TrucoCard {
                id: format!("{rank}-{suit}-{}", &suffix[..4]),
                rank: rank.to_string(),
                suit: suit.to_string(),
                name: format!("{rank} de {}", capitalize(suit)),
                power,
                is_manilha,
            });
        }
    }
    deck
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_manilha_rank_wraps_around_from_the_last_rank_to_the_first() {
        assert_eq!(get_manilha_rank("4"), "5");
        assert_eq!(get_manilha_rank("3"), "4");
    }

    #[test]
    fn compute_card_power_ranks_the_manilha_above_every_non_manilha_card() {
        let (manilha_power, is_manilha) = compute_card_power("5", "paus", "5");
        assert!(is_manilha);
        let (highest_non_manilha, _) = compute_card_power("3", "paus", "5");
        assert!(manilha_power > highest_non_manilha);
    }

    #[test]
    fn create_deck_has_forty_unique_cards() {
        let deck = create_deck("4");
        assert_eq!(deck.len(), 40);
        let unique_ids: std::collections::HashSet<&str> = deck.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(unique_ids.len(), 40, "todo card deveria ter um id único");
    }

    #[test]
    fn create_deck_marks_exactly_four_manilhas_one_per_suit() {
        let deck = create_deck("Q");
        assert_eq!(deck.iter().filter(|c| c.is_manilha).count(), 4);
    }
}
