import type { ReactNode } from 'react'
import { cn } from '../utils/cn'

interface BentoGridProps {
  cols?: 1 | 2 | 3 | 'auto'

  className?: string

  children: ReactNode
}

export function BentoGrid({ cols = 'auto', className, children }: BentoGridProps) {
  return (
    <div
      className={cn(
        'gap-3',
        cols === 1 && 'grid grid-cols-1',
        cols === 2 && 'grid grid-cols-[repeat(2,minmax(0,1fr))]',
        cols === 3 && 'grid grid-cols-3',
        // Wrapping flex, not auto-fit: a full-width card occupies every grid track, so auto-fit never
        // collapses the empty ones and a row of two cards left half the section blank.
        cols === 'auto' && 'flex flex-wrap *:min-w-0 *:flex-[1_1_260px]',
        className,
      )}
    >
      {children}
    </div>
  )
}
