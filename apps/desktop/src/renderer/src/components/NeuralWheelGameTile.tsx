import { t } from '@mika/sdk/i18n'
import { motion } from 'framer-motion'
import { Download } from 'lucide-react'
import type { SteamTopGame } from '../hooks/useSteamTopGames'
import { capitalizeLocal } from '../lib/format-number'

export const RADIUS = 165
const TILE_SIZE = 64

export type LaunchState = { appId: string; status: 'launching' | 'failed'; message?: string }

export function tilePosition(index: number, total: number): { left: number; top: number } {
  const count = Math.max(1, total)
  const angle = (index * 360) / count - 90
  const radian = (angle * Math.PI) / 180
  return { left: Math.cos(radian) * RADIUS, top: Math.sin(radian) * RADIUS }
}

function GameArt({ game }: { game: SteamTopGame }) {
  if (game.art) {
    return <img src={game.art} alt="" className="h-full w-full object-cover" draggable={false} />
  }
  return (
    <div className="flex h-full w-full items-center justify-center bg-bg-surface text-lg font-black text-text-main/70">
      {capitalizeLocal(Array.from(game.name.trim())[0] ?? '') || '?'}
    </div>
  )
}

function resolveStatusText(
  state: 'launching' | 'failed' | 'idle',
  game: SteamTopGame,
  launch: LaunchState | null,
): string {
  if (state === 'failed') return launch?.message ?? t('status.failed')
  if (state === 'launching') return t('status.opening')
  return game.installed ? game.playtimeLabel : t('shell.neuralWheel.game.installViaSteam')
}

function resolveBorderColor(state: 'launching' | 'failed' | 'idle', accentColor: string): string | undefined {
  if (state === 'failed') return undefined
  if (state === 'launching') return accentColor
  return 'var(--border-subtle)'
}

export function GameTile({
  game,
  index,
  total,
  accentColor,
  launch,
  onLaunch,
}: {
  game: SteamTopGame
  index: number
  total: number
  accentColor: string
  launch: LaunchState | null
  onLaunch: (game: SteamTopGame) => void
}) {
  const { left, top } = tilePosition(index, total)
  const state = launch?.appId === game.appId ? launch.status : 'idle'
  const isLaunching = state === 'launching'
  const hasFailed = state === 'failed'

  const ariaLabel = [game.name, game.playtimeLabel, game.installed ? null : t('shell.neuralWheel.game.installViaSteam')]
    .filter(Boolean)
    .join(' · ')
  const statusText = resolveStatusText(state, game, launch)
  const borderColor = resolveBorderColor(state, accentColor)

  return (
    <motion.button
      type="button"
      aria-label={ariaLabel}
      disabled={isLaunching}
      onClick={() => onLaunch(game)}
      className="absolute flex flex-col items-center gap-1.5 rounded-2xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed"
      style={{ left, top, transform: 'translate(-50%, -50%)' }}
      initial={{ scale: 0, opacity: 0 }}
      animate={{ scale: 1, opacity: 1 }}
      exit={{ scale: 0, opacity: 0 }}
      transition={{ type: 'spring', damping: 20, stiffness: 300, delay: index * 0.03 }}
      whileHover={{ scale: 1.12 }}
      whileTap={{ scale: 0.94 }}
    >
      <div
        className={`overflow-hidden rounded-2xl border-2 shadow-xl transition-colors duration-200 ${
          hasFailed ? 'border-danger' : isLaunching ? 'animate-pulse' : ''
        }`}
        style={{ width: TILE_SIZE, height: TILE_SIZE, borderColor, opacity: game.installed ? 1 : 0.55 }}
      >
        <GameArt game={game} />
      </div>

      <div className="flex max-w-[100px] flex-col items-center gap-0.5">
        <span className="line-clamp-2 text-center text-micro font-bold leading-tight text-text-main drop-shadow">
          {game.name}
        </span>
        <span className="flex items-center gap-1 text-micro font-mono uppercase tracking-wide text-text-main/60">
          {!game.installed && <Download size={8} aria-hidden="true" />}
          {statusText}
        </span>
      </div>
    </motion.button>
  )
}
