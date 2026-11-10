import { t } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { useCallback, useEffect, useId, useRef, useState } from 'react'
import { createUiLogger } from '../logger'
import { accentVars } from '../utils/accent'

const log = createUiLogger('ui:glass-slider')

interface GlassSliderProps {
  label: string
  description?: string
  configPath?: string
  value: number
  min: number
  max: number
  step?: number
  accentHue?: number
  showValue?: boolean | ((v: number) => string)
  minLabel?: string
  maxLabel?: string
  config?: Record<string, unknown>
  disabled?: boolean
  onChange?: (value: number) => void
  onSave?: (path: string, value: number) => Promise<void>
  /** O usuário soltou o controle (mouse/touch/tecla/blur) — hora de salvar no modo controlado. */
  onCommit?: (value: number) => void
  /** Id de texto externo que descreve o campo (erro/ajuda renderizados fora do controle). */
  describedBy?: string
}

export function GlassSlider({
  label,
  description,
  configPath,
  value,
  min,
  max,
  step = 1,
  accentHue: accentHueProp,
  showValue = true,
  minLabel,
  maxLabel,
  disabled = false,
  onChange,
  onSave,
  onCommit,
  describedBy,
}: GlassSliderProps) {
  const id = useId()
  const [draft, setDraft] = useState(value)
  const [saveError, setSaveError] = useState<string | null>(null)
  // mouseup + blur (and keyup after each arrow) all commit: without this the same value hit IPC twice.
  const committed = useRef(value)

  useEffect(() => {
    setDraft(value)
    committed.current = value
  }, [value])

  const pct = ((draft - min) / (max - min)) * 100
  const displayValue = typeof showValue === 'function' ? showValue(draft) : showValue ? String(draft) : null
  const formatBound = (bound: number) => (typeof showValue === 'function' ? showValue(bound) : String(bound))

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = Number(e.target.value)
    setDraft(val)
    onChange?.(val)
  }
  const handleCommit = useCallback(async () => {
    if (draft === committed.current) return
    committed.current = draft
    onCommit?.(draft)
    if (!onSave || !configPath) return

    try {
      await onSave(configPath, draft)
      setSaveError(null)
    } catch (error) {
      const message = toErrorMessage(error)
      log.error('failed to save setting', { configPath, error: message })
      setSaveError(message)
      onChange?.(value)
      setDraft(value)
      committed.current = value
    }
  }, [configPath, draft, onSave, onChange, onCommit, value])

  return (
    <div className={`space-y-2${disabled ? ' opacity-50' : ''}`}>
      <div className="flex flex-wrap items-center gap-2">
        <label htmlFor={id} className="text-[13px] font-medium text-text-main">
          {label}
        </label>
        {displayValue && (
          <span className="whitespace-nowrap rounded-md border border-border-subtle bg-surface-raise px-2 py-0.5 font-mono text-caption font-medium text-text-main">
            {displayValue}
          </span>
        )}
      </div>

      <div className="space-y-2">
        <div className="relative flex items-center">
          <div className="relative w-full">
            <div
              aria-hidden="true"
              className="absolute inset-y-0 my-auto h-1.5 w-full overflow-hidden rounded-full bg-surface-raise-strong"
            >
              <div
                className="h-full rounded-full"
                style={{
                  width: `calc(${pct}% + ${8 - pct * 0.16}px)`,
                  backgroundColor: 'var(--accent)',
                }}
              />
            </div>
            <input
              data-sdk-control="slider"
              id={id}
              type="range"
              min={min}
              max={max}
              step={step}
              value={draft}
              disabled={disabled}
              aria-valuenow={draft}
              aria-valuemin={min}
              aria-valuemax={max}
              aria-valuetext={displayValue ?? undefined}
              aria-describedby={describedBy}
              onChange={handleChange}
              onMouseUp={() => void handleCommit()}
              onTouchEnd={() => void handleCommit()}
              onBlur={() => void handleCommit()}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void handleCommit()
              }}
              onKeyUp={(e) => {
                if (
                  e.key === 'ArrowLeft' ||
                  e.key === 'ArrowRight' ||
                  e.key === 'ArrowUp' ||
                  e.key === 'ArrowDown' ||
                  e.key === 'Home' ||
                  e.key === 'End' ||
                  e.key === 'PageUp' ||
                  e.key === 'PageDown'
                ) {
                  void handleCommit()
                }
              }}
              className="relative block h-4 w-full cursor-pointer appearance-none bg-transparent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus rounded-full disabled:cursor-not-allowed [&::-webkit-slider-thumb]:-mt-[5px] [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-accent-rim [&::-webkit-slider-thumb]:bg-text-main [&::-webkit-slider-thumb]:transition-transform [&::-webkit-slider-thumb]:hover:scale-110 [&::-webkit-slider-thumb]:active:scale-95 [&::-webkit-slider-runnable-track]:h-1.5"
              style={accentVars(accentHueProp)}
            />
          </div>
        </div>

        <div className="flex justify-between gap-2 text-caption tabular-nums text-text-muted">
          <span>{minLabel ?? formatBound(min)}</span>
          <span className="text-right">{maxLabel ?? formatBound(max)}</span>
        </div>
      </div>

      {saveError && (
        <p role="alert" className="text-caption leading-snug text-danger">
          {saveError || t('sdkUi.saveError')}
        </p>
      )}

      {description && <p className="text-caption leading-snug text-text-muted">{description}</p>}
    </div>
  )
}
