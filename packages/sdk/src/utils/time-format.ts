import { type TranslationDict, t } from '../i18n'

type RelativeTimeTranslate = (key: keyof TranslationDict) => string

// Short "time ago" label from an epoch-MILLISECONDS timestamp; convert Unix seconds (e.g. Steam's `unlocktime`) with `* 1000` first.
// `translate` defaults to pt-BR `t()` for main-process callers without a UI locale; renderer call sites should pass `useI18n()`'s `t`.
export function formatRelativeTime(epochMs: number, translate: RelativeTimeTranslate = t): string {
  if (!epochMs) return ''
  const diffMs = Date.now() - epochMs
  const minutes = Math.floor(diffMs / 60_000)
  if (minutes < 1) return translate('time.now')
  if (minutes < 60) return translate('time.minutesAgo').replace('{{count}}', String(minutes))
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return translate('time.hoursAgo').replace('{{count}}', String(hours))
  const days = Math.floor(hours / 24)
  if (days < 30) return translate('time.daysAgo').replace('{{count}}', String(days))
  return translate('time.monthsAgo').replace('{{count}}', String(Math.floor(days / 30)))
}

export function formatDurationMs(ms: number | undefined): string {
  if (!ms || ms <= 0) return '0min'
  const minutes = Math.round(ms / 60_000)
  if (minutes < 60) return `${minutes}min`
  const hours = Math.floor(minutes / 60)
  const rest = minutes % 60
  return rest ? `${hours}h${rest}min` : `${hours}h`
}
