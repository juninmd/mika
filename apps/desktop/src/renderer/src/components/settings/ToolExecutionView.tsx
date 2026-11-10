import type { ToolDetail } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { maskSensitiveArgs } from '../../lib/sensitive-tool-arg'
import { getElectronApi } from '../../lib/shell-api'
import { useToolExecutionStore } from '../../store/useToolExecutionStore'
import { ArgsForm, ResultBlock } from './ToolExecutionArgs'
import { ToolExecutionHistory } from './ToolExecutionHistory'
import { type ArgMap, type ExecutionStatus, parseArgsBySchema } from './tool-execution-args'

type ToolExecutionViewProps = {
  tool: ToolDetail
  pluginId?: string
}

let executionCounter = 0

export function ToolExecutionView({ tool, pluginId }: ToolExecutionViewProps) {
  const { t } = useI18n()
  const [args, setArgs] = useState<ArgMap>({})
  const [status, setStatus] = useState<ExecutionStatus>('idle')
  const [result, setResult] = useState<unknown>(null)
  const [error, setError] = useState<string | null>(null)
  const [showHistory, setShowHistory] = useState(false)
  const resultRef = useRef<HTMLDivElement>(null)
  const addEntry = useToolExecutionStore((s) => s.addEntry)

  const handleArgChange = useCallback((name: string, value: string) => {
    setArgs((prev) => ({ ...prev, [name]: value }))
  }, [])

  const handleExecute = useCallback(async () => {
    const startedAt = Date.now()
    setStatus('executing')
    setResult(null)
    setError(null)

    const { parsed, error } = parseArgsBySchema(args, tool.inputSchema ?? [], t)
    if (error) {
      setStatus('error')
      setError(error)
      return
    }

    try {
      const api = getElectronApi()
      const res = await api.runtime.executeTool(tool.name, parsed)

      if (res && typeof res === 'object' && 'success' in res && res.success === false) {
        throw new Error((res as { error?: string }).error ?? t('settings.toolExecution.executionFailed'))
      }
      setResult(maskSensitiveArgs(res))
      setStatus('success')
      addEntry({
        id: `exec-${Date.now()}-${++executionCounter}`,
        toolName: tool.name,
        pluginId: pluginId ?? 'unknown',
        args: parsed,
        result: res,
        status: 'success',
        durationMs: Date.now() - startedAt,
        timestamp: startedAt,
      })
    } catch (err) {
      const message = toDisplayError(err, t('settings.toolExecution.executionFailed'))
      setError(message)
      setStatus('error')
      addEntry({
        id: `exec-${Date.now()}-${++executionCounter}`,
        toolName: tool.name,
        pluginId: pluginId ?? 'unknown',
        args: parsed,
        result: null,
        status: 'error',
        error: message,
        durationMs: Date.now() - startedAt,
        timestamp: startedAt,
      })
    }

    resultRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  }, [tool, pluginId, args, addEntry, t])

  const reset = useCallback(() => {
    setArgs({})
    setStatus('idle')
    setResult(null)
    setError(null)
  }, [])

  return (
    <div className="mt-4 space-y-3 border-t border-border-subtle pt-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="text-micro font-black uppercase tracking-[0.2em] text-text-muted">
            {t('settings.toolExecution.dataInterface')}
          </div>
          <div className="inline-flex items-center gap-1 rounded-md border border-border-subtle bg-surface-raise/40 px-2 py-1 text-micro font-extrabold uppercase tracking-wider text-text-muted">
            {tool.inputSchema.length} {t('settings.toolExecution.argsSuffix')}
          </div>
        </div>

        <div className="flex items-center gap-2">
          {status !== 'idle' && (
            <button
              type="button"
              onClick={reset}
              className="rounded-md border border-border-subtle bg-surface-raise px-2.5 py-1 text-micro font-bold uppercase tracking-wider text-text-muted transition-colors hover:border-border-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('settings.toolExecution.clear')}
            </button>
          )}
          <button
            type="button"
            onClick={handleExecute}
            disabled={status === 'executing'}
            className={`rounded-lg border px-3 py-1.5 text-micro font-black uppercase tracking-widest transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
              status === 'executing'
                ? 'border-info-rim bg-info-soft text-info cursor-wait'
                : 'border-info-rim bg-info-soft text-info hover:bg-info hover:text-white active:scale-95'
            }`}
          >
            {status === 'executing' ? t('settings.toolExecution.executing') : t('settings.toolExecution.executeButton')}
          </button>
        </div>
      </div>

      <ArgsForm params={tool.inputSchema} args={args} onChange={handleArgChange} />

      {tool.inputSchema.length === 0 && status === 'idle' && (
        <div className="rounded-lg border border-dashed border-border-subtle p-4 text-center">
          <p className="text-micro text-text-subtle uppercase font-bold tracking-widest">
            {t('settings.toolExecution.noParams')}
          </p>
        </div>
      )}

      <div ref={resultRef}>
        {status === 'success' && (
          <div role="status" className="space-y-2 rounded-lg border border-ok-rim bg-ok-soft p-3">
            <div className="flex items-center gap-2">
              <span aria-hidden="true" className="h-1.5 w-1.5 rounded-sm bg-ok shadow-sm shadow-ok/20" />
              <span className="text-micro font-black uppercase tracking-widest text-ok">
                {t('settings.toolExecution.result')}
              </span>
            </div>
            <ResultBlock data={result} />
          </div>
        )}

        {status === 'error' && (
          <div role="alert" className="rounded-lg border border-danger-rim bg-danger-soft p-3">
            <div className="flex items-center gap-2 mb-1.5">
              <span aria-hidden="true" className="h-1.5 w-1.5 rounded-sm bg-danger" />
              <span className="text-micro font-black uppercase tracking-widest text-danger">
                {t('settings.toolExecution.error')}
              </span>
            </div>
            <p className="text-caption leading-relaxed text-danger">{error}</p>
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={() => setShowHistory((p) => !p)}
        className="flex w-full items-center justify-center gap-2 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2 text-micro font-bold uppercase tracking-wider text-text-muted transition-colors hover:border-border-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <span aria-hidden="true">{showHistory ? '▴' : '▾'}</span> {t('settings.toolExecution.executionHistory')}
      </button>

      {showHistory && <ToolExecutionHistory pluginId={pluginId} toolName={tool.name} inline />}
    </div>
  )
}
