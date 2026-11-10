import { toDisplayError, useCopyToClipboard } from '@mika/sdk-ui'
import { SPRING } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { Check, Copy, History, X } from 'lucide-react'
import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useI18n } from '../hooks/useI18n'
import { useTimeoutRef } from '../hooks/useTransientFlag'
import { useChatStore } from '../store/useChatStore'
import { useUIStore } from '../store/useUIStore'
import type { ActiveToolState, AppStatus } from './app/app-shell.types'
import { FencedText } from './FencedText'
import { bubbleHideDelayMs } from './hud/bubble-timing'
import { emotionAccent } from './hud/emotion-accent'
import type { BubbleAnchor } from './hud/hud-layout'
import { ThinkingDots } from './SpeechThinkingDots'
import { SPEECH_STATUS_DOT as STATUS_DOT, SPEECH_STATUS_LABELS as STATUS_LABELS } from './speech-status'

type SpeechBubbleProps = {
  status: AppStatus
  activeTool?: ActiveToolState
  onRetry?: () => void
  anchor: BubbleAnchor
  expression?: string
}

const PEEK_COUNT = 2
const ICON_BTN =
  'flex h-6 w-6 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-raise hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus'

/** Fades the bubble after a reading time proportional to its length — never mid-speech, never under the pointer. */
function useBubbleAutoHide(status: AppStatus, text: string, visible: boolean, hovered: boolean) {
  const shownAtRef = useRef(Date.now())
  const { set, clear } = useTimeoutRef()
  // Reading time counts from the last change of what is on screen: a new reply, the last streamed
  // chunk, or the bubble reappearing. The time she spends speaking an unchanged text counts too.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `text` is trigger-only — a new text restarts the reading clock.
  useEffect(() => {
    if (visible) shownAtRef.current = Date.now()
  }, [visible, text])
  useEffect(() => {
    clear()
    if (!visible) return
    const delay = bubbleHideDelayMs({ status, text, shownForMs: Date.now() - shownAtRef.current, hovered })
    if (delay !== null) set(() => useChatStore.getState().setBubbleVisible(false), delay)
  }, [status, text, visible, hovered, set, clear])
}

/** Streaming reveal: only the newly arrived tail fades in, so a growing reply reads smoothly instead of jumping. */
function RevealText({ text, streaming }: { text: string; streaming: boolean }) {
  const shownRef = useRef(0)
  const prev = shownRef.current
  useEffect(() => {
    shownRef.current = text.length
  }, [text])
  if (text.includes('`')) return <FencedText content={text} compact />
  const split = streaming && prev > 0 && prev < text.length ? prev : text.length
  return (
    <>
      {text.slice(0, split)}
      {split < text.length && (
        <span key={split} className="hud-reveal">
          {text.slice(split)}
        </span>
      )}
    </>
  )
}

/** The last exchanges before this reply, shown when the pointer rests on the bubble. */
function HistoryPeek({ current }: { current: string }) {
  const { t } = useI18n()
  const recent = useChatStore(
    useShallow((s) => {
      const list = s.messages.at(-1)?.content === current ? s.messages.slice(0, -1) : s.messages
      return list.slice(-PEEK_COUNT)
    }),
  )
  if (recent.length === 0) return null
  return (
    <div className="mb-2 flex flex-col gap-1 border-b border-border-subtle pb-2">
      {recent.map((m) => (
        <p key={m.id} className="truncate text-caption leading-snug text-text-muted">
          <span className="font-semibold text-text-subtle">
            {m.role === 'user' ? t('shell.bubble.you') : t('shell.bubble.mika')}:
          </span>{' '}
          {m.content}
        </p>
      ))}
      <button
        type="button"
        onClick={() => useUIStore.getState().toggleOverlay('history')}
        className="flex items-center gap-1 self-start rounded-md px-1 py-0.5 text-caption font-semibold text-accent transition-colors hover:bg-accent-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <History aria-hidden="true" className="size-3" />
        {t('shell.bubble.openHistory')}
      </button>
    </div>
  )
}

function BubbleTail({ anchor }: { anchor: BubbleAnchor }) {
  // Drawn under the body, so only the outward half of the rotated square shows: a tail that points
  // at her mouth (side bubble) or her chin (bubble below).
  const style =
    anchor.side === 'right'
      ? { left: -5, top: anchor.tailOffset - 5 }
      : anchor.side === 'left'
        ? { right: -5, top: anchor.tailOffset - 5 }
        : { left: `calc(50% + ${anchor.tailOffset}px - 5px)`, top: -5 }
  return (
    <span aria-hidden="true" className="hud-glass-strong absolute h-2.5 w-2.5 rotate-45 rounded-[2px]" style={style} />
  )
}

