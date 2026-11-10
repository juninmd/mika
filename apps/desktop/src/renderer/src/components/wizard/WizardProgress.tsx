import { BrainIcon, CheckIcon, EyeIcon, GlobeIcon, HistoryIcon, PaletteIcon, SpeakerIcon, UserIcon } from '../Icons'
import type { WizardCopy } from './wizard-copy'

// Same order as `WIZARD_STEPS`: body, brain, voice, memory, eyes/ears.
const STEP_ICONS = [GlobeIcon, PaletteIcon, BrainIcon, SpeakerIcon, HistoryIcon, EyeIcon, UserIcon, CheckIcon] as const

const BADGE_TONE = {
  completed: 'border-ok-rim bg-ok-soft text-ok',
  active: 'border-accent-rim bg-accent-soft text-accent',
  upcoming: 'border-border-subtle bg-surface-raise text-text-muted',
} as const

function StepBadge({ index, state }: { index: number; state: keyof typeof BADGE_TONE }) {
  const Icon = STEP_ICONS[index] ?? CheckIcon
  return (
    <span
      aria-hidden="true"
      className={`relative flex size-7 shrink-0 items-center justify-center rounded-full border transition-[background-color,border-color,color] duration-300 ${BADGE_TONE[state]}`}
    >
      {state === 'active' && <span className="ping-soft absolute inset-0 rounded-full border border-accent-rim" />}
      {state === 'completed' ? <CheckIcon className="pop-in size-3.5" /> : <Icon className="size-3.5" />}
    </span>
  )
}

/** Trilho entre dois passos: enche da esquerda para a direita quando o passo anterior conclui. */
function StepConnector({ filled }: { filled: boolean }) {
  return (
    <span aria-hidden="true" className="relative h-px w-4 shrink-0 overflow-hidden bg-border-subtle sm:w-6">
      <span
        className="absolute inset-0 origin-left bg-ok transition-transform duration-500 ease-[cubic-bezier(0.22,1,0.36,1)]"
        style={{ transform: `scaleX(${filled ? 1 : 0})` }}
      />
    </span>
  )
}

export function WizardProgress({ activeStepIndex, copy }: { activeStepIndex: number; copy: WizardCopy }) {
  return (
    <ol className="flex items-center gap-2">
      {copy.steps.map((item, index) => {
        const isActive = index === activeStepIndex
        const isCompleted = index < activeStepIndex
        const state = isCompleted ? 'completed' : isActive ? 'active' : 'upcoming'

        return (
          <li
            key={item.name}
            title={item.description}
            aria-current={isActive ? 'step' : undefined}
            className="flex min-w-0 items-center gap-2"
          >
            <StepBadge index={index} state={state} />
            {/* Eight labels overflow the 720px card: only the active step keeps its name. */}
            <span className={isActive ? 'truncate text-xs font-semibold text-text-main' : 'sr-only'}>{item.name}</span>
            {index < copy.steps.length - 1 && <StepConnector filled={isCompleted} />}
          </li>
        )
      })}
    </ol>
  )
}
