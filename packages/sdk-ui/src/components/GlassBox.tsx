import type { ElementType, ReactNode } from 'react'

const VARIANTS = {
  card: 'rounded-xl border border-border-subtle bg-surface-raise p-4',
  section: 'rounded-2xl border border-border-subtle bg-surface-raise p-5',
  compact: 'rounded-xl border border-border-subtle bg-surface-raise p-3',
  flat: 'rounded-lg border border-border-subtle bg-surface-raise',
} as const

interface GlassBoxProps {
  variant?: keyof typeof VARIANTS
  hover?: boolean
  as?: ElementType
  children: ReactNode
  className?: string
}

export function GlassBox({
  variant = 'card',
  hover = false,
  as: Tag = 'div',
  children,
  className = '',
}: GlassBoxProps) {
  return (
    <Tag
      className={`${VARIANTS[variant]} transition-colors duration-150 ${
        hover ? 'hover:border-border-strong hover:bg-surface-raise-strong' : ''
      } ${className}`}
    >
      {children}
    </Tag>
  )
}
