export type TrucoTaunts = {
  trucoShouts: string[]
  taunts: string[]
  winTaunts: string[]
  loseTaunts: string[]
  foldLines: string[]
  acceptLines: string[]
  raiseLines: string[]
}

export const EMPTY_TAUNTS: TrucoTaunts = {
  trucoShouts: [],
  taunts: [],
  winTaunts: [],
  loseTaunts: [],
  foldLines: [],
  acceptLines: [],
  raiseLines: [],
}

export function toStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string' && v.trim().length > 0) : []
}
