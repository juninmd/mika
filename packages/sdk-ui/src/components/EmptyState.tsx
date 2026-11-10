import { motion } from 'framer-motion'
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { cn } from '../utils/cn'

interface EmptyStateProps {
  title: string

  hint?: string
  icon?: LucideIcon

  action?: ReactNode

  size?: 'inline' | 'block'
  className?: string
}

export function EmptyState({ title, hint, icon: Icon, action, size = 'inline', className }: EmptyStateProps) {
  const block = size === 'block'
  return (
    <div
      role="status"
      className={cn(
        'flex flex-col items-center justify-center gap-2 text-center',
        block ? 'gap-3 py-14' : 'gap-1.5 py-6',
        className,
      )}
    >
      {Icon && (
        <motion.span
          aria-hidden="true"
          className="inline-flex"
          animate={{ y: [0, -3, 0] }}
          transition={{ duration: 3.4, repeat: Number.POSITIVE_INFINITY, ease: 'easeInOut' }}
        >
          <Icon size={block ? 24 : 16} className="text-text-subtle opacity-60" />
        </motion.span>
      )}
      <p className={cn('font-medium text-text-muted', block ? 'text-sm' : 'text-caption')}>{title}</p>
      {hint && <p className="max-w-sm text-caption leading-snug text-text-subtle">{hint}</p>}
      {action}
    </div>
  )
}
