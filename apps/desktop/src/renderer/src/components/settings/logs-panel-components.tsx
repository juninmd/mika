import type { LogItem } from '@mika/sdk/core/logger-format'
import { formatLocalTime } from '@mika/sdk-ui/settings'
import { ChevronRight } from 'lucide-react'
import { memo } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { LEVEL_ICON } from './logs-panel-filters'

export { LogToolbar } from './LogToolbar'

const HumanLogLine = memo(function HumanLogLine({
  log,
  onPickTrace,
}: {
  log: LogItem
  onPickTrace?: (traceId: string) => void
}) {
  const { t, locale } = useI18n()
  // "Humano" reads like a chat: local wall-clock time; the ISO stamp stays in the tooltip and in "Diagnóstico".
  const clock =
    log.at > 0
      ? formatLocalTime(log.at, locale, { hour: '2-digit', minute: '2-digit', second: '2-digit' })
      : log.timestamp
  return (
    <div className="group rounded-r border-l-2 border-accent-rim bg-accent-soft px-2 py-1 transition duration-100 hover:bg-surface-raise">
      <span className="mr-2 select-none font-mono text-caption text-text-subtle" title={log.timestamp}>
        {clock}
      </span>
      <span className="select-all whitespace-pre-wrap font-sans text-caption leading-relaxed text-text-main">
        {log.msg}
      </span>
      <RepeatBadge count={log.repeatCount} />
      {log.traceId && onPickTrace && (
        <button
          type="button"
          onClick={() => onPickTrace(log.traceId as string)}
          className="ml-2 rounded border border-border-subtle px-1.5 py-0.5 font-mono text-caption text-text-muted transition-colors hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          title={t('settings.logs.traceLineTitle')}
        >
          {log.traceId}
        </button>
      )}
    </div>
  )
})

function RepeatBadge({ count }: { count?: number }) {
  const { t } = useI18n()
  if (!count || count < 2) return null
  return (
    <span
      className="ml-2 rounded-full border border-border-subtle bg-surface-raise px-1.5 font-mono text-caption text-text-muted select-none"
      title={t('settings.logs.repeatCount').replace('{{count}}', String(count))}
    >
      ×{count}
    </span>
  )
}

export const LogLine = memo(function LogLine({
  log,
  onPickTrace,
}: {
  log: LogItem
  onPickTrace?: (traceId: string) => void
}) {
  const { t } = useI18n()
  if (log.audience === 'human') return <HumanLogLine log={log} onPickTrace={onPickTrace} />

  const isDebug = log.level === 'debug'
  const isError = log.level === 'error'
  const isWarn = log.level === 'warn'

  let rowStyle = 'text-text-main font-sans'
  let metaStyle = 'text-text-muted/80 font-mono text-caption block ml-6 mt-0.5'

  if (isDebug) {
    rowStyle = 'text-ok/60 font-mono text-caption'
    metaStyle = 'text-ok/40 block ml-6 mt-0.5'
  } else if (isError) {
    rowStyle = 'text-danger bg-danger-soft px-2 py-0.5 rounded border-l-2 border-danger block font-sans'
    metaStyle = 'text-danger/70 font-mono text-caption block mt-1'
  } else if (isWarn) {
    rowStyle = 'text-warn bg-warn-soft px-2 py-0.5 rounded border-l-2 border-warn block font-sans'
    metaStyle = 'text-warn/70 font-mono text-caption block mt-1'
  }

  return (
    <div className="group hover:bg-surface-raise p-0.5 rounded transition duration-100">
      <div className={rowStyle}>
        <span className="text-caption text-text-subtle select-none mr-2 font-mono">{log.timestamp}</span>
        <span className="mr-1.5 inline-flex select-none align-middle">
          {(() => {
            const LevelIcon = LEVEL_ICON[log.level]
            return <LevelIcon aria-hidden="true" size={12} />
          })()}
        </span>
        <span
          className={`mr-2 font-black ${isDebug ? 'text-ok/75' : 'text-text-muted'} border-r border-border-subtle pr-2`}
        >
          {log.scope}
        </span>
        <span className="whitespace-pre-wrap select-all font-medium leading-relaxed">{log.msg}</span>
        <RepeatBadge count={log.repeatCount} />
        {log.traceId && onPickTrace && (
          <button
            type="button"
            onClick={() => onPickTrace(log.traceId as string)}
            className="ml-2 rounded border border-border-subtle px-1 font-mono text-caption text-text-subtle transition-colors hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            title={t('settings.logs.traceLineTitle')}
          >
            {log.traceId}
          </button>
        )}
      </div>
      {log.meta && Object.keys(log.meta).length > 0 && (
        <span className={metaStyle}>
          <ChevronRight size={10} className="inline mr-1" />
          {JSON.stringify(log.meta)}
        </span>
      )}
    </div>
  )
})
