import { t } from '@mika/sdk/i18n'
import { toDisplayError } from '@mika/sdk-ui'
import { useEffect, useState } from 'react'
import { callPluginRpc, unwrapRpcData } from '../lib/plugin-rpc'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'

const GAMER_COMPANION_ID = 'mika.gamer-companion'

export interface SteamTopGame {
  appId: string
  name: string
  playtimeMinutes: number
  playtimeLabel: string
  installed: boolean
  art: string | null
}

export type SteamTopSource = 'local' | 'api' | 'installed'

interface SteamTopGamesState {
  status: 'loading' | 'ready' | 'no-steam' | 'error'
  source: SteamTopSource
  games: SteamTopGame[]
  error?: string
}

type TopGamesData = { steamFound: boolean; source: SteamTopSource; games: SteamTopGame[] }

const LOADING: SteamTopGamesState = { status: 'loading', source: 'local', games: [] }

const cached = new Map<number, SteamTopGamesState>()
const inFlight = new Map<number, Promise<SteamTopGamesState>>()

async function loadTopGames(limit: number): Promise<SteamTopGamesState> {
  const cachedForLimit = cached.get(limit)
  if (cachedForLimit) return cachedForLimit
  const inFlightForLimit = inFlight.get(limit)
  if (inFlightForLimit) return inFlightForLimit

  const promise = (async (): Promise<SteamTopGamesState> => {
    try {
      const data = unwrapRpcData<TopGamesData | null | undefined>(
        await callPluginRpc<unknown>(getElectronApi(), GAMER_COMPANION_ID, 'steam:top-games', { limit }),
      )
      if (!data) {
        return { status: 'error', source: 'local', games: [], error: t('shell.neuralWheel.game.emptyResponse') }
      }
      const { steamFound, source, games } = data
      if (!steamFound) return { status: 'no-steam', source, games: [] }
      return { status: 'ready', source, games }
    } catch (error) {
      const message = toDisplayError(error, String(error))
      rendererLog.warn('[NeuralWheel] não deu para listar jogos da Steam', { error: message })
      return { status: 'error', source: 'local', games: [], error: message }
    }
  })()
    .then((state) => {
      if (state.status === 'ready' || state.status === 'no-steam') cached.set(limit, state)
      return state
    })
    .finally(() => {
      inFlight.delete(limit)
    })

  inFlight.set(limit, promise)
  return promise
}

export function useSteamTopGames(enabled: boolean, limit = 8): SteamTopGamesState {
  const [state, setState] = useState<SteamTopGamesState>(() => cached.get(limit) ?? LOADING)

  useEffect(() => {
    if (!enabled) return
    const cachedForLimit = cached.get(limit)
    if (cachedForLimit) {
      setState(cachedForLimit)
      return
    }

    let active = true
    setState(LOADING)
    void loadTopGames(limit).then((result) => {
      if (active) setState(result)
    })
    return () => {
      active = false
    }
  }, [enabled, limit])

  return state
}
