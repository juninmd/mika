import { toErrorMessage } from '@mika/sdk/utils/object'
import type React from 'react'
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'

export { createUiLogger, type UiLogEntry, type UiLogger, type UiLogLevel } from './logger'

export interface RendererApi {
  openSettings?: () => void
  mika: {
    executeTool(name: string, args: Record<string, unknown>): Promise<unknown>
    getSettingsConfig(): Promise<{ config: Record<string, unknown> }>
    saveSetting(path: string, value: unknown): Promise<unknown>
    sendChatMessage?: (message: string) => Promise<unknown>
  }
  rpc: {
    call<T = unknown>(method: string, args?: unknown): Promise<T>
  }
}

export interface PluginContextValue {
  pluginId: string
  api: RendererApi
}

const PluginContext = createContext<PluginContextValue | null>(null)

export const PluginProvider: React.FC<{ value: PluginContextValue; children: React.ReactNode }> = ({
  value,
  children,
}) => {
  return <PluginContext.Provider value={value}>{children}</PluginContext.Provider>
}

export function usePluginContext() {
  const context = useContext(PluginContext)
  if (!context) {
    throw new Error('usePluginContext must be used within a PluginProvider')
  }
  return context
}

type ToolExecutionResult<T> = {
  success?: boolean
  data?: T
  error?: string
}

function unwrapToolData<T>(result: ToolExecutionResult<T> | null | undefined): T | null {
  if (!result || result.success === false) return null

  const hasEnvelope = typeof result === 'object' && Object.hasOwn(result, 'data')
  return (hasEnvelope ? result.data : result) as T
}

function runToolCall(toolName: string, args: Record<string, unknown>, executeTool: RendererApi['mika']['executeTool']) {
  const requestKey = `tool:${toolName}:${stableArgsKey(args)}`
  const pending =
    inFlightToolCalls.get(requestKey) ??
    executeTool(toolName, args).finally(() => {
      inFlightToolCalls.delete(requestKey)
    })
  if (!inFlightToolCalls.has(requestKey)) inFlightToolCalls.set(requestKey, pending)
  return { requestKey, pending }
}

const EMPTY_ARGS: Record<string, unknown> = Object.freeze({})
const inFlightToolCalls = new Map<string, Promise<unknown>>()

function stableArgsKey(args: Record<string, unknown>): string {
  try {
    return JSON.stringify(args)
  } catch {
    return String(Object.keys(args).sort())
  }
}

const resultCache = new Map<string, unknown>()
const RESULT_CACHE_LIMIT = 50

function setCachedResult(key: string, value: unknown) {
  if (resultCache.has(key)) resultCache.delete(key)
  resultCache.set(key, value)
  if (resultCache.size > RESULT_CACHE_LIMIT) {
    const oldestKey = resultCache.keys().next().value
    if (oldestKey !== undefined) resultCache.delete(oldestKey)
  }
}

export function usePluginTool<T = unknown>(toolName: string, initialArgs: Record<string, unknown> = EMPTY_ARGS) {
  const { api } = usePluginContext()
  const initialArgsKey = useMemo(() => stableArgsKey(initialArgs), [initialArgs])
  const cacheKey = `tool:${toolName}:${initialArgsKey}`

  const [data, setData] = useState<T | null>((resultCache.get(cacheKey) as T) ?? null)
  const [loading, setLoading] = useState(!resultCache.has(cacheKey))
  const [error, setError] = useState<string | null>(null)
  const initialArgsRef = useRef(initialArgs)
  const callIdRef = useRef(0)

  useEffect(() => {
    initialArgsRef.current = initialArgs
  }, [initialArgs])

  const execute = useCallback(
    async (args?: Record<string, unknown>) => {
      const callId = ++callIdRef.current
      setLoading(true)
      try {
        const nextArgs = args ?? initialArgsRef.current
        const { requestKey, pending } = runToolCall(toolName, nextArgs, api.mika.executeTool)
        const result = (await pending) as ToolExecutionResult<T> | undefined
        if (callId !== callIdRef.current) return
        if (result && result.success === false) {
          setError(result.error || `Tool ${toolName} failed`)
          setData(null)
          return
        }
        const finalData = unwrapToolData(result)
        setData(finalData)
        setCachedResult(requestKey, finalData)
        setError(null)
      } catch (err) {
        if (callId !== callIdRef.current) return
        setError(toErrorMessage(err))
      } finally {
        if (callId === callIdRef.current) setLoading(false)
      }
    },
    [api, toolName],
  )

  const executeRef = useRef(execute)
  useEffect(() => {
    executeRef.current = execute
  }, [execute])

  const prevCacheKeyRef = useRef(cacheKey)
  useEffect(() => {
    if (prevCacheKeyRef.current === cacheKey) return
    prevCacheKeyRef.current = cacheKey

    callIdRef.current += 1
    setData((resultCache.get(cacheKey) as T) ?? null)
    setLoading(!resultCache.has(cacheKey))
    setError(null)
  }, [cacheKey])

  const lastExecutedKeyRef = useRef<string | null>(null)

  useEffect(() => {
    if (lastExecutedKeyRef.current === cacheKey) return
    lastExecutedKeyRef.current = cacheKey
    void executeRef.current()
  }, [cacheKey])

  return { data, loading, error, refresh: execute }
}

interface UseCopyToClipboardResult {
  copied: boolean
  copy: (text: string) => Promise<void>
}

export function useCopyToClipboard(resetDelayMs = 1800): UseCopyToClipboardResult {
  const [copied, setCopied] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(
    () => () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    },
    [],
  )

  const copy = useCallback(
    async (text: string) => {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => setCopied(false), resetDelayMs)
    },
    [resetDelayMs],
  )

  return { copied, copy }
}

interface MikaUIConfig {
  views?: Record<string, React.ComponentType>
  settingsPanels?: Record<string, React.ComponentType>
}

export function defineMikaUI(config: MikaUIConfig) {
  return config
}
export { setBounded } from './utils/bounded-cache'
export { isNetworkError, toDisplayError } from './utils/error-message'
export { getByPath } from './utils/get-by-path'
export { formatLocalDate, formatLocalNumber, formatLocalTime } from './utils/locale-format'
