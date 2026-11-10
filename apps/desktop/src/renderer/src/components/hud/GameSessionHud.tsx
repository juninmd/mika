import { Gamepad2 } from 'lucide-react'
import { memo, useEffect, useState } from 'react'
import type { GameSession } from '../../hooks/useGameSession'
import { useI18n } from '../../hooks/useI18n'
import { useMoodState } from '../../hooks/useMoodState'
import { getMoodConfig, moodName } from '../mood-hud-config'
import type { HudRect } from './hud-layout'

/** `h:mm` past the hour, `m min` before — a session timer, not a stopwatch. */
function formatSession(ms: number): string {
  const minutes = Math.max(0, Math.floor(ms / 60_000))
  const h = Math.floor(minutes / 60)
  return h > 0 ? `${h}:${String(minutes % 60).padStart(2, '0')}` : `${minutes} min`
}

/** Re-renders once a minute (the display's own resolution), never per second. */
function useSessionElapsed(startTime: string): string {
  const start = Date.parse(startTime)
  const [now, setNow] = useState(Date.now)
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 60_000)
    return () => clearInterval(id)
  }, [])
  return Number.isFinite(start) ? formatSession(now - start) : ''
}

type Props = { session: GameSession; anchor: HudRect; variant: 'chip' | 'panel' }

/** Compact game session HUD: game, session timer and her mood; the panel variant adds events and mood name. */
export const GameSessionHud = memo(function GameSessionHud({ session, anchor, variant }: Props) {
  const { t } = useI18n()
  const elapsed = useSessionElapsed(session.startTime)
  const { mood } = useMoodState()
  const moodCfg = mood ? getMoodConfig(mood.label) : null
  const events = session.events?.length ?? 0

  return (
    <div
      role="status"
      aria-label={t('shell.game.aria').replace('{{game}}', session.gameName).replace('{{time}}', elapsed)}
      className="hud-glass pointer-events-none absolute flex items-center gap-2 overflow-hidden rounded-full py-1 pl-1 pr-3"
      style={{ left: anchor.x, top: anchor.y, maxWidth: anchor.width, height: anchor.height }}
    >
      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-accent-soft text-accent">
        <Gamepad2 aria-hidden="true" className="size-3.5" />
      </span>
      <span className="min-w-0 truncate text-caption font-bold text-text-main">{session.gameName}</span>
      <span className="shrink-0 font-mono text-caption tabular-nums text-text-muted">{elapsed}</span>
      {moodCfg && mood && (
        <span className="flex shrink-0 items-center gap-1 text-caption font-semibold" style={{ color: moodCfg.color }}>
          <span aria-hidden="true">{moodCfg.emoji}</span>
          {variant === 'panel' && moodName(mood.label, t)}
        </span>
      )}
      {variant === 'panel' && events > 0 && (
        <span className="shrink-0 text-caption text-text-muted">{t('shell.game.events', { count: events })}</span>
      )}
    </div>
  )
})
