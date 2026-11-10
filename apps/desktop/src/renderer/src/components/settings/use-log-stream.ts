import type { LogLevel } from '@mika/sdk/core/logger-format'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react'
import type { useI18n } from '../../hooks/useI18n'
import { downloadBlobAs } from '../../lib/download-blob'
import { getElectronApi } from '../../lib/shell-api'
import { useUIStore } from '../../store/useUIStore'
import {
  type AudienceFilter,
  appendLogs,
  FLUSH_INTERVAL_MS,
  logExportHeader,
  MAX_LOGS,
  matchesFilters,
  type StampedLog,
} from './logs-panel-filters'
import { configStr } from './settings-utils'

type Options = {
  config: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
  t: ReturnType<typeof useI18n>['t']
}

export function useLogStream({ config, onSave, t }: Options) {
  const [logs, setLogs] = useState<StampedLog[]>([])
  const [totalCaptured, setTotalCaptured] = useState(0)

  const [audienceFilter, setAudienceFilter] = useState<AudienceFilter>('human')
  const [traceFilter, setTraceFilter] = useState<string | null>(null)
  const [searchText, setSearchText] = useState('')
  const [isPaused, setIsPaused] = useState(false)
  const [autoScroll, setAutoScroll] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)

  const containerRef = useRef<HTMLDivElement>(null)
  const pendingRef = useRef<StampedLog[]>([])
  const seqRef = useRef(0)
  const loadSeqRef = useRef(0)
  const isPausedRef = useRef(isPaused)

  useEffect(() => {
    isPausedRef.current = isPaused
  }, [isPaused])

  const currentLogLevel = (configStr(config, 'system.logLevel', 'info') || 'info') as LogLevel

  const loadLogs = useCallback(() => {
    const seq = ++loadSeqRef.current
    const api = getElectronApi()

    api.runtime
      .getRecentLogs()
      .then(({ logs: initialLogs }) => {
        if (seq !== loadSeqRef.current) return
        setLogs(initialLogs.map((item) => ({ seq: seqRef.current++, item })).slice(-MAX_LOGS))
        setTotalCaptured(seqRef.current)
      })
      .catch((err: unknown) => {
        if (seq !== loadSeqRef.current) return
        setLoadError(toDisplayError(err, String(err)))
      })
  }, [])

  useEffect(() => {
    loadLogs()
    return () => {
      loadSeqRef.current++
    }
  }, [loadLogs])

  useEffect(() => {
    const api = getElectronApi()
    return api.runtime.onLog((item) => {
      if (isPausedRef.current) return
      pendingRef.current.push({ seq: seqRef.current++, item })
    })
  }, [])

  useEffect(() => {
    const id = window.setInterval(() => {
      const pending = pendingRef.current
      if (pending.length === 0) return
      pendingRef.current = []
      setLogs((prev) => appendLogs(prev, pending, MAX_LOGS))
      setTotalCaptured(seqRef.current)
    }, FLUSH_INTERVAL_MS)
    return () => window.clearInterval(id)
  }, [])

  const handleScroll = () => {
    const el = containerRef.current
    if (!el) return
    setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 25)
  }

  const deferredSearchText = useDeferredValue(searchText)
  const searchIndex = useMemo(() => {
    const map = new Map<number, string>()
    for (const { seq, item } of logs) {
      map.set(
        seq,
        `${item.msg.toLowerCase()} ${item.scope.toLowerCase()} ${
          item.meta ? JSON.stringify(item.meta).toLowerCase() : ''
        }`,
      )
    }
    return map
  }, [logs])

  const filteredLogs = useMemo(() => {
    const query = deferredSearchText.trim().toLowerCase()
    return logs.filter((entry) =>
      matchesFilters(entry, { audienceFilter, traceFilter, minLevel: currentLogLevel, query, searchIndex }),
    )
  }, [logs, currentLogLevel, audienceFilter, traceFilter, deferredSearchText, searchIndex])

  // biome-ignore lint/correctness/useExhaustiveDependencies: filteredLogs.length is a trigger-only dep — it re-runs the scroll-to-bottom on new log lines, even though the body reads containerRef, not filteredLogs.
  useEffect(() => {
    const el = containerRef.current
    if (autoScroll && el) {
      el.scrollTop = el.scrollHeight
    }
  }, [filteredLogs.length, autoScroll])

  const handleLogLevelChange = (level: LogLevel) => {
    void (async () => {
      try {
        await onSave('system.logLevel', level)
      } catch (err) {
        useUIStore.getState().addNotification('error', toDisplayError(err, t('sdkUi.saveError')))
      }
    })()
  }

  const clearLogs = () => {
    pendingRef.current = []
    setLogs([])
    setTotalCaptured(0)
  }

  const retryLoad = () => {
    setLoadError(null)
    loadLogs()
  }

  const isTruncated = totalCaptured > MAX_LOGS

  const exportLogs = () => {
    const text = logExportHeader(totalCaptured, t) + filteredLogs.map(({ item }) => item.formatted).join('\n')
    const blob = new Blob([text], { type: 'text/plain' })
    const suffix = isTruncated ? '-truncated' : ''
    downloadBlobAs(blob, `mika-logs-${new Date().toISOString().replace(/[:.]/g, '-')}${suffix}.txt`)
  }

  return {
    audienceFilter,
    setAudienceFilter,
    traceFilter,
    setTraceFilter,
    searchText,
    setSearchText,
    isPaused,
    setIsPaused,
    autoScroll,
    loadError,
    containerRef,
    currentLogLevel,
    handleScroll,
    filteredLogs,
    handleLogLevelChange,
    clearLogs,
    retryLoad,
    isTruncated,
    totalCaptured,
    exportLogs,
  }
}
