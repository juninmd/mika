import type { ChatMessage } from '@mika/sdk'
import { Maximize2, Mic, MicOff, MoreHorizontal, Send, X } from 'lucide-react'
import { memo, useEffect, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { setMiniMode, setMiniModeStyle } from '../../hooks/useMiniMode'
import type { MiniModeStyle } from '../../lib/mini-mode'
import type { AppStatus } from '../app/app-shell.types'

/** Official app icon (`apps/desktop/build/icon.png`, cropped to the face) — the dock's face of Mika. */
const ICON_URL = '/mika-icon.webp'
/** The compact chat shows only the tail of the conversation; history keeps the rest. */
const VISIBLE_MESSAGES = 30

type Side = 'dock-right' | 'dock-left'

export type EdgeDockProps = {
  side: Side
  appStatus: AppStatus
  speaking: boolean
  micEnabled: boolean
  vadIsSpeaking: boolean
  chatOpen: boolean
  messages: ChatMessage[]
  input: string
  setInput: (next: string) => void
  onSend: () => void | Promise<void>
  onToggleChat: () => void
  onToggleMic: () => void
}

type Presence = 'speaking' | 'listening' | 'thinking' | 'idle'

function presence(status: AppStatus, speaking: boolean, hearing: boolean): Presence {
  if (speaking || status === 'speaking') return 'speaking'
  if (hearing || status === 'listening') return 'listening'
  if (status === 'thinking' || status === 'acting') return 'thinking'
  return 'idle'
}

const RING: Record<Presence, string> = {
  speaking: 'edge-dock-ring border-accent',
  listening: 'border-ok opacity-100',
  thinking: 'border-accent-rim opacity-100',
  idle: 'opacity-0',
}

function MikaFace({ state, size }: { state: Presence; size: number }) {
  return (
    <span className="relative block shrink-0" style={{ width: size, height: size }}>
      <img src={ICON_URL} alt="" draggable={false} className="h-full w-full rounded-full object-cover" />
      <span aria-hidden="true" className={`absolute -inset-1 rounded-full border-2 ${RING[state]}`} />
    </span>
  )
}

const RAIL_BUTTON =
  'pointer-events-auto flex h-11 w-11 items-center justify-center rounded-2xl border text-text-muted transition hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main focus:outline-none focus-visible:ring-2 focus-visible:ring-focus'

function Rail(props: EdgeDockProps & { state: Presence }) {
  const { t } = useI18n()
  const { chatOpen, micEnabled, vadIsSpeaking, state, onToggleChat, onToggleMic } = props
  return (
    <nav
      aria-label={t('shell.edgeDock.label')}
      className="pointer-events-auto flex flex-col items-center gap-2.5 rounded-3xl border border-accent-rim/40 bg-bg-surface/85 px-2 py-2.5 shadow-lg backdrop-blur-xl"
    >
      <button
        type="button"
        onClick={onToggleChat}
        aria-pressed={chatOpen}
        title={t('shell.edgeDock.talk')}
        className="rounded-full focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <MikaFace state={state} size={52} />
      </button>
      <span aria-hidden="true" className="h-px w-6 bg-border-subtle" />
      <button
        type="button"
        onClick={onToggleMic}
        aria-pressed={micEnabled}
        title={t('shell.edgeDock.mic')}
        className={`${RAIL_BUTTON} ${micEnabled ? (vadIsSpeaking ? 'border-ok-rim bg-ok-soft text-ok' : 'border-accent-rim bg-accent-soft text-accent-text') : 'border-transparent'}`}
      >
        {micEnabled ? <Mic size={18} /> : <MicOff size={18} />}
      </button>
      <button
        type="button"
        onClick={() => setMiniMode(false)}
        title={t('shell.edgeDock.expand')}
        className={`${RAIL_BUTTON} border-transparent`}
      >
        <Maximize2 size={18} />
      </button>
    </nav>
  )
}

function DockMenu({ side, onClose }: { side: Side; onClose: () => void }) {
  const { t } = useI18n()
  const pick = (style: MiniModeStyle) => {
    setMiniModeStyle(style)
    onClose()
  }
  const item =
    'flex w-full items-center rounded-lg px-2.5 py-2 text-left text-caption text-text-main hover:bg-surface-raise-strong focus:outline-none focus-visible:ring-2 focus-visible:ring-focus'
  return (
    <div
      role="menu"
      className="absolute right-2 top-12 z-10 w-56 rounded-xl border border-accent-rim/40 bg-bg-surface p-1.5 shadow-lg"
    >
      <button
        type="button"
        role="menuitem"
        className={item}
        onClick={() => pick(side === 'dock-right' ? 'dock-left' : 'dock-right')}
      >
        {t('shell.edgeDock.switchSide')}
      </button>
      <button type="button" role="menuitem" className={item} onClick={() => pick('head')}>
        {t('shell.edgeDock.headStyle')}
      </button>
    </div>
  )
}

function Messages({ messages }: { messages: ChatMessage[] }) {
  const { t } = useI18n()
  const end = useRef<HTMLDivElement>(null)
  const visible = messages.filter((m) => m.role === 'user' || m.role === 'assistant').slice(-VISIBLE_MESSAGES)
  const last = visible.at(-1)
  useEffect(() => {
    if (last) end.current?.scrollIntoView({ block: 'end' })
  }, [last])
  if (visible.length === 0)
    return <p className="m-auto px-6 text-center text-caption text-text-muted">{t('shell.edgeDock.empty')}</p>
  return (
    <>
      {visible.map((m) => (
        <p
          key={m.id}
          className={`max-w-[85%] whitespace-pre-wrap break-words rounded-2xl border px-3 py-2 text-caption leading-relaxed ${
            m.role === 'user'
              ? 'self-end rounded-br-sm border-accent-rim/50 bg-accent-soft text-text-main'
              : 'self-start rounded-bl-sm border-border-subtle bg-surface-raise text-text-main'
          }`}
        >
          {m.content}
        </p>
      ))}
      <div ref={end} />
    </>
  )
}

function CompactChat(props: EdgeDockProps & { state: Presence }) {
  const { t } = useI18n()
  const [menuOpen, setMenuOpen] = useState(false)
  const { side, state, messages, input, setInput, onSend, onToggleChat } = props
  return (
    <section
      aria-label={t('shell.edgeDock.talk')}
      className="pointer-events-auto relative flex h-full min-w-0 flex-1 flex-col rounded-3xl border border-accent-rim/40 bg-bg-surface/90 shadow-xl backdrop-blur-xl"
    >
      <header className="flex items-center gap-2.5 border-b border-border-subtle py-2.5 pl-3.5 pr-2">
        <MikaFace state={state} size={28} />
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold text-text-main">Mika</p>
          <p className="text-caption text-text-muted">{t(`shell.edgeDock.state.${state}`)}</p>
        </div>
        <button
          type="button"
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          title={t('shell.edgeDock.options')}
          onClick={() => setMenuOpen((open) => !open)}
          className={`${RAIL_BUTTON} h-8 w-8 border-transparent`}
        >
          <MoreHorizontal size={16} />
        </button>
        <button
          type="button"
          title={t('shell.edgeDock.close')}
          onClick={onToggleChat}
          className={`${RAIL_BUTTON} h-8 w-8 border-transparent`}
        >
          <X size={16} />
        </button>
      </header>
      {menuOpen && <DockMenu side={side} onClose={() => setMenuOpen(false)} />}
      <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-3">
        <Messages messages={messages} />
      </div>
      <form
        className="m-2.5 flex items-center gap-1.5 rounded-full border border-border-subtle bg-surface-sunken py-1 pl-3.5 pr-1"
        onSubmit={(e) => {
          e.preventDefault()
          if (input.trim()) void onSend()
        }}
      >
        <input
          id="edge-dock-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder={t('shell.edgeDock.placeholder')}
          autoComplete="off"
          // biome-ignore lint/a11y/noAutofocus: the user just opened the compact chat to type in it.
          autoFocus
          className="min-w-0 flex-1 bg-transparent text-caption text-text-main outline-none placeholder:text-text-muted"
        />
        <button
          type="submit"
          title={t('shell.edgeDock.send')}
          disabled={!input.trim()}
          className="flex h-8 w-8 items-center justify-center rounded-full bg-accent text-bg-surface transition disabled:opacity-40 focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <Send size={15} />
        </button>
      </form>
    </section>
  )
}

/**
 * Edge dock (mini mode style `dock-right`/`dock-left`): Mika's official icon on a rail glued to the side
 * of the screen, the mic and a way back to the full window, plus a compact chat that opens beside it.
 * Everything else in the window is transparent and lets clicks through (`useClickThrough`).
 */
export const EdgeDock = memo(function EdgeDock(props: EdgeDockProps) {
  const state = presence(props.appStatus, props.speaking, props.micEnabled && props.vadIsSpeaking)
  return (
    <div
      className={`pointer-events-none absolute inset-0 flex items-center gap-3 p-2 ${props.side === 'dock-right' ? 'flex-row-reverse' : 'flex-row'}`}
    >
      <Rail {...props} state={state} />
      {props.chatOpen && <CompactChat {...props} state={state} />}
    </div>
  )
})
