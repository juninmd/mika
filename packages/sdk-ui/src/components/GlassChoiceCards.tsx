import { Check, type LucideIcon } from 'lucide-react'
import { MOTION_CLASS } from '../motion'
import { cn } from '../utils/cn'
import { choiceFocusIndex, choiceGridCols, choiceTargetIndex, isDenseChoice } from './glass-choice-cards-helpers'
import type { GlassSelectOption } from './glass-select-helpers'

interface GlassChoiceCardsProps {
  id: string
  label: string
  /** Id do rótulo visível; sem ele, o grupo é nomeado por `aria-label`. */
  labelledBy?: string
  describedBy?: string
  options: GlassSelectOption[]
  value: string
  disabled: boolean
  onSelect: (value: string) => void
}

/** Opção de card com ícone lucide já dimensionado para a marca do card. */
export function choiceOption(label: string, value: string, description?: string, Icon?: LucideIcon): GlassSelectOption {
  return { label, value, description, icon: Icon ? <Icon size={16} strokeWidth={2.2} aria-hidden="true" /> : undefined }
}

function ChoiceMark({ option, active, dense }: { option: GlassSelectOption; active: boolean; dense: boolean }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'flex shrink-0 items-center justify-center rounded-lg border text-xs font-bold uppercase transition-colors duration-200',
        dense ? 'h-7 w-7' : 'h-9 w-9',
        active
          ? 'border-accent-rim bg-accent-soft text-accent'
          : 'border-border-subtle bg-bg-surface text-text-muted group-enabled:group-hover:border-border-strong group-enabled:group-hover:text-text-main',
      )}
    >
      {option.icon ?? option.label.trim().charAt(0)}
    </span>
  )
}

function ChoiceCard({
  option,
  active,
  focusable,
  dense,
  disabled,
  onSelect,
  onKeyDown,
}: {
  option: GlassSelectOption
  active: boolean
  focusable: boolean
  dense: boolean
  disabled: boolean
  onSelect: () => void
  onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void
}) {
  return (
    // biome-ignore lint/a11y/useSemanticElements: card com ícone+descrição não cabe num <input type="radio">; teclado de radiogroup é implementado no grupo.
    <button
      type="button"
      role="radio"
      aria-checked={active}
      tabIndex={focusable ? 0 : -1}
      disabled={disabled}
      title={option.description}
      onClick={() => !active && onSelect()}
      onKeyDown={onKeyDown}
      className={cn(
        'group relative flex items-start gap-2.5 overflow-hidden rounded-xl border text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed',
        MOTION_CLASS.interactive,
        MOTION_CLASS.press,
        dense ? 'p-2.5' : 'p-3',
        active
          ? 'border-accent-rim bg-accent-soft shadow-[0_0_18px_var(--accent-glow)]'
          : cn(
              'border-border-subtle bg-surface-raise enabled:hover:border-border-strong enabled:hover:bg-surface-raise-strong',
              MOTION_CLASS.lift,
            ),
      )}
    >
      {active && (
        <span
          aria-hidden="true"
          className="absolute inset-x-0 top-0 h-0.5 bg-accent shadow-[0_0_10px_var(--accent-glow)]"
        />
      )}
      <ChoiceMark option={option} active={active} dense={dense} />
      <span className="min-w-0 flex-1 self-center">
        <span
          className={cn(
            'block break-words text-sm font-semibold leading-tight',
            active ? 'text-accent-text' : 'text-text-main',
          )}
        >
          {option.label}
        </span>
        {option.description && (
          <span className="mt-1 block break-words text-caption leading-snug text-text-muted">{option.description}</span>
        )}
      </span>
      <span
        aria-hidden="true"
        className={cn(
          'flex h-4 w-4 shrink-0 items-center justify-center rounded-full border',
          MOTION_CLASS.interactive,
          dense ? 'self-center' : 'mt-0.5',
          active ? 'scale-100 border-accent bg-accent text-bg-deep' : 'scale-75 border-border-subtle text-transparent',
        )}
      >
        <Check size={11} strokeWidth={3} />
      </span>
    </button>
  )
}

export function GlassChoiceCards({
  id,
  label,
  labelledBy,
  describedBy,
  options,
  value,
  disabled,
  onSelect,
}: GlassChoiceCardsProps) {
  const focusIndex = choiceFocusIndex(options, value)
  const dense = isDenseChoice(options)
  return (
    <div className="@container">
      <div
        id={id}
        role="radiogroup"
        aria-label={labelledBy ? undefined : label}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        aria-disabled={disabled || undefined}
        data-sdk-control="choice-cards"
        className={cn('grid gap-2', choiceGridCols(options.length, dense))}
      >
        {options.map((option, index) => (
          <ChoiceCard
            key={option.value}
            option={option}
            active={option.value === value}
            focusable={index === focusIndex}
            dense={dense}
            disabled={disabled}
            onSelect={() => onSelect(option.value)}
            onKeyDown={(event) => {
              const nextIndex = choiceTargetIndex(event.key, index, options.length)
              if (nextIndex === null) return
              event.preventDefault()
              const next = options[nextIndex]
              if (next && next.value !== value) onSelect(next.value)
              event.currentTarget.parentElement?.querySelectorAll<HTMLElement>('[role="radio"]')[nextIndex]?.focus()
            }}
          />
        ))}
      </div>
    </div>
  )
}
