import type { LucideIcon } from 'lucide-react'
import { Check, CircleDashed } from 'lucide-react'
import { MOTION_CLASS } from '../motion'
import { useAccent } from './AccentContext'

type FeatureCardProps = {
  icon?: LucideIcon
  title: string
  description: string
  enabled: boolean
  onToggle: (enabled: boolean) => void
  accentHue?: number
  detail?: string
  status?: 'active' | 'inactive' | 'pending' | 'error'
}

const STATUS_LABELS: Record<NonNullable<FeatureCardProps['status']>, string> = {
  active: 'Ativo',
  inactive: 'Inativo',
  pending: 'Pendente',
  error: 'Erro',
}

export function FeatureCard({
  icon: Icon,
  title,
  description,
  enabled,
  onToggle,
  accentHue: accentHueProp,
  detail,
  status = 'inactive',
}: FeatureCardProps) {
  const accentHue = useAccent(accentHueProp)
  const accentColor = `hsl(${accentHue} 90% 60%)`
  const accentSoft = `hsl(${accentHue} 50% 50% / 0.15)`

  return (
    <button
      type="button"
      onClick={() => onToggle(!enabled)}
      aria-pressed={enabled}
      className={`group w-full rounded-xl border p-4 text-left hover:border-border-strong hover:bg-surface-raise/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${MOTION_CLASS.interactive} ${MOTION_CLASS.press}`}
      style={{
        borderColor: enabled ? accentSoft : 'var(--border-subtle)',
        backgroundColor: enabled ? `hsl(${accentHue} 50% 50% / 0.06)` : undefined,
      }}
    >
      <div className="flex items-start gap-3">
        <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-surface-raise">
          {Icon ? (
            <Icon size={16} aria-hidden="true" style={{ color: enabled ? accentColor : 'var(--text-muted)' }} />
          ) : (
            <div
              aria-hidden="true"
              className="h-3 w-3 rounded-full"
              style={{ backgroundColor: enabled ? accentColor : 'var(--text-muted)' }}
            />
          )}
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <p className="text-[12px] font-semibold text-text-main">{title}</p>
            <span
              className="text-micro font-bold uppercase tracking-wider"
              style={{ color: status === 'active' ? accentColor : 'var(--text-muted)' }}
            >
              {STATUS_LABELS[status]}
            </span>
          </div>
          <p className="mt-1 text-caption leading-relaxed text-text-muted">{description}</p>
          {detail && <p className="mt-2 text-caption font-mono text-text-muted">{detail}</p>}
        </div>

        <div className="shrink-0">
          {enabled ? (
            <Check size={18} aria-hidden="true" style={{ color: accentColor }} />
          ) : (
            <CircleDashed size={18} aria-hidden="true" style={{ color: 'var(--text-muted)' }} />
          )}
        </div>
      </div>
    </button>
  )
}
