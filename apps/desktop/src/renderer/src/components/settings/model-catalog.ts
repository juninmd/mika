import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { getElectronApi } from '../../lib/shell-api'

export type ModelCapability = 'vision' | 'tools'
export type ModelCatalogKind = 'chat' | 'embedding'

export function hasCapability(model: LLMModelCatalogItem, capability?: ModelCapability): boolean {
  if (capability === 'vision') return Boolean(model.supportsVision)
  if (capability === 'tools') return Boolean(model.supportsTools)
  return true
}

export function isEmbeddingModelChange(kind: ModelCatalogKind, modelId: string, currentValue: string): boolean {
  return kind === 'embedding' && modelId !== currentValue
}

export function formatContext(tokens?: number): string | null {
  if (!tokens) return null
  return tokens >= 1000 ? `${Math.round(tokens / 1000)}k` : String(tokens)
}

export function resolvePlaceholder(custom: string | undefined, fallback: string): string {
  return custom ?? fallback
}

export function useModelCatalog(
  provider: string | undefined,
  kind: 'chat' | 'embedding',
  fallbackModels: readonly LLMModelCatalogItem[] | undefined,
  apiKey?: string,
): {
  models: LLMModelCatalogItem[]
  loading: boolean
  error: string | null
  usingFallback: boolean
  reload: () => void
} {
  const { t } = useI18n()
  const [remote, setRemote] = useState<LLMModelCatalogItem[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const requestRef = useRef<{ provider: string | undefined; kind: string } | null>(null)

  const load = useCallback(async () => {
    if (!provider) return
    const request = { provider, kind }
    requestRef.current = request
    setLoading(true)
    setError(null)
    try {
      const res = await getElectronApi().runtime.getLlmModels({ provider, kind, apiKey })
      if (requestRef.current !== request) return
      setRemote(res.models ?? [])
      setError(res.error ?? null)
    } catch (e) {
      if (requestRef.current !== request) return
      setError(toDisplayError(e, t('settings.modelPicker.loadError')))
      setRemote([])
    } finally {
      if (requestRef.current === request) setLoading(false)
    }
  }, [provider, kind, apiKey, t])

  useEffect(() => {
    void load()
  }, [load])

  const usingFallback = remote.length === 0 && (fallbackModels?.length ?? 0) > 0
  const models = useMemo<LLMModelCatalogItem[]>(
    () => (usingFallback ? [...(fallbackModels ?? [])] : remote),
    [usingFallback, fallbackModels, remote],
  )

  return { models, loading, error, usingFallback, reload: () => void load() }
}
