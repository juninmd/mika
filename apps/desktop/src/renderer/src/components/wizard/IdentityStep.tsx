import { AuroraField, BentoGrid, GlassCard, GlassSection } from '@mika/sdk-ui/settings'
import { User } from 'lucide-react'
import { ErrorBanner, PrimaryButton, SecondaryButton, StepHeader } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'

type IdentityStepProps = {
  backLabel: string
  config: Record<string, unknown>
  copy: WizardCopy['identity']
  error: string
  onBack: () => void
  onNext: () => void
  saveConfig: (path: string, value: unknown) => Promise<void>
  saving: boolean
}

// Mesmas chaves de Configurações > Usuário (`user.name`/`user.nickname`) e o mesmo `AuroraField`
// que salva no blur — reaproveitado, não uma cópia do formulário.
export function IdentityStep({
  backLabel,
  config,
  copy,
  error,
  onBack,
  onNext,
  saveConfig,
  saving,
}: IdentityStepProps) {
  return (
    <>
      <StepHeader step={copy.step} title={copy.title} description={copy.description} />

      <div className="mt-5">
        <GlassSection title={copy.sectionTitle} icon={User} description={copy.sectionDescription}>
          <BentoGrid cols="auto">
            <GlassCard span="full">
              <div className="grid gap-4 sm:grid-cols-2">
                <AuroraField
                  label={copy.nameLabel}
                  configPath="user.name"
                  config={config}
                  onSave={saveConfig}
                  placeholder={copy.namePlaceholder}
                />
                <AuroraField
                  label={copy.nicknameLabel}
                  configPath="user.nickname"
                  config={config}
                  onSave={saveConfig}
                  placeholder={copy.nicknamePlaceholder}
                />
              </div>
            </GlassCard>
          </BentoGrid>
        </GlassSection>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="mt-5 flex flex-col gap-2.5 sm:flex-row sm:justify-between">
        <SecondaryButton onClick={onBack} disabled={saving}>
          {backLabel}
        </SecondaryButton>
        <PrimaryButton onClick={onNext} disabled={saving}>
          {saving ? copy.saving : copy.continue}
        </PrimaryButton>
      </div>
    </>
  )
}
