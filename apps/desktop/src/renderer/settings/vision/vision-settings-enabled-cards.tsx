import type { TranslationDict } from '@mika/sdk/i18n'
import {
  AuroraField,
  GlassCard,
  GlassSelect,
  GlassSlider,
  GlassToggle,
  ProviderChoiceGrid,
  SectionHeader,
} from '@mika/sdk-ui/settings'
import { ModelPickerField } from '../../src/components/settings/ModelPickerField'
import { VisionMissionPanel } from './vision-mission-panel'
import type { VisionPrivacy } from './vision-readiness'
import {
  getAnalysisModes,
  getProviderChoices,
  SliderCard,
  type SliderControl,
  ToggleCard,
  type ToggleControl,
} from './vision-settings-components'

type VisionSettingsEnabledCardsProps = {
  privacy: VisionPrivacy
  t: (key: keyof TranslationDict) => string
  provider: string
  model: string
  analysisMode: string
  captureIntervalMs: number
  proactiveCommentaryChance: number
  codeAwareness: boolean
  gameAwareness: boolean
  readClipboardImages: boolean
  visualDiaryEnabled: boolean
  visionAssistActive: boolean
  visionAssistAutoComment: boolean
  adaptiveCompression: boolean
  visualDiaryIntervalCycles: number
  textDensityThreshold: number
  chatProvider: string
  fieldConfig: Record<string, string>
  captureSliders: SliderControl[]
  behaviorToggles: ToggleControl[]
  awarenessToggles: ToggleControl[]
  onSave: (path: string, value: unknown) => Promise<void>
  boolSave: (p: string, v: boolean) => Promise<void>
  numSave: (p: string, v: number) => Promise<void>
  stringSave: (p: string, v: string) => Promise<void>
}

export function VisionSettingsEnabledCards({
  privacy,
  t,
  provider,
  model,
  analysisMode,
  captureIntervalMs,
  proactiveCommentaryChance,
  codeAwareness,
  gameAwareness,
  readClipboardImages,
  visualDiaryEnabled,
  visionAssistActive,
  visionAssistAutoComment,
  adaptiveCompression,
  visualDiaryIntervalCycles,
  textDensityThreshold,
  chatProvider,
  fieldConfig,
  captureSliders,
  behaviorToggles,
  awarenessToggles,
  onSave,
  boolSave,
  numSave,
  stringSave,
}: VisionSettingsEnabledCardsProps) {
  return (
    <>
      <VisionMissionPanel
        provider={provider}
        analysisMode={analysisMode}
        captureIntervalMs={captureIntervalMs}
        proactiveCommentaryChance={proactiveCommentaryChance}
        codeAwareness={codeAwareness}
        gameAwareness={gameAwareness}
        readClipboardImages={readClipboardImages}
        visualDiaryEnabled={visualDiaryEnabled}
        visionAssistAutoComment={visionAssistAutoComment}
        privacy={privacy}
      />

      <GlassCard span="full">
        <SectionHeader
          title={t('settings.vision.field.provider.label')}
          description={t('settings.vision.providerSection.description')}
          className="mb-3"
        />
        <ProviderChoiceGrid
          providers={getProviderChoices(t)}
          activeId={provider}
          onSelect={(p) => void stringSave('vision.provider', p.id)}
          renderDetails={(p) => (
            <ModelPickerField
              label={t('settings.vision.field.model.label')}
              description={t('settings.vision.panel.modelPickerDescription')}
              provider={p.id === 'default' ? chatProvider : p.id}
              value={model}
              configPath="vision.model"
              onSave={onSave}
              capability="vision"
            />
          )}
        />
      </GlassCard>

      {captureSliders.slice(0, 2).map((control) => (
        <SliderCard key={control.configPath} control={control} onSave={numSave} />
      ))}

      <ToggleCard
        control={{
          label: t('settings.vision.panel.adaptiveCompressionLabel'),
          description: t('settings.vision.panel.adaptiveCompressionDescription'),
          configPath: 'vision.adaptiveCompression',
          value: adaptiveCompression,
        }}
        onSave={boolSave}
      />

      {captureSliders.slice(2, 6).map((control) => (
        <SliderCard key={control.configPath} control={control} onSave={numSave} />
      ))}

      <SliderCard control={captureSliders[6]} onSave={numSave} />

      <GlassCard span="full">
        <GlassSelect
          label={t('settings.vision.field.analysisMode.label')}
          configPath="vision.analysisMode"
          options={getAnalysisModes(t)}
          config={fieldConfig}
          onSave={stringSave}
        />
      </GlassCard>

      <GlassCard span="full">
        <AuroraField
          label={t('settings.vision.panel.blocklistLabel')}
          description={t('settings.vision.field.captureBlocklist.description')}
          configPath="vision.captureBlocklist"
          config={fieldConfig}
          onSave={onSave}
          placeholder={t('settings.vision.panel.blocklistPlaceholder')}
        />
      </GlassCard>

      <GlassCard span="full">
        <div className="space-y-3">
          <SectionHeader title={t('settings.vision.panel.behaviorHeader')} />
          {behaviorToggles.map((control) => (
            <GlassToggle key={control.configPath} {...control} onSave={boolSave} />
          ))}
          {visionAssistActive && (
            <div className="pl-4 border-l border-border-subtle">
              <GlassToggle
                label={t('settings.vision.panel.autoCommentLabel')}
                configPath="vision.visionAssistAutoComment"
                value={visionAssistAutoComment}
                onSave={boolSave}
              />
            </div>
          )}
        </div>
      </GlassCard>

      <GlassCard span="full">
        <div className="space-y-3">
          <SectionHeader title={t('settings.vision.panel.awarenessHeader')} />
          {awarenessToggles.map((control) => (
            <GlassToggle key={control.configPath} {...control} onSave={boolSave} />
          ))}
          {codeAwareness && (
            <div className="pl-4 border-l border-border-subtle">
              <GlassSlider
                label={t('settings.vision.field.textDensityThreshold.label')}
                description={t('settings.vision.field.textDensityThreshold.description')}
                configPath="vision.textDensityThreshold"
                value={textDensityThreshold}
                min={0}
                max={1}
                step={0.01}
                showValue={(v: number) => `${Math.round(v * 100)}%`}
                onSave={numSave}
              />
            </div>
          )}
          {visualDiaryEnabled && (
            <div className="pl-4 border-l border-border-subtle">
              <GlassSlider
                label={t('settings.vision.panel.diaryIntervalLabel')}
                configPath="vision.visualDiaryIntervalCycles"
                value={visualDiaryIntervalCycles}
                min={1}
                max={100}
                showValue={(v: number) => `${v} ${t('settings.vision.panel.cyclesSuffix')}`}
                onSave={numSave}
              />
            </div>
          )}
        </div>
      </GlassCard>
    </>
  )
}
