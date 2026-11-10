import { AnimatePresence, motion } from 'framer-motion'
import { memo, useEffect, useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import { getFriendlyToolLabel } from '../lib/tool-labels'
import type { ThinkingStep } from './app/app-shell.types'
import { BreathingDots, OrbitSpinner, ThinkingStepItem } from './ThinkingIndicators'

/** Bolha unica do rodape: o que foi ouvido, o que esta sendo pensado e qual ferramenta rodou. */
interface ThinkingHUDProps {
  visible: boolean
  activeTool?: string | null
  steps?: ThinkingStep[]
  thinkingText?: string
  /** O que o usuario acabou de falar. Aparece mesmo sem turno em andamento. */
  transcript?: string
}

export const ThinkingHUD = memo(function ThinkingHUD({
  visible,
  activeTool,
  steps = [],
  thinkingText,
  transcript,
}: ThinkingHUDProps) {
  const { t } = useI18n()
  const [useSpinner, setUseSpinner] = useState(false)

  useEffect(() => {
    if (!visible) {
      setUseSpinner(false)
      return
    }
    const id = setTimeout(() => setUseSpinner(true), 3000)
    return () => clearTimeout(id)
  }, [visible])

  return (
    <AnimatePresence>
      {(visible || Boolean(transcript)) && (
        <motion.div
          role="status"
          aria-live="polite"
          aria-label={t('shell.thinking.label')}
          className="pointer-events-none relative flex justify-center"
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 8 }}
          transition={{ duration: 0.2 }}
        >
          <div className="pointer-events-auto flex max-w-[min(80vw,24rem)] flex-col gap-2 rounded-[2rem] border border-accent-rim bg-bg-surface/90 backdrop-blur-xl shadow-2xl shadow-black/50 px-4 py-3 transition duration-500 ease-out">
            <div className="absolute inset-0 rounded-[2rem] overflow-hidden pointer-events-none">
              <div
                className="absolute -top-1/2 -left-1/4 h-full w-full opacity-15"
                style={{
                  background: 'linear-gradient(135deg, var(--neural-purple), var(--neural-cyan))',
                  filter: 'blur(40px)',
                }}
              />
            </div>
            <div className="relative">
              {visible && (
                <div className="flex items-center gap-2.5 mb-2">
                  <AnimatePresence mode="wait">
                    {useSpinner ? (
                      <motion.div
                        key="spinner"
                        initial={{ opacity: 0, scale: 0.8 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0 }}
                      >
                        <OrbitSpinner />
                      </motion.div>
                    ) : (
                      <motion.div key="dots" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
                        <BreathingDots />
                      </motion.div>
                    )}
                  </AnimatePresence>
                  {/* Sólido em token semântico: o gradiente recortado em texto perdia contraste sobre
                      o fundo claro do modo semi-transparente. */}
                  <span className="text-xs font-semibold uppercase tracking-wider text-text-main">
                    {t('status.thinking')}
                  </span>
                </div>
              )}

              {transcript && (
                <p className="m-0 mb-1 line-clamp-3 text-xs italic text-text-main [overflow-wrap:anywhere]">
                  &ldquo;{transcript}&rdquo;
                </p>
              )}

              {activeTool && (
                <div
                  className="flex items-center gap-1.5 rounded-xl px-2.5 py-1.5 border border-info-rim/60 bg-info-soft shadow-xs shadow-info/10"
                  role="status"
                  aria-label={getFriendlyToolLabel(activeTool)}
                >
                  <span className="text-caption font-medium text-info">{getFriendlyToolLabel(activeTool)}</span>
                </div>
              )}

              {(() => {
                const activeToolLabel = activeTool ? getFriendlyToolLabel(activeTool) : null
                const visibleSteps = activeToolLabel
                  ? steps.filter((step) => !(step.status === 'active' && step.label === activeToolLabel))
                  : steps

                if (visibleSteps.length === 0) return null

                return (
                  <ol className="flex flex-col gap-1 pl-0 mt-2" aria-label={t('shell.thinking.steps')}>
                    {visibleSteps.map((step) => (
                      <ThinkingStepItem key={step.id} step={step} />
                    ))}
                  </ol>
                )
              })()}

              {thinkingText && (
                <p className="m-0 text-caption italic text-text-muted line-clamp-2 mt-1 [overflow-wrap:anywhere]">
                  {thinkingText}
                </p>
              )}
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  )
})
