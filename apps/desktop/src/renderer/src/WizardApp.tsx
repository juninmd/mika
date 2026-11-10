import { MOTION, StatusPill } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { useEffect, useRef } from 'react'
import { AvatarStep } from './components/wizard/AvatarStep'
import { DoneStep } from './components/wizard/DoneStep'
import { EmbeddingStep } from './components/wizard/EmbeddingStep'
import { IdentityStep } from './components/wizard/IdentityStep'
import { LanguageStep } from './components/wizard/LanguageStep'
import { LlmStep } from './components/wizard/LlmStep'
import { MikaBubble } from './components/wizard/MikaBubble'
import { PrivacyStep } from './components/wizard/PrivacyStep'
import { SttStep } from './components/wizard/SttStep'
import { TtsStep } from './components/wizard/TtsStep'
import { WizardBackdrop } from './components/wizard/WizardBackdrop'
import { WizardProgress } from './components/wizard/WizardProgress'
import type { Step } from './components/wizard/wizard-data'
import { useTheme } from './hooks/useTheme'
import { useWizardState } from './use-wizard-state'

export function WizardApp({ onComplete }: { onComplete: () => void }) {
  useTheme()
  const {
    locale,
    copy,
    step,
    error,
    privacyChoices,
    legalAccepted,
    retryLegal,
    ageConfirmed,
    saving,
    provider,
    model,
    apiKey,
    selectedProvider,
    modelCatalog,
    modelsLoading,
    reloadModels,
    tts,
    stt,
    memoryEnabled,
    reaction,
    llmConfigured,
    activeStepIndex,
    config,
    saveConfig,
    setError,
    setStep,
    setModel,
    setApiKey,
    setTts,
    setStt,
    setMemoryEnabled,
    handleLocaleChange,
    handleFinishLanguage,
    handleTogglePrivacy,
    setLegalAccepted,
    setAgeConfirmed,
    handleFinishPrivacy,
    handleProviderChange,
    handleFinishLlm,
    handleSkipLlm,
    handleFinishTts,
    handleFinishStt,
    handleFinishEmbedding,
    handleFinishAvatar,
    handleFinishIdentity,
  } = useWizardState()
  // A step swap remounts the section: without moving focus into it, keyboard/screen-reader users stay on a
  // button that no longer exists. Skipped on first paint so the wizard doesn't steal focus from its own autofocus.
  const stepRef = useRef<HTMLElement>(null)
  const firstStepRef = useRef(true)
  // biome-ignore lint/correctness/useExhaustiveDependencies: `step` is the trigger, not a value read inside.
  useEffect(() => {
    if (firstStepRef.current) {
      firstStepRef.current = false
      return
    }
    stepRef.current?.focus()
  }, [step])
  const backTo = (to: Step) => () => {
    setError('')
    setStep(to)
  }

  return (
    <div className="fixed inset-0 overflow-auto">
      <WizardBackdrop />

      <div className="relative flex min-h-full justify-center p-2 sm:p-3">
        <div className="my-auto flex w-full max-w-[720px] flex-col gap-2">
          <div className="flex flex-col gap-2 rounded-[22px] border border-border-subtle bg-bg-surface/80 p-3 shadow-xl backdrop-blur">
            <div className="flex items-center gap-2">
              <StatusPill
                variant={llmConfigured ? 'ok' : 'pending'}
                label={copy.badge}
                dot
                className="w-fit uppercase tracking-[0.14em]"
              />
            </div>
            <WizardProgress activeStepIndex={activeStepIndex} copy={copy} />
            {step !== 'language' && (
              <MikaBubble copy={copy.mika} llmConfigured={llmConfigured} reaction={reaction} step={step} />
            )}
          </div>

          {/* Keyed by step: each step enters with the shared page motion (no exit — Next never waits). */}
          <motion.section
            key={step}
            ref={stepRef}
            tabIndex={-1}
            aria-live="polite"
            {...MOTION.page}
            className="rounded-[22px] border border-border-subtle bg-surface-raise p-3.5 shadow-xl outline-none sm:p-4"
          >
            {step === 'language' && (
              <LanguageStep
                copy={copy.language}
                error={error}
                locale={locale}
                onLocaleChange={handleLocaleChange}
                onNext={handleFinishLanguage}
              />
            )}

            {step === 'privacy' && (
              <PrivacyStep
                choices={privacyChoices}
                copy={copy.privacy}
                error={error}
                backLabel={copy.back}
                saving={saving}
                legalAccepted={legalAccepted}
                onLegalChange={setLegalAccepted}
                ageConfirmed={ageConfirmed}
                onAgeConfirmedChange={setAgeConfirmed}
                onLegalRetry={retryLegal}
                onToggle={handleTogglePrivacy}
                onBack={backTo('embedding')}
                onNext={handleFinishPrivacy}
              />
            )}

            {step === 'llm' && (
              <LlmStep
                copy={copy.llm}
                backLabel={copy.back}
                provider={provider}
                model={model}
                apiKey={apiKey}
                saving={saving}
                error={error}
                selectedProvider={selectedProvider}
                modelCatalog={modelCatalog}
                modelsLoading={modelsLoading}
                onModelsChanged={reloadModels}
                onProviderChange={handleProviderChange}
                onModelChange={setModel}
                onApiKeyChange={setApiKey}
                onBack={backTo('avatar')}
                onNext={handleFinishLlm}
                onSkip={handleSkipLlm}
              />
            )}

            {step === 'tts' && (
              <TtsStep
                copy={copy.tts}
                tts={tts}
                saving={saving}
                error={error}
                onTtsChange={setTts}
                onNext={handleFinishTts}
                onBack={backTo('llm')}
              />
            )}

            {step === 'stt' && (
              <SttStep
                copy={copy.stt}
                stt={stt}
                saving={saving}
                error={error}
                onSttChange={setStt}
                onNext={handleFinishStt}
                onBack={backTo('tts')}
              />
            )}

            {step === 'embedding' && (
              <EmbeddingStep
                copy={copy.embedding}
                backLabel={copy.back}
                config={config}
                saveConfig={saveConfig}
                error={error}
                saving={saving}
                memoryEnabled={memoryEnabled}
                onMemoryChange={setMemoryEnabled}
                onBack={backTo('stt')}
                onNext={handleFinishEmbedding}
              />
            )}

            {step === 'avatar' && (
              <AvatarStep
                copy={copy.avatar}
                backLabel={copy.back}
                config={config}
                saveConfig={saveConfig}
                onBack={backTo('language')}
                onNext={handleFinishAvatar}
              />
            )}

            {step === 'identity' && (
              <IdentityStep
                copy={copy.identity}
                backLabel={copy.back}
                config={config}
                saveConfig={saveConfig}
                saving={saving}
                error={error}
                onBack={backTo('privacy')}
                onNext={handleFinishIdentity}
              />
            )}

            {step === 'done' && (
              <DoneStep
                copy={copy.done}
                assetsCopy={copy.assets}
                llmConfigured={llmConfigured}
                llmProvider={provider}
                llmModel={model}
                tts={tts}
                listening={privacyChoices.microphone}
                stt={stt}
                onBack={backTo('identity')}
                onComplete={onComplete}
              />
            )}
          </motion.section>
        </div>
      </div>
    </div>
  )
}
