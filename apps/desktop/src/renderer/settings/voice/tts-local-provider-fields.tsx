import { AuroraField, GlassSelect, GlassSlider } from '@mika/sdk-ui/settings'
import { openExternalLink } from '../../src/lib/shell-links'
import type { ProviderSpecificFieldsProps } from './TtsProviderSpecificFields'
import {
  OPENAI_TTS_VOICE_HINT,
  phononIsNonCommercial,
  phononModelPage,
  phononModels,
  phononVoices,
  piperVoices,
  supertonicLanguages,
  supertonicVoices,
} from './tts-provider-constants'
import { TtsSettingsCard } from './tts-provider-subcomponents'

export function renderPiperFields(props: ProviderSpecificFieldsProps) {
  const { t, cfg, onSave, numSave, strSave, piperRate } = props
  return (
    <TtsSettingsCard>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="sm:col-span-2">
          <GlassSelect
            label={t('settings.voiceTts.panel.voiceLabel')}
            configPath="voice.piperVoice"
            options={piperVoices(t, cfg['voice.piperVoice'] as string)}
            config={cfg}
            onSave={strSave}
          />
        </div>
        <GlassSlider
          label={t('settings.voiceTts.panel.speedLabel')}
          configPath="voice.piperRate"
          value={piperRate}
          min={0.5}
          max={2.0}
          step={0.05}
          showValue={(v) => `${v.toFixed(2)}x`}
          onSave={numSave}
        />
        <div className="sm:col-span-2">
          <AuroraField
            label={t('settings.voiceTts.panel.executablePathLabel')}
            configPath="voice.piperBinaryPath"
            config={cfg}
            onSave={onSave}
            placeholder={t('settings.voiceTts.panel.piperPathPlaceholder')}
          />
        </div>
        <p className="sm:col-span-2 text-caption leading-snug text-text-muted">
          {t('settings.voiceTts.panel.piperHint')}
        </p>
      </div>
    </TtsSettingsCard>
  )
}

export function renderSupertonicFields(props: ProviderSpecificFieldsProps) {
  const { t, cfg, numSave, strSave, supertonicRate, supertonicSteps } = props
  return (
    <TtsSettingsCard>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="sm:col-span-2">
          <GlassSelect
            label={t('settings.voiceTts.panel.voiceLabel')}
            configPath="voice.supertonicVoice"
            options={supertonicVoices(t)}
            config={cfg}
            onSave={strSave}
          />
        </div>
        <div className="sm:col-span-2">
          <GlassSelect
            label={t('settings.voiceTts.field.supertonicLanguage.label')}
            configPath="voice.supertonicLanguage"
            options={supertonicLanguages(t)}
            config={cfg}
            onSave={strSave}
          />
        </div>
        <GlassSlider
          label={t('settings.voiceTts.panel.speedLabel')}
          configPath="voice.supertonicRate"
          value={supertonicRate}
          min={0.5}
          max={2.0}
          step={0.05}
          showValue={(v) => `${v.toFixed(2)}x`}
          onSave={numSave}
        />
        <GlassSlider
          label={t('settings.voiceTts.panel.refinementSteps')}
          configPath="voice.supertonicSteps"
          value={supertonicSteps}
          min={2}
          max={32}
          step={1}
          showValue={(v) => `${v} ${t('settings.voiceTts.panel.stepsSuffix')}`}
          onSave={numSave}
        />
        <p className="sm:col-span-2 text-caption leading-snug text-text-muted">
          {t('settings.voiceTts.panel.supertonicHint')}
        </p>
      </div>
    </TtsSettingsCard>
  )
}

