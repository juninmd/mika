export type EpisodicMemory = {
  id: string
  content: string
  type: string
  timestamp: number
  metadata?: string
}

export type LearnedFact = {
  id: string
  subject: string
  fact: string
  confidence: number
  updatedAt: number
}

export type PersonRecord = {
  id: string
  primaryName: string
  aliases: string[]
  type: string
  notes: string[]
}

export type RelationshipEdge = {
  id: string
  personId: string
  label: string
  direction: 'user_to_person' | 'person_to_person'
  confidence: number
}
