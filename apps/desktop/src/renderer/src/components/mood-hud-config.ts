import type { TranslationDict } from '@mika/sdk/i18n'

export interface MoodState {
  valence: number
  arousal: number
  label: string
}

export interface MoodRecord extends MoodState {
  at: number
}

export const MAX_HISTORY = 3

const MOOD_CONFIG: Record<string, { emoji: string; color: string; glow: string }> = {
  excited: { emoji: '🤩', color: 'var(--info)', glow: 'var(--info-soft)' },
  happy: { emoji: '😊', color: 'var(--ok)', glow: 'var(--ok-soft)' },
  content: { emoji: '😌', color: 'var(--ok)', glow: 'var(--ok-soft)' },
  tense: { emoji: '😤', color: 'var(--danger)', glow: 'var(--danger-soft)' },
  sad: { emoji: '🥺', color: 'var(--info)', glow: 'var(--info-soft)' },
  bored: { emoji: '😑', color: 'var(--text-muted)', glow: 'var(--surface-raise)' },
  neutral: { emoji: '😐', color: 'var(--accent)', glow: 'var(--accent-soft)' },
}

export function getMoodConfig(label: string) {
  return MOOD_CONFIG[label] ?? MOOD_CONFIG.neutral
}

const MOOD_NAMES: Record<string, keyof TranslationDict> = {
  excited: 'shell.mood.name.excited',
  happy: 'shell.mood.name.happy',
  content: 'shell.mood.name.content',
  tense: 'shell.mood.name.tense',
  sad: 'shell.mood.name.sad',
  bored: 'shell.mood.name.bored',
  neutral: 'shell.mood.name.neutral',
}

/** The backend labels are English identifiers; an unknown one is shown as-is rather than hidden. */
export function moodName(label: string, t: (key: keyof TranslationDict) => string): string {
  const key = MOOD_NAMES[label]
  return key ? t(key) : label
}
