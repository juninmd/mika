import { GlassToggle } from '@mika/sdk-ui/settings'
import { EmbeddingProviderPanel } from '../../../settings/llm/settings'
import { ErrorBanner, PrimaryButton, SecondaryButton, StepHeader } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'

type EmbeddingStepProps = {
  backLabel: string
  config: Record<string, unknown>
  copy: WizardCopy['embedding']
  error: string
  memoryEnabled: boolean
  onBack: () => void
  onMemoryChange: (enabled: boolean) => void
  onNext: () => void
  saveConfig: (path: string, value: unknown) => Promise<void>
  saving: boolean
}

// The provider panel is the same one as Settings > LLM (autosaves per field); only the memory
// switch is batched, because it is a privacy decision that "Continue" commits.
export function EmbeddingStep(props: EmbeddingStepProps) {
  const { copy } = props
  return (
    <>
      <StepHeader step={copy.step} title={copy.title} description={copy.description} />

      <div className="mt-5 rounded-2xl border border-border-subtle bg-surface-raise p-3.5">
        <GlassToggle
          label={copy.memoryTitle}
          description={copy.memoryDescription}
          value={props.memoryEnabled}
          disabled={props.saving}
          onChange={() => props.onMemoryChange(!props.memoryEnabled)}
        />
      </div>

      <div className="mt-3">
        <EmbeddingProviderPanel config={props.config} onSave={props.saveConfig} />
      </div>

      {props.error && <ErrorBanner message={props.error} />}

      <div className="mt-5 flex flex-col gap-2.5 sm:flex-row sm:justify-between">
        <SecondaryButton onClick={props.onBack} disabled={props.saving}>
          {props.backLabel}
        </SecondaryButton>
        <PrimaryButton onClick={props.onNext} disabled={props.saving}>
          {props.saving ? copy.saving : copy.continue}
        </PrimaryButton>
      </div>
    </>
  )
}
