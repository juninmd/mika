import { BentoGrid, GlassCard, GlassToggle } from '@mika/sdk-ui/settings'
import { LegalConsent } from './LegalConsent'
import { ErrorBanner, PrimaryButton, SecondaryButton, StepHeader } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'

export type PrivacyChoices = {
  microphone: boolean
  screen: boolean
}

type PrivacyStepProps = {
  choices: PrivacyChoices
  copy: WizardCopy['privacy']
  error: string
  backLabel: string
  onBack: () => void
  onNext: () => void
  onToggle: (choice: keyof PrivacyChoices) => void
  saving: boolean
  legalAccepted: boolean
  onLegalChange: (accepted: boolean) => void
  ageConfirmed: boolean
  onAgeConfirmedChange: (confirmed: boolean) => void
  onLegalRetry: () => void
}

function ConsentSwitch({
  checked,
  description,
  disabled,
  label,
  onClick,
}: {
  checked: boolean
  description: string
  disabled?: boolean
  label: string
  onClick: () => void
}) {
  return (
    <GlassCard span={1} className={checked ? 'border-accent-rim bg-accent-soft' : ''}>
      <GlassToggle label={label} description={description} value={checked} disabled={disabled} onChange={onClick} />
    </GlassCard>
  )
}

export function PrivacyStep({
  choices,
  copy,
  error,
  backLabel,
  onBack,
  onNext,
  onToggle,
  saving,
  legalAccepted,
  onLegalChange,
  ageConfirmed,
  onAgeConfirmedChange,
  onLegalRetry,
}: PrivacyStepProps) {
  return (
    <>
      <StepHeader step={copy.step} title={copy.title} description={copy.description} />

      <div className="mt-4">
        <BentoGrid cols={2}>
          <ConsentSwitch
            checked={choices.screen}
            label={copy.screenTitle}
            description={copy.screenDescription}
            disabled={saving}
            onClick={() => onToggle('screen')}
          />
          <ConsentSwitch
            checked={choices.microphone}
            label={copy.microphoneTitle}
            description={copy.microphoneDescription}
            disabled={saving}
            onClick={() => onToggle('microphone')}
          />
        </BentoGrid>
      </div>

      <div className="mt-3 rounded-xl border border-warn-rim bg-warn-soft p-3">
        <div className="text-xs font-bold text-warn">{copy.privateTitle}</div>
        <p className="mt-0.5 text-xs leading-5 text-text-muted">{copy.privateDescription}</p>
      </div>

      <div className="mt-3">
        <LegalConsent
          accepted={legalAccepted}
          onAcceptedChange={onLegalChange}
          ageConfirmed={ageConfirmed}
          onAgeConfirmedChange={onAgeConfirmedChange}
          disabled={saving}
          onRetry={onLegalRetry}
        />
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="mt-5 flex flex-col gap-2.5 sm:flex-row sm:justify-between">
        <SecondaryButton onClick={onBack} disabled={saving}>
          {backLabel}
        </SecondaryButton>
        <PrimaryButton onClick={onNext} disabled={saving || !legalAccepted || !ageConfirmed}>
          {saving ? copy.saving : copy.continue}
        </PrimaryButton>
      </div>
    </>
  )
}
