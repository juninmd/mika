import { useCallback, useEffect, useRef, useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import type { Message } from './chat-history-components'
import { FETCH_TIMEOUT_MS, toHistoryMessages, withTimeout } from './chat-history-panel-utils'
import { mergeHistoryMessages } from './chat-history-utils'

export function useChatHistoryData(sessionMessages: Message[]) {
  const { t } = useI18n()
  const [history, setHistory] = useState<Message[]>([])
  const [loading, setLoading] = useState(true)
  const [fetchError, setFetchError] = useState<string | null>(null)
  const fetchRequestRef = useRef(0)

  const fetchHistory = useCallback(async () => {
    const requestId = ++fetchRequestRef.current
    setLoading(true)
    setFetchError(null)
    try {
      const result = await withTimeout(getElectronApi().runtime.getChatHistory(), FETCH_TIMEOUT_MS)
      if (fetchRequestRef.current !== requestId) return
      setHistory(toHistoryMessages(result.messages || []))
    } catch (err) {
      if (fetchRequestRef.current !== requestId) return
      const isTimeout = err instanceof Error && err.message === 'timeout'
      setFetchError(isTimeout ? t('shell.history.timeout') : t('shell.history.loadFailed'))
    } finally {
      if (fetchRequestRef.current === requestId) setLoading(false)
    }
  }, [t])

  useEffect(() => {
    fetchHistory().catch((err: unknown) => rendererLog.error('[ChatHistory] fetch failed', { error: err }))
  }, [fetchHistory])

  useEffect(() => {
    if (!loading) {
      setHistory((current) => mergeHistoryMessages(current, sessionMessages))
    }
  }, [sessionMessages, loading])

  return { history, setHistory, loading, fetchError, fetchHistory }
}
