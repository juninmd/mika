import { toDisplayError } from '@mika/sdk-ui'
import { StatusPill } from '@mika/sdk-ui/settings'
import { AlertTriangle, RotateCw } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import type { LLMProviderHealth, OpenRouterKeyStatus } from '../../../../shared/electron-api-system-types'
import { useI18n } from '../../hooks/useI18n'
import { getElectronApi } from '../../lib/shell-api'
import { useUIStore } from '../../store/useUIStore'
import { OpenRouterQuotaLine } from './OpenRouterQuotaLine'

function isDataPolicyBlock(item: LLMProviderHealth): boolean {
  const reason = (item.reason ?? '').toLowerCase()
  return reason.includes('data policy') || reason.includes('zero data retention')
}

/**
 * WP9 item 6: quando o MESMO `provider:model` serve chat e visão, uma falha só de visão fica
 * indistinguível de uma falha de chat sem isto — o usuário via "provedor desativado" sem saber que
 * a Mika só parou de VER a tela, não de conversar. `true` só quando toda superfície que já reportou
 * falha para este par é `"vision"` (nunca também `"chat"`).
 */
export function isVisionOnlyFailure(item: LLMProviderHealth): boolean {
  const surfaces = item.surfaces ?? []
  return surfaces.length > 0 && surfaces.every((surface) => surface === 'vision')
}

/**
 * Extracted so the fatal/transient distinction (specs/03-resilience.md — "Duas classes de falha")
 * can be unit-tested without rendering the DOM (this repo's vitest environment is `node`, not
 * jsdom — full render is E2E's job, see AGENTS.md).
 */
export function providerHealthPresentation(item: LLMProviderHealth): {
  variant: 'error' | 'warn'
  badgeKey: 'shell.providerHealth.fatalBadge' | 'shell.providerHealth.transientBadge'
  hintKey: 'shell.providerHealth.fatalHint' | 'shell.providerHealth.transientHint'
} {
  return item.fatal
    ? { variant: 'error', badgeKey: 'shell.providerHealth.fatalBadge', hintKey: 'shell.providerHealth.fatalHint' }
    : {
        variant: 'warn',
        badgeKey: 'shell.providerHealth.transientBadge',
        hintKey: 'shell.providerHealth.transientHint',
      }
}

export function ProviderHealthBanner() {
  const { t } = useI18n()
  const [disabled, setDisabled] = useState<LLMProviderHealth[]>([])
  const [quota, setQuota] = useState<OpenRouterKeyStatus | null>(null)

  const load = useCallback(() => {
    getElectronApi()
      .runtime.getProviderHealth()
      .then(({ health }) => {
        setDisabled(health.filter((item) => item.state === 'disabled'))
      })
      .catch(() => {
        setDisabled([])
      })
  }, [])

  useEffect(() => {
    load()
    return getElectronApi().runtime.onProviderHealth((item) => {
      setDisabled((current) => {
        const rest = current.filter((entry) => entry.key !== item.key)
        return item.state === 'disabled' ? [...rest, item] : rest
      })
    })
  }, [load])

  const openrouterDown = disabled.some((item) => item.provider === 'openrouter')

  useEffect(() => {
    if (!openrouterDown) {
      setQuota(null)
      return
    }
    let alive = true
    getElectronApi()
      .runtime.getOpenRouterQuota()
      .then((result) => {
        if (alive) setQuota(result.ok && result.quota ? result.quota : null)
      })
      .catch(() => {
        if (alive) setQuota(null)
      })
    return () => {
      alive = false
    }
  }, [openrouterDown])

  const retry = useCallback(
    (item: LLMProviderHealth) => {
      void getElectronApi()
        .runtime.resetProviderCircuit(item.provider, item.model)
        .then(load)
        .catch((err) => {
          useUIStore.getState().addNotification('error', toDisplayError(err, t('shell.providerHealth.retryFailed')))
        })
    },
    [load, t],
  )

  if (disabled.length === 0) return null

  return (
    <div className="mb-4 space-y-2">
      {disabled.map((item) => (
        <div
          key={item.key}
          role="alert"
          className="flex items-start gap-3 rounded-xl border border-warn-rim bg-warn-soft p-4 shadow-sm shadow-warn/10"
        >
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-warn" aria-hidden="true" />
          <div className="min-w-0 flex-1">
            <p className="flex flex-wrap items-center gap-2 text-sm font-bold text-warn">
              <span>
                {item.provider}{' '}
                {item.model
                  ? `${t('shell.providerHealth.disabledFor')} ${item.model}`
                  : t('shell.providerHealth.disabled')}
              </span>
              <StatusPill
                variant={providerHealthPresentation(item).variant}
                label={t(providerHealthPresentation(item).badgeKey)}
              />
              {isVisionOnlyFailure(item) ? (
                <StatusPill
                  variant="neutral"
                  label={t('shell.providerHealth.visionOnlyBadge')}
                  title={t('shell.providerHealth.visionOnlyHint')}
                />
              ) : null}
            </p>
            <p className="mt-0.5 break-words text-xs text-warn/70">
              {item.reason}
              {item.statusCode ? ` (HTTP ${item.statusCode})` : ''}
            </p>
            <p className="mt-1 text-xs text-text-muted">{t(providerHealthPresentation(item).hintKey)}</p>
            {isDataPolicyBlock(item) ? (
              <p className="mt-1 text-xs text-text-muted">
                {t('shell.providerHealth.dataPolicyIntro')} <span className="font-mono">{item.model}</span>{' '}
                {t('shell.providerHealth.dataPolicyBody')} <span className="font-mono">:free</span>{' '}
                {t('shell.providerHealth.dataPolicyOutro')}{' '}
                <span className="font-mono">openrouter.ai/settings/privacy</span>.
              </p>
            ) : item.provider === 'openrouter' && quota ? (
              <OpenRouterQuotaLine quota={quota} />
            ) : null}
          </div>
          <button
            type="button"
            onClick={() => retry(item)}
            className="flex shrink-0 items-center gap-1.5 rounded-lg border border-warn-rim bg-warn-soft/80 px-2.5 py-1.5 text-xs font-bold text-warn transition active:scale-95 hover:bg-warn hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            <RotateCw className="h-3 w-3" aria-hidden="true" />
            {t('shell.providerHealth.retry')}
          </button>
        </div>
      ))}
    </div>
  )
}
