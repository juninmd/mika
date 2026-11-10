export interface MemoryEntry {
  id: string
  content: string
  embedding?: number[]
  metadata?: Record<string, unknown>
  createdAt: number
  score?: number
}

export interface PersonRecord {
  id: string
  name: string
  aliases: string[]
  category: 'user' | 'friend' | 'public-figure' | 'streamer' | 'anime-character' | 'other'
  relationshipLevel: number
  socialHandles: Record<string, string>
  notes: string
  lastSeenAt?: number
  createdAt: number
  updatedAt: number
}

export interface CharacterRecord {
  id: string
  name: string
  franchise: string
  category: 'anime' | 'game' | 'movie' | 'series' | 'book' | 'other'
  description: string
  notes: string
  createdAt: number
  updatedAt: number
}

export interface KnowledgeNode {
  id: string
  content: string
  type: 'concept' | 'tool-idea' | 'snippet' | 'user-fact' | 'general'
  confidence: number
  relatedIds: string[]
  source?: string
  createdAt: number
  updatedAt: number
}

export interface LearnedFact {
  id: string
  content: string
  category: 'preference' | 'personal' | 'habit' | 'opinion' | 'skill'
  confidence: number
  createdAt: number
  updatedAt: number
}

export interface EmotionalState {
  mood: string
  intensity: number
  valence: number
  arousal: number
  energy: number
  affinity: number
  relationshipStatus: 'acquaintance' | 'friend' | 'close-friend' | 'companion'
  updatedAt: number
}

export interface CuriosityItem {
  id: string
  topic: string
  priority: 'low' | 'medium' | 'high'
  status: 'pending' | 'in-progress' | 'done' | 'dropped'
  source?: string
  notes?: string
  createdAt: number
  updatedAt: number
}

export interface ConversationStyle {
  formality: 'informal' | 'neutral' | 'formal'
  language: string
  traits: string[]
  responseBrevity: 'concise' | 'balanced' | 'elaborate'
  humor: boolean
  emojiUsage: 'none' | 'moderate' | 'heavy'
  updatedAt: number
}

export type PersonCategory =
  | 'actor'
  | 'musician'
  | 'athlete'
  | 'streamer'
  | 'content-creator'
  | 'politician'
  | 'tech-figure'
  | 'public-figure'
  | 'friend'
  | 'user'
  | 'anime-character'
  | 'game-character'
  | 'fictional-character'
  | 'other'

export type PersonAffinity = 'liked' | 'disliked' | 'neutral'

export interface PersonProfile {
  id: string
  name: string
  aliases: string[]
  category: PersonCategory
  biography: string
  profession: string
  nationality?: string
  birthDate?: string
  photoUrl?: string
  socialHandles: Record<string, string>
  links: Array<{ label: string; url: string }>
  mentions: number
  facts: string[]
  notes: string
  affinity?: PersonAffinity
  franchise?: string
  lastSeenAt?: number
  createdAt: number
  updatedAt: number
}
