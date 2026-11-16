import { Fragment, memo, useCallback, useRef, useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import { formatAccelerator, shortcutById } from '../lib/app-shortcuts'
import { isMacPlatform } from '../lib/keyboard-shortcuts'
import type { AppStatus } from './app/app-shell.types'
import { DockButton } from './dock/DockButton'
import { DockDragHandle } from './dock/DockDragHandle'
import { DOCK_GROUPS, DOCK_ICONS, DOCK_SHORT_LABEL, type DockKey, WHEEL_KEY } from './dock/dock-buttons'
import type { HudPolicy } from './hud/hud-policy'
import { useTooltipTrigger } from './ui/Tooltip'

type T = {
  status: AppStatus
  policy: HudPolicy
  micEnabled: boolean
  vadActive?: boolean
  onToggleMic: () => void
  onSetMic?: (enabled: boolean) => void
  onOpenInput: () => void
  onOpenHistory: () => void
  onOpenSettings: () => void
  onOpenWheel?: () => void
  onStopSpeech: () => void
  ttsIsPlaying: boolean
  showInput: boolean
  showHistory: boolean
  /** Pointer or keyboard is on the dock: the legend (label + shortcut) is showing. */
  showLegend: boolean
  sttEnabled?: boolean
  inputMode?: 'ptt' | 'continuous'
  updateAvailable?: boolean
  providerDown?: boolean
}

type ItemState = {
  title: string
  kbd?: string
  onClick: () => void
  active?: boolean
  pulse?: boolean
  hold?: boolean
  badge?: 'warn' | 'danger'
}

function useDockItems(p: T): Partial<Record<DockKey, ItemState>> {
  const { t } = useI18n()
  const mac = isMacPlatform()
  const keyOf = (id: string) => formatAccelerator(shortcutById(id).accelerator, mac)
  const pttMode = p.inputMode === 'ptt' && Boolean(p.onSetMic)
  const micTitle = !(p.sttEnabled ?? true)
    ? t('shell.dock.micOff')
    : pttMode
      ? t('shell.dock.micPtt')
      : t('shell.dock.mic')
  const settingsNote = p.providerDown
    ? t('shell.dock.providerDown')
    : p.updateAvailable
      ? t('shell.dock.updateAvailable')
      : null
  return {
    // Contextual: only while she is actually talking — a stop button with nothing to stop is a dead button.
    stop: p.ttsIsPlaying ? { title: t('shell.dock.stopSpeech'), onClick: p.onStopSpeech } : undefined,
    mic: {
      title: micTitle,
      kbd: pttMode ? keyOf('pushToTalk') : keyOf('toggleMic'),
      onClick: p.onToggleMic,
      active: p.micEnabled,
      pulse: p.vadActive,
      hold: pttMode,
    },
    chat: { title: `${t('shell.dock.chat')}`, kbd: keyOf('toggleInput'), onClick: p.onOpenInput, active: p.showInput },
    history: {
      title: `${t('shell.dock.history')} (${keyOf('history')})`,
      kbd: keyOf('history'),
      onClick: p.onOpenHistory,
      active: p.showHistory,
    },
    wheel: { title: t('shell.dock.wheel'), kbd: WHEEL_KEY, onClick: () => p.onOpenWheel?.() },
    settings: {
      title: settingsNote ? `${t('shell.dock.settings')} — ${settingsNote}` : t('shell.dock.settings'),
      onClick: p.onOpenSettings,
      badge: p.providerDown ? 'danger' : p.updateAvailable ? 'warn' : undefined,
    },
  }
}

function DockLegend({ label, kbd, visible }: { label: string; kbd?: string; visible: boolean }) {
  return (
    <div
      aria-hidden="true"
      className={`hud-glass pointer-events-none absolute bottom-full left-1/2 mb-2 flex -translate-x-1/2 items-center gap-2 whitespace-nowrap rounded-full px-3 py-1 transition-[opacity,transform] duration-150 ${
        visible ? 'translate-y-0 opacity-100' : 'translate-y-1 opacity-0'
      }`}
    >
      <span className="text-micro font-semibold uppercase tracking-[0.12em] text-text-main">{label}</span>
      {kbd && (
        <kbd className="rounded border border-border-strong bg-surface-raise px-1.5 font-mono text-micro leading-4 text-text-muted">
          {kbd}
        </kbd>
      )}
    </div>
  )
}

const MAG_MAX = 0.22
const MAG_REACH_PX = 90

/**
 * macOS-style magnification: buttons near the pointer grow, neighbours a little less. Writes the CSS
 * `scale` property straight to the DOM (no React state per pointer move) so the hover lift in
 * `DockButton`, which uses `transform`, still composes. Off under reduced motion.
 */
const dockButtons = (dock: HTMLElement | null) => dock?.querySelectorAll<HTMLElement>('[data-dock-button]') ?? []

function useDockMagnify(dockRef: React.RefObject<HTMLDivElement | null>) {
  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (e.pointerType !== 'mouse' || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
      for (const button of dockButtons(dockRef.current)) {
        const rect = button.getBoundingClientRect()
        const distance = Math.abs(e.clientX - (rect.left + rect.width / 2))
        button.style.scale = String(1 + MAG_MAX * Math.max(0, 1 - distance / MAG_REACH_PX))
      }
    },
    [dockRef],
  )
  const onPointerLeave = useCallback(() => {
    for (const button of dockButtons(dockRef.current)) button.style.scale = ''
  }, [dockRef])
  return { onPointerMove, onPointerLeave }
}

