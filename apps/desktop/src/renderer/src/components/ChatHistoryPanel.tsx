import { toDisplayError } from '@mika/sdk-ui'
import { ConfirmButton } from '@mika/sdk-ui/settings'
import { Trash2 } from 'lucide-react'
import type { CSSProperties } from 'react'
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react'
import { useFocusTrap } from '../hooks/useFocusTrap'
import { useI18n } from '../hooks/useI18n'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { useUIStore } from '../store/useUIStore'
import { ActionBtn, ChatHistoryExportMenu, HistoryScrollContent, type Message } from './chat-history-components'
import { downloadBlob, historyToMarkdown } from './chat-history-panel-utils'
import { HistorySearchBar } from './chat-history-search-bar'
import { CloseIcon } from './Icons'
import { useChatHistoryData } from './use-chat-history-data'
import { useHistoryAutoScroll, useHistoryEscape } from './use-chat-history-scroll'

type ChatHistoryPanelProps = {
  messages: Message[]
  onClose: () => void
  onClear: () => void
  onRegenerateLast?: () => void
}

export function ChatHistoryPanel({
  messages: sessionMessages,
  onClose,
  onClear,
  onRegenerateLast,
}: ChatHistoryPanelProps) {
  const { t, locale } = useI18n()
  const [search, setSearch] = useState('')
  const searchRef = useRef<HTMLInputElement>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)
  const handleDialogTab = useFocusTrap(dialogRef)
  const { history, setHistory, loading, fetchError, fetchHistory } = useChatHistoryData(sessionMessages)

  useEffect(() => {
    searchRef.current?.focus()
  }, [])

  useHistoryAutoScroll(scrollRef, search, history.length)
  useHistoryEscape(search, setSearch, onClose)

  const exportJSON = useCallback(() => {
    const blob = new Blob([JSON.stringify(history, null, 2)], { type: 'application/json' })
    downloadBlob(blob, 'json')
  }, [history])

  const exportMarkdown = useCallback(() => {
    const md = historyToMarkdown(history, t, locale)
    const blob = new Blob([md], { type: 'text/markdown' })
    downloadBlob(blob, 'md')
  }, [history, t, locale])

  const deferredSearch = useDeferredValue(search)
  const filtered = useMemo(() => {
    if (!deferredSearch.trim()) return history
    const q = deferredSearch.trim().toLowerCase()
    return history.filter((m) => m.content.toLowerCase().includes(q))
  }, [history, deferredSearch])

  // `highlightText` recebe a régua de recorte; sem ela o painel filtra mas nunca marca o trecho.
  // Grupo de captura para o `split` preservar o casamento, e escape para o texto virar literal.
  const highlightRegex = useMemo(() => {
    const q = deferredSearch.trim()
    if (!q) return null
    return new RegExp(`(${q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi')
  }, [deferredSearch])

  const userCount = useMemo(() => history.filter((m) => m.role === 'user').length, [history])
  const mikaCount = history.length - userCount

  return (
    <>
      <button
        type="button"
        className="fixed inset-0 z-40 cursor-default border-0 bg-surface-raise/20 p-0 backdrop-blur-[1px] transition-opacity animate-in fade-in duration-300"
        style={{ WebkitAppRegion: 'no-drag' } as CSSProperties}
        onClick={onClose}
        onKeyDown={(e) => e.key === 'Escape' && onClose()}
        aria-label={t('shell.history.close')}
      />

      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="chat-history-title"
        className="fixed right-0 top-0 z-50 flex h-full w-[min(92vw,400px)] flex-col border-l border-border-subtle bg-bg-surface/90 backdrop-blur-[100px] rounded-l-[2rem] text-text-main shadow-[0_8px_32px_rgba(0,0,0,0.15)] transition-transform animate-in slide-in-from-right duration-300 ease-out"
        style={{ WebkitAppRegion: 'no-drag' } as CSSProperties}
        onKeyDown={(e) => {
          // Swallowed here (React stops the native event at the root) so the shell's Escape binding does not
          // close the panel on the same press: with a search typed, Esc clears it first.
          if (e.key === 'Escape' && search) {
            e.stopPropagation()
            setSearch('')
            return
          }
          handleDialogTab(e)
        }}
      >
        <div className="flex items-center justify-between border-b border-border-subtle px-6 py-5">
          <div>
            <div className="inline-flex items-center gap-1.5 rounded-md border border-accent-rim/40 bg-accent-soft px-2.5 py-1 text-micro font-extrabold uppercase tracking-widest text-accent shadow-xs">
              {t('shell.history.badge')}
            </div>
            <div id="chat-history-title" className="mt-2 text-xl font-bold tracking-[-0.03em]">
              {t('shell.history.title')}
            </div>
          </div>
          <div className="flex gap-2">
            {history.length > 0 && (
              <ChatHistoryExportMenu onExportJSON={exportJSON} onExportMarkdown={exportMarkdown} />
            )}
            {history.length > 0 && (
              <ConfirmButton
                label=""
                confirmLabel=""
                ariaLabel={t('shell.history.clearAria')}
                icon={Trash2}
                onConfirm={async () => {
                  try {
                    await getElectronApi().runtime.clearChatHistory()
                    setHistory([])
                    setSearch('')
                    onClear()
                    useUIStore.getState().addNotification('success', t('shell.history.cleared'))
                  } catch (err) {
                    rendererLog.error('[ChatHistory] clear failed', { error: err })
                    useUIStore.getState().addNotification('error', toDisplayError(err, t('shell.history.clearFailed')))
                  }
                }}
              />
            )}
            <ActionBtn onClick={onClose} title={t('button.close')}>
              <CloseIcon className="h-4 w-4" aria-hidden="true" />
            </ActionBtn>
          </div>
        </div>

        <HistorySearchBar inputRef={searchRef} value={search} onChange={setSearch} />

        <HistoryScrollContent
          scrollRef={scrollRef}
          loading={loading}
          fetchError={fetchError}
          filtered={filtered}
          search={deferredSearch}
          highlightRegex={highlightRegex}
          onRetry={() =>
            fetchHistory().catch((err: unknown) => rendererLog.error('[ChatHistory] fetch failed', { error: err }))
          }
          onRegenerateLast={search.trim() ? undefined : onRegenerateLast}
        />

        <div className="border-t border-border-subtle px-6 py-4 flex items-center justify-between">
          <div
            role="status"
            aria-live="polite"
            className="text-micro text-text-muted font-medium uppercase tracking-widest"
          >
            {search
              ? t('shell.history.resultsCount', { count: filtered.length })
              : t('shell.history.interactionsCount', { count: history.length })}
          </div>
          {!search && history.length > 0 && (
            <div className="flex gap-2 text-caption text-text-muted">
              <span className="rounded-full border border-border-subtle bg-surface-raise/50 px-2 py-0.5 font-medium">
                {t('shell.history.roleUser')}: {userCount}
              </span>
              <span className="rounded-full border border-border-subtle bg-surface-raise/50 px-2 py-0.5 font-medium">
                {t('shell.history.roleMika')}: {mikaCount}
              </span>
            </div>
          )}
        </div>
      </div>
    </>
  )
}
