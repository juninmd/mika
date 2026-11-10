import { useEffect, useState } from 'react'
import type { LlmModelProfile } from '../../../shared/electron-api-system-types'
import { rendererLog } from '../../src/lib/renderer-logger'
import { getElectronApi } from '../../src/lib/shell-api'

export const UNKNOWN_PROFILE: LlmModelProfile = {
  contextWindow: null,
  maxOutputTokens: null,
  reasoning: null,
  topP: null,
  thinkingBudgetMax: null,
}

/** Perfil do modelo escolhido, lido do mesmo cache que o turno usa (`model_profile::resolve`). */
export function useModelProfile(provider: string, model: string): LlmModelProfile {
  const [profile, setProfile] = useState<LlmModelProfile>(UNKNOWN_PROFILE)
  useEffect(() => {
    let current = true
    setProfile(UNKNOWN_PROFILE)
    if (!model.trim()) return
    getElectronApi()
      .runtime.getModelProfile(provider, model)
      .then((next) => current && setProfile(next))
      .catch((error: unknown) =>
        rendererLog.warn('[useModelProfile] perfil indisponível', { provider, error: String(error) }),
      )
    return () => {
      current = false
    }
  }, [provider, model])
  return profile
}
