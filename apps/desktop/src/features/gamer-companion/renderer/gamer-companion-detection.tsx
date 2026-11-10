import { GlassCard, GlassSlider, GlassToggle, SectionDivider, SectionHeader } from '@mika/sdk-ui/settings'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import type { Translate } from '../../../renderer/src/hooks/useI18n'

export function GamerCompanionDetection({
  t,
  watcherEnabled,
  watcherIntervalSeconds,
  backseatEnabled,
  backseatAggressiveness,
  onSave,
}: {
  t: Translate
  watcherEnabled: boolean
  watcherIntervalSeconds: number
  backseatEnabled: boolean
  backseatAggressiveness: number
  onSave: FeatureComponentProps['onSave']
}) {
  return (
    <GlassCard>
      <SectionHeader title={t('settings.gamerCompanion.panel.sectionDetection')} className="mb-3" />
      <div className="space-y-4">
        <GlassToggle
          label={t('settings.gamerCompanion.panel.autoDetectLabel')}
          description={t('settings.gamerCompanion.panel.autoDetectDescription')}
          configPath="gamer-companion.gameWatcherEnabled"
          value={watcherEnabled}
          onSave={onSave}
        />
        <GlassSlider
          label={t('settings.gamerCompanion.panel.intervalLabel')}
          configPath="gamer-companion.gameWatcherIntervalSeconds"
          value={watcherIntervalSeconds}
          min={10}
          max={300}
          step={5}
          showValue={(v) => `${v}s`}
          onSave={onSave}
        />
        <SectionDivider />
        <GlassToggle
          label={t('settings.gamerCompanion.panel.backseatLabel')}
          description={t('settings.gamerCompanion.panel.backseatDescription')}
          configPath="gamer-companion.backseatGamingEnabled"
          value={backseatEnabled}
          onSave={onSave}
        />
        {backseatEnabled && (
          <GlassSlider
            label={t('settings.gamerCompanion.panel.aggressivenessLabel')}
            configPath="gamer-companion.backseatAggressiveness"
            value={backseatAggressiveness}
            min={0}
            max={100}
            step={1}
            showValue={(v) => `${v}`}
            onSave={onSave}
          />
        )}
      </div>
    </GlassCard>
  )
}