function BubbleHeader({ status, text, onDismiss }: { status: AppStatus; text: string; onDismiss: () => void }) {
  const { t } = useI18n()
  const { copied, copy } = useCopyToClipboard()
  const handleCopy = useCallback(() => {
    copy(text).catch((err: unknown) => {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('button.copyFailed')))
    })
  }, [text, copy, t])
  return (
    <div className="mb-1.5 flex items-center justify-between gap-3">
      <div className="flex min-w-0 items-center gap-1.5 whitespace-nowrap text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
        <span className={`h-1.5 w-1.5 rounded-full ${STATUS_DOT[status]}`} aria-hidden="true" />
        {/* At rest the header names who is talking; "listening"/"idle" over her own reply read as nonsense. */}
        <span>{t(status === 'idle' || status === 'listening' ? 'shell.bubble.mika' : STATUS_LABELS[status])}</span>
      </div>
      {text && (
        <div className="-mr-1 flex shrink-0 items-center gap-0.5">
          <button
            type="button"
            onClick={handleCopy}
            title={copied ? t('button.copied') : t('button.copy')}
            aria-label={copied ? t('button.copied') : t('button.copy')}
            className={ICON_BTN}
          >
            {copied ? (
              <Check className="h-3 w-3 text-ok" aria-hidden="true" />
            ) : (
              <Copy className="h-3 w-3" aria-hidden="true" />
            )}
          </button>
          <button
            type="button"
            onClick={onDismiss}
            title={t('button.close')}
            aria-label={t('button.close')}
            className={ICON_BTN}
          >
            <X className="h-3 w-3" aria-hidden="true" />
          </button>
        </div>
      )}
    </div>
  )
}

function useMountedWhile(show: boolean): boolean {
  const { set, clear } = useTimeoutRef()
  const [mounted, setMounted] = useState(show)
  useEffect(() => {
    if (show) {
      clear()
      setMounted(true)
      return
    }
    set(() => setMounted(false), 400)
  }, [show, clear, set])
  return mounted
}

export const SpeechBubble = memo(function SpeechBubble({
  status,
  activeTool,
  onRetry,
  anchor,
  expression,
}: SpeechBubbleProps) {
  const { t } = useI18n()
  const textRef = useRef<HTMLDivElement>(null)
  const bubbleText = useChatStore((s) => s.bubbleText)
  const bubbleVisible = useChatStore((s) => s.bubbleVisible)
  const [hovered, setHovered] = useState(false)

  // Um erro tem de aparecer mesmo com o balão recolhido: `handleSendError` fecha o balão e deixa
  // `status` em `error` por alguns segundos, e o botão de retry precisa ser clicável.
  const isVisible = bubbleVisible || status === 'error'
  const mounted = useMountedWhile(bubbleVisible || status !== 'idle')
  useBubbleAutoHide(status, bubbleText, bubbleVisible, hovered)

  // biome-ignore lint/correctness/useExhaustiveDependencies: bubbleText is a trigger-only dep — it re-runs the scroll-to-bottom whenever new text streams in, even though the body reads textRef, not bubbleText.
  useEffect(() => {
    if (textRef.current) textRef.current.scrollTop = textRef.current.scrollHeight
  }, [bubbleText])

  const handleDismiss = useCallback(() => useChatStore.getState().setBubbleVisible(false), [])

  if (!mounted) return null

  const accent = emotionAccent(expression)
  const busy = status === 'thinking' || status === 'acting'
  const showDots = busy && (!bubbleText || status === 'acting')
  // A left-side bubble grows leftward from her head: anchor its right edge, not its left.
  const wrapper =
    anchor.side === 'right'
      ? { left: anchor.x, top: anchor.y }
      : anchor.side === 'left'
        ? { right: `calc(100% - ${anchor.x + anchor.maxWidth}px)`, top: anchor.y }
        : { left: anchor.x, top: anchor.y, transform: 'translateX(-50%)' }

  return (
    <div aria-live="polite" aria-atomic="true" className="pointer-events-none absolute" style={wrapper}>
      <motion.div
        initial={false}
        animate={isVisible ? { opacity: 1, y: 0, scale: 1 } : { opacity: 0, y: 6, scale: 0.98 }}
        transition={SPRING.soft}
        style={{ transformOrigin: { right: 'left top', left: 'right top', below: 'center top' }[anchor.side] }}
        className={`relative ${isVisible ? 'pointer-events-auto' : 'pointer-events-none'}`}
        onPointerEnter={() => setHovered(true)}
        onPointerLeave={() => setHovered(false)}
      >
        <BubbleTail anchor={anchor} />
        <div
          className="hud-glass-strong relative overflow-hidden rounded-2xl px-3.5 pb-3 pt-2.5"
          style={{ width: 'max-content', maxWidth: anchor.maxWidth, minWidth: Math.min(160, anchor.maxWidth) }}
        >
          <span
            aria-hidden="true"
            className="absolute inset-x-3 top-0 h-[2px] rounded-full"
            style={{ background: `linear-gradient(90deg, transparent, ${accent}, transparent)` }}
          />
          {hovered && !busy && <HistoryPeek current={bubbleText} />}
          <BubbleHeader status={status} text={bubbleText} onDismiss={handleDismiss} />

          {bubbleText && (
            // `<div>`, not `<p>`: a fenced code block renders a `<div>` (CodeBlock), and a block element
            // inside a `<p>` gets closed early by the parser, truncating what auto-scroll sees.
            <div
              ref={textRef}
              className="hud-halo m-0 overflow-y-auto break-words text-sm leading-relaxed text-text-main [overflow-wrap:anywhere] [scrollbar-width:thin]"
              style={{ maxHeight: Math.max(64, anchor.maxHeight - 56) }}
            >
              <RevealText text={bubbleText} streaming={busy} />
            </div>
          )}
          {showDots && <ThinkingDots activeTool={activeTool} />}

          {status === 'error' && onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="mt-2.5 min-h-8 rounded-lg border border-danger-rim bg-danger-soft px-3.5 py-1.5 text-caption font-bold uppercase tracking-[0.12em] text-danger transition hover:bg-danger hover:text-white active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('button.retry')}
            </button>
          )}
        </div>
      </motion.div>
    </div>
  )
})
