import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard, GlassSelect, GlassToggle, ProviderChoiceGrid, SectionHeader } from '@mika/sdk-ui/settings'
import type { ReactNode } from 'react'
import { configBool, configStr } from '../../src/components/settings/settings-utils'
import { MicCalibrationCard } from './mic-calibration-card'
import { ShortcutsCard } from './shortcuts-card'
import { asrLanguageOptions, sttCardId, sttProviderChoices, vadEngineChoices } from './stt-provider-choices'
import { SileroVadDetails, SttProviderDetails } from './stt-provider-details'

type Option = { label: string; value: string; description?: string; icon?: ReactNode }

type SttSettingsCardsProps = {
  t: (key: keyof TranslationDict) => string
  sttEnabled: boolean
  voiceActivityThreshold: number
  vadSpeechThreshold: number
  vadEndpointingEnabled: boolean
  vadSilenceMs: number
  inputModeDraft: 'ptt' | 'continuous'
  setInputModeDraft: (value: 'ptt' | 'continuous') => void
  asrProviderDraft: string
  selectAsrProvider: (value: string) => Promise<void>
  cfg: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
  doSave: (path: string, value: unknown) => Promise<void>
  numSave: (p: string, v: number) => Promise<void>
  boolSave: (p: string, v: boolean) => Promise<void>
  INPUT_MODE_OPTIONS: Option[]
  WHISPER_MODEL_OPTIONS: Option[]
}

export function SttSettingsCards({
  t,
  sttEnabled,
  voiceActivityThreshold,
  vadSpeechThreshold,
  vadEndpointingEnabled,
  vadSilenceMs,
  inputModeDraft,
  setInputModeDraft,
  asrProviderDraft,
  selectAsrProvider,
  cfg,
  onSave,
  doSave,
  numSave,
  boolSave,
  INPUT_MODE_OPTIONS,
  WHISPER_MODEL_OPTIONS,
}: SttSettingsCardsProps) {
  const vadEngine = configStr(cfg, 'voice.vadEngine', 'energy')
  const browserStt = asrProviderDraft === 'browser'
  return (
    <>
      <GlassCard span="full">
        <div className="grid gap-4">
          <div className="grid items-end gap-4 sm:grid-cols-2">
            <GlassToggle
              label={t('settings.voiceStt.panel.sttEnabled')}
              configPath="voice.sttEnabled"
              value={sttEnabled}
              onSave={boolSave}
            />
            <GlassSelect
              label={t('settings.voiceStt.panel.asrLanguage')}
              configPath="voice.asrLanguage"
              config={cfg}
              options={asrLanguageOptions(configStr(cfg, 'voice.asrLanguage', 'pt'))}
              searchable
              onSave={onSave}
            />
          </div>
          <GlassSelect
            label={t('settings.voiceStt.panel.inputMode.label')}
            configPath="voice.inputMode"
            options={INPUT_MODE_OPTIONS}
            config={{ 'voice.inputMode': inputModeDraft }}
            onSave={async (path, val) => {
              setInputModeDraft(val as 'ptt' | 'continuous')
              await doSave(path, val)
            }}
          />
        </div>
      </GlassCard>

      <GlassCard span="full">
        <SectionHeader
          title={t('settings.voiceStt.panel.asrProvider')}
          description={t('settings.voiceStt.providerSection.description')}
          className="mb-3"
        />
        <ProviderChoiceGrid
          providers={sttProviderChoices(t)}
          activeId={sttCardId(asrProviderDraft)}
          onSelect={(p) => void selectAsrProvider(p.id)}
          renderDetails={(p) => (
            <SttProviderDetails
              t={t}
              providerId={p.id}
              cfg={cfg}
              onSave={onSave}
              selectAsrProvider={selectAsrProvider}
              whisperModelOptions={WHISPER_MODEL_OPTIONS}
            />
          )}
        />
      </GlassCard>

      <GlassCard span="full">
        <SectionHeader
          title={t('settings.voiceStt.field.vadEngine.label')}
          description={t(
            browserStt ? 'settings.voiceTts.panel.vadBrowserHint' : 'settings.voiceStt.field.vadEngine.helperText',
          )}
          className="mb-3"
        />
        <ProviderChoiceGrid
          providers={vadEngineChoices(t)}
          activeId={vadEngine}
          disabled={browserStt}
          onSelect={(p) => void doSave('voice.vadEngine', p.id)}
          renderDetails={(p) => (browserStt || p.id !== 'silero' ? null : <SileroVadDetails t={t} />)}
        />
      </GlassCard>

      <MicCalibrationCard
        t={t}
        voiceActivityThreshold={voiceActivityThreshold}
        vadSpeechThreshold={vadSpeechThreshold}
        vadEndpointingEnabled={vadEndpointingEnabled}
        vadSilenceMs={vadSilenceMs}
        vadIgnoreTyping={configBool(cfg, 'voice.vadIgnoreTyping', true)}
        browserStt={browserStt}
        numSave={numSave}
        boolSave={boolSave}
      />

      <ShortcutsCard t={t} />
    </>
  )
}
