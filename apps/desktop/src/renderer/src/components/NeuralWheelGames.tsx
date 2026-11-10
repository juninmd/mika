import { toDisplayError } from '@mika/sdk-ui'
import { AnimatePresence, motion } from 'framer-motion'
import { Gamepad2 } from 'lucide-react'
import { memo, useCallback, useRef, useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import { type SteamTopGame, useSteamTopGames } from '../hooks/useSteamTopGames'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { GameTile, type LaunchState, RADIUS, tilePosition } from './NeuralWheelGameTile'

function CenterNotice({ title, hint }: { title: string; hint?: string }) {
  return (
    <motion.div
      role={hint ? 'alert' : 'status'}
      aria-live="polite"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="absolute -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-border-subtle bg-bg-surface/95 px-4 py-3 text-center shadow-2xl shadow-black/40"
      style={{ left: 0, top: -RADIUS, width: 240 }}
    >
      <p className="text-caption font-bold text-text-main">{title}</p>
      {hint && <p className="mt-1 text-caption leading-snug text-text-muted font-medium">{hint}</p>}
    </motion.div>
  )
}

export const NeuralWheelGames = memo(function NeuralWheelGames({
  active,
  accentColor,
  onLaunched,
}: {
  active: boolean
  accentColor: string
  onLaunched: () => void
}) {
  const { t } = useI18n()
  const { status, source, games, error } = useSteamTopGames(active, 8)
  const [launch, setLaunch] = useState<LaunchState | null>(null)

  const launchingAppIdRef = useRef<string | null>(null)

  const handleLaunch = useCallback(
    async (game: SteamTopGame) => {
      if (launchingAppIdRef.current === game.appId) return
      launchingAppIdRef.current = game.appId
      setLaunch({ appId: game.appId, status: 'launching' })
      try {
        const result = (await getElectronApi().runtime.executeTool('steam', {
          action: 'library',
          mode: 'open',
          query: game.appId,
        })) as { success?: boolean; error?: string }

        if (result?.success === false) {
          setLaunch({
            appId: game.appId,
            status: 'failed',
            message: result.error ?? t('shell.wheel.games.launchFailed'),
          })
          rendererLog.warn('[NeuralWheel] falha ao abrir jogo', { appId: game.appId, error: result.error })
          return
        }
        onLaunched()
      } catch (err) {
        const message = toDisplayError(err, String(err))
        setLaunch({ appId: game.appId, status: 'failed', message })
        rendererLog.error('[NeuralWheel] erro ao abrir jogo', { appId: game.appId, error: message })
      } finally {
        if (launchingAppIdRef.current === game.appId) launchingAppIdRef.current = null
      }
    },
    [onLaunched, t],
  )

  if (status === 'loading') {
    return <CenterNotice title={t('shell.wheel.games.loading')} />
  }

  if (status === 'no-steam') {
    return <CenterNotice title={t('shell.wheel.games.steamNotFound')} hint={t('shell.wheel.games.steamNotFoundHint')} />
  }

  if (status === 'error') {
    return <CenterNotice title={t('shell.wheel.games.readError')} hint={error} />
  }

  if (games.length === 0) {
    return <CenterNotice title={t('shell.wheel.games.empty')} hint={t('shell.wheel.games.emptyHint')} />
  }

  return (
    <>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        className="absolute flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded-full border border-border-strong/50 bg-bg-surface/90 px-3 py-1 text-micro font-black uppercase tracking-[0.15em] text-text-main shadow-lg z-10"
        style={{ left: 0, top: -RADIUS - 68 }}
      >
        <Gamepad2 size={10} aria-hidden="true" />
        {source === 'installed' ? t('shell.wheel.games.installed') : t('shell.wheel.games.topPlayed')}
      </motion.div>

      <svg
        aria-hidden="true"
        className="absolute inset-0 w-[500px] h-[500px] -translate-x-1/2 -translate-y-1/2 pointer-events-none opacity-20 text-border-strong"
      >
        <title>{t('shell.wheel.games.connectionLines')}</title>
        {games.map((game, i) => {
          const { left, top } = tilePosition(i, games.length)
          return (
            <line
              key={game.appId}
              x1="250"
              y1="250"
              x2={250 + left}
              y2={250 + top}
              stroke="currentColor"
              strokeWidth="2"
            />
          )
        })}
      </svg>

      <AnimatePresence>
        {games.map((game, index) => (
          <GameTile
            key={game.appId}
            game={game}
            index={index}
            total={games.length}
            accentColor={accentColor}
            launch={launch}
            onLaunch={handleLaunch}
          />
        ))}
      </AnimatePresence>
    </>
  )
})
