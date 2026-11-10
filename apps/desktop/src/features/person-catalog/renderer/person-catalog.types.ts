export interface PersonMention {
  id: string
  source: 'chat' | 'voice' | 'vision' | 'manual' | 'import'
  text: string
  timestamp: number
  url?: string
  appName?: string
  windowTitle?: string
  screenshotPath?: string
  context?: string
}

/** A remembered fact about this person, in the user's own words (Rust `PersonFact`). */
export interface PersonFact {
  id: string
  text: string
  confidence: number
  source?: string
  updatedAt: number
}

/** "Which brother?" — a relation-only mention that fits ≥2 people (`person_catalog_clarifications`). */
export interface PendingClarification {
  id: string
  label: string
  facts: string[]
  createdAt: number
  candidates: Array<{ id: string; name: string }>
}

export interface PersonAttribute {
  key: string
  value: string
  confidence: 'low' | 'medium' | 'high'
  source?: string
  updatedAt: number
}

import type { PersonType } from '../../../shared/person-types'

export { PERSON_TYPE_VALUES, type PersonType } from '../../../shared/person-types'

export interface PersonRelationship {
  name: string
  type: string
  affinity?: number
  trust?: number
  description?: string
  lastInteractionAt?: number
}

export type RelationshipType =
  | 'stranger'
  | 'acquaintance'
  | 'friend'
  | 'close_friend'
  | 'best_friend'
  | 'rival'
  | 'enemy'
  | 'vvip'
  | 'creator'
  | 'plays_with'

export interface PersonRecord {
  id: string
  primaryName: string
  aliases: string[]
  type: PersonType

  bio?: string
  gender?: string
  birthDate?: string
  birthPlace?: string
  deathDate?: string
  nationality?: string
  languages?: string[]

  occupation?: string[]
  knownFor?: string[]
  activeYears?: string
  awards?: string[]
  socialLinks?: Record<string, string>

  relationships?: PersonRelationship[]
  relationshipType?: RelationshipType
  /** Free-text kinship/role with the USER (e.g. "irmão", "chefe") — distinct from
   * relationshipType (the affinity ladder) and from role (occupation). Mirrors Rust's
   * `PersonRecord::relation_to_user` (work package "people catalog + relationship graph"). */
  relationToUser?: string
  affinity: number
  trustLevel: number
  origin?: string
  role?: string

  physicalDescription?: string

  abilities?: string[]
  affiliation?: string
  species?: string
  firstAppearance?: string
  voiceActor?: string

  mentions: PersonMention[]
  mentionCount: number
  firstSeenAt?: number
  lastSeenAt?: number

  userRating?: string
  userNotes?: string

  attributes: PersonAttribute[]
  tags: string[]
  imageUrl?: string
  notes?: string
  enriched?: boolean

  confidence?: number

  ambiguousWith?: string[]
  /** Long-term facts about this person (people memory); omitted when empty. */
  facts?: PersonFact[]
  /** Known only by relation so far ("Esposa") — the first real name replaces `primaryName`. */
  unnamed?: boolean
  createdAt: number
  updatedAt: number
}

export interface PersonCatalogIndex {
  version: number
  persons: PersonRecord[]
  aliasMap: Record<string, string>

  blocked?: string[]

  socialImportedAt?: number
}

export type UpsertInput = {
  id?: string
  name: string
  aliases?: string[]
  type?: PersonType
  bio?: string
  gender?: string
  birthDate?: string
  birthPlace?: string
  deathDate?: string
  nationality?: string
  languages?: string[]
  occupation?: string[]
  knownFor?: string[]
  activeYears?: string
  awards?: string[]
  socialLinks?: Record<string, string>
  relationships?: PersonRelationship[]
  relationshipType?: RelationshipType
  relationToUser?: string
  affinity?: number
  trustLevel?: number
  origin?: string
  role?: string
  physicalDescription?: string
  abilities?: string[]
  affiliation?: string
  species?: string
  firstAppearance?: string
  voiceActor?: string
  userRating?: string
  userNotes?: string
  attributes?: Array<{ key: string; value: string; confidence?: PersonAttribute['confidence'] }>
  mention?: Omit<PersonMention, 'id'>
  tags?: string[]
  notes?: string
  imageUrl?: string
}

export type UpsertOutcome = {
  created: boolean
  person: PersonRecord | null

  blocked?: boolean

  ambiguousWith?: string[]
}

export type SocialPersonImport = {
  primaryName: string
  aliases?: string[]

  type?: string
  universe?: string
  appearance?: string
  notes?: string[]
  tags?: string[]
  affinity?: number
  lastMentionedAt?: number
  createdAt?: number
}
