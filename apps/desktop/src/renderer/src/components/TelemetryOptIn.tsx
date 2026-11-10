import { toDisplayError } from '@mika/sdk-ui'
import { GlassToggle } from '@mika/sdk-ui/settings'
import { AnimatePresence, motion } from 'framer-motion'
import { Shield, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import { layerStyle } from '../lib/layer-styles'
import { rendererLog } from '../lib/renderer-logger'
import { useUIStore } from '../store/useUIStore'
import { GlassCard } from './settings/ds'
import { TelemetryDisclosure } from './TelemetryDisclosure'
import { FOCUSABLE_SELECTOR } from './tool-approval-focus'

type TelemetryOptInProps = {
  onDismiss: () => void
}

const RING = 'focus:outline-none focus-visible:ring-2 focus-visible:ring-focus'

export function TelemetryOptIn({ onDismiss }: TelemetryOptInProps) {
  const { t } = useI18n()
  const [dontShow, setDontShow] = useState(false)
  const cardRef = useRef<HTMLDivElement>(null)
  const keepOffRef = useRef<HTMLButtonElement>(null)

  const handleClose = () => {
    if (dontShow) {
      void handleDecision(false)
      return
    }
    onDismiss()
  }

  const handleDecision = async (enabled: boolean) => {
    const eAPI = (
      window as unknown as {
        electronAPI?: {
          runtime: {
            setTelemetryOptIn: (p: { enabled: boolean; optInShown: boolean }) => Promise<{ success: boolean }>
          }
        }
      }
    ).electronAPI
    if (eAPI?.runtime?.setTelemetryOptIn) {
      try {
        await eAPI.runtime.setTelemetryOptIn({ enabled, optInShown: dontShow })
      } catch (error) {
        rendererLog.warn('[telemetry] persistência do opt-in falhou; diálogo fechado mesmo assim', {
          error: toDisplayError(error, String(error)),
        })
        useUIStore.getState().addNotification('error', t('shell.telemetry.saveError'))
      }
    }
    onDismiss()
  }

  const closeRef = useRef(handleClose)
  closeRef.current = handleClose

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.preventDefault()
      closeRef.current()
    }

    document.addEventListener('keydown', handleEscape)
    return () => document.removeEventListener('keydown', handleEscape)
  }, [])

  useEffect(() => {
    const previousActive = document.activeElement as HTMLElement | null
    keepOffRef.current?.focus()
    return () => {
      previousActive?.focus?.()
    }
  }, [])

  const handleCardKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'Tab') return
    const nodes = cardRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)
    if (!nodes || nodes.length === 0) return
    const first = nodes[0]
    const last = nodes[nodes.length - 1]
    const active = document.activeElement
    if (e.shiftKey && (active === first || active === cardRef.current)) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && active === last) {
      e.preventDefault()
      first.focus()
    }
  }

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 flex items-center justify-center p-4 bg-bg-deep/70 backdrop-blur-sm"
        style={layerStyle('critical')}
        onClick={handleClose}
      >
        <motion.div
          ref={cardRef}
          role="dialog"
          aria-modal="true"
          aria-labelledby="telemetry-optin-title"
          tabIndex={-1}
          onClick={(e) => e.stopPropagation()}
          onKeyDown={handleCardKeyDown}
          initial={{ scale: 0.95, opacity: 0, y: 20 }}
          animate={{ scale: 1, opacity: 1, y: 0 }}
          exit={{ scale: 0.95, opacity: 0, y: 20 }}
          transition={{ type: 'spring', stiffness: 300, damping: 25 }}
          className="relative w-full max-w-md max-h-[calc(100vh-2rem)] overflow-y-auto rounded-xl bg-bg-deep backdrop-blur-2xl"
        >
          <GlassCard accent accentHue={155} className="p-0 overflow-hidden">
            <div className="p-5">
              <div className="flex items-start justify-between gap-4">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-ok-rim bg-ok-soft text-ok">
                    <Shield size={20} aria-hidden="true" />
                  </div>
                  <div>
                    <h2 id="telemetry-optin-title" className="text-base font-semibold text-text-main">
                      {t('shell.telemetry.title')}
                    </h2>
                    <p className="text-xs text-text-muted">{t('shell.telemetry.subtitle')}</p>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={handleClose}
                  className={`rounded-lg p-1 text-text-muted transition-colors hover:bg-surface-raise hover:text-text-main active:scale-95 ${RING}`}
                  aria-label={t('shell.telemetry.close')}
                >
                  <X size={16} aria-hidden="true" />
                </button>
              </div>

              <TelemetryDisclosure />

              <div className="mt-5 flex flex-col gap-3">
                <div className="flex gap-3">
                  <button
                    type="button"
                    onClick={() => void handleDecision(true)}
                    className={`flex-1 rounded-lg border border-ok-rim bg-ok-soft px-4 py-2.5 text-xs font-semibold text-ok transition-colors hover:bg-ok hover:text-white active:scale-[0.98] shadow-xs shadow-ok/20 ${RING}`}
                  >
                    {t('shell.telemetry.enable')}
                  </button>
                  <button
                    ref={keepOffRef}
                    type="button"
                    onClick={() => void handleDecision(false)}
                    className={`flex-1 rounded-lg border border-border-subtle bg-surface-raise px-4 py-2.5 text-xs font-semibold text-text-main transition-colors hover:border-border-strong hover:bg-surface-raise-strong active:scale-[0.98] ${RING}`}
                  >
                    {t('shell.telemetry.keepOff')}
                  </button>
                </div>

                <GlassToggle label={t('shell.telemetry.dontShow')} value={dontShow} onChange={setDontShow} />
              </div>
            </div>
          </GlassCard>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  )
}
