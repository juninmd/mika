import { BentoGrid } from '@mika/sdk-ui/settings'
import { sttProviderChoices } from '../../../settings/voice/stt-provider-choices'
import { useI18n } from '../../hooks/useI18n'
import { ErrorBanner, OptionCard, PrimaryButton, SecondaryButton, StepHeader } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'
import type { SttSelection } from './wizard-stt'

type SttStepProps = {
  copy: WizardCopy['stt']
  error: string
  onBack: () => void
  onNext: () => void
  onSttChange: (value: SttSelection) => void
  saving: boolean
  stt: SttSelection
}

// Cards vindos do MESMO leque das Configurações > Transcrição (`sttProviderChoices`): um catálogo
// só para os provedores de escuta, sem tradução duplicada no wizard. O passo escolhe QUEM escuta;
// o ligar a escuta segue no passo de privacidade (microfone).
export function SttStep({ copy, error, onBack, onNext, onSttChange, saving, stt }: SttStepProps) {
  const { t } = useI18n()
  return (
    <>
      <StepHeader step={copy.step} title={copy.title} description={copy.description} />

      <div className="mt-4">
        <BentoGrid cols={3}>
          {sttProviderChoices(t).map((choice) => (
            <OptionCard
              key={choice.id}
              selected={stt === choice.id}
              title={choice.label}
              description={choice.desc}
              onClick={() => onSttChange(choice.id as SttSelection)}
            />
          ))}
        </BentoGrid>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="mt-5 flex flex-col gap-2.5 sm:flex-row sm:justify-between">
        <SecondaryButton onClick={onBack} disabled={saving}>
          {copy.back}
        </SecondaryButton>
        <PrimaryButton onClick={onNext} disabled={saving}>
          {saving ? copy.saving : copy.complete}
        </PrimaryButton>
      </div>
    </>
  )
}
