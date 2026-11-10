//! `startTrucoRound` — porte de `handlers/truco-engine.ts` + `handlers/truco-game.ts::trucoPayload`.
//! Só o SORTEIO inicial (vira, mãos, manilha): `playTurn` continua exclusivamente client-side
//! (`renderer/widgets/TrucoBoard.tsx`) tanto no TS quanto aqui — nenhum dos dois lados chama
//! `playTurn` do processo principal, então não há gap ao deixá-lo de fora deste porte.
//!
//! **Zoeira da Mika (`taunts`) sempre vazia.** `generateTrucoTaunts` (`handlers/truco-taunts.ts`)
//! chama `generateText` contra `ILLMFactory` e já cai para `EMPTY_TAUNTS` se a chamada falhar ou
//! estourar o timeout de 3s — o MESMO fallback que este porte sempre toma, porque não há cliente de
//! LLM alcançável a partir de `modules/gamer/` hoje (mesma lacuna documentada em `session_memory.rs`
//! para `buildSummary`). `tauntsUnavailable: true` é honesto com o estado real, não uma simulação de
//! resposta de LLM.

use rand::seq::SliceRandom;
use serde::Serialize;

use super::truco_deck::{compute_card_power, create_deck, get_manilha_rank, TrucoCard};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrucoTaunts {
    pub truco_shouts: Vec<String>,
    pub taunts: Vec<String>,
    pub win_taunts: Vec<String>,
    pub lose_taunts: Vec<String>,
    pub fold_lines: Vec<String>,
    pub accept_lines: Vec<String>,
    pub raise_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrucoScore {
    pub user: i32,
    pub mika: i32,
}

/// `bazas` fica sempre vazio aqui: só é populado por `playTurn`, que não é chamado deste lado (ver o
/// cabeçalho do arquivo) — o tipo `Vec<serde_json::Value>` evita declarar `BazaRecord` sem nenhum
/// produtor.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrucoGameState {
    pub user_hand: Vec<TrucoCard>,
    pub mika_hand: Vec<TrucoCard>,
    pub vira: TrucoCard,
    pub manilha_rank: String,
    pub score: TrucoScore,
    pub stake: i32,
    pub bazas: Vec<serde_json::Value>,
    pub taunts: TrucoTaunts,
    pub taunts_unavailable: bool,
    pub turn: String,
}

/// Porte de `startTrucoRound(score = { user: 0, mika: 0 }, EMPTY_TAUNTS, true)` — a chamada de
/// `trucoPayload` sempre passa placar zerado para uma partida nova.
pub fn start_truco_round() -> TrucoGameState {
    let mut deck = create_deck("4");
    deck.shuffle(&mut rand::rng());
    let vira = deck.pop().expect("baralho de truco recém-criado tem 40 cartas");
    let manilha_rank = get_manilha_rank(&vira.rank);

    // Recomputa power/isManilha nas 39 cartas restantes contra a manilha REAL — mesma correção que
    // `truco-engine.ts` já documenta (a vira já saiu via `pop()`, então não pode reentrar nas mãos).
    let remaining: Vec<TrucoCard> = deck
        .into_iter()
        .map(|card| {
            let (power, is_manilha) = compute_card_power(&card.rank, &card.suit, &manilha_rank);
            TrucoCard { power, is_manilha, ..card }
        })
        .collect();

    let user_hand = remaining[0..3].to_vec();
    let mika_hand = remaining[3..6].to_vec();

    TrucoGameState {
        user_hand,
        mika_hand,
        vira,
        manilha_rank,
        score: TrucoScore { user: 0, mika: 0 },
        stake: 1,
        bazas: vec![],
        taunts: TrucoTaunts::default(),
        taunts_unavailable: true,
        turn: "user".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deals_three_cards_each_and_a_separate_vira() {
        let state = start_truco_round();
        assert_eq!(state.user_hand.len(), 3);
        assert_eq!(state.mika_hand.len(), 3);

        let mut ids: Vec<&str> =
            state.user_hand.iter().chain(state.mika_hand.iter()).map(|c| c.id.as_str()).collect();
        ids.push(state.vira.id.as_str());
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), 7, "vira e as 6 cartas das mãos devem ser todas distintas");
    }

    #[test]
    fn hand_card_power_is_recomputed_against_the_real_manilha_not_against_rank_four() {
        let state = start_truco_round();
        for card in state.user_hand.iter().chain(state.mika_hand.iter()) {
            let (expected_power, expected_manilha) = compute_card_power(&card.rank, &card.suit, &state.manilha_rank);
            assert_eq!(card.power, expected_power);
            assert_eq!(card.is_manilha, expected_manilha);
        }
    }

    #[test]
    fn starts_scoreless_with_the_user_going_first_and_no_taunts_available() {
        let state = start_truco_round();
        assert_eq!(state.score.user, 0);
        assert_eq!(state.score.mika, 0);
        assert_eq!(state.turn, "user");
        assert!(state.taunts_unavailable);
        assert!(state.taunts.taunts.is_empty());
    }
}
