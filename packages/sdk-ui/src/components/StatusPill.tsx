import type { TranslationDict } from '@mika/sdk/i18n'
import { t } from '@mika/sdk/i18n'
import { cn } from '../utils/cn'

export type PillVariant =
  | 'ok'
  | 'warn'
  | 'error'
  | 'loading'
  | 'inactive'
  | 'new'
  | 'pending'
  | 'neutral'
  | 'accent'
  | 'info'

interface StatusPillProps {
  variant: PillVariant

  label?: string

  dot?: boolean

  className?: string

  title?: string
}

const DEFAULT_LABEL_KEYS: Partial<Record<PillVariant, keyof TranslationDict>> = {
  ok: 'sdkUi.statusPill.ok',
  warn: 'sdkUi.statusPill.warn',
  error: 'sdkUi.statusPill.error',
  loading: 'sdkUi.statusPill.loading',
  inactive: 'sdkUi.statusPill.inactive',
  new: 'sdkUi.statusPill.new',
  pending: 'sdkUi.statusPill.pending',
}

function defaultLabel(variant: PillVariant): string {
  const key = DEFAULT_LABEL_KEYS[variant]
  return key ? t(key) : ''
}

const VARIANTS: Record<PillVariant, string> = {
  ok: 'bg-ok-soft text-ok border-ok-rim',
  warn: 'bg-warn-soft text-warn border-warn-rim',
  error: 'bg-danger-soft text-danger border-danger-rim',
  loading: 'bg-accent-soft text-accent border-accent-rim animate-pulse',

  inactive: 'bg-surface-raise text-text-subtle border-border-subtle',
  new: 'bg-accent-soft text-accent-text border-accent-rim',
  pending: 'bg-surface-raise text-text-muted border-border-subtle',
  neutral: 'bg-surface-raise text-text-muted border-border-subtle',
  accent: 'bg-accent-soft text-accent border-accent-rim',
  info: 'bg-info-soft text-info border-info-rim',
}

const DOT_COLORS: Record<PillVariant, string> = {
  ok: 'bg-ok',
  warn: 'bg-warn',
  error: 'bg-danger',
  loading: 'bg-accent',
  inactive: 'bg-text-subtle',
  new: 'bg-accent',
  pending: 'bg-text-muted',
  neutral: 'bg-text-muted',
  accent: 'bg-accent',
  info: 'bg-info',
}

/** Estados "ao vivo": o ponto emite um pulso para dizer que algo está ativo agora. */
const LIVE_VARIANTS: ReadonlySet<PillVariant> = new Set(['ok', 'loading'])

export function StatusPill({ variant, label, className, dot = false, title }: StatusPillProps) {
  const text = label ?? defaultLabel(variant)
  return (
    <span
      title={title ?? text}
      className={cn(
        'inline-flex items-center justify-center rounded-full border px-2 py-px text-micro font-black uppercase tracking-[0.12em]',
        VARIANTS[variant],
        className,
      )}
    >
      {dot && (
        <span className="relative mr-1.5 inline-flex h-1.5 w-1.5">
          {LIVE_VARIANTS.has(variant) && (
            <span
              aria-hidden="true"
              className={cn('absolute inset-0 rounded-full opacity-60 motion-safe:animate-ping', DOT_COLORS[variant])}
            />
          )}
          <span className={cn('relative h-1.5 w-1.5 rounded-full', DOT_COLORS[variant])} />
        </span>
      )}
      {text}
    </span>
  )
}
