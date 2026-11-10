import { GlassCard, GlassSelect, GlassSlider, GlassToggle } from '@mika/sdk-ui/settings'
import type { ComponentProps } from 'react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import type { Translate } from '../../../renderer/src/hooks/useI18n'
import { formalityOptions, humorOptions, jokeOptions, lengthOptions, moodOptions } from './behavior-options'

type ChoiceOptions = ComponentProps<typeof GlassSelect>['options']

type Props = {
  t: Translate
  config: FeatureComponentProps['config']
  save: (path: string, value: unknown) => Promise<void>
  energyLevel: number
  slangUsage: boolean
}

export function BehaviorStyleCards({ t, config, save, energyLevel, slangUsage }: Props) {
  const select = (label: string, description: string, configPath: string, options: ChoiceOptions) => (
    <GlassCard span="full">
      <GlassSelect
        label={label}
        description={description}
        configPath={configPath}
        config={config}
        options={options}
        onSave={save}
      />
    </GlassCard>
  )
  return (
    <div className="grid gap-4">
      {select(
        t('settings.character.field.moodBaseline.label'),
        t('settings.character.behaviorPanel.moodBaselineDescription'),
        'character.moodBaseline',
        moodOptions(t),
      )}

      <div className="grid gap-4 lg:grid-cols-2">
        <GlassCard>
          <GlassSlider
            label={t('settings.character.field.energyLevel.label')}
            description={t('settings.character.behaviorPanel.energyLevelDescription')}
            configPath="character.energyLevel"
            value={energyLevel}
            min={0}
            max={100}
            step={1}
            showValue
            onSave={save}
          />
        </GlassCard>
        <GlassCard>
          <GlassToggle
            label={t('settings.character.behaviorPanel.slangUsageLabel')}
            description={t('settings.character.behaviorPanel.slangUsageDescription')}
            configPath="character.slangUsage"
            value={slangUsage}
            onSave={save}
          />
        </GlassCard>
      </div>

      {select(
        t('settings.character.field.humorStyle.label'),
        t('settings.character.behaviorPanel.humorStyleDescription'),
        'character.humorStyle',
        humorOptions(t),
      )}
      {select(
        t('settings.character.behaviorPanel.formalityLabel'),
        t('settings.character.behaviorPanel.formalityDescription'),
        'character.formalityLevel',
        formalityOptions(t),
      )}
      {select(
        t('settings.character.field.jokeFrequency.label'),
        t('settings.character.behaviorPanel.jokeFrequencyDescription'),
        'character.jokeFrequency',
        jokeOptions(t),
      )}
      {select(
        t('settings.character.field.responseLength.label'),
        t('settings.character.behaviorPanel.responseLengthDescription'),
        'character.responseLength',
        lengthOptions(t),
      )}
    </div>
  )
}
