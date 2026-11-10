import { motion, useReducedMotion } from 'framer-motion'
import { Check } from 'lucide-react'
import type { ThinkingStep } from './app/app-shell.types'

function getStepDotClass(status: ThinkingStep['status']): string {
  switch (status) {
    case 'done':
      return 'bg-ok shadow-[0_0_8px_var(--ok-soft)]'
    case 'active':
      return 'bg-info animate-pulse shadow-sm shadow-info/40'
    case 'error':
      return 'bg-danger shadow-[0_0_8px_var(--danger-soft)]'
    default:
      return 'bg-surface-raise-strong'
  }
}

function getStepTextClass(status: ThinkingStep['status']): string {
  switch (status) {
    case 'done':
      return 'text-text-muted/80'
    case 'active':
      return 'font-medium'
    case 'error':
      return 'text-danger'
    default:
      return 'text-text-muted'
  }
}

export function ThinkingStepItem({ step }: { step: ThinkingStep }) {
  return (
    <li className="flex items-center gap-2 relative z-10">
      {step.status === 'done' ? (
        <Check aria-hidden="true" size={12} className="shrink-0 text-ok" />
      ) : (
        <span className={`h-1.5 w-1.5 rounded-full ${getStepDotClass(step.status)}`} aria-hidden="true" />
      )}
      <span
        className={`text-caption transition duration-300 ${getStepTextClass(step.status)}`}
        style={step.status === 'active' ? { color: 'var(--neural-cyan)' } : undefined}
      >
        {step.label}
      </span>
    </li>
  )
}

export function BreathingDots() {
  const prefersReducedMotion = useReducedMotion()

  if (prefersReducedMotion) {
    return (
      <div className="flex items-center gap-1.5" aria-hidden="true">
        {[0, 1, 2].map((i) => (
          <span
            key={i}
            className="h-2 w-2 rounded-full"
            style={{ backgroundColor: 'var(--neural-cyan)', opacity: 0.75 }}
          />
        ))}
      </div>
    )
  }

  return (
    <div className="flex items-center gap-1.5" aria-hidden="true">
      {[0, 1, 2].map((i) => (
        <motion.span
          key={i}
          className="h-2 w-2 rounded-full"
          style={{ backgroundColor: 'var(--neural-cyan)' }}
          animate={{ scale: [1, 1.35, 1], opacity: [0.5, 1, 0.5] }}
          transition={{ duration: 1.2, repeat: Infinity, ease: 'easeInOut', delay: i * 0.22 }}
        />
      ))}
    </div>
  )
}

export function OrbitSpinner() {
  const prefersReducedMotion = useReducedMotion()
  const dots = [0, 1, 2, 3].map((i) => (
    <span
      key={i}
      className="absolute h-1.5 w-1.5 rounded-full"
      style={{
        backgroundColor: 'var(--neural-cyan)',
        opacity: 0.35 + i * 0.2,
        top: '50%',
        left: '50%',
        transformOrigin: 'center center',
        transform: `rotate(${i * 90}deg) translateY(-8px) translate(-50%, -50%)`,
      }}
    />
  ))

  if (prefersReducedMotion) {
    return (
      <div className="relative h-5 w-5" aria-hidden="true">
        {dots}
      </div>
    )
  }

  return (
    <motion.div
      className="relative h-5 w-5"
      animate={{ rotate: 360 }}
      transition={{ duration: 1.5, repeat: Infinity, ease: 'linear' }}
      aria-hidden="true"
    >
      {dots}
    </motion.div>
  )
}
