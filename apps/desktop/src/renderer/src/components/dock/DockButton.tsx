import { memo, useCallback } from 'react'
import type { DockButtonProps } from './dock-types'

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
      className={`relative flex h-9 w-9 cursor-pointer items-center justify-center rounded-xl border transition-[transform,background-color,border-color,color,box-shadow] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)] enabled:hover:-translate-y-0.5 active:scale-95 active:translate-y-0 motion-reduce:enabled:hover:translate-y-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${tone}`}
    >
      {pulse && !disabled && (
        <span
          aria-hidden="true"
          className="ping-soft pointer-events-none absolute inset-0 rounded-xl border border-ok-rim"
        />
      )}
      {children}
    </button>
  )
})
