import { t } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { motion } from 'framer-motion'
import { useCallback, useId, useState } from 'react'
import { createUiLogger } from '../logger'
import { SPRING } from '../motion'
import { accentVars } from '../utils/accent'
import { StatusPill } from './StatusPill'

const log = createUiLogger('ui:glass-toggle')

interface GlassToggleProps {
  label: string
  description?: string
  configPath?: string
  value: boolean
  accentHue?: number
  badge?: string
  onChange?: (value: boolean) => void
  onSave?: (path: string, value: boolean) => Promise<void>
  disabled?: boolean
  /** Só o interruptor: para linhas de card que já mostram o nome (o rótulo segue no aria-label). */
  hideLabel?: boolean
}

export function GlassToggle({
  label,
  description,
  configPath,
  value,
  accentHue: accentHueProp,
  badge,
  onChange,
  onSave,
  disabled = false,
  hideLabel = false,
}: GlassToggleProps) {
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const descId = useId()
  const errorId = useId()

  const toggle = useCallback(async () => {
    const nextVal = !value
    onChange?.(nextVal)
    if (onSave && configPath) {
      setSaving(true)
      try {
        await onSave(configPath, nextVal)
        setSaveError(null)
      } catch (error) {
        const message = toErrorMessage(error)
        log.error('failed to save setting', { configPath, error: message })
        setSaveError(message)
        onChange?.(value)
      } finally {
        setSaving(false)
      }
    }
  }, [configPath, value, onChange, onSave])

  return (
    <div className="flex w-full items-center justify-between gap-4 py-0.5" style={accentVars(accentHueProp)}>
      <div className={hideLabel ? 'sr-only' : 'min-w-0 flex-1'}>
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-[13px] font-medium leading-none text-text-main">{label}</span>
          {badge && <StatusPill variant="accent" label={badge} />}
        </div>
        {description && (
          <p id={descId} className="mt-1 max-w-prose text-caption leading-snug text-text-muted">
            {description}
          </p>
        )}
        {saveError && (
          <p id={errorId} role="alert" className="mt-1 text-caption leading-snug text-danger">
            {saveError || t('sdkUi.saveError')}
          </p>
        )}
      </div>

      <button
        type="button"
        role="switch"
        data-sdk-control="toggle"
        aria-checked={value}
        aria-label={label}
        aria-describedby={
          [description ? descId : null, saveError ? errorId : null].filter(Boolean).join(' ') || undefined
        }
        disabled={saving || disabled}
        onClick={() => void toggle()}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            void toggle()
          }
        }}
        tabIndex={0}
        className="group relative shrink-0 rounded-full disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 focus-visible:ring-offset-bg-surface"
      >
        <div
          aria-hidden="true"
          className={`h-7 w-12 rounded-full border transition-colors duration-200 ${
            value
              ? 'border-accent-rim bg-accent-soft group-hover:bg-accent-soft/80'
              : 'border-border-subtle bg-surface-raise group-hover:border-border-strong'
          }`}
        />
        <motion.div
          aria-hidden="true"
          initial={false}
          animate={{ x: value ? 24 : 4 }}
          transition={SPRING.snappy}
          className={`absolute top-1 h-5 w-5 rounded-full transition-colors ${
            value ? 'bg-text-main' : 'bg-text-muted group-hover:bg-text-main'
          }`}
          style={value ? { boxShadow: '0 0 10px var(--accent), 0 2px 4px rgba(0,0,0,0.3)' } : undefined}
        />
      </button>
    </div>
  )
}
