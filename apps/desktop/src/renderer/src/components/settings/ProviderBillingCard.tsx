import { getLocale } from '@mika/sdk/i18n'
import { formatLocalNumber, toDisplayError } from '@mika/sdk-ui'
import { GlassCard, SectionHeader, StatTile, StatusPill } from '@mika/sdk-ui/settings'
import { RotateCw, Wallet } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import type { ProviderBalance, ProviderBilling, ProviderLocalUsage } from '../../../../shared/electron-api-system-types'
import { type Translate, useI18n } from '../../hooks/useI18n'
import { getElectronApi } from '../../lib/shell-api'
import { type BillingVerdict, billingVerdict } from './provider-billing'

type PillVariant = Parameters<typeof StatusPill>[0]['variant']

const VERDICT_PILL: Record<BillingVerdict, PillVariant> = {
  ok: 'ok',
  low: 'warn',
  exhausted: 'error',
  unknown: 'neutral',
  free: 'info',
  missingKey: 'pending',
  error: 'error',
}

const num = (value: number) => formatLocalNumber(value, getLocale(), { maximumFractionDigits: 1 })
const money = (value: number) =>
  formatLocalNumber(value, getLocale(), { style: 'currency', currency: 'USD', maximumFractionDigits: 4 })

function amount(value: number | null, unit: ProviderBalance['unit'] | ProviderLocalUsage['unit'], t: Translate) {
  if (value === null) return t('settings.providerBilling.noCap')
  return unit === 'usd' ? money(value) : `${num(value)} ${t(`settings.providerBilling.unit.${unit}`)}`
}

function resetLabel(resetsAt: ProviderBalance['resetsAt']): string | null {
  if (resetsAt === null) return null
  return typeof resetsAt === 'number' ? new Date(resetsAt).toLocaleDateString(getLocale()) : resetsAt
}

function BalanceTiles({ balance, t }: { balance: ProviderBalance; t: Translate }) {
  const reset = resetLabel(balance.resetsAt)
  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      <StatTile
        label={t('settings.providerBilling.remaining')}
        value={amount(balance.remaining, balance.unit, t)}
        tone={balance.exhausted ? 'danger' : 'ok'}
      />
      <StatTile label={t('settings.providerBilling.used')} value={amount(balance.used, balance.unit, t)} />
      <StatTile label={t('settings.providerBilling.limit')} value={amount(balance.limit, balance.unit, t)} />
      <StatTile label={t('settings.providerBilling.resets')} value={reset ?? '—'} hint={balance.plan ?? undefined} />
    </div>
  )
}

function UsageTiles({ usage, t }: { usage: ProviderLocalUsage; t: Translate }) {
  const cost = usage.costUsd === null ? t('settings.providerBilling.costUnknown') : money(usage.costUsd)
  const last = usage.lastUsedAt
    ? new Date(usage.lastUsedAt).toLocaleString(getLocale())
    : t('settings.providerBilling.never')
  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      <StatTile label={t('settings.providerBilling.localUsage')} value={amount(usage.amount, usage.unit, t)} />
      <StatTile label={t('settings.providerBilling.requests')} value={num(usage.requests)} />
      {usage.unit === 'tokens' && <StatTile label={t('settings.providerBilling.cost')} value={cost} />}
      <StatTile label={t('settings.providerBilling.lastUsed')} value={last} />
    </div>
  )
}

function apiNote(billing: ProviderBilling, t: Translate): string | null {
  const { status, error } = billing.api
  if (status === 'unavailable') return t('settings.providerBilling.apiUnavailable')
  if (status === 'notApplicable') return t('settings.providerBilling.notApplicable')
  if (status === 'missingKey') return t('settings.providerBilling.missingKey')
  if (status === 'error') return `${t('settings.providerBilling.error')}: ${error ?? ''}`
  return null
}

function useProviderBilling(provider: string, t: Translate) {
  const [billing, setBilling] = useState<ProviderBilling | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tick, setTick] = useState(0)
  useEffect(() => {
    let alive = true
    void tick
    getElectronApi()
      .runtime.getProviderBilling(provider)
      .then((result) => {
        if (!alive) return
        setBilling(result)
        setError(null)
      })
      .catch((e) => {
        if (alive) setError(toDisplayError(e, t('settings.providerBilling.error')))
      })
    return () => {
      alive = false
    }
  }, [provider, t, tick])
  return { billing, error, refresh: useCallback(() => setTick((n) => n + 1), []) }
}

/** Créditos, cota e uso atual do provedor ativo de uma tela de provider (LLM, TTS, STT, visão…). */
export function ProviderBillingCard({ provider }: { provider: string }) {
  const { t } = useI18n()
  const { billing, error, refresh } = useProviderBilling(provider, t)
  const verdict = billing ? billingVerdict(billing) : null
  const note = billing ? apiNote(billing, t) : error
  return (
    <GlassCard span="full">
      <section data-testid="provider-billing" data-provider={provider} data-verdict={verdict ?? 'loading'}>
        <SectionHeader
          title={t('settings.providerBilling.title')}
          description={t('settings.providerBilling.description')}
          className="mb-3"
          actions={
            <div className="flex items-center gap-2">
              <StatusPill
                variant={verdict ? VERDICT_PILL[verdict] : 'loading'}
                label={t(`settings.providerBilling.verdict.${verdict ?? 'loading'}`)}
                dot
              />
              <button
                type="button"
                onClick={refresh}
                aria-label={t('settings.providerBilling.refresh')}
                className="rounded-lg border border-border-subtle p-1.5 text-text-muted transition hover:border-border-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </div>
          }
        />
        <div className="space-y-3">
          {billing?.api.balance && <BalanceTiles balance={billing.api.balance} t={t} />}
          {note && (
            <p className="flex items-center gap-2 text-xs text-text-muted" role="note">
              <Wallet className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
              {note}
            </p>
          )}
          {billing?.usage && <UsageTiles usage={billing.usage} t={t} />}
        </div>
      </section>
    </GlassCard>
  )
}
