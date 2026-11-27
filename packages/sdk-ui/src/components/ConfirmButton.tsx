import { ConfirmButton as SonidoConfirmButton } from '@juninmd/sonido'
import { t } from '@mika/sdk/i18n'
import type { LucideIcon } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'

export interface ConfirmButtonProps {
  label?: string
  confirmLabel?: string
  cancelLabel?: string
  ariaLabel: string
  icon?: LucideIcon
  onConfirm: () => void
  disabled?: boolean
  className?: string
}

/**
 * Two-step destructive action. The flow, Esc handling and focus return are Sonido's; the app
 * keeps its labels, icon, aria-label and the 4 s auto-dismiss (a question left open closes by
 * itself). `key` closes a pending question when `disabled` flips or the timer expires, which is
 * what the old local `useEffect` did. An empty `confirmLabel` (icon-only clear buttons) falls back
 * to the translated "Confirmar" so the armed button is never nameless.
 */
export function ConfirmButton({
  label,
  confirmLabel = t('sdkUi.confirmButton.confirm'),
  cancelLabel = t('sdkUi.confirmButton.cancel'),
  ariaLabel,
  icon: Icon,
  onConfirm,
  disabled = false,
  className = '',
}: ConfirmButtonProps) {
  const [cycle, setCycle] = useState(0)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const scheduleClose = useCallback(() => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => setCycle((n) => n + 1), 4000)
  }, [])
  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current)
    },
    [],
  )

  const button = (
    <SonidoConfirmButton
      key={`${disabled ? 'disabled' : 'ready'}-${cycle}`}
      ask=""
      confirmLabel={confirmLabel || t('sdkUi.confirmButton.confirm')}
      cancelLabel={cancelLabel}
      onConfirm={onConfirm}
      disabled={disabled}
      size="sm"
      aria-label={ariaLabel}
      iconStart={Icon ? <Icon size={11} aria-hidden="true" /> : undefined}
    >
      {label || undefined}
    </SonidoConfirmButton>
  )
  // Sonido drops `className` in the asking state; the wrapper keeps layout classes (ml-auto,
  // mb-3) stable across both steps. `display: contents` keeps the extra event wrapper out of
  // layout, so the two-step button still sits exactly where the old <button> sat.
  return (
    <div style={{ display: 'contents' }} onClickCapture={scheduleClose}>
      {className ? <div className={className}>{button}</div> : button}
    </div>
  )
}
