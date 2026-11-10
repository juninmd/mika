import type { LogItem } from '@mika/sdk/core/logger-format'
import { t } from '@mika/sdk/i18n'
import { describe, expect, it } from 'vitest'
import { appendLogs, logExportHeader, MAX_LOGS, matchesFilters } from '../logs-panel-filters'

const entry = (level: LogItem['level']) => ({
  seq: 1,
  item: { level, scope: 's', msg: 'm', timestamp: '', at: 0, formatted: '', audience: 'human' } as LogItem,
})
const base = { audienceFilter: 'all' as const, traceFilter: null, query: '', searchIndex: new Map<number, string>() }

describe('logs level control', () => {
  // The capture level and the view filter used to be two controls that could contradict each other
  // (capture "warn" + view "debug" showed an empty list). Now the capture level is the view floor.
  it('shows the chosen level and everything more severe, nothing below', () => {
    const at = (minLevel: LogItem['level']) =>
      (['debug', 'info', 'warn', 'error'] as const).filter((l) => matchesFilters(entry(l), { ...base, minLevel }))
    expect(at('debug')).toEqual(['debug', 'info', 'warn', 'error'])
    expect(at('warn')).toEqual(['warn', 'error'])
    expect(at('error')).toEqual(['error'])
  })
})

describe('appendLogs', () => {
  const log = (seq: number, msg: string, repeatCount?: number) => ({
    seq,
    item: {
      level: 'warn',
      scope: 's',
      msg,
      timestamp: '',
      at: seq,
      formatted: msg,
      audience: 'human',
      repeatCount,
    } as LogItem,
  })

  // The backend collapses consecutive identical lines, but once the first one was already streamed
  // it sends the updated counter as a new record — appended as-is it showed the same line twice.
  it('folds an updated repeat counter into the line already on screen instead of duplicating it', () => {
    const merged = appendLogs([log(1, 'offline')], [log(2, 'offline', 2), log(3, 'offline', 3)], 500)
    expect(merged).toHaveLength(1)
    expect(merged[0]).toMatchObject({ seq: 1, item: { repeatCount: 3, at: 3 } })
  })

  it('keeps a first occurrence and a different line as new entries, and caps the length', () => {
    expect(appendLogs([log(1, 'a')], [log(2, 'a'), log(3, 'b', 2)], 500)).toHaveLength(3)
    expect(appendLogs([log(1, 'a'), log(2, 'b')], [log(3, 'c')], 2).map((e) => e.seq)).toEqual([2, 3])
  })
})

describe('logExportHeader', () => {
  // Regression: the truncation notice written into the exported file was hard-coded Portuguese.
  it('is empty while nothing was dropped and a localized notice with both numbers after', () => {
    const translate = (key: 'settings.logs.export.truncatedHeader', params: { max: number; total: number }) =>
      t(key, params, 'en-US')
    expect(logExportHeader(MAX_LOGS, translate)).toBe('')
    const header = logExportHeader(MAX_LOGS + 40, translate)
    expect(header).toContain(String(MAX_LOGS))
    expect(header).toContain(String(MAX_LOGS + 40))
    expect(header).not.toMatch(/Truncado|últimas/)
    expect(header.endsWith('\n')).toBe(true)
  })
})
