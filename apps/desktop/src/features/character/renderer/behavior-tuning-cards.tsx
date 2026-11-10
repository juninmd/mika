import { GlassCard, GlassSelect, GlassSlider, GlassToggle } from '@mika/sdk-ui/settings'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import type { Translate } from '../../../renderer/src/hooks/useI18n'
import { memoryOptions, swearOptions } from './behavior-options'

type Props = {
  t: Translate
  config: FeatureComponentProps['config']
  save: (path: string, value: unknown) => Promise<void>
  proactiveness: number
  empathyLevel: number
  sarcasmLevel: number
  chaosFactor: number
  tsundereIntensity: number
  slangEnabled: boolean
  emojiUsage: boolean
  quietHoursStart: number
  quietHoursEnd: number
}

const clockHour = (hour: number) => `${String(hour).padStart(2, '0')}:00`

export function BehaviorTuningCards({
  t,
  config,
  save,
  proactiveness,
  empathyLevel,
  sarcasmLevel,
  chaosFactor,
  tsundereIntensity,
  slangEnabled,
  emojiUsage,
  quietHoursStart,
  quietHoursEnd,
}: Props) {
  return (
    <>
      <div className="grid gap-4 lg:grid-cols-2">
        <GlassCard>
          <GlassSlider
            label={t('settings.character.field.proactiveness.label')}
            description={t('settings.character.behaviorPanel.proactivenessDescription')}
            configPath="character.proactiveness"
            value={proactiveness}
            min={0}
            max={100}
            step={1}
            showValue
            onSave={save}
          />
        </GlassCard>

        <GlassCard>
          <GlassSlider
            label={t('settings.character.field.empathyLevel.label')}
            description={t('settings.character.behaviorPanel.empathyLevelDescription')}
            configPath="character.empathyLevel"
            value={empathyLevel}
            min={0}
            max={100}
            step={1}
            showValue
            onSave={save}
          />
        </GlassCard>
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <GlassCard>
          <GlassSlider
            label={t('settings.character.field.sarcasmLevel.label')}
            description={t('settings.character.behaviorPanel.sarcasmLevelDescription')}
            configPath="character.sarcasmLevel"
            value={sarcasmLevel}
            min={0}
            max={100}
            step={1}
            showValue
            onSave={save}
          />
        </GlassCard>

        <GlassCard>
          <GlassSlider
            label={t('settings.character.field.chaosFactor.label')}
            description={t('settings.character.behaviorPanel.chaosFactorDescription')}
            configPath="character.chaosFactor"
            value={chaosFactor}
            min={0}
            max={100}
            step={1}
            showValue
            onSave={save}
          />
        </GlassCard>

        <GlassCard>
          <GlassSlider
            label={t('settings.character.field.tsundereIntensity.label')}
            description={t('settings.character.behaviorPanel.tsundereIntensityDescription')}
            configPath="character.tsundereIntensity"
            value={tsundereIntensity}
            min={0}
            max={100}
            step={1}
            showValue
            onSave={save}
          />
        </GlassCard>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <GlassToggle
          label={t('settings.character.behaviorPanel.slangEnabledLabel')}
          description={t('settings.character.behaviorPanel.slangEnabledDescription')}
          configPath="character.slangEnabled"
          value={slangEnabled}
          onSave={save}
        />

        <GlassToggle
          label={t('settings.character.field.emojiUsage.label')}
          description={t('settings.character.behaviorPanel.emojiUsageDescription')}
          configPath="character.emojiUsage"
          value={emojiUsage}
          onSave={save}
        />
      </div>

      <div className="grid gap-4">
        <GlassCard>
          <GlassSelect
            label={t('settings.character.field.swearTolerance.label')}
            description={t('settings.character.behaviorPanel.swearToleranceDescription')}
            configPath="character.swearTolerance"
            config={config}
            options={swearOptions(t)}
            onSave={save}
          />
        </GlassCard>

        <GlassCard>
          <GlassSelect
            label={t('settings.character.behaviorPanel.memoryRecallLabel')}
            description={t('settings.character.behaviorPanel.memoryRecallDescription')}
            configPath="character.memoryRecall"
            config={config}
            options={memoryOptions(t)}
            onSave={save}
          />
        </GlassCard>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <GlassCard>
          <GlassSlider
            label={t('settings.character.field.quietHoursStart.label')}
            description={t('settings.character.field.quietHoursStart.description')}
            configPath="character.quietHoursStart"
            value={quietHoursStart}
            min={0}
            max={23}
            step={1}
            showValue={clockHour}
            onSave={save}
          />
        </GlassCard>

        <GlassCard>
          <GlassSlider
            label={t('settings.character.field.quietHoursEnd.label')}
            description={t('settings.character.field.quietHoursEnd.description')}
            configPath="character.quietHoursEnd"
            value={quietHoursEnd}
            min={0}
            max={23}
            step={1}
            showValue={clockHour}
            onSave={save}
          />
        </GlassCard>
      </div>
    </>
  )
}
