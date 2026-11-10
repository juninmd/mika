import { motion } from 'framer-motion'
import type { LucideIcon } from 'lucide-react'
import { Tooltip, useTooltipTrigger } from './ui/Tooltip'

export type SenseStatus = 'active' | 'idle' | 'off' | 'error'

const DOT_COLOR: Record<SenseStatus, string> = {
  active: 'var(--ok)',
  idle: 'var(--text-muted)',
  off: 'var(--surface-raise-strong)',
  error: 'var(--danger)',
}

interface SenseDotProps {
  icon: LucideIcon
  status: SenseStatus
  label: string
  detail?: string
}

export function SenseDot({ icon: Icon, status, label, detail }: SenseDotProps) {
  const { visible, triggerProps } = useTooltipTrigger()

  const color = DOT_COLOR[status]
  const isActive = status === 'active'
  const isError = status === 'error'

  return (
    <div className="relative flex flex-col items-center gap-0.5" {...triggerProps} role="status" aria-label={label}>
      <motion.span
        className="flex text-text-muted"
        aria-hidden="true"
        animate={
          isError ? { x: [-2, 2, -2, 2, 0], opacity: 1 } : isActive ? { opacity: [0.6, 1, 0.6] } : { opacity: 0.55 }
        }
        transition={
          isError
            ? { duration: 0.35, ease: 'easeInOut' }
            : isActive
              ? { duration: 1.5, repeat: Infinity, ease: 'easeInOut' }
              : {}
        }
      >
        <Icon size={14} />
      </motion.span>

      <motion.span
        className="h-1.5 w-1.5 rounded-full"
        style={{ backgroundColor: color }}
        animate={
          isActive
            ? {
                scale: [1, 1.18, 1],
                boxShadow: [`0 0 4px ${color}`, `0 0 10px ${color}`, `0 0 4px ${color}`],
              }
            : isError
              ? { boxShadow: '0 0 8px var(--danger-soft)' }
              : { scale: 1, boxShadow: 'none' }
        }
        transition={isActive ? { duration: 1.5 / 1.5, repeat: Infinity, ease: 'easeInOut' } : { duration: 0.3 }}
      />

      <Tooltip label={label} detail={detail} visible={visible} />
    </div>
  )
}
