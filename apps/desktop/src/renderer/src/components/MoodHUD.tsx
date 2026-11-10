import { bipolarToPercent } from '@mika/sdk/utils/clamp'
import { AnimatePresence, motion } from 'framer-motion'
import { memo } from 'react'
import { useEffectQuality } from '../hooks/useEffectQuality'
import { useI18n } from '../hooks/useI18n'
import { useMoodState } from '../hooks/useMoodState'
import { fixed } from '../lib/format-number'
import { MoodHistoryTrail } from './MoodHistoryTrail'
import { getMoodConfig, moodName } from './mood-hud-config'
import { Tooltip, useTooltipTrigger } from './ui/Tooltip'

export const MoodHUD = memo(function MoodHUD() {
  const { t } = useI18n()
  const visible = useEffectQuality() !== 'paused'
  const { mood, history, pulse } = useMoodState()
  const { visible: tooltipVisible, triggerProps: tooltipTriggerProps } = useTooltipTrigger()

  if (!mood) return null

  const cfg = getMoodConfig(mood.label)
  const name = moodName(mood.label, t)
  const dotX = Math.round(bipolarToPercent(mood.valence))
  const dotY = Math.round((1 - mood.arousal) * 50)

  const arousalPulseDuration = mood.arousal > 0.6 ? 0.6 : mood.arousal < -0.4 ? 2 : 1.2
  const valencePercent = bipolarToPercent(mood.valence)

  return (
    <motion.div
      role="status"
      className="pointer-events-auto relative flex flex-col gap-1.5 rounded-lg border border-border-subtle bg-bg-surface/85 px-3 py-2 backdrop-blur-md shadow-md shadow-black/20 hover:border-border-strong transition-colors"
      animate={{ boxShadow: pulse ? `0 0 14px ${cfg.glow}` : '0 0 0px transparent' }}
      transition={{ duration: 0.3 }}
      aria-label={t('shell.mood.aria').replace('{{mood}}', name)}
      {...tooltipTriggerProps}
    >
      <Tooltip
        label={t('shell.mood.tooltip')
          .replace('{{mood}}', name)
          .replace('{{valence}}', fixed(mood.valence, 2))
          .replace('{{arousal}}', fixed(mood.arousal, 2))}
        visible={tooltipVisible}
      />
      <div className="flex items-center gap-2">
        <div className="relative flex items-center justify-center">
          <motion.div
            className="absolute h-5 w-5 rounded-full border border-current opacity-40"
            animate={visible ? { scale: [1, 1.2, 1] } : { scale: 1 }}
            transition={
              visible
                ? {
                    repeat: Infinity,
                    duration: arousalPulseDuration,
                    ease: 'easeInOut',
                  }
                : { duration: 0.2 }
            }
            style={{ color: cfg.color }}
          />

          <div className="relative h-4 w-4 rounded-full border border-border-subtle bg-surface-raise overflow-hidden shrink-0">
            <div className="absolute inset-x-0 top-1/2 h-px bg-surface-raise -translate-y-1/2" aria-hidden="true" />
            <div className="absolute inset-y-0 left-1/2 w-px bg-surface-raise -translate-x-1/2" aria-hidden="true" />
            <motion.div
              className="absolute h-1.5 w-1.5 rounded-full"
              animate={{
                left: `${dotX}%`,
                top: `${dotY}%`,
                backgroundColor: cfg.color,
                boxShadow: `0 0 6px ${cfg.glow}`,
              }}
              transition={{ type: 'spring', stiffness: 120, damping: 18 }}
              style={{ translateX: '-50%', translateY: '-50%' }}
            />
          </div>
        </div>

        <AnimatePresence mode="wait">
          <motion.span
            key={mood.label}
            className="text-caption inline-block"
            initial={{ scale: 0.6, opacity: 0 }}
            animate={{ scale: pulse ? 1.15 : 1, opacity: 1 }}
            exit={{ scale: 0.6, opacity: 0 }}
            transition={{ type: 'spring', stiffness: 300, damping: 20 }}
            aria-hidden="true"
          >
            {cfg.emoji}
          </motion.span>
        </AnimatePresence>

        <AnimatePresence mode="wait">
          <motion.span
            key={mood.label}
            className="text-caption font-medium tracking-wide"
            style={{ color: cfg.color }}
            initial={{ opacity: 0, x: 4 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -4 }}
            transition={{ duration: 0.2 }}
          >
            {name}
          </motion.span>
        </AnimatePresence>

        {history.length > 0 && <MoodHistoryTrail history={history} />}
      </div>

      <motion.div
        className="h-1 w-full rounded-full overflow-hidden bg-surface-raise"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.1 }}
        role="progressbar"
        aria-valuenow={Math.round(valencePercent)}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={t('shell.mood.valence')}
      >
        <motion.div
          // scaleX, not width: a width tween relayouts the overlay (where the avatar renders) every frame.
          className="h-full w-full origin-left rounded-full"
          initial={false}
          animate={{ scaleX: valencePercent / 100 }}
          transition={{ duration: 0.5, ease: [0.22, 1, 0.36, 1] }}
          style={{
            background: 'linear-gradient(90deg, var(--danger), var(--warn), var(--ok))',
          }}
        />
      </motion.div>
    </motion.div>
  )
})
