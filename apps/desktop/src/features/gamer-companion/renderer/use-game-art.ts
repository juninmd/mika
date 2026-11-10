import { setBounded } from '@mika/sdk-ui'
import { useEffect, useState } from 'react'
import { unwrapRpcData } from '../../../renderer/src/lib/plugin-rpc'

type RpcCaller = { call<T>(method: string, args?: unknown): Promise<T> }

export type SteamArtKind = 'poster' | 'header' | 'logo' | 'icon'
export type GameArt = { dataUri: string; kind: SteamArtKind }

const ART_KINDS = new Set<string>(['poster', 'header', 'logo', 'icon'])

type ArtData = { art?: string | null; kind?: string | null }

function toGameArt(data: ArtData | null | undefined): GameArt | null {
  const dataUri = data?.art
  if (!dataUri) return null
  const kind = data?.kind
  return { dataUri, kind: kind && ART_KINDS.has(kind) ? (kind as SteamArtKind) : 'header' }
}

const resolved = new Map<string, GameArt | null>()

const MAX_CACHED_ART = 60
const inFlight = new Map<string, Promise<GameArt | null>>()

async function resolveArt(rpc: RpcCaller, appId: string): Promise<GameArt | null> {
  if (resolved.has(appId)) return resolved.get(appId) ?? null

  const pending = inFlight.get(appId)
  if (pending) return pending

  const promise = rpc
    .call<unknown>('steam:game-art', { appId })
    .then((response) => {
      const art = toGameArt(unwrapRpcData<ArtData | null | undefined>(response))
      setBounded(resolved, appId, art, MAX_CACHED_ART)
      return art
    })
    .catch(() => null)
    .finally(() => inFlight.delete(appId))

  inFlight.set(appId, promise)
  return promise
}

export function useGameArt(rpc: RpcCaller | null | undefined, appId: string | undefined) {
  const [art, setArt] = useState<GameArt | null | undefined>(() =>
    appId && resolved.has(appId) ? resolved.get(appId) : undefined,
  )

  useEffect(() => {
    if (!appId) {
      setArt(null)
      return
    }
    if (resolved.has(appId)) {
      setArt(resolved.get(appId) ?? null)
      return
    }
    if (!rpc) return

    let active = true
    setArt(undefined)
    void resolveArt(rpc, appId).then((result) => {
      if (active) setArt(result)
    })
    return () => {
      active = false
    }
  }, [rpc, appId])

  return art
}
