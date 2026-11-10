import type { ReactNode } from 'react'
import { MOTION_CLASS } from '../motion'
import { accentVars } from '../utils/accent'
import { cn } from '../utils/cn'
import { useAccent } from './AccentContext'

interface GlassCardProps {
  span?: 'full' | 1 | 2

  accent?: boolean

  accentHue?: number

  className?: string

  hover?: boolean

  glow?: boolean

  children: ReactNode
}

export function GlassCard({
  span,
  accent,
  accentHue: accentHueProp,
  className = '',
  hover = true,
  glow = false,
  children,
}: GlassCardProps) {
  const inheritedHue = useAccent()
  const accentHue = accentHueProp ?? inheritedHue
  const spanClass =
    span === 'full' ? 'col-span-full basis-full!' : span === 2 ? 'col-span-2 grow-2!' : span === 1 ? 'col-span-1' : ''
  return (
    <div
      className={cn(
        'group relative rounded-xl border border-border-subtle bg-glass p-4',
        MOTION_CLASS.interactive,
        // Sombra em classe, não em `style`: o inline vencia o `hover:shadow-*` e o hover nunca aparecia.
        'shadow-[0_10px_28px_rgba(15,23,42,0.12),inset_0_1px_0_var(--border-subtle)]',
        spanClass,
        // Emphasis by tint only: a purple stroke (the old 2px left bar) read as a stray border.
        accent && 'bg-[linear-gradient(to_right,var(--accent-soft),var(--glass-layer))]',
        hover && 'hover:border-border-strong hover:shadow-[0_16px_40px_rgba(15,23,42,0.18)]',
        glow && 'hover:shadow-[0_0_24px_var(--accent-glow)]',
        className,
      )}
      style={accentHueProp !== undefined ? accentVars(accentHueProp) : undefined}
      data-accent-hue={accentHue}
    >
      {glow && (
        <div
          className="pointer-events-none absolute inset-0 rounded-xl opacity-0 transition-opacity duration-200 group-hover:opacity-100"
          style={{ background: 'radial-gradient(circle at 50% 50%, var(--accent-soft), transparent)' }}
          aria-hidden="true"
        />
      )}
      {children}
    </div>
  )
}
