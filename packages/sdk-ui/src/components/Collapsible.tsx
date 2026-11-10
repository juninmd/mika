import { AnimatePresence, motion } from 'framer-motion'
import type { ReactNode } from 'react'
import { DURATION, EASE } from '../motion'

interface CollapsibleProps {
  open: boolean
  children: ReactNode
  className?: string
  fast?: boolean
  id?: string
}

export function Collapsible({ open, children, className = '', fast = false, id }: CollapsibleProps) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          id={id}
          role="region"
          aria-hidden={!open}
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: 'auto', opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          transition={{ duration: fast ? DURATION.base : DURATION.slow, ease: EASE.out }}
          className={className}
          style={{ overflow: 'hidden' }}
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  )
}
