import { getLocale } from '@mika/sdk/i18n'
import { formatLocalNumber } from '@mika/sdk-ui'
import { Activity, CalendarClock, Clock, Coins, Gauge, type LucideIcon, Receipt } from 'lucide-react'
import type { ProviderBalance, ProviderLocalUsage } from '../../../../shared/electron-api-system-types'
import type { Translate } from '../../hooks/useI18n'
import type { BillingVerdict } from './provider-billing'

export type Tone = 'ok' | 'warn' | 'danger' | 'info'

export const VERDICT_TONE: Record<BillingVerdict, Tone> = {
  ok: 'ok',
  low: 'warn',
  exhausted: 'danger',
  unknown: 'info',
  free: 'info',
  missingKey: 'warn',
  error: 'danger',
}

const TONE_TEXT: Record<Tone, string> = { ok: 'text-ok', warn: 'text-warn', danger: 'text-danger', info: 'text-info' }
export const TONE_SOFT: Record<Tone, string> = {
  ok: 'border-ok-rim bg-ok-soft text-ok',
  warn: 'border-warn-rim bg-warn-soft text-warn',
  danger: 'border-danger-rim bg-danger-soft text-danger',
  info: 'border-info-rim bg-info-soft text-info',
}

const num = (value: number) => formatLocalNumber(value, getLocale(), { maximumFractionDigits: 1 })
export const money = (value: number) =>
  formatLocalNumber(value, getLocale(), { style: 'currency', currency: 'USD', maximumFractionDigits: 4 })

export function amount(
  value: number | null,
  unit: ProviderBalance['unit'] | ProviderLocalUsage['unit'],
  t: Translate,
): string {
  if (value === null) return t('settings.providerBilling.noCap')
  return unit === 'usd' ? money(value) : `${num(value)} ${t(`settings.providerBilling.unit.${unit}`)}`
}

const PERIODS = ['daily', 'weekly', 'monthly'] as const

/** Data (ElevenLabs, epoch ms) ou período do OpenRouter (`daily`/`weekly`/`monthly`), já traduzidos. */
function resetLabel(resetsAt: ProviderBalance['resetsAt'], t: Translate): string | null {
  if (resetsAt === null) return null
  if (typeof resetsAt === 'number')
    return `${t('settings.providerBilling.resets')} ${new Date(resetsAt).toLocaleDateString(getLocale())}`
  const period = PERIODS.find((p) => p === resetsAt)
  return period ? t(`settings.providerBilling.period.${period}`) : resetsAt
}

/** Fração restante 0..1; `null` quando a chave não tem teto (não há o que medir). */
function remainingFraction(balance: ProviderBalance): number | null {
  if (!balance.limit || balance.remaining === null) return null
  return Math.min(1, Math.max(0, balance.remaining / balance.limit))
}

const RING = 2 * Math.PI * 34

function Ring({ fraction, tone, label }: { fraction: number | null; tone: Tone; label: string }) {
  const pct = fraction === null ? null : Math.round(fraction * 100)
  return (
    <div className="relative grid h-24 w-24 shrink-0 place-items-center">
      <meter className="sr-only" min={0} max={100} value={pct ?? 100} aria-label={label} />
      <svg viewBox="0 0 80 80" className="absolute inset-0 -rotate-90" aria-hidden="true">
        <circle cx="40" cy="40" r="34" fill="none" strokeWidth="7" className="stroke-current text-border-subtle" />
        <circle
          cx="40"
          cy="40"
          r="34"
          fill="none"
          strokeWidth="7"
          strokeLinecap="round"
          strokeDasharray={RING}
          strokeDashoffset={RING * (1 - (fraction ?? 1))}
          className={`stroke-current ${TONE_TEXT[tone]} transition-[stroke-dashoffset] duration-700 ease-out motion-reduce:transition-none`}
        />
      </svg>
      <span className={`text-lg font-bold tabular-nums ${TONE_TEXT[tone]}`}>{pct === null ? '∞' : `${pct}%`}</span>
    </div>
  )
}

function Chip({ icon: Icon, children }: { icon: LucideIcon; children: string }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-border-subtle bg-surface-raise px-2.5 py-1 text-xs text-text-muted">
      <Icon className="h-3 w-3" aria-hidden="true" />
      {children}
    </span>
  )
}

export function BalanceHero({ balance, tone, t }: { balance: ProviderBalance; tone: Tone; t: Translate }) {
  const fraction = remainingFraction(balance)
  const reset = resetLabel(balance.resetsAt, t)
  return (
    <div className="flex flex-col gap-5 sm:flex-row sm:items-center">
      <Ring fraction={fraction} tone={tone} label={t('settings.providerBilling.remaining')} />
      <div className="min-w-0 flex-1 space-y-3">
        <div>
          <p className="text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
            {t('settings.providerBilling.remaining')}
          </p>
          <p className={`text-3xl font-bold tabular-nums tracking-tight ${TONE_TEXT[tone]}`}>
            {amount(balance.remaining, balance.unit, t)}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Chip
            icon={Activity}
          >{`${t('settings.providerBilling.used')} ${amount(balance.used, balance.unit, t)}`}</Chip>
          <Chip icon={Gauge}>{`${t('settings.providerBilling.limit')} ${amount(balance.limit, balance.unit, t)}`}</Chip>
          {reset && <Chip icon={CalendarClock}>{reset}</Chip>}
          {balance.plan && <Chip icon={Receipt}>{balance.plan}</Chip>}
        </div>
      </div>
    </div>
  )
}

function Fact({ icon: Icon, label, value }: { icon: LucideIcon; label: string; value: string }) {
  return (
    <div
      className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2.5"
      role="status"
      aria-label={`${label}: ${value}`}
    >
      <p className="flex items-center gap-1.5 text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
        <Icon className="h-3 w-3" aria-hidden="true" />
        {label}
      </p>
      <p className="mt-1 truncate text-sm font-semibold tabular-nums text-text-main">{value}</p>
    </div>
  )
}

export function UsageFacts({ usage, t }: { usage: ProviderLocalUsage; t: Translate }) {
  const cost = usage.costUsd === null ? t('settings.providerBilling.costUnknown') : money(usage.costUsd)
  const last = usage.lastUsedAt
    ? new Date(usage.lastUsedAt).toLocaleString(getLocale())
    : t('settings.providerBilling.never')
  return (
    <div>
      <p className="mb-2 text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
        {t('settings.providerBilling.localUsage')}
      </p>
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
        <Fact icon={Activity} label={t('settings.providerBilling.used')} value={amount(usage.amount, usage.unit, t)} />
        <Fact icon={Receipt} label={t('settings.providerBilling.requests')} value={num(usage.requests)} />
        {usage.unit === 'tokens' && <Fact icon={Coins} label={t('settings.providerBilling.cost')} value={cost} />}
        <Fact icon={Clock} label={t('settings.providerBilling.lastUsed')} value={last} />
      </div>
    </div>
  )
}
