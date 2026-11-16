import type { SupportedLocale } from '@mika/sdk/contracts'
import { LOCALE_OPTIONS } from '@mika/sdk/i18n'
import { BentoGrid } from '@mika/sdk-ui/settings'
import { Languages } from 'lucide-react'
import { ErrorBanner, OptionCard, PrimaryButton, StepHeader } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'

interface LanguageStepProps {
  copy: WizardCopy['language']
  error?: string
  locale: SupportedLocale
  onLocaleChange: (locale: SupportedLocale) => void
  onNext: () => void
}

export function LanguageStep({ copy, error, locale, onLocaleChange, onNext }: LanguageStepProps) {
  return (
    <>
      <StepHeader step={copy.step} title={copy.title} description={copy.description} />

      <div className="mt-4">
        <BentoGrid cols={2}>
          {LOCALE_OPTIONS.map((option) => (
            <OptionCard
              key={option.locale}
              selected={locale === option.locale}
              title={option.nativeName}
              description={option.englishName}
              icon={<Languages size={16} />}
              onClick={() => onLocaleChange(option.locale)}
            />
          ))}
        </BentoGrid>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="mt-5 flex justify-end">
        <PrimaryButton onClick={onNext}>{copy.continue}</PrimaryButton>
      </div>
    </>
  )
}
