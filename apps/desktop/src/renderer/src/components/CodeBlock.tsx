import { useCopyToClipboard } from '@mika/sdk-ui'
import { Check, Copy } from 'lucide-react'
import { useCallback, useMemo } from 'react'
import { useI18n } from '../hooks/useI18n'
import { useUIStore } from '../store/useUIStore'

// Speech-bubble ("FALANDO") real estate is tiny (max-w ~18rem) — past either cap, `compact` mode
// truncates and offers "ver no chat" instead of pushing the avatar off-frame with a wall of code.
const COMPACT_MAX_LINES = 6
const COMPACT_MAX_CHARS = 320

function CopyCodeBtn({ code }: { code: string }) {
  const { t } = useI18n()
  const { copied, copy } = useCopyToClipboard()
  const handleCopy = useCallback(() => {
    copy(code).catch(() => {})
  }, [code, copy])
  const label = copied ? t('button.copied') : t('button.copy')

  return (
    <button
      type="button"
      onClick={handleCopy}
      title={label}
      aria-label={label}
      className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-raise-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
    >
      {copied ? (
        <Check className="h-3 w-3 text-ok" aria-hidden="true" />
      ) : (
        <Copy className="h-3 w-3" aria-hidden="true" />
      )}
    </button>
  )
}

export function InlineCode({ value }: { value: string }) {
  return <code className="rounded bg-surface-raise px-1 py-0.5 font-mono text-[0.9em] text-text-main">{value}</code>
}

export function CodeBlock({ lang, code, compact }: { lang: string | null; code: string; compact?: boolean }) {
  const { t } = useI18n()
  const lineCount = useMemo(() => code.split('\n').length, [code])
  const isTruncated = Boolean(compact) && (lineCount > COMPACT_MAX_LINES || code.length > COMPACT_MAX_CHARS)
  const shown = isTruncated
    ? lineCount > COMPACT_MAX_LINES
      ? code.split('\n').slice(0, COMPACT_MAX_LINES).join('\n')
      : code.slice(0, COMPACT_MAX_CHARS)
    : code
  const handleViewInChat = useCallback(() => useUIStore.getState().setShowHistory(true), [])

  return (
    <div className="my-1.5 overflow-hidden rounded-lg border border-border-subtle bg-surface-raise/60 not-italic">
      <div className="flex items-center justify-between gap-2 border-b border-border-subtle/60 px-2 py-1">
        <span className="truncate font-mono text-micro uppercase tracking-wide text-text-muted">{lang ?? 'code'}</span>
        <CopyCodeBtn code={code} />
      </div>
      <div className={compact ? 'max-h-40 overflow-y-auto' : undefined}>
        <pre className="overflow-x-auto px-2.5 py-2 text-xs leading-relaxed">
          <code className="font-mono text-text-main">{shown}</code>
        </pre>
      </div>
      {isTruncated && (
        <button
          type="button"
          onClick={handleViewInChat}
          className="w-full border-t border-border-subtle/60 px-2 py-1 text-center text-micro font-semibold text-accent hover:bg-surface-raise-strong focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          {t('shell.history.viewInChat')}
        </button>
      )}
    </div>
  )
}
