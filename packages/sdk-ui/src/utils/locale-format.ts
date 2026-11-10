// Callers pass the active app locale instead of hardcoding a BCP-47 tag.
export function formatLocalDate(value: number | Date, locale?: string, options?: Intl.DateTimeFormatOptions): string {
  const date = value instanceof Date ? value : new Date(value)
  return date.toLocaleDateString(locale, options)
}

export function formatLocalTime(value: number | Date, locale?: string, options?: Intl.DateTimeFormatOptions): string {
  const date = value instanceof Date ? value : new Date(value)
  return date.toLocaleTimeString(locale, options)
}

export function formatLocalNumber(value: number, locale?: string, options?: Intl.NumberFormatOptions): string {
  return value.toLocaleString(locale, options)
}

const DURATION_UNITS = [
  ['day', 86400],
  ['hour', 3600],
  ['minute', 60],
  ['second', 1],
] as const

const narrow = (count: number, unit: string, locale?: string) =>
  count.toLocaleString(locale, { style: 'unit', unit, unitDisplay: 'narrow' })

/** Seconds → the two largest non-zero units, narrow and localized ("1h 30m", "1小时 30分钟"). */
export function formatLocalDuration(seconds: number, locale?: string): string {
  let rest = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0
  const parts: string[] = []
  for (const [unit, size] of DURATION_UNITS) {
    const count = Math.floor(rest / size)
    rest -= count * size
    if (count > 0 && parts.length < 2) parts.push(narrow(count, unit, locale))
  }
  return parts.length > 0 ? parts.join(' ') : narrow(0, 'second', locale)
}
