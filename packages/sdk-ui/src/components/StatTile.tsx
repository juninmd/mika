import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { MOTION_CLASS } from '../motion'
import { cn } from '../utils/cn'

export type StatTone = 'neutral' | 'accent' | 'ok' | 'warn' | 'danger' | 'info'

const TONES: Record<StatTone, string> = {
  neutral: 'border-border-subtle bg-glass',
  accent: 'border-accent-rim bg-accent-soft',
  ok: 'border-ok-rim bg-ok-soft',
  warn: 'border-warn-rim bg-warn-soft',
  danger: 'border-danger-rim bg-danger-soft',
  info: 'border-info-rim bg-info-soft',
}

const TONE_LABEL: Record<StatTone, string> = {
  neutral: 'text-text-muted',
  accent: 'text-accent',
  ok: 'text-ok',
  warn: 'text-warn',
  danger: 'text-danger',
  info: 'text-info',
}

interface StatTileProps {
  label: string

  value: ReactNode

  hint?: string
  icon?: LucideIcon
  tone?: StatTone

  onClick?: () => void
  className?: string
}

export function StatTile({ label, value, hint, icon: Icon, tone = 'neutral', onClick, className }: StatTileProps) {
  const Tag = onClick ? 'button' : 'div'
  const hoverTitle = hint ?? (typeof value === 'string' ? value : undefined)
  return (
    <Tag
      type={onClick ? 'button' : undefined}
      role={onClick ? undefined : 'status'}
      aria-label={typeof value === 'string' || typeof value === 'number' ? `${label}: ${value}` : label}
      onClick={onClick}
      title={hoverTitle}
      className={cn(
        'group flex min-h-[84px] min-w-[140px] flex-col justify-center rounded-xl border p-4 text-left',
        MOTION_CLASS.interactive,
        TONES[tone],
        onClick &&
          'hover:border-accent-rim hover:bg-accent-soft active:scale-[0.97] motion-reduce:active:scale-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus',
        className,
      )}
    >
      <div
        className={cn(
          'flex items-center gap-1.5 text-caption font-medium uppercase tracking-[0.14em]',
          TONE_LABEL[tone],
        )}
      >
        {Icon && <Icon size={12} aria-hidden="true" />}
        {label}
      </div>
      {/* `key` = o valor: quando muda, o número entra de novo (sobe 4 px) em vez de trocar seco. */}
      <p
        key={typeof value === 'string' || typeof value === 'number' ? value : undefined}
        className="mt-1.5 truncate font-display text-lg font-bold tracking-tight text-text-main motion-safe:animate-in motion-safe:fade-in motion-safe:slide-in-from-bottom-1 motion-safe:duration-300"
      >
        {value}
      </p>
      {hint && <span className="mt-1 text-caption leading-snug text-text-muted">{hint}</span>}
    </Tag>
  )
}
