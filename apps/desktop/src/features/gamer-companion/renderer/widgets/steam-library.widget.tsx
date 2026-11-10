import { toDisplayError, usePluginContext, usePluginTool } from '@mika/sdk-ui'
import { MessageCircle } from 'lucide-react'
import { useCallback, useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { useUIStore } from '../../../../renderer/src/store/useUIStore'
import { ART_ASPECT, appIdOf, GameCard, GRID_CLASS, type LibraryData, type SteamGame } from './steam-game-card'

export function SteamLibraryWidget() {
  const { api } = usePluginContext()
  const { t } = useI18n()
  const [mode, setMode] = useState<'recent' | 'top'>('recent')

  const { data, loading, error, refresh } = usePluginTool<LibraryData>('steam', {
    action: 'library',
    mode: mode === 'recent' ? 'recent' : 'list',
    limit: 6,
  })

  const games: SteamGame[] = mode === 'top' ? (data?.topByPlaytime ?? data?.games ?? []) : (data?.games ?? [])

  const maxMinutes = games.reduce((m, g) => {
    const v = g.playtime2weeksMinutes ?? g.playtimeMinutes ?? 0
    return Math.max(v, m)
  }, 1)

  const askMika = useCallback(
    async (name: string) => {
      if (!api.mika.sendChatMessage) {
        useUIStore.getState().addNotification('error', t('settings.gamerCompanion.widget.chatUnavailable'))
        return
      }
      try {
        await api.mika.sendChatMessage(`Fala sobre "${name}" — vale a pena jogar agora?`)
      } catch (err) {
        useUIStore
          .getState()
          .addNotification('error', toDisplayError(err, t('settings.gamerCompanion.widget.sendFailed')))
      }
    },
    [api, t],
  )

  const askGeneral = useCallback(async () => {
    if (!api.mika.sendChatMessage) {
      useUIStore.getState().addNotification('error', t('settings.gamerCompanion.widget.chatUnavailable'))
      return
    }
    const names = games
      .slice(0, 5)
      .map((g) => g.name)
      .join(', ')
    try {
      await api.mika.sendChatMessage(
        `Olha minha biblioteca Steam: ${names}. Qual você me recomenda jogar agora e por quê?`,
      )
    } catch (err) {
      useUIStore
        .getState()
        .addNotification('error', toDisplayError(err, t('settings.gamerCompanion.widget.sendFailed')))
    }
  }, [games, api, t])

  return (
    <div className="min-h-screen w-full p-4 sm:p-6">
      <section className="mx-auto w-full max-w-5xl rounded-2xl border border-border-subtle bg-bg-surface p-4">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <svg viewBox="0 0 24 24" className="h-4 w-4 text-text-muted" fill="currentColor" aria-hidden="true">
              <path d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.029 4.524 4.524s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.004.105.004.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 11.999-5.373 11.999-12S18.605 0 11.979 0z" />
            </svg>
            <p className="text-micro font-bold uppercase tracking-[0.15em] text-text-muted">Steam</p>
            {data?.count != null && (
              <span className="text-caption text-text-subtle font-mono">
                {t('settings.gamerCompanion.widget.steam.gamesCount').replace('{{count}}', String(data.count))}
              </span>
            )}
          </div>
          <button
            type="button"
            aria-label={t('settings.gamerCompanion.widget.steam.refresh')}
            onClick={() => void refresh()}
            className="flex h-7 w-7 items-center justify-center rounded-lg border border-border-subtle bg-surface-raise text-text-muted transition hover:text-info focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              aria-hidden="true"
              className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`}
            >
              <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
            </svg>
          </button>
        </div>

        <div className="mb-3 flex gap-1.5">
          {(['recent', 'top'] as const).map((m) => (
            <button
              key={m}
              type="button"
              aria-pressed={mode === m}
              onClick={() => setMode(m)}
              className={`rounded-lg px-2.5 py-1 text-caption font-semibold transition border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                mode === m
                  ? 'bg-info-soft text-info border-info-rim'
                  : 'bg-surface-raise text-text-muted border-border-subtle hover:text-text-main hover:bg-surface-raise-strong'
              }`}
            >
              {t(
                m === 'recent'
                  ? 'settings.gamerCompanion.widget.steam.recent'
                  : 'settings.gamerCompanion.widget.steam.mostPlayed',
              )}
            </button>
          ))}
        </div>

        {error && (
          <p role="alert" className="rounded-lg border border-danger-rim bg-danger-soft px-3 py-2 text-xs text-danger">
            {error.includes('apiKey') || error.includes('steamId') || error.includes('Configure')
              ? t('settings.gamerCompanion.widget.steam.configureHint')
              : error}
          </p>
        )}

        {loading && !data && (
          <div className={GRID_CLASS} data-testid="steam-library-skeleton">
            {[1, 2, 3, 4, 5, 6].map((i) => (
              <div key={i} className={`animate-pulse rounded-xl bg-surface-raise ${ART_ASPECT.header}`} />
            ))}
          </div>
        )}

        {games.length > 0 && (
          <>
            <div className={GRID_CLASS} data-testid="steam-library-grid">
              {games.map((game, i) => (
                <GameCard
                  key={appIdOf(game) ?? `${game.name}-${i}`}
                  game={game}
                  maxMinutes={maxMinutes}
                  onAsk={askMika}
                  rpc={api.rpc ?? null}
                />
              ))}
            </div>
            <button
              type="button"
              onClick={() => void askGeneral()}
              className="mt-3 flex w-full items-center justify-center gap-1.5 rounded-xl border border-info-rim bg-info-soft py-2 text-xs font-semibold text-info transition hover:bg-info/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <MessageCircle aria-hidden="true" className="size-4" />
              {t('settings.gamerCompanion.widget.steam.askRecommendation')}
            </button>
          </>
        )}
      </section>
    </div>
  )
}
