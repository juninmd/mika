import { StatTile as SonidoStatTile, type StatTileTone } from '@juninmd/sonido'
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'

export type StatTone = 'neutral' | 'accent' | 'ok' | 'warn' | 'danger' | 'info'

/** Mika's tone names (`ok`/`warn`) onto Sonido's (`success`/`warning`). */
const TONES: Record<StatTone, StatTileTone> = {
  neutral: 'neutral',
  accent: 'accent',
  ok: 'success',
  warn: 'warning',
  danger: 'danger',
  info: 'info',
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

/**
 * A compact number block. `role="status"` stays on a wrapper because the app (and its E2E)
 * announces the tile by label; Sonido's StatTile only carries the `aria-label`.
 */
export function StatTile({ label, value, hint, icon: Icon, tone = 'neutral', onClick, className }: StatTileProps) {
  const plain = typeof value === 'string' || typeof value === 'number'
  const hoverTitle = hint ?? (typeof value === 'string' ? value : undefined)
  const tile = (
    <SonidoStatTile
      label={label}
      value={value}
      hint={hint}
      icon={Icon ? <Icon size={12} aria-hidden="true" /> : undefined}
      tone={TONES[tone]}
      onClick={onClick}
      className={className}
    />
  )
  if (onClick) return tile
  return (
    <div role="status" aria-label={plain ? `${label}: ${value}` : label} title={hoverTitle}>
      {tile}
    </div>
  )
}
