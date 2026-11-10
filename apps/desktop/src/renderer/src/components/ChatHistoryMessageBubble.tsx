import { toDisplayError, useCopyToClipboard } from '@mika/sdk-ui'
import { AlertCircle, Check, Copy, Flag, Mic, RefreshCw } from 'lucide-react'
import { memo, useCallback } from 'react'
import { useI18n } from '../hooks/useI18n'
import { highlightText } from '../lib/highlight-text'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { useUIStore } from '../store/useUIStore'
import type { ChatHistoryMessage } from './chat-history-utils'
import { FencedText } from './FencedText'
import { SparkIcon } from './Icons'

type Message = ChatHistoryMessage

// Re-exported for chat-history-components.tsx's barrel — the function itself now lives in
// lib/highlight-text.tsx so FencedText can import it without a circular dependency on this file.
export { highlightText }

const MESSAGE_SNIPPET_MAX_LENGTH = 40

function buildMessageSnippet(content: string): string {
  const trimmed = content.trim()
  if (!trimmed) return ''
  return trimmed.length > MESSAGE_SNIPPET_MAX_LENGTH ? `${trimmed.slice(0, MESSAGE_SNIPPET_MAX_LENGTH)}...` : trimmed
}

function withMessageSnippet(label: string, content: string): string {
  const snippet = buildMessageSnippet(content)
  return snippet ? `${label}: ${snippet}` : label
}

function CopyMessageBtn({ content }: { content: string }) {
  const { t } = useI18n()
  const { copied, copy } = useCopyToClipboard()

  const handleCopy = useCallback(() => {
    copy(content).catch((err) => rendererLog.warn('Copy failed:', err))
  }, [content, copy])

  const label = withMessageSnippet(copied ? t('button.copied') : t('button.copy'), content)

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="opacity-0 group-hover:opacity-100 focus:opacity-100 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus transition-opacity duration-150 p-1.5 rounded-md text-text-muted hover:text-text-main hover:bg-surface-raise-strong"
      title={label}
      aria-label={label}
    >
      {copied ? (
        <Check className="h-3 w-3 text-ok" aria-hidden="true" />
      ) : (
        <Copy className="h-3 w-3" aria-hidden="true" />
      )}
    </button>
  )
}

/** Saves the response as a local report (`reports/<ts>.json`); nothing leaves this computer. */
function ReportMessageBtn({ content }: { content: string }) {
  const { t } = useI18n()
  const label = withMessageSnippet(t('button.report'), content)

  const handleReport = useCallback(() => {
    const { addNotification } = useUIStore.getState()
    getElectronApi()
      .runtime.reportChatResponse(content)
      .then(({ file }) => addNotification('success', t('shell.history.reportSaved', { file })))
      .catch((error) => {
        rendererLog.warn('Report failed:', error)
        addNotification('error', toDisplayError(error, t('shell.history.reportError')))
      })
  }, [content, t])

  return (
    <button
      type="button"
      onClick={handleReport}
      className="opacity-0 group-hover:opacity-100 focus:opacity-100 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus transition-opacity duration-150 p-1.5 rounded-md text-text-muted hover:text-text-main hover:bg-surface-raise-strong"
      title={label}
      aria-label={label}
    >
      <Flag className="h-3 w-3" aria-hidden="true" />
    </button>
  )
}

function RegenerateMessageBtn({ content, onRegenerate }: { content: string; onRegenerate: () => void }) {
  const { t } = useI18n()
  const label = withMessageSnippet(t('button.regenerate'), content)

  return (
    <button
      type="button"
      onClick={onRegenerate}
      className="opacity-0 group-hover:opacity-100 focus:opacity-100 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus transition-opacity duration-150 p-1.5 rounded-md text-text-muted hover:text-text-main hover:bg-surface-raise-strong"
      title={label}
      aria-label={label}
    >
      <RefreshCw className="h-3 w-3" aria-hidden="true" />
    </button>
  )
}

function getRoleMeta(
  role: string,
  t: (k: 'shell.history.roleUser' | 'shell.history.roleSystem' | 'shell.history.roleMika') => string,
) {
  if (role === 'system') {
    return {
      label: t('shell.history.roleSystem'),
      labelClass: 'text-danger',
      style: 'w-full text-center border-danger-rim/40 bg-danger-soft/50 rounded-2xl',
      justify: 'justify-center w-full',
      textClass: 'text-danger font-mono text-xs',
    }
  }
  if (role === 'user') {
    return {
      label: t('shell.history.roleUser'),
      labelClass: 'text-info',
      style: 'max-w-[85%] rounded-2xl rounded-tr-sm border border-info-rim/40 bg-info-soft/50 shadow-sm',
      justify: 'justify-end',
      textClass: 'text-text-main/95',
    }
  }
  return {
    label: t('shell.history.roleMika'),
    labelClass: 'text-accent',
    style: 'max-w-[85%] rounded-2xl rounded-tl-sm border border-accent-rim/35 bg-accent-soft/40 shadow-sm',
    justify: 'justify-start',
    textClass: 'text-text-main/95',
  }
}

export const MessageBubble = memo(function MessageBubble({
  message,
  highlight,
  highlightRegex,
  onRegenerate,
}: {
  message: Message
  highlight?: string
  highlightRegex?: RegExp | null
  onRegenerate?: () => void
}) {
  const { t, locale } = useI18n()
  const isSystem = message.role === 'system'
  const isUser = message.role === 'user'
  const isAssistant = message.role === 'assistant'
  const meta = getRoleMeta(message.role, t)

  return (
    <article
      className={`flex ${meta.justify} animate-in fade-in slide-in-from-bottom-1.5 duration-250`}
      style={{ contentVisibility: 'auto', containIntrinsicSize: 'auto 100px' }}
    >
      <div
        className={`group relative px-4 py-3 transition-[transform,box-shadow] duration-150 ease-out backdrop-blur-md hover:-translate-y-0.5 hover:shadow-lg motion-reduce:transition-none ${meta.style}`}
      >
        <div className="mb-1.5 flex items-center gap-2 text-micro font-bold uppercase tracking-wider text-text-muted">
          {!isUser && !isSystem && <SparkIcon className="h-3 w-3 text-accent" aria-hidden="true" />}
          {isSystem && <AlertCircle className="h-3 w-3 text-danger" aria-hidden="true" />}
          <span className={meta.labelClass}>{meta.label}</span>
          {isUser && message.source === 'voice' && (
            <span
              className="text-info/80"
              role="img"
              title={t('shell.history.spokenTitle')}
              aria-label={t('shell.history.spokenTitle')}
            >
              <Mic className="h-3 w-3" aria-hidden="true" />
            </span>
          )}
          <div className="ml-auto flex items-center gap-0.5">
            {isAssistant && onRegenerate && (
              <RegenerateMessageBtn content={message.content} onRegenerate={onRegenerate} />
            )}
            {isAssistant && <ReportMessageBtn content={message.content} />}
            <CopyMessageBtn content={message.content} />
          </div>
        </div>
        <div className={`text-[13px] leading-relaxed whitespace-pre-wrap ${meta.textClass}`}>
          <FencedText content={message.content} highlight={highlight} highlightRegex={highlightRegex} />
        </div>
        <div
          className="mt-2 text-caption font-mono tracking-tight text-text-muted/80 font-medium"
          title={new Date(message.timestamp).toLocaleString(locale)}
        >
          {new Date(message.timestamp).toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })}
        </div>
      </div>
    </article>
  )
})
