import { BentoGrid } from '@mika/sdk-ui/settings'
import { audioMimeFromBase64 } from '../../../settings/voice/tts-provider-constants'
import { useTtsAudioTest } from '../../../settings/voice/tts-provider-subcomponents'
import { runtimeApi } from '../../wizard-app-saves'
import { SpeakerIcon, StopIcon } from '../Icons'
import { ErrorBanner, OptionCard, PrimaryButton, SecondaryButton, StepHeader } from './WizardPrimitives'
import type { WizardCopy } from './wizard-copy'
import type { TtsId } from './wizard-data'
import { TTS_OPTION_IDS, type TtsSelection } from './wizard-tts'

type TtsStepProps = {
  copy: WizardCopy['tts']
  error: string
  onBack: () => void
  onNext: () => void
  onTtsChange: (value: TtsId) => void
  saving: boolean
  tts: TtsSelection
}

// Same path as Settings > Voz "Testar voz" (`mika.voice` plugin test → the provider's real synthesis
// of the fixed test phrase), with the choice not yet saved passed as a transient override.
function VoicePreview({ copy, provider }: { copy: WizardCopy['tts']; provider: Exclude<TtsId, 'none'> }) {
  const { status, result, isPlaying, handleTestClick } = useTtsAudioTest({
    testFn: () =>
      runtimeApi().testPlugin({
        pluginId: 'mika.voice',
        config: { 'voice.ttsProvider': provider, ttsProvider: provider },
      }),
    runningMessage: copy.previewLoading,
    audioMimeFromBase64,
  })
  const loading = status === 'testing'
  return (
    <div className="mt-3 flex flex-wrap items-center gap-2.5 rounded-xl border border-border-subtle bg-bg-deep px-3 py-2">
      <button
        type="button"
        onClick={() => void handleTestClick()}
        disabled={loading}
        aria-pressed={isPlaying}
        className="inline-flex items-center gap-1.5 rounded-lg border border-accent-rim bg-accent-soft px-3 py-1.5 text-xs font-bold text-text-main transition hover:bg-accent/25 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-60"
      >
        {isPlaying ? (
          <StopIcon className="size-3.5" aria-hidden="true" />
        ) : (
          <SpeakerIcon className="size-3.5" aria-hidden="true" />
        )}
        {isPlaying ? copy.previewStop : loading ? copy.previewLoading : copy.preview}
      </button>
      <p
        role="status"
        className={`min-w-0 flex-1 text-caption leading-4 ${status === 'error' ? 'text-danger' : 'text-text-muted'}`}
      >
        {status === 'error' && result ? result : copy.previewHint}
      </p>
    </div>
  )
}

export function TtsStep({ copy, error, onBack, onNext, onTtsChange, saving, tts }: TtsStepProps) {
  return (
    <>
      <StepHeader step={copy.step} title={copy.title} description={copy.description} />

      <div className="mt-4">
        <BentoGrid cols={3}>
          {TTS_OPTION_IDS.map((id) => (
            <OptionCard
              key={id}
              selected={tts === id}
              title={copy.options[id].name}
              description={copy.options[id].description}
              onClick={() => onTtsChange(id)}
            />
          ))}
        </BentoGrid>
      </div>

      {tts && tts !== 'none' && <VoicePreview key={tts} copy={copy} provider={tts} />}

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
