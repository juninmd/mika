import type { ChatMessage } from '@mika/sdk'
import { BellRing, BookOpen, CalendarCheck, Lightbulb, Trophy, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { bubbleDwellMs } from '../../hooks/mika-events-helpers'
import { useI18n } from '../../hooks/useI18n'
import { type DockCardContent, dockCard } from '../../lib/dock-cards'
import { getElectronApi } from '../../lib/shell-api'
import { isForegroundToolEvent } from '../../lib/tool-call-origin'
import { BUBBLE_MIN_TAIL_MS } from '../hud/bubble-timing'

const MAX_CARDS = 3
/** A card stays this long after it arrived (paused while the pointer rests on the dock). */
const CARD_MS = 20_000

type Bubble = { id: string; text: string }
type Card = DockCardContent & { id: string }

/** What floats beside the dock while the compact chat is closed: her last words, and result cards. */
export type DockFeedState = {
  bubble: Bubble | null
  cards: Card[]
  dismissBubble: () => void
  dismissCard: (id: string) => void
}

function useDockBubble(chatOpen: boolean, messages: ChatMessage[], hold: boolean) {
  const [bubble, setBubble] = useState<Bubble | null>(null)
  const last = messages.findLast((m) => m.role === 'assistant')
  // What was already said before the dock showed up is history, not news.
  const seen = useRef(last?.id)
  useEffect(() => {
    if (!last || last.id === seen.current) return
    seen.current = last.id
    if (!chatOpen) setBubble({ id: last.id, text: last.content })
  }, [last, chatOpen])
  // Proactive thoughts (deadline, habit) come as their own event, not as a chat message.
  useEffect(
    () =>
      getElectronApi()?.runtime?.onBrainProactiveThought?.(({ message }) => {
        if (message?.trim()) setBubble({ id: crypto.randomUUID(), text: message.trim() })
      }),
    [],
  )
  useEffect(() => {
    if (chatOpen) setBubble(null)
  }, [chatOpen])
  useEffect(() => {
    if (!bubble || hold) return
    const timer = setTimeout(() => setBubble(null), Math.max(BUBBLE_MIN_TAIL_MS, bubbleDwellMs(bubble.text)))
    return () => clearTimeout(timer)
  }, [bubble, hold])
  return { bubble, dismissBubble: () => setBubble(null) }
}

function useDockCards(hovered: boolean) {
  const [cards, setCards] = useState<Card[]>([])
  useEffect(
    () =>
      getElectronApi()?.runtime?.onToolResult?.(({ name, result, origin }) => {
        const card = isForegroundToolEvent(origin) ? dockCard(name, result) : null
        if (card) setCards((prev) => [...prev, { ...card, id: crypto.randomUUID() }].slice(-MAX_CARDS))
      }),
    [],
  )
  useEffect(() => {
    if (cards.length === 0 || hovered) return
    const timer = setTimeout(() => setCards((prev) => prev.slice(1)), CARD_MS)
    return () => clearTimeout(timer)
  }, [cards, hovered])
  return { cards, dismissCard: (id: string) => setCards((prev) => prev.filter((c) => c.id !== id)) }
}

/** `busy`: she is still thinking or speaking, so her words stay up; `hovered` holds everything. */
export function useDockFeed(opts: {
  chatOpen: boolean
  messages: ChatMessage[]
  busy: boolean
  hovered: boolean
}): DockFeedState {
  return { ...useDockBubble(opts.chatOpen, opts.messages, opts.busy || opts.hovered), ...useDockCards(opts.hovered) }
}

const ITEM =
  'pointer-events-auto relative w-full rounded-2xl border border-accent-rim/40 bg-bg-surface/95 py-2.5 pl-3.5 pr-9 text-left shadow-lg backdrop-blur-xl'
const DISMISS =
  'absolute right-1.5 top-1.5 flex h-7 w-7 items-center justify-center rounded-lg text-text-muted hover:bg-surface-raise-strong hover:text-text-main focus:outline-none focus-visible:ring-2 focus-visible:ring-focus'

const CARD_ICON = { reminder: BellRing, hint: Lightbulb, recap: BookOpen, daily: CalendarCheck } as const

function CardTitle({ card }: { card: Card }) {
  const { t, locale } = useI18n()
  if (card.kind === 'hint') return <>{t(`shell.edgeDock.card.level.${card.level ?? 'tip'}`)}</>
  if (card.kind === 'recap') return <>{t('shell.edgeDock.card.recap')}</>
  if (card.kind === 'daily')
    return (
      <>
        {[t('shell.edgeDock.card.daily'), card.daily?.passenger && t('shell.edgeDock.card.dailyPassenger')]
          .filter(Boolean)
          .join(' · ')}
      </>
    )
  const at = card.at ? new Date(card.at).toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' }) : ''
  return <>{[t('shell.edgeDock.card.reminder'), at].filter(Boolean).join(' · ')}</>
}

/** The day in numbers, plus up to three achievements of the day. */
function DailyStats({ daily }: { daily: NonNullable<Card['daily']> }) {
  const { t } = useI18n()
  return (
    <>
      <p className="mt-0.5 text-caption text-text-muted">
        {t('shell.edgeDock.card.dailyStats', { sessions: daily.sessions, deaths: daily.deaths, streak: daily.streak })}
      </p>
      {daily.achievements.slice(0, 3).map((achievement) => (
        <p key={achievement} className="mt-0.5 flex items-center gap-1 text-caption text-text-main">
          <Trophy aria-hidden="true" size={12} className="shrink-0 text-accent-text" />
          <span className="truncate">{achievement}</span>
        </p>
      ))}
    </>
  )
}

function ResultCard({ card, onDismiss }: { card: Card; onDismiss: () => void }) {
  const { t } = useI18n()
  const Icon = CARD_ICON[card.kind]
  return (
    <article className={ITEM}>
      <p className="flex items-center gap-1.5 text-micro font-semibold uppercase tracking-[0.12em] text-accent-text">
        <Icon aria-hidden="true" size={13} />
        <CardTitle card={card} />
      </p>
      {card.subject && <p className="mt-1 text-caption font-semibold text-text-main">{card.subject}</p>}
      {card.daily && <DailyStats daily={card.daily} />}
      {card.body && <p className="mt-0.5 line-clamp-5 text-caption leading-relaxed text-text-muted">{card.body}</p>}
      <button type="button" title={t('shell.thought.dismiss')} onClick={onDismiss} className={DISMISS}>
        <X size={14} />
      </button>
    </article>
  )
}

/** Bubble and cards, stacked beside the rail; clicking the bubble opens the conversation. */
export function DockFeed({
  feed,
  onOpenChat,
  hoverProps,
}: {
  feed: DockFeedState
  onOpenChat: () => void
  hoverProps: { onMouseEnter: () => void; onMouseLeave: () => void }
}) {
  const { t } = useI18n()
  if (!feed.bubble && feed.cards.length === 0) return null
  return (
    <section
      aria-live="polite"
      aria-label={t('shell.edgeDock.label')}
      className="pointer-events-none flex w-72 flex-col justify-center gap-2"
    >
      {feed.bubble && (
        <div className="relative" {...hoverProps}>
          <button type="button" title={t('shell.edgeDock.openChat')} onClick={onOpenChat} className={ITEM}>
            <span className="text-micro font-semibold uppercase tracking-[0.12em] text-accent-text">
              {t('shell.bubble.mika')}
            </span>
            <span className="mt-0.5 line-clamp-6 block text-caption leading-relaxed text-text-main">
              {feed.bubble.text}
            </span>
          </button>
          <button
            type="button"
            title={t('shell.thought.dismiss')}
            onClick={feed.dismissBubble}
            className={`${DISMISS} pointer-events-auto`}
          >
            <X size={14} />
          </button>
        </div>
      )}
      {feed.cards.map((card) => (
        <div key={card.id} className="w-full" {...hoverProps}>
          <ResultCard card={card} onDismiss={() => feed.dismissCard(card.id)} />
        </div>
      ))}
    </section>
  )
}
