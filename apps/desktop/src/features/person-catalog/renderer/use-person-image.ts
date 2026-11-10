import { setBounded } from '@mika/sdk-ui'
import { useEffect, useState } from 'react'

type RpcCaller = { call<T>(method: string, args?: unknown): Promise<T> }
type ImageResponse = { success: boolean; data?: { dataUrl?: string }; error?: string }

const resolved = new Map<string, string>()

const MAX_CACHED_IMAGES = 60
const inFlight = new Map<string, Promise<string | null>>()

async function resolveImage(rpc: RpcCaller, url: string): Promise<string | null> {
  const cached = resolved.get(url)
  if (cached) return cached

  const pending = inFlight.get(url)
  if (pending) return pending

  const promise = rpc
    .call<ImageResponse>('person_catalog_image', { url })
    .then((response) => {
      const dataUrl = response?.success ? response.data?.dataUrl : undefined
      if (dataUrl) setBounded(resolved, url, dataUrl, MAX_CACHED_IMAGES)
      return dataUrl ?? null
    })
    .catch(() => null)
    .finally(() => inFlight.delete(url))

  inFlight.set(url, promise)
  return promise
}

export function usePersonImage(rpc: RpcCaller | null | undefined, url: string | undefined) {
  const [dataUrl, setDataUrl] = useState<string | null | undefined>(() =>
    url ? (resolved.get(url) ?? undefined) : null,
  )

  useEffect(() => {
    if (!url) {
      setDataUrl(null)
      return
    }
    const cached = resolved.get(url)
    if (cached) {
      setDataUrl(cached)
      return
    }
    if (!rpc) return

    let active = true
    setDataUrl(undefined)
    void resolveImage(rpc, url).then((result) => {
      if (active) setDataUrl(result)
    })
    return () => {
      active = false
    }
  }, [rpc, url])

  return dataUrl
}
