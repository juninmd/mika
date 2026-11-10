import type { Translate } from '../../src/hooks/useI18n'

export type Observation = {
  id: string
  analysis: string
  foregroundApp?: { title?: string; processName?: string }
  isLikelyGame?: boolean
  isFunny?: boolean
  memeContext?: string
  at: number
  people?: Array<{ name: string; type: string; confidence?: string }>
}

export type ObservationPayload = Omit<Observation, 'at' | 'id'> & { at?: number }

export const MAX_HISTORY = 6

export function relativeTime(ms: number, t: Translate): string {
  const diff = Math.max(0, Math.round((Date.now() - ms) / 1000))
  if (diff < 5) return t('settings.vision.monitor.time.now')
  if (diff < 60) return t('settings.vision.monitor.time.secondsAgo').replace('{{count}}', String(diff))
  return t('settings.vision.monitor.time.minutesAgo').replace('{{count}}', String(Math.round(diff / 60)))
}

export function observationFromPayload(payload: ObservationPayload): Observation {
  return {
    analysis: payload.analysis,
    foregroundApp: payload.foregroundApp,
    isLikelyGame: payload.isLikelyGame,
    isFunny: payload.isFunny,
    memeContext: payload.memeContext,
    at: payload.at ?? Date.now(),
    people: payload.people,
    id: crypto.randomUUID(),
  }
}
