import type { TranslationDict } from '@mika/sdk/i18n'

type TKey = keyof TranslationDict
type Translate = (key: TKey) => string

/** Quick "when" chips, expressed in the `reminder` tool's own `amount`+`unit` (no new backend path). */
export type QuickWhen = { id: 'in10m' | 'in1h' | 'tomorrow9'; labelKey: TKey }

export const QUICK_WHEN: QuickWhen[] = [
  { id: 'in10m', labelKey: 'settings.scheduler.quick.in10m' },
  { id: 'in1h', labelKey: 'settings.scheduler.quick.in1h' },
  { id: 'tomorrow9', labelKey: 'settings.scheduler.quick.tomorrow9' },
]

/** Minutes from `now` until tomorrow at 09:00 local — `atTime: "09:00"` would land TODAY before 9h. */
export function minutesUntilTomorrowAt9(now: Date): number {
  const target = new Date(now)
  target.setDate(target.getDate() + 1)
  target.setHours(9, 0, 0, 0)
  return Math.max(1, Math.ceil((target.getTime() - now.getTime()) / 60_000))
}

export function quickWhenArgs(id: QuickWhen['id'], now: Date): { amount: number; unit: 'minutes' | 'hours' } {
  if (id === 'in10m') return { amount: 10, unit: 'minutes' }
  if (id === 'in1h') return { amount: 1, unit: 'hours' }
  return { amount: minutesUntilTomorrowAt9(now), unit: 'minutes' }
}

const pad = (n: number) => String(n).padStart(2, '0')

/** `HH:mm` of an epoch in local time — what the form's time field expects when editing. */
export function toHhMm(epochMs: number): string {
  const d = new Date(epochMs)
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function dayDiff(a: Date, b: Date): number {
  const start = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  return Math.round((start(a) - start(b)) / 86_400_000)
}

/** "hoje 21:54" / "amanhã 09:00" / "sex., 3 de out. 18:30" — never seconds. */
export function formatDue(epochMs: number, now: Date, locale: string, t: Translate): string {
  const due = new Date(epochMs)
  const time = due.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
  const diff = dayDiff(due, now)
  if (diff === 0) return `${t('settings.scheduler.due.today')} ${time}`
  if (diff === 1) return `${t('settings.scheduler.due.tomorrow')} ${time}`
  const day = due.toLocaleDateString(locale, { weekday: 'short', day: 'numeric', month: 'short' })
  return `${day} ${time}`
}
