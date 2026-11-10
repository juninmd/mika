import { MermaidDiagram } from '../../../renderer/src/components/settings/MermaidDiagram'
import {
  SettingsChecklist,
  SettingsFeatureList,
  SettingsRiskBanner,
} from '../../../renderer/src/components/settings/RichSettingBlocks'
import { labelOf } from '../../../renderer/src/components/settings/settings-utils'
import type { Translate } from '../../../renderer/src/hooks/useI18n'
import { BEHAVIOR_FLOW, moodOptions } from './behavior-options'

type Props = {
  t: Translate
  moodBaseline: string
  energyLevel: number
  empathyLevel: number
  slangUsage: boolean
  slangEnabled: boolean
  emojiUsage: boolean
  swearTolerance: string
  memoryRecall: string
}

export function BehaviorInsights({
  t,
  moodBaseline,
  energyLevel,
  empathyLevel,
  slangUsage,
  slangEnabled,
  emojiUsage,
  swearTolerance,
  memoryRecall,
}: Props) {
  const onOff = (value: boolean) =>
    value ? t('settings.character.behaviorPanel.stateOn') : t('settings.character.behaviorPanel.stateOff')

  return (
    <>
      <SettingsRiskBanner
        tone="neutral"
        title={t('settings.character.behaviorPanel.riskTitle')}
        detail={t('settings.character.behaviorPanel.riskDetail')}
      />

      <MermaidDiagram
        chart={BEHAVIOR_FLOW}
        title={t('settings.character.behaviorPanel.flowTitle')}
        description={t('settings.character.behaviorPanel.flowDescription')}
        legend={[
          {
            label: t('settings.character.behaviorPanel.flowLegendInput'),
            value: t('settings.character.behaviorPanel.flowLegendInputValue'),
          },
          {
            label: t('settings.character.behaviorPanel.flowLegendDecision'),
            value: t('settings.character.behaviorPanel.flowLegendDecisionValue'),
          },
          {
            label: t('settings.character.behaviorPanel.flowLegendOutput'),
            value: t('settings.character.behaviorPanel.flowLegendOutputValue'),
          },
        ]}
      />

      <SettingsChecklist
        title={t('settings.character.behaviorPanel.checklistTitle')}
        items={[
          {
            label: `${t('settings.character.behaviorPanel.statMood')}: ${labelOf(moodOptions(t), moodBaseline)}`,
            ok: true,
          },
          { label: `${t('settings.character.behaviorPanel.statEnergy')}: ${energyLevel}%`, ok: energyLevel > 30 },
          { label: `${t('settings.character.behaviorPanel.statEmpathy')}: ${empathyLevel}%`, ok: empathyLevel > 40 },
          {
            label: `${t('settings.character.behaviorPanel.slangUsageLabel')}: ${onOff(slangUsage)}`,
            ok: slangUsage,
          },
          {
            label: `${t('settings.character.behaviorPanel.slangEnabledLabel')}: ${onOff(slangEnabled)}`,
            ok: slangEnabled,
          },
          {
            label: `${t('settings.character.field.emojiUsage.label')}: ${onOff(emojiUsage)}`,
            ok: emojiUsage,
          },
        ]}
      />

      <SettingsFeatureList
        title={t('settings.character.behaviorPanel.activeCapabilitiesTitle')}
        items={[
          { label: t('settings.character.behaviorPanel.capabilityMoodAdaptation'), enabled: true },
          { label: t('settings.character.behaviorPanel.capabilityEnergyControl'), enabled: energyLevel > 0 },
          { label: t('settings.character.behaviorPanel.capabilityFormalitySelection'), enabled: true },
          {
            label: t('settings.character.behaviorPanel.capabilitySwearFilter'),
            enabled: swearTolerance !== 'moderate',
          },
          {
            label: t('settings.character.behaviorPanel.capabilityContextualMemory'),
            enabled: memoryRecall !== 'obsessive',
          },
        ]}
      />
    </>
  )
}
