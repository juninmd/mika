import type { TranslationDict } from '@mika/sdk/i18n'
import { capitalizeLocal } from './format-number'

export type MatchEffectId = 'matrix' | 'constellation' | 'sakura' | 'synthwave' | 'fireworks'

type MatchEffectMode = 'toggle' | 'show'

export interface MatchEffectCommand {
  effect: MatchEffectId
  mode: MatchEffectMode
}

export const MATCH_EFFECT_IDS = ['matrix', 'constellation', 'sakura', 'synthwave', 'fireworks'] as const

// Matrix, Sakura e Synthwave são nomes próprios; só estes dois mudam com o idioma.
const TRANSLATED_EFFECT_LABELS: Partial<Record<MatchEffectId, keyof TranslationDict>> = {
  constellation: 'shell.effects.constellation',
  fireworks: 'shell.effects.fireworks',
}

export function matchEffectLabel(effect: MatchEffectId, t: (key: keyof TranslationDict) => string): string {
  const key = TRANSLATED_EFFECT_LABELS[effect]
  return key ? t(key) : capitalizeLocal(effect)
}

const EFFECT_IDS: ReadonlySet<string> = new Set(MATCH_EFFECT_IDS)

export function parseMatchEffectToolResult(result: unknown): MatchEffectCommand | null {
  if (typeof result !== 'object' || result === null) return null
  const envelope = result as { success?: unknown; data?: unknown }
  if (envelope.success === false) return null
  if (typeof envelope.data !== 'object' || envelope.data === null) return null
  const data = envelope.data as { type?: unknown; effect?: unknown; triggered?: unknown }
  if (data.triggered !== true) return null
  if (typeof data.effect !== 'string' || !EFFECT_IDS.has(data.effect)) return null

  return { effect: data.effect as MatchEffectId, mode: data.type === 'toggle' ? 'toggle' : 'show' }
}
