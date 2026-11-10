import type { TranslationDict } from '@mika/sdk/i18n'
import { toDisplayError, usePluginContext } from '@mika/sdk-ui'
import { CircleDot, Grid3x3, Hand, type LucideIcon, Spade } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { unwrapRpcData } from '../../../../renderer/src/lib/plugin-rpc'
import { TrucoBoard } from './TrucoBoard'
import type { TrucoGameState } from './truco-engine'

type MiniGameId = 'jokenpo' | 'tictactoe' | 'pong' | 'truco'

const IFRAME_RESULT_GAMES = new Set<MiniGameId>(['jokenpo', 'tictactoe'])

function isIframeResultMessage(
  data: unknown,
): data is { game: 'jokenpo' | 'tictactoe'; result: 'win' | 'lose' | 'draw' } {
  if (!data || typeof data !== 'object') return false
  const { game, result } = data as { game?: unknown; result?: unknown }
  return (game === 'jokenpo' || game === 'tictactoe') && (result === 'win' || result === 'lose' || result === 'draw')
}

type MiniGamePayload =
  | { kind?: 'html'; html: string; message: string }
  | { kind: 'json'; game: 'truco'; state: TrucoGameState; message: string }

type RequestedGame = { game: MiniGameId | null }

type LoadedGame = { html: string | null; trucoState: TrucoGameState | null }

const EMPTY_GAME: LoadedGame = { html: null, trucoState: null }

function readGamePayload(payload: MiniGamePayload | undefined): LoadedGame | null {
  if (!payload) return null
  if ('state' in payload && payload.kind === 'json') return { html: null, trucoState: payload.state }
  if ('html' in payload) return { html: payload.html, trucoState: null }
  return null
}

// Pong e Truco são nomes próprios de jogo, sem tradução; os outros dois descrevem a mecânica.
const GAMES: ReadonlyArray<{ id: MiniGameId; labelKey: keyof TranslationDict; icon: LucideIcon }> = [
  { id: 'jokenpo', labelKey: 'settings.gamerCompanion.widget.miniGames.game.jokenpo', icon: Hand },
  { id: 'tictactoe', labelKey: 'settings.gamerCompanion.widget.miniGames.game.tictactoe', icon: Grid3x3 },
  { id: 'pong', labelKey: 'settings.gamerCompanion.widget.miniGames.game.pong', icon: CircleDot },
  { id: 'truco', labelKey: 'settings.gamerCompanion.widget.miniGames.game.truco', icon: Spade },
]

export function MiniGamesWidget() {
  const { t } = useI18n()
  const { api } = usePluginContext()
  const [activeGame, setActiveGame] = useState<MiniGameId | null>(null)
  const [loadedGame, setLoadedGame] = useState<LoadedGame>(EMPTY_GAME)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const requestIdRef = useRef(0)
  const loadingRef = useRef(false)
  const iframeRef = useRef<HTMLIFrameElement>(null)

  const loadGame = useCallback(
    async (game: MiniGameId) => {
      const requestId = ++requestIdRef.current
      loadingRef.current = true
      setLoading(true)
      setError(null)
      setLoadedGame(EMPTY_GAME)
      let timeoutId: ReturnType<typeof setTimeout> | undefined
      try {
        const timeoutPromise = new Promise<never>(
          (_, reject) =>
            (timeoutId = setTimeout(
              () => reject(new Error(t('settings.gamerCompanion.widget.miniGame.timeout'))),
              6000,
            )),
        )
        const result = await Promise.race([api.rpc.call<unknown>('mini-game:payload', { game }), timeoutPromise])
        if (requestId !== requestIdRef.current) return
        const loaded = readGamePayload(unwrapRpcData<MiniGamePayload | undefined>(result))
        if (loaded) {
          setActiveGame(game)
          setLoadedGame(loaded)
        } else {
          setActiveGame(null)
          setError(t('settings.gamerCompanion.widget.miniGame.loadFailed'))
        }
      } catch (err) {
        if (requestId !== requestIdRef.current) return
        setActiveGame(null)
        setError(toDisplayError(err, String(err)))
      } finally {
        if (timeoutId) clearTimeout(timeoutId)
        if (requestId === requestIdRef.current) {
          loadingRef.current = false
          setLoading(false)
        }
      }
    },
    [api, t],
  )

  useEffect(() => {
    let cancelled = false
    const openRequestedGame = async () => {
      if (loadingRef.current) return
      try {
        const requested = unwrapRpcData<RequestedGame | undefined>(await api.rpc.call<unknown>('mini-game:requested'))
        if (!cancelled && requested?.game) await loadGame(requested.game)
      } catch (err) {
        if (!cancelled) setError(toDisplayError(err, String(err)))
      }
    }
    void openRequestedGame()
    window.addEventListener('focus', openRequestedGame)
    return () => {
      cancelled = true
      window.removeEventListener('focus', openRequestedGame)
    }
  }, [api, loadGame])

  useEffect(() => {
    if (!activeGame || !IFRAME_RESULT_GAMES.has(activeGame)) return
    const onMessage = (event: MessageEvent) => {
      if (event.source !== iframeRef.current?.contentWindow) return
      if (!isIframeResultMessage(event.data)) return
      api.rpc.call('mini-game:result', { game: event.data.game, result: event.data.result }).catch(() => {})
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [activeGame, api])

  return (
    <div className="flex min-h-screen w-full flex-col p-4 sm:p-6">
      <section className="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-3 rounded-2xl border border-border-subtle bg-bg-surface p-4">
        <p className="text-micro font-bold uppercase tracking-[0.15em] text-text-muted">
          {t('settings.gamerCompanion.widget.miniGames.title')}
        </p>

        <div className="flex flex-wrap gap-2">
          {GAMES.map((game) => (
            <button
              key={game.id}
              type="button"
              aria-pressed={activeGame === game.id}
              disabled={loading}
              onClick={() => void loadGame(game.id)}
              className={`flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-caption font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50 ${
                activeGame === game.id && !loading
                  ? 'border-accent-rim bg-accent-soft text-accent'
                  : 'border-border-subtle bg-surface-raise text-text-muted hover:bg-surface-raise-strong hover:text-text-main'
              }`}
            >
              <game.icon aria-hidden="true" className="size-4" />
              {t(game.labelKey)}
            </button>
          ))}
        </div>

        {loading && (
          <p className="animate-pulse text-xs text-text-muted" role="status">
            {t('settings.gamerCompanion.widget.miniGames.loading')}
          </p>
        )}

        {error && (
          <p role="alert" className="rounded-lg border border-danger-rim bg-danger-soft px-3 py-2 text-xs text-danger">
            {error}
          </p>
        )}

        {activeGame === 'truco' && loadedGame.trucoState && !loading && (
          <TrucoBoard initialState={loadedGame.trucoState} />
        )}

        {loadedGame.html && activeGame && activeGame !== 'truco' && !loading && (
          <iframe
            ref={iframeRef}
            title={t('settings.gamerCompanion.widget.miniGame.iframeTitle', { game: activeGame })}
            sandbox="allow-scripts"
            srcDoc={loadedGame.html}
            className="min-h-0 w-full flex-1 rounded-xl border border-border-subtle bg-bg-deep"
          />
        )}

        {!loadedGame.html && !loadedGame.trucoState && !loading && !error && (
          <div className="flex flex-1 items-center justify-center rounded-xl border border-dashed border-border-subtle p-6">
            <p className="max-w-xs text-center text-xs leading-relaxed text-text-muted">
              {t('settings.gamerCompanion.widget.miniGames.emptyState')}
            </p>
          </div>
        )}
      </section>
    </div>
  )
}
