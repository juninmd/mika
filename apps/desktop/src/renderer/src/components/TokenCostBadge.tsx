import { AnimatePresence, motion } from 'framer-motion'
import { memo, useEffect, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useI18n } from '../hooks/useI18n'
import { useTransientFlag } from '../hooks/useTransientFlag'
import { useChatStore } from '../store/useChatStore'
import { DEFAULT_SESSION_USAGE } from './app/app-shell.types'
import { usageBadgeState } from './token-usage-badge'
import { Tooltip, useTooltipTrigger } from './ui/Tooltip'

export const TokenCostBadge = memo(function TokenCostBadge() {
  const { t } = useI18n()
  const usage = useChatStore(useShallow((s) => s.sessionUsage ?? DEFAULT_SESSION_USAGE))
  const [flash, triggerFlash] = useTransientFlag(450)
  const prevCost = useRef(0)
  const { visible: tooltipVisible, triggerProps } = useTooltipTrigger()

  useEffect(() => {
    if (usage.costUsd > prevCost.current) triggerFlash()
    prevCost.current = usage.costUsd
  }, [usage.costUsd, triggerFlash])

  const state = usageBadgeState(usage)
  if (!state) return null

  const contextLabel =
    state.contextPercent === null
      ? null
      : t('shell.usageBadge.context').replace('{{percent}}', String(state.contextPercent))
  const ariaLabel = [
    state.cost ? t('shell.usageBadge.costAria').replace('{{cost}}', state.cost) : null,
    contextLabel,
    usage.unknownCostTurns > 0 ? t('shell.usageBadge.unknownCost', { count: usage.unknownCostTurns }) : null,
  ]
    .filter(Boolean)
    .join('. ')

  return (
    <motion.div
      role="status"
      aria-label={ariaLabel}
      className="pointer-events-auto relative flex items-center gap-1.5 rounded-full border border-border-subtle bg-bg-surface/85 px-2.5 py-1 shadow-sm shadow-black/20 backdrop-blur-sm transition-colors duration-300"
      animate={
        flash
          ? { borderColor: 'var(--info)', backgroundColor: 'var(--info-soft)' }
          : { borderColor: 'var(--border-subtle)', backgroundColor: 'var(--surface)' }
      }
      transition={{ duration: 0.15 }}
      {...triggerProps}
    >
      <Tooltip label={t('shell.usageBadge.tooltip')} visible={tooltipVisible} />
      {state.cost && (
        <AnimatePresence mode="wait">
          <motion.span
            key={state.cost}
            className="font-mono text-caption font-medium tabular-nums text-text-main"
            initial={{ opacity: 0, y: -3 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.18 }}
          >
            {state.cost}
          </motion.span>
        </AnimatePresence>
      )}
      {contextLabel && <span className="text-caption font-medium text-warn">{contextLabel}</span>}
    </motion.div>
  )
})
