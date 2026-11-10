import type { LucideIcon } from 'lucide-react'
import type { CSSProperties, ReactNode } from 'react'

interface SectionLabelProps {
  children: ReactNode

  count?: number
  /** Ícone temático à esquerda — a mesma marca de 14px em todo cabeçalho trackeado. */
  icon?: LucideIcon
  className?: string
  style?: CSSProperties
}

/** Cabeçalho trackeado canônico (`uppercase tracking-[0.12em]`) de card e subgrupo. */
export function SectionLabel({ children, count, icon: Icon, className = '', style }: SectionLabelProps) {
  return (
    <p
      className={`flex items-center gap-2 text-caption font-semibold uppercase tracking-[0.12em] text-text-muted ${className}`}
      style={style}
    >
      {Icon && <Icon size={14} className="shrink-0 text-accent" aria-hidden="true" />}
      {children}
      {count !== undefined && (
        <span className="rounded-full border border-border-subtle bg-surface-raise px-1.5 font-mono text-caption tabular-nums tracking-normal text-text-subtle">
          {count}
        </span>
      )}
    </p>
  )
}
