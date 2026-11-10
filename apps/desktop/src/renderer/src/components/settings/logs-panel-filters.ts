import type { LogItem, LogLevel } from '@mika/sdk/core/logger-format'
import { AlertTriangle, Bug, Info, type LucideIcon, XCircle } from 'lucide-react'

export const LEVEL_ICON: Record<LogLevel, LucideIcon> = {
  debug: Bug,
  info: Info,
  warn: AlertTriangle,
  error: XCircle,
}

export type StampedLog = {
  seq: number
  item: LogItem
}

export const MAX_LOGS = 500

/** First line of an exported log when older lines were dropped; empty when nothing was truncated. */
export function logExportHeader(
  totalCaptured: number,
  t: (key: 'settings.logs.export.truncatedHeader', params: { max: number; total: number }) => string,
): string {
  if (totalCaptured <= MAX_LOGS) return ''
  return `${t('settings.logs.export.truncatedHeader', { max: MAX_LOGS, total: totalCaptured })}\n`
}
export const FLUSH_INTERVAL_MS = 200

export type AudienceFilter = 'human' | 'ai' | 'all'

type LogFilters = {
  audienceFilter: AudienceFilter
  traceFilter: string | null
  /** The ONE level control: what `system.logLevel` records is also the floor of what is shown. */
  minLevel: LogLevel

  query: string
  searchIndex: Map<number, string>
}

const LEVEL_RANK: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 }

export function matchesFilters({ seq, item }: StampedLog, filters: LogFilters): boolean {
  if (filters.audienceFilter !== 'all' && (item.audience ?? 'ai') !== filters.audienceFilter) return false
  if (filters.traceFilter && item.traceId !== filters.traceFilter) return false
  if ((LEVEL_RANK[item.level] ?? 0) < LEVEL_RANK[filters.minLevel]) return false
  if (filters.query === '') return true
  return (filters.searchIndex.get(seq) ?? '').includes(filters.query)
}

const sameLine = (a: LogItem, b: LogItem) =>
  a.level === b.level && a.scope === b.scope && a.msg === b.msg && a.audience === b.audience

/**
 * Appends streamed lines. A record with `repeatCount > 1` that matches the last line is the backend's
 * updated counter for a line already on screen (its batch was drained before the repeat): it replaces
 * that line in place, keeping its `seq`, instead of showing the same message twice.
 */
export function appendLogs(prev: StampedLog[], incoming: StampedLog[], max: number): StampedLog[] {
  const next = [...prev]
  for (const entry of incoming) {
    const last = next.at(-1)
    if ((entry.item.repeatCount ?? 1) > 1 && last && sameLine(last.item, entry.item)) {
      next[next.length - 1] = { seq: last.seq, item: entry.item }
    } else {
      next.push(entry)
    }
  }
  return next.slice(-max)
}
