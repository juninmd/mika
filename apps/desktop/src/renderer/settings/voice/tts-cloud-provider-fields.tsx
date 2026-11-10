import { getLocale } from '@mika/sdk/i18n'
import { AuroraField, GlassSelect, GlassSlider, GlassToggle } from '@mika/sdk-ui/settings'
import { fixed } from '../../src/lib/format-number'
import { ElevenLabsVoicePicker } from './eleven-labs-voice-picker'
import type { ProviderSpecificFieldsProps } from './TtsProviderSpecificFields'
import { azureVoices, elevenModels, googleVoices } from './tts-provider-constants'
import { TextLimitTimeoutSliders, TtsSettingsCard } from './tts-provider-subcomponents'

export function renderElevenLabsFields(props: ProviderSpecificFieldsProps) {
  const {
    t,
    cfg,
    onSave,
    numSave,
    strSave,
    doSave,
    elevenLabsApiKey,
    elevenLabsVoiceId,
    ttsStability,
    elevenLabsSimilarityBoost,
    elevenLabsStyle,
    elevenLabsSpeakerBoost,
    elevenLabsSpeed,
    elevenLabsTextLimit,
    elevenLabsTimeoutMs,
  } = props
  return (
    <TtsSettingsCard>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="sm:col-span-2">
          <p className="mb-2 text-[13px] font-medium text-text-main">{t('settings.voiceTts.panel.voiceLabel')}</p>
          <ElevenLabsVoicePicker
            apiKey={elevenLabsApiKey}
            selectedId={elevenLabsVoiceId}
            onSelect={(id) => doSave('voice.elevenLabsVoiceId', id)}
          />
        </div>
        <div className="sm:col-span-2">
          <GlassSelect
            label={t('settings.voiceTts.panel.modelLabel')}
            configPath="voice.elevenLabsModel"
            options={elevenModels(t)}
            config={cfg}
            onSave={strSave}
          />
        </div>
        <div className="grid grid-cols-2 gap-3 sm:col-span-2">
          <GlassSlider
            label={t('settings.voiceTts.panel.stability')}
            configPath="voice.ttsStability"
            value={ttsStability}
            min={0}
            max={1}
            step={0.05}
            showValue={(v) => fixed(v, 2)}
            onSave={numSave}
          />
          <GlassSlider
            label={t('settings.voiceTts.panel.clarity')}
            configPath="voice.elevenLabsSimilarityBoost"
            value={elevenLabsSimilarityBoost}
            min={0}
            max={1}
            step={0.05}
            showValue={(v) => fixed(v, 2)}
            onSave={numSave}
          />
          <GlassSlider
            label={t('settings.voiceTts.panel.style')}
            configPath="voice.elevenLabsStyle"
            value={elevenLabsStyle}
            min={0}
            max={1}
            step={0.05}
            showValue={(v) => fixed(v, 2)}
            onSave={numSave}
          />
          <GlassSlider
            label={t('settings.voiceTts.panel.speedLabel')}
            configPath="voice.elevenLabsSpeed"
            value={elevenLabsSpeed}
            min={0.7}
            max={1.2}
            step={0.01}
            showValue={(v) => `${fixed(v, 2)}x`}
            onSave={numSave}
          />
          <GlassToggle
            label={t('settings.voiceTts.panel.speakerBoost')}
            configPath="voice.elevenLabsSpeakerBoost"
            value={elevenLabsSpeakerBoost}
            onSave={boolSaveFrom(onSave)}
          />
        </div>
        <TextLimitTimeoutSliders
          className="sm:col-span-2"
          textLimitPath="voice.elevenLabsTextLimit"
          textLimit={elevenLabsTextLimit}
          timeoutPath="voice.elevenLabsTimeoutMs"
          timeoutMs={elevenLabsTimeoutMs}
          onSave={numSave}
        />
      </div>
    </TtsSettingsCard>
  )
}

