import { ModelStation } from '../settings/skin/ModelStation'
import { PrimaryButton, SecondaryButton, StepHeader } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'

type AvatarStepProps = {
  backLabel: string
  config: Record<string, unknown>
  copy: WizardCopy['avatar']
  onBack: () => void
  onNext: () => void
  saveConfig: (path: string, value: unknown) => Promise<void>
}

// `ModelStation` é o MESMO componente de Configurações > Skin: mesma galeria (Live2D + VRM, com
// thumbnails reais), mesma tecla `character.modelPath`. Reaproveitado, não duplicado.
export function AvatarStep({ backLabel, config, copy, onBack, onNext, saveConfig }: AvatarStepProps) {
  return (
    <>
      <StepHeader step={copy.step} title={copy.title} description={copy.description} />

      <div className="mt-5">
        <ModelStation config={config} save={saveConfig} />
      </div>

      <div className="mt-5 flex flex-col gap-2.5 sm:flex-row sm:justify-between">
        <SecondaryButton onClick={onBack}>{backLabel}</SecondaryButton>
        <PrimaryButton onClick={onNext}>{copy.continue}</PrimaryButton>
      </div>
    </>
  )
}
