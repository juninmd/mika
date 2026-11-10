import type { TranslationDict } from '@mika/sdk/i18n'
import { AnimatePresence, motion } from 'framer-motion'
import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useI18n } from '../hooks/useI18n'
import { layerStyle } from '../lib/layer-styles'
import { getElectronApi } from '../lib/shell-api'
import type { HudRect } from './hud/hud-layout'

interface Thought {
  id: string
  type: string
  message: string
  at: number
}

const DISPLAY_MS = 7000
const MAX_STACK = 3

const TYPE_CONFIG: Record<string, { icon: string; color: string; borderColor: string; label: keyof TranslationDict }> =
  {
    'habit-roaster': {
      icon: '🔥',
      color: 'var(--warn)',
      borderColor: 'var(--warn-rim)',
      label: 'shell.thought.habit',
    },
    'deadline-warning': {
      icon: '⏰',
      color: 'var(--danger)',
      borderColor: 'var(--danger-rim)',
      label: 'shell.thought.deadline',
    },
  }

function getConfig(type: string) {
  return (
    TYPE_CONFIG[type] ?? {
      icon: '💡',
      color: 'var(--accent)',
      borderColor: 'var(--accent-rim)',
      label: 'shell.thought.idea',
    }
  )
}

const ThoughtCard = memo(function ThoughtCard({
  thought,
  onDismiss,
}: {
  thought: Thought
  onDismiss: (id: string) => void
}) {
  const { t } = useI18n()
  const cfg = getConfig(thought.type)

  return (
    <motion.div
      layout
      initial={{ opacity: 0, x: 60, scale: 0.94 }}
      animate={{ opacity: 1, x: 0, scale: 1 }}
      exit={{ opacity: 0, x: 40, scale: 0.94 }}
      transition={{ type: 'spring', stiffness: 320, damping: 28 }}
      className="pointer-events-auto flex max-w-full items-start gap-2.5 rounded-2xl border bg-bg-surface/95 px-4 py-3 backdrop-blur-xl shadow-2xl"
      style={{ borderColor: cfg.borderColor }}
      role="status"
    >
      <span className="shrink-0 text-base leading-none mt-0.5" aria-hidden="true">
        {cfg.icon}
      </span>
      <div className="flex-1 min-w-0">
        <p className="text-micro font-bold uppercase tracking-widest mb-0.5" style={{ color: cfg.color }}>
          {t(cfg.label)}
        </p>
        <p className="text-caption leading-snug text-text-main break-words [overflow-wrap:anywhere]">
          {thought.message}
        </p>
      </div>
      <button
        type="button"
        onClick={() => onDismiss(thought.id)}
        className="-mr-1.5 -mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-xs leading-none text-text-muted transition-colors hover:bg-surface-raise hover:text-text-main focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        aria-label={t('shell.thought.dismiss')}
      >
        ✕
      </button>
    </motion.div>
  )
})

/**
 * `anchor` is the HUD toast column (right side, below the bubble zone, clear of her face); toasts
 * stack from its top, thoughts from its bottom, so the two feeds meet only when both are full.
 */
export const ProactiveThoughtBubble = memo(function ProactiveThoughtBubble({ anchor }: { anchor: HudRect }) {
  const { t } = useI18n()
  const [thoughts, setThoughts] = useState<Thought[]>([])
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>())
  const isHoveredRef = useRef(false)

  const dismiss = useCallback((id: string) => {
    setThoughts((prev) => prev.filter((t) => t.id !== id))
    const timer = timers.current.get(id)
    if (timer) {
      clearTimeout(timer)
      timers.current.delete(id)
    }
  }, [])

  useEffect(() => {
    const eAPI = getElectronApi()
    if (!eAPI?.runtime?.onBrainProactiveThought) return

    const scheduleDismiss = (id: string) => {
      const timer = setTimeout(() => {
        if (isHoveredRef.current) {
          scheduleDismiss(id)
          return
        }
        dismiss(id)
      }, DISPLAY_MS)
      timers.current.set(id, timer)
    }

    const unsub = eAPI.runtime.onBrainProactiveThought((payload) => {
      const raw = payload as { type?: string; message?: string; source?: string; context?: string }
      const text = (raw.message ?? raw.context ?? '').trim()
      if (!text) return
      const id = crypto.randomUUID()
      setThoughts((prev) => {
        const next = [...prev, { id, type: raw.type ?? raw.source ?? 'idea', message: text, at: Date.now() }]
        return next.length > MAX_STACK ? next.slice(-MAX_STACK) : next
      })
      scheduleDismiss(id)
    })

    return () => {
      unsub()
      for (const timer of timers.current.values()) clearTimeout(timer)
    }
  }, [dismiss])

  const content = (
    <section
      className="pointer-events-none fixed flex flex-col items-end justify-end gap-2"
      style={layerStyle('overlay', { left: anchor.x, top: anchor.y, width: anchor.width, height: anchor.height })}
      aria-label={t('shell.thought.region')}
      aria-live="polite"
      aria-atomic="false"
      onMouseEnter={() => {
        isHoveredRef.current = true
      }}
      onMouseLeave={() => {
        isHoveredRef.current = false
      }}
      onFocus={() => {
        isHoveredRef.current = true
      }}
      onBlur={() => {
        isHoveredRef.current = false
      }}
    >
      <AnimatePresence initial={false} mode="popLayout">
        {thoughts.map((t) => (
          <ThoughtCard key={t.id} thought={t} onDismiss={dismiss} />
        ))}
      </AnimatePresence>
    </section>
  )

  return createPortal(content, document.body)
})
