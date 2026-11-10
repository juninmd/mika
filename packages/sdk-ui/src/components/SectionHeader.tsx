import type { ReactNode } from 'react'

interface SectionHeaderProps {
  title: string
  badge?: string
  badgeClassName?: string
  description?: string
  actions?: ReactNode
  className?: string
  headingClassName?: string
}

export function SectionHeader({
  title,
  badge,
  badgeClassName = '',
  description,
  actions,
  className = '',
  headingClassName = '',
}: SectionHeaderProps) {
  return (
    <div className={`flex flex-wrap items-center justify-between gap-3 ${className}`}>
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className={`text-sm font-semibold text-text-main ${headingClassName}`}>{title}</h3>
          {badge && (
            <span
              className={`rounded-md border border-border-subtle bg-surface-raise px-2 py-0.5 text-micro font-medium uppercase tracking-[0.1em] text-text-muted ${badgeClassName}`}
            >
              {badge}
            </span>
          )}
        </div>
        {description && <p className="mt-0.5 text-caption leading-relaxed text-text-muted">{description}</p>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  )
}
