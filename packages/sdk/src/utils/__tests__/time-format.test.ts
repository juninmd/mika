import { describe, expect, it, vi } from 'vitest'
import { formatDurationMs, formatRelativeTime } from '../time-format'

const translate = (key: string) => {
  const table: Record<string, string> = {
    'time.now': 'agora',
    'time.minutesAgo': 'há {{count}}min',
    'time.hoursAgo': 'há {{count}}h',
    'time.daysAgo': 'há {{count}}d',
    'time.monthsAgo': 'há {{count}}mo',
  }
  return table[key] ?? key
}

describe('formatRelativeTime', () => {
  it('returns empty string for a falsy timestamp (0, unset)', () => {
    expect(formatRelativeTime(0, translate)).toBe('')
  })

  it('buckets across now/minutes/hours/days/months boundaries', () => {
    const now = new Date('2026-01-15T12:00:00Z').getTime()
    vi.setSystemTime(now)

    expect(formatRelativeTime(now - 30_000, translate)).toBe('agora')
    expect(formatRelativeTime(now - 5 * 60_000, translate)).toBe('há 5min')
    expect(formatRelativeTime(now - 3 * 60 * 60_000, translate)).toBe('há 3h')
    expect(formatRelativeTime(now - 5 * 24 * 60 * 60_000, translate)).toBe('há 5d')
    expect(formatRelativeTime(now - 90 * 24 * 60 * 60_000, translate)).toBe('há 3mo')

    vi.useRealTimers()
  })

  it('defaults to the pt-BR t() when no translate function is provided', () => {
    // Converts Steam-style unlocktime (Unix seconds) callers must multiply by 1000 first.
    expect(formatRelativeTime(Date.now())).toBe('agora')
  })
})

describe('formatDurationMs', () => {
  it('treats undefined and non-positive values as 0min', () => {
    expect(formatDurationMs(undefined)).toBe('0min')
    expect(formatDurationMs(0)).toBe('0min')
    expect(formatDurationMs(-1000)).toBe('0min')
  })

  it('formats sub-hour durations in minutes', () => {
    expect(formatDurationMs(5 * 60_000)).toBe('5min')
    expect(formatDurationMs(59 * 60_000)).toBe('59min')
  })

  it('formats hour-plus durations as "XhYmin", omitting minutes when exact', () => {
    expect(formatDurationMs(60 * 60_000)).toBe('1h')
    expect(formatDurationMs(90 * 60_000)).toBe('1h30min')
    expect(formatDurationMs(125 * 60_000)).toBe('2h5min')
  })
})