export function renderPhononFields(props: ProviderSpecificFieldsProps) {
  const { t, cfg, onSave, strSave, huggingFaceToken } = props
  const model = String(cfg['voice.phononModel'] ?? 'phonon-40m-pt')
  const nonCommercial = phononIsNonCommercial(model)
  return (
    <TtsSettingsCard>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="sm:col-span-2">
          <GlassSelect
            label={t('settings.voiceTts.field.phononModel.label')}
            configPath="voice.phononModel"
            options={phononModels(t)}
            config={cfg}
            onSave={strSave}
          />
        </div>
        <div className="sm:col-span-2">
          <GlassSelect
            label={t('settings.voiceTts.field.phononVoice.label')}
            configPath="voice.phononVoice"
            options={phononVoices(t)}
            config={cfg}
            onSave={strSave}
          />
        </div>
        {nonCommercial && (
          <p className="sm:col-span-2 rounded-lg border border-warn-rim bg-warn-soft px-3 py-2 text-caption leading-snug text-warn/90">
            {t('settings.voiceTts.panel.phononNonCommercial')}
          </p>
        )}
        {!nonCommercial && (
          <p className="sm:col-span-2 rounded-lg border border-ok-rim/50 bg-ok-soft px-3 py-2 text-caption leading-snug text-ok">
            {t('settings.voiceTts.panel.phononApache')}
          </p>
        )}
        {nonCommercial && (
          <div className="sm:col-span-2">
            <AuroraField
              label={t('settings.voiceTts.field.huggingFaceToken.label')}
              description={t('settings.voiceTts.field.huggingFaceToken.helperText')}
              type="password"
              configPath="apiKeys.huggingFace"
              config={{ 'apiKeys.huggingFace': huggingFaceToken }}
              onSave={onSave}
            />
          </div>
        )}
        <p className="sm:col-span-2 text-caption leading-snug text-text-muted">
          {t('settings.voiceTts.panel.phononHint')}
        </p>
        <div className="sm:col-span-2">
          <button
            type="button"
            onClick={() =>
              void openExternalLink(phononModelPage(model), t('settings.voiceTts.panel.phononOpenPageError'))
            }
            className="inline-flex items-center gap-1.5 rounded-lg border border-border-subtle px-3 py-1.5 text-caption font-bold text-text-main transition hover:bg-surface-raise focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus active:scale-95"
          >
            {t('settings.voiceTts.panel.phononModelPage')}
          </button>
        </div>
      </div>
    </TtsSettingsCard>
  )
}

export function renderOpenAiFields(props: ProviderSpecificFieldsProps) {
  const { t, cfg, onSave, numSave, openaiTtsSpeed } = props
  return (
    <TtsSettingsCard>
      <div className="grid gap-4 sm:grid-cols-2">
        <AuroraField
          label={t('settings.voiceTts.panel.voiceLabel')}
          configPath="voice.openaiTtsVoice"
          config={cfg}
          onSave={onSave}
          placeholder="af_heart"
        />
        <AuroraField
          label={t('settings.voiceTts.panel.modelLabel')}
          configPath="voice.openaiTtsModel"
          config={cfg}
          onSave={onSave}
          placeholder="kokoro"
        />
        <GlassSlider
          label={t('settings.voiceTts.panel.speedLabel')}
          configPath="voice.openaiTtsSpeed"
          value={openaiTtsSpeed}
          min={0.25}
          max={4}
          step={0.05}
          showValue={(v) => `${v.toFixed(2)}x`}
          onSave={numSave}
        />
        <p className="sm:col-span-2 text-caption leading-snug text-text-muted">
          {t('settings.voiceTts.panel.openaiHint')} {OPENAI_TTS_VOICE_HINT}
        </p>
      </div>
    </TtsSettingsCard>
  )
}

export function renderSystemFields(props: ProviderSpecificFieldsProps) {
  const { t } = props
  return (
    <p className="rounded-lg border border-warn-rim bg-warn-soft px-3 py-2 text-caption text-warn/90">
      {t('settings.voiceTts.panel.systemModePrefix')}{' '}
      <span className="font-bold">{t('settings.voiceTts.panel.systemModeLabel')}</span>{' '}
      {t('settings.voiceTts.panel.systemModeSuffix')}
    </p>
  )
}
