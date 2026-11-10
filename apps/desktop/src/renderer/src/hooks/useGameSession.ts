import { useCallback, useState } from 'react'
import { callPluginRpc } from '../lib/plugin-rpc'
import { getElectronApi } from '../lib/shell-api'
import { useVisiblePolling } from './useVisiblePolling'

export type GameSession = { id: string; gameName: string; startTime: string; events?: unknown[] }

const PLUGIN_ID = 'mika.gamer-companion'
/** Game detection itself runs on a multi-second cadence in Rust; polling faster only burns IPC. */
const POLL_MS = 15_000

/** The running game session (`session-current`), or null — polled only while the window is visible. */
export function useGameSession(): GameSession | null {
  const [session, setSession] = useState<GameSession | null>(null)
  const load = useCallback(() => {
    callPluginRpc<GameSession | null>(getElectronApi(), PLUGIN_ID, 'session-current')
      .then((current) => setSession((prev) => (prev?.id === current?.id ? prev : (current ?? null))))
      .catch(() => setSession(null))
  }, [])
  useVisiblePolling(load, POLL_MS)
  return session
}
