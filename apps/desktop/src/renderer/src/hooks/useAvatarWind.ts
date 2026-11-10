import { useEffect, useState } from 'react'
import { windStrengthFromConfig } from '../components/avatar/vrm-wind'
import { prefersReducedMotion } from '../lib/effect-quality'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'

interface WindConfig {
  enabled?: unknown
  strength?: unknown
}

/**
 * Wind strength 0..1 for the avatar (both backends), from `character.windEnabled` and
 * `character.windStrength`, following live config changes; 0 under reduced motion.
 */
export function useAvatarWind(): number {
  const [wind, setWind] = useState<WindConfig>({})

  useEffect(() => {
    let eAPI: ReturnType<typeof getElectronApi>
    try {
      eAPI = getElectronApi()
    } catch {
      return
    }
    let alive = true
    eAPI.runtime
      .getSettingsConfig()
      .then(({ config }) => {
        const character = config.character as Record<string, unknown> | undefined
        if (alive && character) setWind({ enabled: character.windEnabled, strength: character.windStrength })
      })
      .catch((error: unknown) => rendererLog.debug('[useAvatarWind] config indisponível, vento padrão', { error }))
    const unsubscribe = eAPI.runtime.onConfigChanged((change) => {
      if (change.path === 'character.windEnabled') setWind((prev) => ({ ...prev, enabled: change.value }))
      if (change.path === 'character.windStrength') setWind((prev) => ({ ...prev, strength: change.value }))
    })
    return () => {
      alive = false
      unsubscribe?.()
    }
  }, [])

  return windStrengthFromConfig(wind.enabled, wind.strength, prefersReducedMotion())
}
