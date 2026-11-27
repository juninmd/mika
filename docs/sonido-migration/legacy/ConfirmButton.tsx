import { t } from '@mika/sdk/i18n'
import type { LucideIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

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
  const [armed, setArmed] = useState(false)
  const confirmBtnRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (!armed) return
    confirmBtnRef.current?.focus()
    const timer = setTimeout(() => setArmed(false), 4000)
    return () => clearTimeout(timer)
  }, [armed])

  if (!armed) {
    return (
      <button
        type="button"
        onClick={() => setArmed(true)}
        disabled={disabled}
        aria-label={ariaLabel}
        className={`inline-flex min-h-8 shrink-0 items-center gap-1.5 rounded border border-border-subtle bg-surface-raise px-2.5 py-1.5 text-caption text-text-muted transition-colors hover:border-danger-rim hover:bg-danger-soft hover:text-danger focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus disabled:opacity-50 ${className}`}
      >
        {Icon ? <Icon size={11} aria-hidden="true" /> : null}
        {label}
      </button>
    )
  }

  return (
    <fieldset
      aria-label={`${t('sdkUi.confirmButton.confirmationPrefix')}: ${ariaLabel}`}
      className="inline-flex shrink-0 items-center gap-1 border-0 m-0 p-0"
      onKeyDown={(event) => {
        if (event.key === 'Escape') setArmed(false)
      }}
    >
      <button
        type="button"
        onClick={() => setArmed(false)}
        className="min-h-8 rounded border border-border-subtle px-2.5 py-1.5 text-caption font-semibold text-text-muted transition-colors hover:border-border-strong hover:bg-surface-raise hover:text-text-main focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
      >
        {cancelLabel}
      </button>
      <button
        ref={confirmBtnRef}
        type="button"
        onClick={() => {
          setArmed(false)
          onConfirm()
        }}
        disabled={disabled}
        aria-label={confirmLabel ? `${confirmLabel}: ${ariaLabel}` : ariaLabel}
        className="min-h-8 rounded bg-danger px-2.5 py-1.5 text-caption font-bold text-white transition-[filter,transform] duration-150 shadow-sm shadow-danger/30 hover:brightness-110 active:scale-[0.98] disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        {confirmLabel}
      </button>
    </fieldset>
  )
}
