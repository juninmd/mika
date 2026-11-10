import { DURATION, MOTION } from '@mika/sdk-ui/settings'
import { AnimatePresence, motion } from 'framer-motion'
import { useEffect, useRef, useState } from 'react'
import { TOOL_APPROVAL_COUNTDOWN_SECONDS } from '../../../shared/tool-approval-constants'
import { useI18n } from '../hooks/useI18n'
import { AlertTriangleIcon, ApprovalCountdownRing, ToolApprovalArgs } from './ToolApprovalParts'
import { trapTab } from './tool-approval-focus'

interface ToolApprovalDialogProps {
  toolCallId: string
  toolName: string
  args: Record<string, unknown>
  onApprove: (scope: 'once' | 'session' | 'always') => void
  onDeny: () => void
  /** Card position from the HUD layout: the bubble slot (under her chin / beside her head), never her face. */
  anchor?: { centerX: number; top: number }
}

const CARD_MAX = 384
const EDGE = 12

/** Anchored: absolute at the bubble slot, clamped inside the window. Unanchored: centered. */
function cardPlacement(anchor?: { centerX: number; top: number }): React.CSSProperties {
  if (!anchor) return { position: 'relative', maxHeight: `calc(100vh - ${EDGE * 2}px)` }
  const width = Math.min(CARD_MAX, window.innerWidth - EDGE * 2)
  const left = Math.min(Math.max(anchor.centerX - width / 2, EDGE), window.innerWidth - width - EDGE)
  return { position: 'absolute', left, top: anchor.top, maxHeight: `calc(100vh - ${anchor.top + EDGE}px)` }
}

export function ToolApprovalDialog({ toolCallId, toolName, args, onApprove, onDeny, anchor }: ToolApprovalDialogProps) {
  const { t } = useI18n()
  const [countdown, setCountdown] = useState(TOOL_APPROVAL_COUNTDOWN_SECONDS)
  const timerRef = useRef<NodeJS.Timeout | null>(null)
  const dialogRef = useRef<HTMLDivElement>(null)
  const denyButtonRef = useRef<HTMLButtonElement>(null)
  const deniedRef = useRef(false)

  const denyRef = useRef(onDeny)
  denyRef.current = onDeny

  useEffect(() => {
    timerRef.current = setInterval(() => {
      setCountdown((prev) => Math.max(0, prev - 1))
    }, 1000)

    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [])

  useEffect(() => {
    if (countdown > 0 || deniedRef.current) return
    deniedRef.current = true
    denyRef.current()
  }, [countdown])

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.preventDefault()
      onDeny()
    }

    document.addEventListener('keydown', handleEscape)
    return () => document.removeEventListener('keydown', handleEscape)
  }, [onDeny])

  useEffect(() => {
    const previousActive = document.activeElement as HTMLElement | null
    denyButtonRef.current?.focus()
    return () => {
      previousActive?.focus?.()
    }
  }, [])

  const handleCardKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Tab') {
      trapTab(dialogRef.current, e)
      return
    }

    // High-risk (destructive/irreversible) dialog: deny stays one key, approve needs Ctrl/Cmd+Enter to avoid accidental release.
    if (e.key === 'n' || e.key === 'N' || e.key === 'd' || e.key === 'D') {
      e.preventDefault()
      onDeny()
      return
    }

    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      onApprove('once')
    }
  }

  const argEntries = Object.entries(args).map(([key, value]) => {
    const displayValue = typeof value === 'string' ? value : JSON.stringify(value)
    return `${key}: ${displayValue}`
  })

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={`approval-${toolCallId}`}
        className={`fixed inset-0 pointer-events-auto ${anchor ? '' : 'flex items-center justify-center'}`}
        style={{ zIndex: 'var(--layer-modal, 80)' }}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: DURATION.fast }}
      >
        <motion.div
          // Dims, never blurs: a blurred backdrop smeared her face (and the game behind a see-through window).
          className="absolute inset-0 bg-bg-deep/40"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onDeny}
          aria-hidden="true"
        />

        <motion.div
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          aria-labelledby="approval-title"
          className="rounded-2xl border border-border-subtle p-6 w-[min(24rem,calc(100vw-1.5rem))] overflow-y-auto shadow-2xl bg-bg-deep backdrop-blur-2xl"
          style={cardPlacement(anchor)}
          {...MOTION.dialog}
          onClick={(e) => e.stopPropagation()}
          onKeyDown={handleCardKeyDown}
          tabIndex={-1}
        >
          <div className="flex items-start gap-3 mb-4">
            <div className="text-warn mt-0.5" aria-hidden="true">
              <AlertTriangleIcon />
            </div>
            <div className="flex-1">
              <h2 id="approval-title" className="font-semibold text-text-main text-sm">
                {t('shell.approval.title')}
              </h2>
            </div>
          </div>

          <div
            role="alert"
            className="mb-4 rounded-lg border border-danger-rim bg-danger-soft px-3 py-2 text-xs font-semibold text-danger"
          >
            {t('shell.approval.irreversible')}
          </div>

          <p className="text-sm text-text-main/80 mb-4 leading-relaxed">
            {t('shell.approval.wants')} <span className="font-mono font-semibold text-accent">{toolName}</span>.
          </p>

          {argEntries.length > 0 && <ToolApprovalArgs argEntries={argEntries} />}

          <div className="mt-6 flex items-center justify-center gap-2 text-caption text-text-muted">
            <ApprovalCountdownRing remaining={countdown} total={TOOL_APPROVAL_COUNTDOWN_SECONDS} />
            <p role="timer" aria-live="polite" className="tabular-nums">
              {t('shell.approval.autoDenyIn')} {countdown}s.
            </p>
          </div>

          <div className="flex gap-3 mt-2">
            <button
              ref={denyButtonRef}
              type="button"
              onClick={onDeny}
              className="flex-1 px-3 py-2 rounded-lg bg-danger text-white text-sm font-bold transition-[filter,transform] duration-150 hover:brightness-110 active:scale-95 focus:outline-none focus-visible:ring-2 focus-visible:ring-focus shadow-sm shadow-danger/25"
            >
              {t('shell.approval.deny')}
            </button>
            <button
              type="button"
              onClick={() => onApprove('once')}
              className="flex-1 px-3 py-2 rounded-lg border border-accent-rim text-accent text-sm font-semibold transition-[background-color,transform] duration-150 hover:bg-accent-soft active:scale-95 focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('shell.approval.once')}
            </button>
          </div>

          <button
            type="button"
            onClick={() => onApprove('session')}
            className="w-full mt-2 px-3 py-1.5 rounded-lg border border-border-subtle text-text-muted text-xs font-medium transition-[background-color,border-color,color,transform] duration-150 hover:border-border-strong hover:bg-surface-raise hover:text-text-main active:scale-95 focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            {t('shell.approval.session')}
          </button>

          <div className="mt-3 text-center">
            <button
              type="button"
              onClick={() => onApprove('always')}
              className="rounded text-caption text-text-muted font-medium underline underline-offset-2 transition-colors duration-200 hover:text-text-main focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              title={t('shell.approval.alwaysHint')}
            >
              {t('shell.approval.always')}
            </button>
          </div>

          <p className="mt-3 text-center text-caption text-text-subtle">{t('shell.approval.keys')}</p>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  )
}
