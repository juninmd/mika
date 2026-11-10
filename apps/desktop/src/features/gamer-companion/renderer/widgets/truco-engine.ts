import { computeCardPower, createDeck, getManilhaRank, type Rank, shuffle, type TrucoCard } from './truco-deck'
import type { TrucoTaunts } from './truco-types'

export type { Rank, Suit, TrucoCard } from './truco-deck'
export { computeCardPower, createDeck, getManilhaRank, SUIT_COLORS, SUIT_SYMBOLS } from './truco-deck'

export interface BazaRecord {
  userCard?: TrucoCard
  mikaCard?: TrucoCard
  winner?: 'user' | 'mika' | 'draw'
}

export interface TrucoGameState {
  userHand: TrucoCard[]
  mikaHand: TrucoCard[]
  vira: TrucoCard
  manilhaRank: Rank
  score: { user: number; mika: number }
  stake: number
  bazas: BazaRecord[]
  roundWinner?: 'user' | 'mika'
  matchWinner?: 'user' | 'mika'
  currentTaunt?: string
  taunts: TrucoTaunts
  tauntsUnavailable?: boolean
  turn: 'user' | 'mika'
}

export function startTrucoRound(
  score = { user: 0, mika: 0 },
  taunts: TrucoTaunts,
  tauntsUnavailable?: boolean,
): TrucoGameState {
  const rawDeck = shuffle(createDeck('4'))
  const viraCard = rawDeck.pop() as TrucoCard
  const manilhaRank = getManilhaRank(viraCard.rank)

  // Recompute power/isManilha on the SAME remaining 39 cards (vira already removed via pop) instead of
  // building a second full 40-card deck from scratch, which could reintroduce the vira card into a hand.
  const remainingDeck = rawDeck.map((card) => {
    const { power, isManilha } = computeCardPower(card.rank, card.suit, manilhaRank)
    return { ...card, power, isManilha }
  })
  const userHand = remainingDeck.slice(0, 3)
  const mikaHand = remainingDeck.slice(3, 6)

  return {
    userHand,
    mikaHand,
    vira: viraCard,
    manilhaRank,
    score,
    stake: 1,
    bazas: [],
    taunts,
    tauntsUnavailable,
    turn: 'user',
  }
}

function pickRandomTaunt(lines: string[]): string | undefined {
  if (!lines || lines.length === 0) return undefined
  const item = lines[Math.floor(Math.random() * lines.length)]
  return item
}

function pickMikaCard(mikaHand: TrucoCard[], userCard?: TrucoCard): TrucoCard {
  if (!userCard || mikaHand.length === 1) {
    return mikaHand[0] as TrucoCard
  }

  const winningCards = mikaHand.filter((c) => c.power > userCard.power).sort((a, b) => a.power - b.power)
  if (winningCards.length > 0) {
    return winningCards[0] as TrucoCard
  }

  const sorted = [...mikaHand].sort((a, b) => a.power - b.power)
  return sorted[0] as TrucoCard
}

function getBazaWinner(userCard: TrucoCard, mikaCard: TrucoCard): 'user' | 'mika' | 'draw' {
  if (userCard.power > mikaCard.power) return 'user'
  if (mikaCard.power > userCard.power) return 'mika'
  return 'draw'
}

function resolveRoundWinner(bazas: BazaRecord[], latestWinner: 'user' | 'mika' | 'draw'): 'user' | 'mika' | undefined {
  const userWins = bazas.filter((b) => b.winner === 'user').length
  const mikaWins = bazas.filter((b) => b.winner === 'mika').length

  if (userWins === 2) return 'user'
  if (mikaWins === 2) return 'mika'
  if (bazas.length === 3) {
    if (userWins > mikaWins) return 'user'
    if (mikaWins > userWins) return 'mika'
    // userWins === mikaWins here means either a 1-1 split (one baza drew) or 0-0 (all three drew).
    // Truco rule: whoever won baza 1 wins the round; if baza 1 drew, baza 2 decides, and so on.
    // Fallback to 'user' only covers the vanishingly rare case where all three bazas drew.
    const firstDecisive = bazas.find(
      (b): b is BazaRecord & { winner: 'user' | 'mika' } => b.winner === 'user' || b.winner === 'mika',
    )
    return firstDecisive?.winner ?? 'user'
  }
  if (bazas.length === 2 && bazas[0]?.winner === 'draw' && latestWinner !== 'draw') return latestWinner
  const firstWinner = bazas[0]?.winner
  if (bazas.length === 2 && latestWinner === 'draw' && (firstWinner === 'user' || firstWinner === 'mika')) {
    return firstWinner
  }
  return undefined
}

function resolveRoundOutcome(state: TrucoGameState, roundWinner: 'user' | 'mika' | undefined) {
  let score = { ...state.score }
  let matchWinner: 'user' | 'mika' | undefined
  let currentTaunt = pickRandomTaunt(state.taunts.taunts)

  if (!roundWinner) return { score, matchWinner, currentTaunt }

  score = {
    user: state.score.user + (roundWinner === 'user' ? state.stake : 0),
    mika: state.score.mika + (roundWinner === 'mika' ? state.stake : 0),
  }
  currentTaunt =
    roundWinner === 'mika' ? pickRandomTaunt(state.taunts.winTaunts) : pickRandomTaunt(state.taunts.loseTaunts)
  if (score.user >= 12) matchWinner = 'user'
  else if (score.mika >= 12) matchWinner = 'mika'

  return { score, matchWinner, currentTaunt }
}

export function playTurn(state: TrucoGameState, userCardId: string): TrucoGameState {
  if (state.roundWinner || state.matchWinner) return state

  const userCardIndex = state.userHand.findIndex((c) => c.id === userCardId)
  if (userCardIndex === -1) return state

  const userCard = state.userHand[userCardIndex] as TrucoCard
  const remainingUserHand = state.userHand.filter((_, i) => i !== userCardIndex)

  const mikaCard = pickMikaCard(state.mikaHand, userCard)
  const remainingMikaHand = state.mikaHand.filter((c) => c.id !== mikaCard.id)

  const bazaWinner = getBazaWinner(userCard, mikaCard)

  const updatedBazas: BazaRecord[] = [...state.bazas, { userCard, mikaCard, winner: bazaWinner }]

  const roundWinner = resolveRoundWinner(updatedBazas, bazaWinner)
  const { score, matchWinner, currentTaunt } = resolveRoundOutcome(state, roundWinner)

  return {
    ...state,
    userHand: remainingUserHand,
    mikaHand: remainingMikaHand,
    bazas: updatedBazas,
    score,
    roundWinner,
    matchWinner,
    currentTaunt,
  }
}
