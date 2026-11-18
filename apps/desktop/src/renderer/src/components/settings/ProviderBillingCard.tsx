import { toDisplayError } from '@mika/sdk-ui'
import { GlassCard, SectionHeader } from '@mika/sdk-ui/settings'
import { CircleSlash, Info, KeyRound, RotateCw, TriangleAlert, Wallet } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import type { ProviderBilling } from '../../../../shared/electron-api-system-types'
import { type Translate, useI18n } from '../../hooks/useI18n'
import { getElectronApi } from '../../lib/shell-api'
import { type BillingVerdict, billingVerdict } from './provider-billing'
import { BalanceHero, TONE_SOFT, type Tone, UsageFacts, VERDICT_TONE } from './provider-billing-parts'

const NOTE_ICON: Partial<Record<BillingVerdict, typeof Info>> = {
  unknown: Info,
  free: CircleSlash,
  missingKey: KeyRound,
  error: TriangleAlert,
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

function VerdictBadge({ verdict, tone, t }: { verdict: BillingVerdict | null; tone: Tone; t: Translate }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-micro font-bold uppercase tracking-[0.12em] ${TONE_SOFT[tone]}`}
    >
      <span className="h-1.5 w-1.5 rounded-full bg-current motion-safe:animate-pulse" aria-hidden="true" />
      {t(`settings.providerBilling.verdict.${verdict ?? 'loading'}`)}
    </span>
  )
}

function Note({ verdict, tone, text }: { verdict: BillingVerdict | null; tone: Tone; text: string }) {
  const Icon = (verdict && NOTE_ICON[verdict]) || Wallet
  return (
    <div role="note" className={`flex items-start gap-3 rounded-xl border p-3 ${TONE_SOFT[tone]}`}>
      <span className="grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-surface-raise">
        <Icon className="h-4 w-4" aria-hidden="true" />
      </span>
      <p className="self-center text-sm text-text-main">{text}</p>
    </div>
  )
}

/** Créditos, cota e uso atual do provedor ativo de uma tela de provider (LLM, TTS, STT, visão…). */
export function ProviderBillingCard({ provider }: { provider: string }) {
  const { t } = useI18n()
  const { billing, error, refresh } = useProviderBilling(provider, t)
  const verdict = billing ? billingVerdict(billing) : error ? 'error' : null
  const tone: Tone = verdict ? VERDICT_TONE[verdict] : 'info'
  const note = billing ? apiNote(billing, t) : error
  return (
    <GlassCard span="full" hover={false}>
      <section data-testid="provider-billing" data-provider={provider} data-verdict={verdict ?? 'loading'}>
        <SectionHeader
          title={t('settings.providerBilling.title')}
          description={t('settings.providerBilling.description')}
          className="mb-4"
          actions={
            <div className="flex items-center gap-2">
              <VerdictBadge verdict={verdict} tone={tone} t={t} />
              <button
                type="button"
                onClick={refresh}
                aria-label={t('settings.providerBilling.refresh')}
                className="rounded-lg border border-border-subtle p-1.5 text-text-muted transition hover:border-border-strong hover:text-text-main active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </div>
          }
        />
        <div className="space-y-4">
          {billing?.api.balance && <BalanceHero balance={billing.api.balance} tone={tone} t={t} />}
          {note && <Note verdict={verdict} tone={tone} text={note} />}
          {billing?.usage && (
            <div className="border-t border-border-subtle pt-4">
              <UsageFacts usage={billing.usage} t={t} />
            </div>
          )}
        </div>
      </section>
    </GlassCard>
  )
}
