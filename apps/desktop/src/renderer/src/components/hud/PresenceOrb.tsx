import {
  AlertTriangle,
  Gamepad2,
  type LucideIcon,
  Mic,
  MicOff,
  ShieldQuestion,
  Sparkles,
  Unplug,
  Volume2,
} from 'lucide-react'
import { memo, type RefObject, useEffect, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { getMicLevel, subscribeMicLevel } from '../../lib/mic-level-bus'
import type { HudRect } from './hud-layout'
import type { HudPolicy } from './hud-policy'
import type { Presence, PresenceKind } from './presence-state'
import { ttsLevelWrite } from './tts-level-write'

const ICON: Record<PresenceKind, LucideIcon> = {
  error: AlertTriangle,
  confirming: ShieldQuestion,
  speaking: Volume2,
  thinking: Sparkles,
  offline: Unplug,
  held: Mic,
  hearing: Mic,
  ready: Mic,
  muted: MicOff,
}

/** Weights of the five speaking bars, center-heavy so the shape reads as a voice, not a meter. */
const BARS = [
  { id: 'b0', w: 0.45 },
  { id: 'b1', w: 0.8 },
  { id: 'b2', w: 1 },
  { id: 'b3', w: 0.8 },
  { id: 'b4', w: 0.45 },
] as const

/** Writes the live level into `--lvl` on the orb: mic via the 10 Hz bus, TTS via rAF over `volumeRef`.
 *  The TTS path skips the style write when the value quantized to 3 decimals did not change — the
 *  status can stay "speaking" for seconds after the audio tail, and re-writing the same `--lvl` for
 *  every one of those frames only costs the style engine work (medido: 600→11 escritas por 10 s). */
function useLevelVar(meter: Presence['meter'], volumeRef: RefObject<number>) {
  const ref = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const write = (v: number) => el.style.setProperty('--lvl', Math.min(1, Math.max(0, v)).toFixed(3))
    if (meter === 'mic') {
      write(getMicLevel().level)
      return subscribeMicLevel((s) => write(s.level))
    }
    if (meter !== 'tts') {
      write(0)
      return
    }
    let frame = 0
    let smooth = 0
    let lastWritten = -1
    let idleFrames = 0
    let idleTimer: ReturnType<typeof setTimeout> | null = null
    const tick = () => {
      const vol = volumeRef.current ?? 0
      smooth = smooth * 0.55 + vol * 0.45
      const quantized = ttsLevelWrite(smooth * 1.6, lastWritten)
      if (quantized !== lastWritten) {
        lastWritten = quantized
        write(quantized)
        idleFrames = 0
      } else if (vol === 0 && quantized === 0) {
        idleFrames++
      }
      if (idleFrames > 30) {
        idleTimer = setTimeout(() => {
          idleTimer = null
          frame = requestAnimationFrame(tick)
        }, 50)
        return
      }
      frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
    return () => {
      if (idleTimer) clearTimeout(idleTimer)
      cancelAnimationFrame(frame)
      lastWritten = -1
      write(0)
    }
  }, [meter, volumeRef])
  return ref
}

function OrbCore({ presence, expanded }: { presence: Presence; expanded: boolean }) {
  const Icon = ICON[presence.kind]
  if (presence.kind === 'speaking') {
    return (
      <span className="relative flex h-3.5 items-center gap-[2px]" aria-hidden="true">
        {BARS.map((bar) => (
          <span
            key={bar.id}
            className="hud-meter-bar h-3.5 w-[2.5px] rounded-full"
            style={{ backgroundColor: presence.tone, ['--w' as string]: bar.w }}
          />
        ))}
      </span>
    )
  }
  if (!expanded) {
    return <span className="relative h-2.5 w-2.5 rounded-full" style={{ backgroundColor: presence.tone }} />
  }
  return <Icon aria-hidden="true" className="relative size-4" style={{ color: presence.tone }} />
}

const HUD_MARGIN = 12
/** Below this much room left of the orb the status word drops under it instead of being truncated to nothing. */
const LABEL_MIN_ROOM = 132

function StatusWord({
  side,
  tone,
  text,
  visible,
}: {
  side: 'left' | 'below'
  tone: string
  text: string
  visible: boolean
}) {
  const place =
    side === 'left'
      ? 'absolute right-full top-1/2 mr-2 max-w-[40vw] -translate-y-1/2'
      : 'relative max-w-[calc(100vw-24px)]'
  return (
    <span
      aria-hidden="true"
      className={`hud-glass pointer-events-none truncate whitespace-nowrap rounded-full px-2.5 py-1 text-caption font-semibold uppercase tracking-[0.12em] text-text-main transition-[opacity,transform] duration-200 ${place} ${
        visible ? 'opacity-100' : 'opacity-0'
      }`}
    >
      <span className="mr-1.5 inline-block h-1.5 w-1.5 rounded-full align-middle" style={{ backgroundColor: tone }} />
      {text}
    </span>
  )
}

