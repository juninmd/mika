import { BrainIcon, CheckIcon, EyeIcon, GlobeIcon, HistoryIcon, PaletteIcon, SpeakerIcon, UserIcon } from '../Icons'
import type { WizardCopy } from './wizard-copy'

// Same order as `WIZARD_STEPS`: body, brain, voice, memory, eyes/ears.
const STEP_ICONS = [GlobeIcon, PaletteIcon, BrainIcon, SpeakerIcon, HistoryIcon, EyeIcon, UserIcon, CheckIcon] as const

const BAR_TONE = {
  completed: 'bg-ok/70',
  active: 'bg-accent shadow-[0_0_10px_var(--accent-glow)]',
  upcoming: 'bg-border-subtle',
} as const

const LABEL_TONE = {
  completed: 'text-ok',
  active: 'text-text-main',
  upcoming: 'text-text-muted',
} as const

type StepState = keyof typeof BAR_TONE

/** One segment of the progress bar: the bar fills as the step completes, the label names the step. */
function StepSegment({ index, state, name }: { index: number; state: StepState; name: string }) {
  const Icon = state === 'completed' ? CheckIcon : (STEP_ICONS[index] ?? CheckIcon)
  return (
    <>
      <span aria-hidden="true" className="relative block h-1 overflow-hidden rounded-full bg-border-subtle">
        <span
          className={`absolute inset-0 origin-left rounded-full transition-transform duration-500 ease-[cubic-bezier(0.22,1,0.36,1)] ${BAR_TONE[state]}`}
          style={{ transform: `scaleX(${state === 'upcoming' ? 0 : 1})` }}
        />
      </span>
      <span
        className={`mt-1.5 flex min-w-0 items-center gap-1 text-micro font-semibold uppercase tracking-[0.08em] transition-colors duration-300 ${LABEL_TONE[state]}`}
      >
        <Icon aria-hidden="true" className={`size-3 shrink-0 ${state === 'completed' ? 'pop-in' : ''}`} />
        {/* Narrow cards keep only the active name; wider ones show every step. */}
        <span className={`truncate ${state === 'active' ? '' : 'max-sm:sr-only'}`}>{name}</span>
      </span>
    </>
  )
}

export function WizardProgress({ activeStepIndex, copy }: { activeStepIndex: number; copy: WizardCopy }) {
  return (
    <ol className="grid w-full gap-1.5" style={{ gridTemplateColumns: `repeat(${copy.steps.length}, minmax(0, 1fr))` }}>
      {copy.steps.map((item, index) => {
        const isActive = index === activeStepIndex
        const state: StepState = index < activeStepIndex ? 'completed' : isActive ? 'active' : 'upcoming'
        return (
          <li key={item.name} title={item.description} aria-current={isActive ? 'step' : undefined} className="min-w-0">
            <StepSegment index={index} state={state} name={item.name} />
          </li>
        )
      })}
    </ol>
  )
}
