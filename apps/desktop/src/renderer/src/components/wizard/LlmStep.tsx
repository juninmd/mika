import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { AuroraField, BentoGrid, GlassCard, GlassSelect } from '@mika/sdk-ui/settings'
import { Cloud, Server } from 'lucide-react'
import { OllamaSetupGuide } from './OllamaSetupGuide'
import { ErrorBanner, OptionCard, PrimaryButton, SecondaryButton, StepHeader } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'
import type { LlmProvider, ProviderId } from './wizard-data'
import { LLM_PROVIDERS } from './wizard-data'

type LlmStepProps = {
  apiKey: string
  backLabel: string
  copy: WizardCopy['llm']
  error: string
  model: string
  modelCatalog: LLMModelCatalogItem[]
  modelsLoading: boolean
  onModelsChanged: () => void
  onApiKeyChange: (value: string) => void
  onBack: () => void
  onModelChange: (value: string) => void
  onNext: () => void
  onProviderChange: (value: ProviderId) => void
  onSkip: () => void
  provider: ProviderId
  saving: boolean
  selectedProvider: LlmProvider
}

/** Runs on this PC (or the user's own host); the rest are hosted APIs. */
const LOCAL_LLM = new Set<string>(['ollama', 'unsloth', 'openaiCompatible'])

export function LlmStep({
  apiKey,
  backLabel,
  copy,
  error,
  model,
  modelCatalog,
  modelsLoading,
  onModelsChanged,
  onApiKeyChange,
  onBack,
  onModelChange,
  onNext,
  onProviderChange,
  onSkip,
  provider,
  saving,
  selectedProvider,
}: LlmStepProps) {
  return (
    <>
      <StepHeader step={copy.step} title={copy.title} description={copy.description} />

      <div className="mt-4">
        <BentoGrid cols={2}>
          {LLM_PROVIDERS.map((item) => (
            <OptionCard
              key={item.id}
              selected={provider === item.id}
              title={item.name}
              description={copy.providers[item.id]}
              disabled={saving}
              icon={LOCAL_LLM.has(item.id) ? <Server size={16} /> : <Cloud size={16} />}
              onClick={() => onProviderChange(item.id)}
            />
          ))}
        </BentoGrid>
      </div>

      <div className="mt-3.5 space-y-3">
        <GlassCard span="full">
          <GlassSelect
            label={copy.modelLabel}
            description={modelsLoading ? copy.modelLoading : copy.modelHint}
            value={model}
            onChange={onModelChange}
            variant="dropdown"
            options={modelCatalog.map((option) => ({ label: option.name, value: option.id }))}
          />

          {selectedProvider.credentialConfigKey && (
            <div className="mt-3">
              <AuroraField
                label={selectedProvider.credentialLabel ?? ''}
                description={selectedProvider.credentialLink ?? ''}
                type="password"
                value={apiKey}
                onChange={onApiKeyChange}
                placeholder={selectedProvider.credentialPlaceholder}
              />
            </div>
          )}
        </GlassCard>
      </div>

      {provider === 'ollama' && (
        <OllamaSetupGuide
          copy={copy.ollama}
          model={model}
          onInstalled={(pulled) => {
            onModelsChanged()
            onModelChange(pulled)
          }}
        />
      )}

      {error && <ErrorBanner message={error} />}

      <div className="mt-3 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
          <SecondaryButton onClick={onBack} disabled={saving}>
            {backLabel}
          </SecondaryButton>
          <SecondaryButton onClick={onSkip} disabled={saving}>
            {copy.skip}
          </SecondaryButton>
        </div>
        <PrimaryButton onClick={onNext} disabled={saving}>
          {saving ? copy.saving : copy.continue}
        </PrimaryButton>
      </div>
      <p className="mt-2 text-caption leading-snug text-text-subtle">{copy.skipHint}</p>
    </>
  )
}
