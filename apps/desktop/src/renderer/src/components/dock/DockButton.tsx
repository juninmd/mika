import { memo, useCallback } from 'react'
import type { DockButtonProps } from './dock-types'

/** Three bars that bounce while the mic is hearing — the "she is listening" cue lives on the button itself. */
function MicLevel() {
  return (
    <span aria-hidden="true" className="pointer-events-none absolute right-1 top-1 flex h-2.5 items-end gap-px">
      <span className="dock-level h-full w-0.5 rounded-full bg-ok" />
      <span className="dock-level h-full w-0.5 rounded-full bg-ok [animation-delay:120ms]" />
      <span className="dock-level h-full w-0.5 rounded-full bg-ok [animation-delay:240ms]" />
    </span>
  )
}

/** Open/on dot under the icon (state readable without color alone) and the live mic level. */
function DockMarkers({ pulse, active }: { pulse: boolean; active: boolean }) {
  return (
    <>
      {pulse && <MicLevel />}
      <span
        aria-hidden="true"
        className={`pointer-events-none absolute bottom-0.5 left-1/2 h-1 w-1 -translate-x-1/2 rounded-full bg-accent transition-opacity duration-150 ${active ? 'opacity-100' : 'opacity-0'}`}
      />
    </>
  )
}

export const DockButton = memo(function DockButton({
  children,
  onClick,
  active,
  pulse,
  title,
  ariaExpanded,
  disabled,
  onHoldStart,
  onHoldEnd,
  onPeek,
}: DockButtonProps) {
  const holdMode = Boolean(onHoldStart && onHoldEnd)
  const handleClick = useCallback(
    (e: React.MouseEvent<HTMLButtonElement>) => {
      e.stopPropagation()
      if (disabled || holdMode) return
      onClick()
      // Foco por clique de mouse não é navegação por teclado: sem soltar aqui o dock ficava "focado"
      // para sempre e nunca mais se recolhia num app que abre o composer com um clique.
      e.currentTarget.blur()
    },
    [disabled, holdMode, onClick],
  )

  const tone = disabled
    ? 'cursor-not-allowed text-text-main/25'
    : active
      ? 'border-accent-rim bg-accent-soft text-accent-text'
      : pulse
        ? 'border-ok-rim bg-ok-soft text-ok'
        : 'border-transparent text-text-muted hover:shadow-[0_6px_16px_-8px_var(--accent-glow)] hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main'

  return (
    <button
      type="button"
      aria-label={title}
      aria-pressed={Boolean(active)}
      aria-expanded={ariaExpanded}
      disabled={disabled}
      onClick={handleClick}
      onPointerEnter={onPeek}
      onFocus={onPeek}
      onPointerDown={holdMode && !disabled ? onHoldStart : undefined}
      onPointerUp={holdMode && !disabled ? onHoldEnd : undefined}
      onPointerLeave={holdMode && !disabled ? onHoldEnd : undefined}
      data-dock-button
      className={`relative flex h-9 w-9 origin-bottom cursor-pointer items-center justify-center rounded-xl border transition-[transform,scale,background-color,border-color,color,box-shadow] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)] enabled:hover:-translate-y-0.5 active:scale-95 active:translate-y-0 motion-reduce:enabled:hover:translate-y-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${tone}`}
    >
      {pulse && !disabled && (
        <span
          aria-hidden="true"
          className="ping-soft pointer-events-none absolute inset-0 rounded-xl border border-ok-rim"
        />
      )}
      {children}
      <DockMarkers pulse={Boolean(pulse && !disabled)} active={Boolean(active)} />
    </button>
  )
})
