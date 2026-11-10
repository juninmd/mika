import { formatDurationMs, formatRelativeTime } from '@mika/sdk/utils/time-format'
import { toDisplayError } from '@mika/sdk-ui'
import { StatTile } from '@mika/sdk-ui/settings'
import { Flame, Gamepad2, History, RefreshCw } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { callPluginRpc } from '../../../renderer/src/lib/plugin-rpc'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'

const PLUGIN_ID = 'mika.gamer-companion'
const RECENT_LIMIT = 4
const TILE_CLS = 'min-h-0 min-w-0 p-2.5'

type Session = { id: string; gameName: string; startTime: string; endTime?: string; durationMs?: number }
type Stats = { todayMs: number; weekMs: number; currentStreakDays: number; longestStreakDays: number }
type Snapshot = { current: Session | null; recent: Session[]; stats: Stats }

async function loadSnapshot(): Promise<Snapshot> {
  const api = getElectronApi()
  const [current, recent, stats] = await Promise.all([
    callPluginRpc<Session | null>(api, PLUGIN_ID, 'session-current'),
    callPluginRpc<Session[]>(api, PLUGIN_ID, 'session-list', { limit: RECENT_LIMIT + 1 }),
    callPluginRpc<Stats>(api, PLUGIN_ID, 'session-stats'),
  ])
  // `session-list` includes the running session; it is already shown on top.
  const finished = (recent ?? []).filter((s) => s.id !== current?.id && s.endTime).slice(0, RECENT_LIMIT)
  return { current: current ?? null, recent: finished, stats }
}

/** Real data only: the running session (`session-current`), the last ones, and playtime/streak. */
export function GamerNowPlaying({
  watcherEnabled,
  intervalSeconds,
}: {
  watcherEnabled: boolean
  intervalSeconds: number
}) {
  const { t } = useI18n()
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      setSnapshot(await loadSnapshot())
      setError(null)
    } catch (err) {
      setError(toDisplayError(err, t('settings.gamerCompanion.nowPlaying.error')))
    } finally {
      setLoading(false)
    }
  }, [t])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const current = snapshot?.current
  const elapsed = current ? formatDurationMs(Date.now() - new Date(current.startTime).getTime()) : null

  return (
    <div className="flex h-full flex-col gap-3 rounded-lg border border-border-subtle bg-surface-raise/40 p-3">
      <div className="flex items-center justify-between gap-2">
        <p className="flex items-center gap-2 text-caption font-black uppercase tracking-[0.14em] text-text-main">
          <Gamepad2 size={15} className="text-accent" aria-hidden="true" />
          {t('settings.gamerCompanion.nowPlaying.title')}
        </p>
        <button
          type="button"
          onClick={() => void refresh()}
          aria-label={t('settings.gamerCompanion.nowPlaying.refresh')}
          className="rounded-lg border border-border-subtle bg-surface-raise p-1.5 text-text-muted transition hover:border-border-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <RefreshCw size={12} className={loading ? 'animate-spin' : undefined} aria-hidden="true" />
        </button>
      </div>

      <div
        role="status"
        className={`rounded-lg border px-3 py-2.5 ${current ? 'border-ok-rim bg-ok-soft' : 'border-border-subtle bg-surface-raise'}`}
      >
        <p className={`truncate text-sm font-bold ${current ? 'text-ok' : 'text-text-muted'}`}>
          {current ? current.gameName : t('settings.gamerCompanion.nowPlaying.idle')}
        </p>
        <p className="mt-0.5 text-caption text-text-muted">
          {current
            ? t('settings.gamerCompanion.nowPlaying.elapsed').replace('{{duration}}', elapsed ?? '')
            : watcherEnabled
              ? t('settings.gamerCompanion.nowPlaying.watching').replace('{{seconds}}', String(intervalSeconds))
              : t('settings.gamerCompanion.nowPlaying.watcherOff')}
        </p>
      </div>

      {error && <p className="text-caption text-danger">{error}</p>}

      {snapshot && (
        <div className="grid grid-cols-3 gap-2">
          <StatTile
            className={TILE_CLS}
            label={t('settings.gamerCompanion.nowPlaying.today')}
            value={formatDurationMs(snapshot.stats.todayMs)}
          />
          <StatTile
            className={TILE_CLS}
            label={t('settings.gamerCompanion.nowPlaying.week')}
            value={formatDurationMs(snapshot.stats.weekMs)}
          />
          <StatTile
            className={TILE_CLS}
            label={t('settings.gamerCompanion.nowPlaying.streak')}
            value={t('settings.gamerCompanion.nowPlaying.days').replace(
              '{{count}}',
              String(snapshot.stats.currentStreakDays),
            )}
            tone={snapshot.stats.currentStreakDays > 0 ? 'ok' : 'neutral'}
          />
        </div>
      )}

      {snapshot && (
        <div className="min-h-0 flex-1">
          <p className="mb-1.5 flex items-center gap-1.5 text-micro font-bold uppercase tracking-[0.12em] text-text-muted">
            <History size={11} aria-hidden="true" />
            {t('settings.gamerCompanion.nowPlaying.recent')}
          </p>
          {snapshot.recent.length === 0 ? (
            <p className="text-caption text-text-subtle">{t('settings.gamerCompanion.nowPlaying.noSessions')}</p>
          ) : (
            <ul className="space-y-1">
              {snapshot.recent.map((session) => (
                <li key={session.id} className="flex items-center justify-between gap-2 text-caption">
                  <span className="min-w-0 truncate font-semibold text-text-main">{session.gameName}</span>
                  <span className="shrink-0 text-text-muted">
                    {formatDurationMs(session.durationMs)} ·{' '}
                    {formatRelativeTime(new Date(session.startTime).getTime(), t)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {snapshot && snapshot.stats.longestStreakDays > 1 && (
        <p className="flex items-center gap-1.5 text-caption text-text-muted">
          <Flame size={12} className="text-warn" aria-hidden="true" />
          {t('settings.gamerCompanion.nowPlaying.bestStreak').replace(
            '{{count}}',
            String(snapshot.stats.longestStreakDays),
          )}
        </p>
      )}
    </div>
  )
}
