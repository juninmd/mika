import { capitalize } from '@mika/sdk/utils/locale-text'

export type Suit = 'ouros' | 'espadas' | 'copas' | 'paus'
export type Rank = '4' | '5' | '6' | '7' | 'Q' | 'J' | 'K' | 'A' | '2' | '3'

export interface TrucoCard {
  id: string
  rank: Rank
  suit: Suit
  name: string
  power: number
  isManilha: boolean
}

export const SUIT_SYMBOLS: Record<Suit, string> = {
  ouros: '♦',
  espadas: '♠',
  copas: '♥',
  paus: '♣',
}

export const SUIT_COLORS: Record<Suit, string> = {
  ouros: '#ef4444',
  espadas: '#38bdf8',
  copas: '#f43f5e',
  paus: '#10b981',
}

const RANK_ORDER: Rank[] = ['4', '5', '6', '7', 'Q', 'J', 'K', 'A', '2', '3']
const SUITS: Suit[] = ['ouros', 'espadas', 'copas', 'paus']

const SUIT_MANILHA_POWER: Record<Suit, number> = {
  ouros: 11,
  espadas: 12,
  copas: 13,
  paus: 14,
}

export function getManilhaRank(viraRank: Rank): Rank {
  const idx = RANK_ORDER.indexOf(viraRank)
  return RANK_ORDER[(idx + 1) % RANK_ORDER.length] ?? '4'
}

export function computeCardPower(rank: Rank, suit: Suit, manilhaRank: Rank): { power: number; isManilha: boolean } {
  if (rank === manilhaRank) {
    return { power: SUIT_MANILHA_POWER[suit], isManilha: true }
  }
  const basePower = RANK_ORDER.indexOf(rank) + 1
  return { power: basePower, isManilha: false }
}

export function createDeck(manilhaRank: Rank): TrucoCard[] {
  const deck: TrucoCard[] = []
  for (const rank of RANK_ORDER) {
    for (const suit of SUITS) {
      const { power, isManilha } = computeCardPower(rank, suit, manilhaRank)
      deck.push({
        id: `${rank}-${suit}-${Math.random().toString(36).slice(2, 6)}`,
        rank,
        suit,
        name: `${rank} de ${capitalize(suit, 'pt-BR')}`,
        power,
        isManilha,
      })
    }
  }
  return deck
}

export function shuffle<T>(array: T[]): T[] {
  const copy = [...array]
  for (let i = copy.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    const temp = copy[i] as T
    copy[i] = copy[j] as T
    copy[j] = temp
  }
  return copy
}
