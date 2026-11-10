import { useCallback, useEffect, useState } from 'react'
import { getElectronApi } from '../lib/shell-api'

// Mirrors `ProviderHealthBanner` (Settings-only) so a circuit-open LLM provider is visible outside Settings too.
export function useProviderHealthIndicator(): boolean {
  const [providerDown, setProviderDown] = useState(false)

  const load = useCallback(() => {
    getElectronApi()
      .runtime.getProviderHealth()
      .then(({ health }) => {
        setProviderDown(health.some((item) => item.state === 'disabled'))
      })
      .catch(() => {
        setProviderDown(false)
      })
  }, [])

  useEffect(() => {
    load()
    return getElectronApi().runtime.onProviderHealth((item) => {
      if (item.state === 'disabled') setProviderDown(true)
      else load()
    })
  }, [load])

  return providerDown
}
