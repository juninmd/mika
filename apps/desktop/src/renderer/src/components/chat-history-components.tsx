import { LazyList } from '@mika/sdk-ui/settings'
import { AlertCircle, Download } from 'lucide-react'
import type { ReactNode } from 'react'
import { useCallback, useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import { MessageBubble } from './ChatHistoryMessageBubble'
import type { ChatHistoryMessage } from './chat-history-utils'
import { MessageIcon } from './Icons'

export { highlightText } from './ChatHistoryMessageBubble'

export type Message = ChatHistoryMessage

export function ActionBtn({
  children,
  onClick,
  danger,
  title,
}: {
  children: ReactNode
  onClick: () => void
  danger?: boolean
  title?: string
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      className={`flex h-10 w-10 items-center justify-center rounded-xl border transition active:scale-95 focus-visible:ring-2 focus-visible:ring-focus focus-visible:outline-none ${
        danger
          ? 'border-danger-rim bg-danger-soft text-danger hover:border-danger hover:bg-danger hover:text-white'
          : 'border-border-subtle bg-surface-raise text-text-muted hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main'
      }`}
    >
      {children}
    </button>
  )
}

export function HistoryScrollContent({
  scrollRef,
  loading,
  fetchError,
  filtered,
  search,
  highlightRegex,
  onRetry,
  onRegenerateLast,
}: {
  scrollRef: React.RefObject<HTMLDivElement | null>
  loading: boolean
  fetchError: string | null
  filtered: Message[]
  search: string
  highlightRegex?: RegExp | null
  onRetry?: () => void
  onRegenerateLast?: () => void
}) {
  const { t } = useI18n()

  return (
    <div
      ref={scrollRef}
      role="log"
      aria-live="polite"
      aria-label={t('shell.history.messagesRegionLabel')}
      className="flex-1 overflow-y-auto px-6 py-6 space-y-4 scrollbar-thin scrollbar-thumb-surface-raise-strong scrollbar-track-transparent"
    >
      {loading ? (
        <div className="flex h-full flex-col items-center justify-center gap-3 text-text-muted">
          <div
            className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"
            aria-hidden="true"
          />
          <span className="text-xs">{t('shell.history.loading')}</span>
        </div>
      ) : fetchError ? (
        <div className="flex h-full flex-col items-center justify-center gap-3 text-danger">
          <AlertCircle className="h-8 w-8 text-danger" aria-hidden="true" />
          <span className="text-sm font-medium">{fetchError}</span>
          {onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="mt-1 rounded-lg border border-border-subtle bg-surface-raise px-3 py-1 text-xs text-text-main hover:bg-surface-raise-strong focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('button.retry')}
            </button>
          )}
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex h-full flex-col items-center justify-center gap-3 text-text-muted">
          <MessageIcon className="h-8 w-8 text-text-muted/30" aria-hidden="true" />
          <span className="text-sm">
            {search.trim() ? t('shell.history.emptySearch') : t('shell.history.emptyHistory')}
          </span>
        </div>
      ) : (
        <LazyList
          items={filtered}
          itemKey={(msg) => msg.id || `${msg.timestamp}-${msg.role}`}
          estimatedItemHeight={72}
          renderItem={(msg, index) => (
            <MessageBubble
              key={msg.id || `${msg.timestamp}-${msg.role}`}
              message={msg}
              highlight={search}
              highlightRegex={highlightRegex}
              onRegenerate={index === filtered.length - 1 ? onRegenerateLast : undefined}
            />
          )}
        />
      )}
    </div>
  )
}

export function ChatHistoryExportMenu({
  onExportJSON,
  onExportMarkdown,
}: {
  onExportJSON: () => void
  onExportMarkdown: () => void
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      setOpen(false)
      e.stopPropagation()
    }
  }, [])

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: this wrapper only catches the Escape key bubbling from its focusable button/menu descendants to close the dropdown — it is not itself an interactive control.
    <div className="relative" onKeyDown={handleKeyDown}>
      <ActionBtn title={t('shell.history.export')} onClick={() => setOpen((prev) => !prev)}>
        <Download className="h-4 w-4" aria-hidden="true" />
      </ActionBtn>
      {open && (
        <>
          <div className="fixed inset-0 z-40 cursor-default" aria-hidden="true" onClick={() => setOpen(false)} />
          <div
            role="menu"
            aria-label={t('shell.history.export')}
            className="absolute right-0 top-12 z-50 flex w-40 flex-col gap-1 rounded-xl border border-border-subtle bg-bg-surface p-1.5 shadow-2xl backdrop-blur-xl animate-in fade-in zoom-in-95 duration-150"
          >
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                onExportJSON()
                setOpen(false)
              }}
              className="flex w-full items-center gap-2 rounded-lg px-3 py-1.5 text-left text-xs font-semibold text-text-muted transition-colors hover:bg-surface-raise-strong hover:text-text-main active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('shell.history.exportJson')}
            </button>
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                onExportMarkdown()
                setOpen(false)
              }}
              className="flex w-full items-center gap-2 rounded-lg px-3 py-1.5 text-left text-xs font-semibold text-text-muted transition-colors hover:bg-surface-raise-strong hover:text-text-main active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('shell.history.exportMd')}
            </button>
          </div>
        </>
      )}
    </div>
  )
}
