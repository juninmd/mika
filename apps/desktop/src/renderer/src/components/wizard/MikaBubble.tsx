import { SparkIcon } from '../Icons'
import type { MikaReaction } from './use-mika-reaction'
import type { WizardCopy } from './wizard-copy'
import type { Step } from './wizard-data'

type MikaBubbleProps = {
  copy: WizardCopy['mika']
  llmConfigured: boolean
  reaction: MikaReaction
  step: Step
}

// Her own words only ever come from the LLM; the asleep/thinking lines are UI copy about her state.
function bubbleText({ copy, llmConfigured, reaction }: MikaBubbleProps): { text: string; spoken: boolean } {
  if (reaction.pending) return { text: copy.thinking, spoken: false }
  if (reaction.text) return { text: reaction.text, spoken: true }
  return { text: llmConfigured ? copy.awake : copy.asleep, spoken: false }
}

export function MikaBubble(props: MikaBubbleProps) {
  const { text, spoken } = bubbleText(props)
  const hint = props.step === 'language' ? undefined : props.copy.stepHints[props.step]
  return (
    <div className="flex items-start gap-2.5">
      <span
        aria-hidden="true"
        className={`flex size-9 shrink-0 items-center justify-center rounded-full border ${
          props.llmConfigured
            ? 'border-accent-rim bg-accent-soft text-accent'
            : 'border-border-subtle bg-surface-raise text-text-muted'
        } ${props.reaction.pending ? 'animate-pulse' : ''}`}
      >
        <SparkIcon className="size-4" />
      </span>
      <div
        role="status"
        aria-live="polite"
        aria-label={props.copy.label}
        className="min-w-0 flex-1 rounded-2xl rounded-tl-sm border border-border-subtle bg-bg-deep px-3 py-2"
      >
        <div className="text-micro font-extrabold uppercase tracking-[0.12em] text-primary">{props.copy.label}</div>
        <p className={`mt-0.5 text-xs leading-5 ${spoken ? 'text-text-main' : 'italic text-text-muted'}`}>{text}</p>
        {/* What this step is for, as narration about the UI — not her voice (her lines come only from the LLM). */}
        {hint && (
          <p className="mt-1 border-t border-border-subtle pt-1 text-caption leading-4 text-text-muted">{hint}</p>
        )}
      </div>
    </div>
  )
}
