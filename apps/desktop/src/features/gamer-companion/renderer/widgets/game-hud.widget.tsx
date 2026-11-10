import { formatDurationMs } from '@mika/sdk/utils/time-format'
import { toDisplayError, usePluginContext } from '@mika/sdk-ui'
import {
  Camera,
  Clapperboard,
  Gamepad2,
  type LucideIcon,
  Map as MapIcon,
  MessageCircle,
  Skull,
  Star,
  Swords,
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { unwrapRpcData } from '../../../../renderer/src/lib/plugin-rpc'

type SessionEvent = {
  timestamp: string
  type: string
  description: string
}

type SessionData = {
  id: string
  gameId: string
  gameName: string
  startTime: string
  endTime?: string
  durationMs?: number
  events: SessionEvent[]
  summary?: string
}

const BASE_POLL_MS = 5000
const MAX_POLL_MS = 30000
const MAX_VISIBLE_EVENTS = 4

const EVENT_TYPE_ICONS: Record<string, LucideIcon> = {
  death: Skull,
  achievement: Star,
  combat: Swords,
  cutscene: Clapperboard,
  exploration: MapIcon,
  dialogue: MessageCircle,
  screenshot: Camera,
}
const FALLBACK_ICON = Gamepad2

function formatElapsed(start: Date): string {
  return formatDurationMs(Date.now() - start.getTime())
}

export function GameHudWidget() {
  const { api } = usePluginContext()
  const { t } = useI18n()
  const [session, setSession] = useState<SessionData | null>(null)
  const [elapsed, setElapsed] = useState('')
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [pollIntervalMs, setPollIntervalMs] = useState(BASE_POLL_MS)
  const pollGenerationRef = useRef(0)

  const poll = useCallback(async () => {
    const generation = ++pollGenerationRef.current
    try {
      const current = unwrapRpcData<SessionData | null | undefined>(await api.rpc.call<unknown>('session-current'))
      if (pollGenerationRef.current !== generation) return
      setSession(current ?? null)
      setError(null)
      setPollIntervalMs(BASE_POLL_MS)
    } catch (err) {
      if (pollGenerationRef.current !== generation) return
      setError(toDisplayError(err, t('settings.gamerCompanion.widget.hud.sessionFailed')))
      setPollIntervalMs((current) => Math.min(current * 2, MAX_POLL_MS))
    } finally {
      if (pollGenerationRef.current === generation) setLoading(false)
    }
  }, [api, t])

  const retry = useCallback(() => {
    setPollIntervalMs(BASE_POLL_MS)
    void poll()
  }, [poll])

  useEffect(() => {
    void poll()
  }, [poll])

  useEffect(() => {
    // Widget oculto (minimizado, atrás do jogo em tela cheia) não consulta o backend à toa.
    const interval = setInterval(() => {
      if (!document.hidden) void poll()
    }, pollIntervalMs)
    return () => clearInterval(interval)
  }, [poll, pollIntervalMs])

  useEffect(() => {
    if (!session) return
    const start = new Date(session.startTime)
    setElapsed(formatElapsed(start))
    const timer = setInterval(() => setElapsed(formatElapsed(start)), 10000)
    return () => clearInterval(timer)
  }, [session])

  if (loading) {
    return (
      <div className="flex min-h-screen items-center justify-center p-6">
        <div className="flex items-center gap-2 rounded-xl border border-accent-rim bg-bg-surface/90 px-4 py-3 backdrop-blur-md">
          <p className="animate-pulse text-xs text-text-muted" role="status">
            {t('settings.gamerCompanion.widget.gameHud.connecting')}
          </p>
        </div>
      </div>
    )
  }

  const recentEvents = session?.events.slice(-MAX_VISIBLE_EVENTS).reverse() ?? []

  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <div className="flex w-full max-w-md flex-col gap-2 rounded-xl border border-accent-rim bg-bg-surface/90 px-4 py-3 backdrop-blur-md">
        {error ? (
          <>
            <p role="alert" className="truncate text-caption text-danger">
              {error}
            </p>
            <button
              type="button"
              onClick={retry}
              className="self-start rounded-lg border border-accent-rim bg-accent-soft px-2 py-1 text-caption font-semibold text-accent transition hover:bg-accent/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('button.retry')}
            </button>
          </>
        ) : !session ? (
          <p className="text-caption leading-tight text-text-muted">
            {t('settings.gamerCompanion.widget.gameHud.noGameDetected')}
          </p>
        ) : (
          <>
            <div className="flex items-center gap-2">
              <span aria-hidden="true" className="relative flex h-2 w-2 shrink-0">
                <span className="absolute inline-flex h-2 w-2 animate-ping rounded-full bg-ok opacity-75" />
                <span className="relative inline-flex h-2 w-2 rounded-full bg-ok" />
              </span>
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <p className="truncate text-caption font-bold leading-tight text-text-main">{session.gameName}</p>
                <div className="flex min-w-0 items-center gap-2 text-caption font-mono text-text-muted">
                  <span className="shrink-0">{elapsed}</span>
                  {session.summary && <span className="min-w-0 truncate text-text-muted">{session.summary}</span>}
                  {session.events.length > 0 && (
                    <span className="shrink-0">{t('shell.game.events', { count: session.events.length })}</span>
                  )}
                </div>
              </div>
            </div>
            {recentEvents.length > 0 && (
              <ul className="flex flex-col gap-1 border-t border-border-subtle pt-1.5">
                {recentEvents.map((event) => {
                  const Icon = EVENT_TYPE_ICONS[event.type] ?? FALLBACK_ICON
                  return (
                    <li key={event.timestamp} className="flex items-center gap-1.5 text-caption leading-tight">
                      <Icon aria-hidden="true" className="size-3.5 shrink-0 text-text-muted" />
                      <span className="truncate text-text-muted">{event.description || event.type}</span>
                    </li>
                  )
                })}
              </ul>
            )}
          </>
        )}
      </div>
    </div>
  )
}
