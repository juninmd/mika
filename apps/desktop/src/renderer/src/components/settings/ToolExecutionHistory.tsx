import { formatLocalTime } from '@mika/sdk-ui'
import { Trash2 } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { useToolExecutionStore } from '../../store/useToolExecutionStore'
import { ConfirmButton, EmptyState, SectionHeader } from './ds'

type ToolExecutionHistoryProps = {
  pluginId?: string
  toolName?: string
  inline?: boolean
}

function formatTimestamp(ts: number, locale: string): string {
  return formatLocalTime(ts, locale, { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}

const MAX_ERROR_LENGTH = 200

function sanitizeError(error: string): string {
  const firstLine = error.split('\n')[0] ?? error
  return firstLine.length > MAX_ERROR_LENGTH ? `${firstLine.slice(0, MAX_ERROR_LENGTH)}…` : firstLine
}

export function ToolExecutionHistory({ pluginId, toolName, inline }: ToolExecutionHistoryProps) {
  const { t, locale } = useI18n()
  const entries = useToolExecutionStore((s) => s.entries)
  const clear = useToolExecutionStore((s) => s.clear)
  const clearPlugin = useToolExecutionStore((s) => s.clearPlugin)

  const filtered = entries.filter((e) => {
    if (pluginId && e.pluginId !== pluginId) return false
    if (toolName && e.toolName !== toolName) return false
    return true
  })

  if (filtered.length === 0) {
    return inline ? (
      <EmptyState title={t('settings.toolHistory.empty')} />
    ) : (
      <section className="rounded-[1.5rem] border border-border-subtle bg-surface-raise shadow-sm p-4 md:p-5">
        <EmptyState title={t('settings.toolHistory.empty')} />
      </section>
    )
  }

  if (inline) {
    return (
      <div className="space-y-1">
        {filtered.map((entry) => (
          <div
            key={entry.id}
            className="flex items-center gap-2 rounded-lg border border-border-subtle bg-surface-raise px-2.5 py-1.5"
          >
            <div
              aria-hidden="true"
              className={`h-1.5 w-1.5 shrink-0 rounded-sm ${
                entry.status === 'success' ? 'bg-ok shadow-sm shadow-ok/20' : 'bg-danger'
              }`}
            />
            <code className="text-caption font-bold text-info min-w-[100px] truncate">{entry.toolName}</code>
            <span className="text-caption text-text-muted min-w-[48px]">
              {formatTimestamp(entry.timestamp, locale)}
            </span>
            <span className="text-caption text-text-subtle min-w-[36px]">{formatDuration(entry.durationMs)}</span>
            {entry.status === 'error' && entry.error && (
              <span className="text-caption text-danger truncate flex-1 ml-1">{sanitizeError(entry.error)}</span>
            )}
          </div>
        ))}
      </div>
    )
  }

  return (
    <section className="rounded-[1.5rem] border border-border-subtle bg-surface-raise shadow-sm p-4 md:p-5">
      <SectionHeader
        title={t('settings.toolExecution.executionHistory')}
        headingClassName="text-base font-bold tracking-tight md:text-lg"
        badge={String(filtered.length)}
        actions={
          <div className="flex gap-2">
            {pluginId && (
              <ConfirmButton
                icon={Trash2}
                label={t('settings.toolHistory.clearPlugin')}
                confirmLabel={t('button.confirm')}
                ariaLabel={t('settings.toolHistory.clearPluginAria')}
                onConfirm={() => clearPlugin(pluginId)}
              />
            )}
            <ConfirmButton
              icon={Trash2}
              label={t('settings.toolHistory.clearAll')}
              confirmLabel={t('button.confirm')}
              ariaLabel={t('settings.toolHistory.clearAllAria')}
              onConfirm={clear}
            />
          </div>
        }
        className="mb-3"
      />

      <div className="space-y-1.5">
        {filtered.map((entry) => {
          const resultText = entry.formattedResult
          return (
            <div
              key={entry.id}
              className="rounded-lg border border-border-subtle bg-surface-raise px-3 py-2 transition-colors hover:border-border-strong"
            >
              <div className="flex items-center gap-3">
                <div
                  aria-hidden="true"
                  className={`h-1.5 w-1.5 shrink-0 rounded-sm ${
                    entry.status === 'success' ? 'bg-ok shadow-sm shadow-ok/20' : 'bg-danger'
                  }`}
                />
                <code className="text-caption font-bold text-info min-w-[120px] truncate">{entry.toolName}</code>
                <span className="text-caption text-text-muted min-w-[52px]">
                  {formatTimestamp(entry.timestamp, locale)}
                </span>
                <span className="text-caption text-text-subtle min-w-[40px]">{formatDuration(entry.durationMs)}</span>
                {entry.status === 'error' && entry.error && (
                  <span className="text-caption text-danger truncate flex-1">{sanitizeError(entry.error)}</span>
                )}
              </div>
              {resultText && (
                <pre className="mt-2 max-h-40 overflow-auto rounded-md border border-border-subtle bg-surface-raise/40 p-2 text-caption font-mono leading-relaxed text-text-main [scrollbar-width:thin] [scrollbar-color:var(--primary)_transparent]">
                  {resultText}
                </pre>
              )}
            </div>
          )
        })}
      </div>
    </section>
  )
}
