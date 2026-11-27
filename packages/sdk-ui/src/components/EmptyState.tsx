import { EmptyState as SonidoEmptyState } from '@juninmd/sonido'
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'

interface EmptyStateProps {
  title: string

  hint?: string
  icon?: LucideIcon

  action?: ReactNode

  size?: 'inline' | 'block'
  className?: string
}

/**
 * Empty/error state. Icon size follows `size` (the compact list variant vs the page variant);
 * the wording, icon and action are the app's. `role="status"` stays on a wrapper so the
 * existing announcements and E2E locators keep working.
 */
export function EmptyState({ title, hint, icon: Icon, action, size = 'inline', className }: EmptyStateProps) {
  const block = size === 'block'
  return (
    <div role="status">
      <SonidoEmptyState
        icon={Icon ? <Icon size={block ? 24 : 16} aria-hidden="true" /> : undefined}
        title={title}
        description={hint}
        action={action}
        className={className}
      />
    </div>
  )
}