/** The animated ring for each state — only `transform`/`opacity` move (CSS classes in styles.css). */
function OrbRings({ presence }: { presence: Presence }) {
  return (
    <>
      {presence.meter === 'mic' && (
        <span
          aria-hidden="true"
          className="hud-meter-ring absolute inset-[3px] rounded-full border-2"
          style={{ borderColor: presence.tone }}
        />
      )}
      {presence.kind === 'thinking' && (
        <span
          aria-hidden="true"
          className="hud-spin absolute inset-[2px] rounded-full border-2 border-transparent"
          style={{ borderTopColor: presence.tone, borderRightColor: presence.tone }}
        />
      )}
      {(presence.kind === 'confirming' || presence.kind === 'error') && (
        <span
          aria-hidden="true"
          className="hud-breathe absolute inset-[2px] rounded-full border-2"
          style={{ borderColor: presence.tone }}
        />
      )}
      {presence.kind === 'offline' && (
        <span
          aria-hidden="true"
          className="absolute inset-[3px] rounded-full border-2 border-dashed"
          style={{ borderColor: presence.tone }}
        />
      )}
    </>
  )
}

/** What the orb shows, per background-mode policy and whether the pointer/keyboard is on it. */
function orbView(v: { presence: Presence; policy: HudPolicy; anchor: HudRect; open: boolean; bubbleShown: boolean }) {
  return {
    expanded: v.presence.active || v.open || v.policy.density !== 'minimal',
    showLabel: v.open || (!v.bubbleShown && (v.presence.active || v.policy.presenceLabelAtRest)),
    showDetails: v.open || v.policy.telemetryAtRest,
    labelSide: v.anchor.x - HUD_MARGIN >= LABEL_MIN_ROOM ? ('left' as const) : ('below' as const),
  }
}

type PresenceOrbProps = {
  presence: Presence
  policy: HudPolicy
  anchor: HudRect
  micEnabled: boolean
  volumeRef: RefObject<number>
  onToggleMic: () => void
  /** Game being played, shown next to the status word. */
  gameName?: string
  /** Telemetry flyout (senses, mood, cost) — mounted only while open, so its loops cost nothing at rest. */
  renderDetails: () => React.ReactNode
  /** The speech bubble is on screen and already names the state — the status word steps aside. */
  bubbleShown?: boolean
  /** Contextual chip that shows whenever it renders something (an active screen effect), open or not. */
  pinned?: React.ReactNode
}

/**
 * One indicator for "what is Mika doing": listening (ring follows the mic), thinking (orbiting
 * arc), speaking (bars follow the TTS level), muted, offline, confirming, error — plus a gaming
 * badge. At rest in transparent mode it is a dot; the glass disc, the status word and the
 * telemetry flyout appear only while something happens or the pointer is on it.
 */
export const PresenceOrb = memo(function PresenceOrb({
  presence,
  policy,
  anchor,
  micEnabled,
  volumeRef,
  onToggleMic,
  gameName,
  renderDetails,
  pinned,
  bubbleShown = false,
}: PresenceOrbProps) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const orbRef = useLevelVar(presence.meter, volumeRef)
  const { expanded, showLabel, showDetails, labelSide } = orbView({ presence, policy, anchor, open, bubbleShown })
  const label = t(presence.labelKey)
  const status = gameName ? `${label} · ${t('shell.presence.playing').replace('{{game}}', gameName)}` : label

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: hover/focus-within reveal of the telemetry flyout, not a widget — the orb button inside is the keyboard target and focusing it opens the same flyout.
    <div
      className="pointer-events-none absolute"
      style={{ left: anchor.x, top: anchor.y, width: anchor.width }}
      onPointerEnter={() => setOpen(true)}
      onPointerLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setOpen(false)
      }}
    >
      <div className="relative flex h-9 items-center justify-center">
        <button
          ref={orbRef}
          type="button"
          onClick={onToggleMic}
          aria-pressed={micEnabled}
          aria-label={`${status}. ${t(micEnabled ? 'shell.presence.muteAction' : 'shell.presence.unmuteAction')}`}
          data-testid={micEnabled ? 'voice-waveform' : undefined}
          data-presence={presence.kind}
          data-hearing={presence.kind === 'hearing' ? 'true' : 'false'}
          data-held={presence.kind === 'held' ? 'true' : 'false'}
          className="pointer-events-auto relative flex h-9 w-9 cursor-pointer items-center justify-center rounded-full outline-none transition-transform duration-150 active:scale-95 focus-visible:ring-2 focus-visible:ring-focus"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          <span
            aria-hidden="true"
            className={`hud-glass absolute inset-0 rounded-full transition-opacity duration-200 ${expanded ? 'opacity-100' : 'opacity-0'}`}
          />
          <OrbRings presence={presence} />
          <OrbCore presence={presence} expanded={expanded} />
          {presence.gaming && (
            <span
              aria-hidden="true"
              className="hud-glass-strong absolute -bottom-1 -right-1 flex h-4 w-4 items-center justify-center rounded-full text-accent"
            >
              <Gamepad2 className="size-2.5" />
            </span>
          )}
        </button>

        {labelSide === 'left' && <StatusWord side="left" tone={presence.tone} text={status} visible={showLabel} />}
      </div>

      <div
        className="absolute top-full mt-2 flex w-max flex-col items-start gap-1.5"
        style={{ left: HUD_MARGIN - anchor.x }}
      >
        {labelSide === 'below' && <StatusWord side="below" tone={presence.tone} text={status} visible={showLabel} />}
        {showDetails && (
          <section
            aria-label={t('shell.hud.telemetryAriaLabel')}
            className="pointer-events-auto flex flex-col items-start gap-1.5"
          >
            {renderDetails()}
          </section>
        )}
        <div className="pointer-events-auto empty:hidden">{pinned}</div>
      </div>
    </div>
  )
})
