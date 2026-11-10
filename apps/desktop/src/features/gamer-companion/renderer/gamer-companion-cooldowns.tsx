import { GlassCard, GlassSlider, GlassToggle, SectionDivider, SectionHeader } from '@mika/sdk-ui/settings'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { configBool, configNum } from '../../../renderer/src/components/settings/settings-utils'
import type { Translate } from '../../../renderer/src/hooks/useI18n'

export function GamerCompanionCooldowns({
  t,
  config,
  onSave,
}: {
  t: Translate
  config: FeatureComponentProps['config']
  onSave: FeatureComponentProps['onSave']
}) {
  const sessionSummaryEnabled = configBool(config, 'gamer-companion.sessionSummaryEnabled', true)
  const bossCooldown = configNum(config, 'gamer-companion.momentCooldowns.boss', 90)
  const achievementCooldown = configNum(config, 'gamer-companion.momentCooldowns.achievement', 60)
  const victoryCooldown = configNum(config, 'gamer-companion.momentCooldowns.victory', 60)
  const deathCooldown = configNum(config, 'gamer-companion.momentCooldowns.death', 45)
  const cutsceneCooldown = configNum(config, 'gamer-companion.momentCooldowns.cutscene', 45)
  const defeatCooldown = configNum(config, 'gamer-companion.momentCooldowns.defeat', 45)

  return (
    <GlassCard span="full">
      <SectionHeader title={t('settings.gamerCompanion.panel.sectionComments')} className="mb-3" />
      <div className="mb-4">
        <GlassToggle
          label={t('settings.gamerCompanion.panel.summaryLabel')}
          description={t('settings.gamerCompanion.panel.summaryDescription')}
          configPath="gamer-companion.sessionSummaryEnabled"
          value={sessionSummaryEnabled}
          onSave={onSave}
        />
      </div>
      <SectionDivider label={t('settings.gamerCompanion.panel.cooldownsSectionLabel')} />
      <div className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <GlassSlider
          label={t('settings.gamerCompanion.panel.cooldownBossLabel')}
          description={t('settings.gamerCompanion.panel.cooldownBossDescription')}
          configPath="gamer-companion.momentCooldowns.boss"
          value={bossCooldown}
          min={0}
          max={3600}
          step={15}
          showValue={(v) => `${v}s`}
          onSave={onSave}
        />
        <GlassSlider
          label={t('settings.gamerCompanion.panel.cooldownAchievementLabel')}
          description={t('settings.gamerCompanion.panel.cooldownAchievementDescription')}
          configPath="gamer-companion.momentCooldowns.achievement"
          value={achievementCooldown}
          min={0}
          max={3600}
          step={15}
          showValue={(v) => `${v}s`}
          onSave={onSave}
        />
        <GlassSlider
          label={t('settings.gamerCompanion.panel.cooldownVictoryLabel')}
          description={t('settings.gamerCompanion.panel.cooldownVictoryDescription')}
          configPath="gamer-companion.momentCooldowns.victory"
          value={victoryCooldown}
          min={0}
          max={3600}
          step={15}
          showValue={(v) => `${v}s`}
          onSave={onSave}
        />
        <GlassSlider
          label={t('settings.gamerCompanion.panel.cooldownDeathLabel')}
          description={t('settings.gamerCompanion.panel.cooldownDeathDescription')}
          configPath="gamer-companion.momentCooldowns.death"
          value={deathCooldown}
          min={0}
          max={3600}
          step={15}
          showValue={(v) => `${v}s`}
          onSave={onSave}
        />
        <GlassSlider
          label={t('settings.gamerCompanion.panel.cooldownCutsceneLabel')}
          description={t('settings.gamerCompanion.panel.cooldownCutsceneDescription')}
          configPath="gamer-companion.momentCooldowns.cutscene"
          value={cutsceneCooldown}
          min={0}
          max={3600}
          step={15}
          showValue={(v) => `${v}s`}
          onSave={onSave}
        />
        <GlassSlider
          label={t('settings.gamerCompanion.panel.cooldownDefeatLabel')}
          description={t('settings.gamerCompanion.panel.cooldownDefeatDescription')}
          configPath="gamer-companion.momentCooldowns.defeat"
          value={defeatCooldown}
          min={0}
          max={3600}
          step={15}
          showValue={(v) => `${v}s`}
          onSave={onSave}
        />
      </div>
    </GlassCard>
  )
}