export const CommandDock = memo(function CommandDock(p: T) {
  const { t } = useI18n()
  const dockRef = useRef<HTMLDivElement>(null)
  const [peek, setPeek] = useState<DockKey>('mic')
  const { visible: dragTooltipVisible, triggerProps: dragTriggerProps } = useTooltipTrigger()
  const items = useDockItems(p)

  const handleKeyNav = useCallback((e: React.KeyboardEvent) => {
    const buttons = dockRef.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')
    if (!buttons?.length) return
    const current = Array.from(buttons).indexOf(document.activeElement as HTMLButtonElement)
    const idx = Math.max(current, 0)
    const map: Record<string, number> = {
      ArrowRight: (idx + 1) % buttons.length,
      ArrowDown: (idx + 1) % buttons.length,
      ArrowLeft: (idx - 1 + buttons.length) % buttons.length,
      ArrowUp: (idx - 1 + buttons.length) % buttons.length,
      Home: 0,
      End: buttons.length - 1,
    }
    if (!(e.key in map)) return
    e.preventDefault()
    buttons[map[e.key] as number]?.focus()
  }, [])

  const magnify = useDockMagnify(dockRef)
  const peeked = items[peek] ?? items.mic
  const surface = p.policy.dockSurface === 'solid' ? 'hud-glass-strong shadow-lg shadow-bg-deep/40' : 'hud-glass-strong'
  const groups = DOCK_GROUPS.map((group) => group.filter((key) => items[key])).filter((group) => group.length)

  return (
    <div
      ref={dockRef}
      role="toolbar"
      aria-label={t('shell.dock.label')}
      aria-orientation="horizontal"
      onKeyDown={handleKeyNav}
      onPointerMove={magnify.onPointerMove}
      onPointerLeave={magnify.onPointerLeave}
      className={`pointer-events-auto relative flex items-center gap-1 rounded-2xl p-1 ${surface}`}
      style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
    >
      <DockLegend label={t(DOCK_SHORT_LABEL[peek])} kbd={peeked?.kbd} visible={p.showLegend} />
      <DockDragHandle dragTooltipVisible={dragTooltipVisible} dragTriggerProps={dragTriggerProps} />
      {groups.map((group, gi) => (
        <Fragment key={group.join('-')}>
          {gi > 0 && <span aria-hidden="true" className="mx-0.5 h-5 w-px bg-border-subtle" />}
          {group.map((key) => {
            const item = items[key] as ItemState
            return (
              <DockButton
                key={key}
                onClick={item.onClick}
                active={item.active}
                pulse={item.pulse}
                title={item.title}
                onPeek={() => setPeek(key)}
                onHoldStart={item.hold ? () => p.onSetMic?.(true) : undefined}
                onHoldEnd={item.hold ? () => p.onSetMic?.(false) : undefined}
              >
                {DOCK_ICONS[key]()}
                {item.badge && (
                  <span
                    className={`pointer-events-none absolute -right-0.5 -top-0.5 h-2 w-2 rounded-full ${item.badge === 'danger' ? 'bg-danger' : 'bg-warn'}`}
                    aria-hidden="true"
                  />
                )}
              </DockButton>
            )
          })}
        </Fragment>
      ))}
    </div>
  )
})
