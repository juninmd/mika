import { toDisplayError } from '@mika/sdk-ui'
import { useEffect, useState } from 'react'
import type { OpenRouterKeyStatus } from '../../../shared/electron-api-system-types'
import type { useI18n } from '../../src/hooks/useI18n'
import { getElectronApi } from '../../src/lib/shell-api'

export function useOpenRouterQuota(apiKey: string, t: ReturnType<typeof useI18n>['t']) {
  const [quota, setQuota] = useState<OpenRouterKeyStatus | null>(null)
  const [quotaError, setQuotaError] = useState<string | null>(null)

  useEffect(() => {
    if (!apiKey.trim()) {
      setQuota(null)
      setQuotaError(null)
      return
    }
    let alive = true
    getElectronApi()
      .runtime.getOpenRouterQuota()
      .then((result) => {
        if (!alive) return
        setQuota(result.ok && result.quota ? result.quota : null)
        setQuotaError(result.ok ? null : (result.error ?? null))
      })
      .catch((e) => {
        if (alive) setQuotaError(toDisplayError(e, t('settings.openrouter.panel.quotaLoadError')))
      })
    return () => {
      alive = false
    }
  }, [apiKey, t])

  return { quota, quotaError }
}
