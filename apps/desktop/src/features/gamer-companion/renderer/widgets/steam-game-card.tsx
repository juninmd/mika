import { Gamepad2 } from 'lucide-react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { type GameArt, type SteamArtKind, useGameArt } from '../use-game-art'

type RpcCaller = { call<T>(method: string, args?: unknown): Promise<T> }

export type SteamGame = {
  name: string
  appId?: string
  appid?: number
  installed?: boolean
  playtime?: string
  playtime2weeks?: string
  playtimeTotal?: string
  playtimeMinutes?: number
  playtime2weeksMinutes?: number
}

export function appIdOf(game: SteamGame): string | undefined {
  if (game.appId) return String(game.appId)
  return game.appid ? String(game.appid) : undefined
}

export type LibraryData = {
  count?: number
  games?: SteamGame[]
  topByPlaytime?: SteamGame[]
}

function PlayBar({ minutes, maxMinutes }: { minutes?: number; maxMinutes: number }) {
  if (!minutes || !maxMinutes) return null
  const pct = Math.min((minutes / maxMinutes) * 100, 100)
  return (
    <div className="mt-1.5 h-0.5 w-full overflow-hidden rounded-full bg-surface-raise">
      <div
        className="h-full w-full origin-left rounded-full bg-info/60"
        style={{ transform: `scaleX(${pct / 100})` }}
      />
    </div>
  )
}

export const ART_ASPECT: Record<SteamArtKind, string> = {
  poster: 'aspect-[2/3]',
  header: 'aspect-[92/43]',
  logo: 'aspect-[16/9]',
  icon: 'aspect-square',
}

const ART_FIT: Record<SteamArtKind, string> = {
  poster: 'object-cover',
  header: 'object-cover',
  logo: 'object-contain p-4',
  icon: 'object-contain p-5',
}

export const GRID_CLASS = 'grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] items-start gap-3'

function GameArtBox({ art }: { art: GameArt | null | undefined }) {
  if (!art) {
    return (
      <div
        className={`flex w-full items-center justify-center bg-surface-raise ${ART_ASPECT.header} ${
          art === undefined ? 'animate-pulse' : ''
        }`}
      >
        {art === undefined ? null : <Gamepad2 aria-hidden="true" className="size-8 text-text-muted" />}
      </div>
    )
  }

  return (
    <img
      src={art.dataUri}
      alt=""
      loading="lazy"
      decoding="async"
      className={`w-full bg-surface-raise/40 ${ART_ASPECT[art.kind]} ${ART_FIT[art.kind]}`}
    />
  )
}

export function GameCard({
  game,
  maxMinutes,
  onAsk,
  rpc,
}: {
  game: SteamGame
  maxMinutes: number
  onAsk: (name: string) => void
  rpc: RpcCaller | null
}) {
  const { t } = useI18n()
  const art = useGameArt(rpc, appIdOf(game))

  return (
    <div className="group relative flex flex-col overflow-hidden rounded-xl border border-border-subtle bg-surface-raise transition hover:border-info-rim hover:bg-surface-raise-strong">
      <GameArtBox art={art} />
      <div className="px-3 py-2">
        <div className="flex items-start justify-between gap-2">
          <p className="min-w-0 flex-1 truncate text-xs font-bold text-text-main leading-tight">{game.name}</p>
          <button
            type="button"
            aria-label={t('settings.gamerCompanion.widget.steamCard.askAria', { game: game.name })}
            onClick={() => onAsk(game.name)}
            className="shrink-0 rounded-lg border border-info-rim bg-info-soft px-2 py-0.5 text-caption text-info transition hover:bg-info/25 opacity-0 group-hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:opacity-100"
          >
            {t('settings.gamerCompanion.widget.steamCard.ask')}
          </button>
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
          {game.playtime2weeks && (
            <span className="rounded bg-ok-soft border border-ok-rim px-1.5 py-0.5 text-caption font-mono text-ok">
              {game.playtime2weeks} recente
            </span>
          )}
          {(game.playtime ?? game.playtimeTotal) && (
            <span className="text-caption font-mono text-text-subtle">{game.playtime ?? game.playtimeTotal} total</span>
          )}
        </div>
        <PlayBar minutes={game.playtime2weeksMinutes ?? game.playtimeMinutes} maxMinutes={maxMinutes} />
      </div>
    </div>
  )
}
