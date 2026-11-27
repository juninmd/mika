import { Badge, type BadgeTone } from '@juninmd/sonido'
import type { TranslationDict } from '@mika/sdk/i18n'
import { t } from '@mika/sdk/i18n'

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

/** The app's variants, mapped to the Sonido tone vocabulary (`ok`/`warn`/`error` are the old names). */
const TONES: Record<PillVariant, BadgeTone> = {
  ok: 'success',
  warn: 'warning',
  error: 'danger',
  loading: 'accent',
  inactive: 'neutral',
  new: 'accent',
  pending: 'neutral',
  neutral: 'neutral',
  accent: 'accent',
  // Sonido's Badge has no `info` tone; aqua is its cool "informação" tone.
  info: 'aqua',
}

/** States that are live right now: the point pulses, as it did in the local implementation. */
const LIVE_VARIANTS: ReadonlySet<PillVariant> = new Set(['ok', 'loading'])

/** Status/count pill. Tone vocabulary from Sonido's `Badge`; the app keeps its variant names. */
export function StatusPill({ variant, label, className, dot = false, title }: StatusPillProps) {
  const text = label ?? defaultLabel(variant)
  return (
    <Badge
      tone={TONES[variant]}
      dot={dot}
      live={dot && LIVE_VARIANTS.has(variant)}
      title={title ?? text}
      className={className}
    >
      {text}
    </Badge>
  )
}
