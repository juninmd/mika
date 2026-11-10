import { GlassSection } from '@mika/sdk-ui/settings'
import { Brain } from 'lucide-react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { configBool, configNum, configStr } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { BehaviorInsights } from './behavior-insights'
import { BehaviorStyleCards } from './behavior-style-cards'
import { BehaviorTuningCards } from './behavior-tuning-cards'

export function BehaviorPanel({ config, onSave }: FeatureComponentProps) {
  const { t } = useI18n()
  const moodBaseline = configStr(config, 'character.moodBaseline', 'playful')
  const energyLevel = configNum(config, 'character.energyLevel', 70)
  const proactiveness = configNum(config, 'character.proactiveness', 50)
  const empathyLevel = configNum(config, 'character.empathyLevel', 70)
  const slangEnabled = configBool(config, 'character.slangEnabled', true)
  const slangUsage = configBool(config, 'character.slangUsage', true)
  const emojiUsage = configBool(config, 'character.emojiUsage', true)
  const swearTolerance = configStr(config, 'character.swearTolerance', 'none')
  const memoryRecall = configStr(config, 'character.memoryRecall', 'selective')
  const sarcasmLevel = configNum(config, 'character.sarcasmLevel', 60)
  const chaosFactor = configNum(config, 'character.chaosFactor', 20)
  const tsundereIntensity = configNum(config, 'character.tsundereIntensity', 30)
  const quietHoursStart = configNum(config, 'character.quietHoursStart', 23)
  const quietHoursEnd = configNum(config, 'character.quietHoursEnd', 7)

  const save = onSave as (path: string, value: unknown) => Promise<void>

  return (
    <GlassSection
      title={t('settings.character.group.behavior')}
      icon={Brain}
      badge={t('settings.character.behaviorPanel.badge')}
    >
      <div className="space-y-5">
        <BehaviorStyleCards t={t} config={config} save={save} energyLevel={energyLevel} slangUsage={slangUsage} />

        <BehaviorTuningCards
          t={t}
          config={config}
          save={save}
          proactiveness={proactiveness}
          empathyLevel={empathyLevel}
          sarcasmLevel={sarcasmLevel}
          chaosFactor={chaosFactor}
          tsundereIntensity={tsundereIntensity}
          slangEnabled={slangEnabled}
          emojiUsage={emojiUsage}
          quietHoursStart={quietHoursStart}
          quietHoursEnd={quietHoursEnd}
        />

        <BehaviorInsights
          t={t}
          moodBaseline={moodBaseline}
          energyLevel={energyLevel}
          empathyLevel={empathyLevel}
          slangUsage={slangUsage}
          slangEnabled={slangEnabled}
          emojiUsage={emojiUsage}
          swearTolerance={swearTolerance}
          memoryRecall={memoryRecall}
        />
      </div>
    </GlassSection>
  )
}