export function renderAzureFields(props: ProviderSpecificFieldsProps) {
  const { t, cfg, numSave, strSave, azureTtsRate, azureTtsPitch, azureTtsTextLimit, azureTtsTimeoutMs } = props
  return (
    <TtsSettingsCard>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="sm:col-span-2">
          <GlassSelect
            label={t('settings.voiceTts.panel.voiceLabel')}
            configPath="voice.azureTtsVoice"
            options={azureVoices(t, getLocale(), cfg['voice.azureTtsVoice'] as string)}
            config={cfg}
            onSave={strSave}
          />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <GlassSlider
            label={t('settings.voiceTts.panel.speedLabel')}
            configPath="voice.azureTtsRate"
            value={azureTtsRate}
            min={-50}
            max={100}
            step={5}
            showValue={(v) => `${v > 0 ? '+' : ''}${Math.round(v)}%`}
            onSave={numSave}
          />
          <GlassSlider
            label={t('settings.voiceTts.panel.pitchLabel')}
            configPath="voice.azureTtsPitch"
            value={azureTtsPitch}
            min={-50}
            max={50}
            step={5}
            showValue={(v) => `${v > 0 ? '+' : ''}${Math.round(v)}%`}
            onSave={numSave}
          />
        </div>
        <TextLimitTimeoutSliders
          textLimitPath="voice.azureTtsTextLimit"
          textLimit={azureTtsTextLimit}
          timeoutPath="voice.azureTtsTimeoutMs"
          timeoutMs={azureTtsTimeoutMs}
          onSave={numSave}
        />
      </div>
    </TtsSettingsCard>
  )
}

export function renderGoogleFields(props: ProviderSpecificFieldsProps) {
  const {
    t,
    cfg,
    onSave,
    numSave,
    strSave,
    googleTtsSpeakingRate,
    googleTtsPitch,
    googleTtsTextLimit,
    googleTtsTimeoutMs,
  } = props
  return (
    <TtsSettingsCard>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="sm:col-span-2">
          <GlassSelect
            label={t('settings.voiceTts.panel.voiceLabel')}
            configPath="voice.googleTtsVoice"
            options={googleVoices(t, getLocale(), cfg['voice.googleTtsVoice'] as string)}
            config={cfg}
            onSave={strSave}
          />
        </div>
        <AuroraField
          label={t('settings.voiceTts.panel.languageCodeLabel')}
          configPath="voice.googleTtsLanguageCode"
          config={cfg}
          onSave={onSave}
          placeholder="pt-BR"
        />
        <div className="grid grid-cols-2 gap-3">
          <GlassSlider
            label={t('settings.voiceTts.panel.speedLabel')}
            configPath="voice.googleTtsSpeakingRate"
            value={googleTtsSpeakingRate}
            min={0.25}
            max={4.0}
            step={0.05}
            showValue={(v) => fixed(v, 2)}
            onSave={numSave}
          />
          <GlassSlider
            label={t('settings.voiceTts.panel.pitchLabel')}
            configPath="voice.googleTtsPitch"
            value={googleTtsPitch}
            min={-20.0}
            max={20.0}
            step={0.5}
            showValue={(v) => `${v > 0 ? '+' : ''}${fixed(v, 1)}`}
            onSave={numSave}
          />
        </div>
        <TextLimitTimeoutSliders
          className="sm:col-span-2"
          textLimitPath="voice.googleTtsTextLimit"
          textLimit={googleTtsTextLimit}
          timeoutPath="voice.googleTtsTimeoutMs"
          timeoutMs={googleTtsTimeoutMs}
          onSave={numSave}
        />
      </div>
    </TtsSettingsCard>
  )
}

function boolSaveFrom(
  onSave: (path: string, value: unknown) => Promise<void>,
): (p: string, v: boolean) => Promise<void> {
  return onSave as (p: string, v: boolean) => Promise<void>
}
