import { z } from 'zod'

export const PERSON_TYPE_VALUES = [
  'real_person',
  'fictional_character',
  'organization',
  'pet',
  'politician',
  'journalist',
  'influencer',
  'athlete',
  'singer',
  'streamer',
  'movie',
  'series',
  'historical',
  'author',
  'comedian',
  'book',
  'vtuber',
  'game',
  'anime',
  'other',
] as const

export type PersonType = (typeof PERSON_TYPE_VALUES)[number]

export const PERSON_TYPE_FIELD = z.preprocess(
  (value) => normalizePersonType(value),
  z.enum(PERSON_TYPE_VALUES).optional(),
)

const LEGACY_PERSON_TYPES: Record<string, PersonType> = { film: 'movie' }

export function normalizePersonType(value: unknown): PersonType | undefined {
  if (typeof value !== 'string') return undefined
  const canonical = LEGACY_PERSON_TYPES[value] ?? value
  return (PERSON_TYPE_VALUES as readonly string[]).includes(canonical) ? (canonical as PersonType) : undefined
}
