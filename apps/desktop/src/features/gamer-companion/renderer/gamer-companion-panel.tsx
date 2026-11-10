import { toDisplayError } from '@mika/sdk-ui'
import { BentoGrid, GlassCard, GlassSection, SectionHeader } from '@mika/sdk-ui/settings'
import { Gamepad2 } from 'lucide-react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { SettingsRiskBanner, SettingsTagInput } from '../../../renderer/src/components/settings/RichSettingBlocks'
import { configBool, configNum, configStrArray } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { useUIStore } from '../../../renderer/src/store/useUIStore'
import { GamerCompanionCooldowns } from './gamer-companion-cooldowns'
import { GamerCompanionDetection } from './gamer-companion-detection'
import { GamerCompanionSteam } from './gamer-companion-steam'
import { GamerCustomGames } from './gamer-custom-games'
import { GamerPartnerLoopPanel } from './gamer-partner-loop-panel'

export function GamerCompanionPanel({ config, onSave }: FeatureComponentProps) {
  const { t } = useI18n()
  const enabled = configBool(config, 'gamer-companion.enabled', true)
  const watcherEnabled = configBool(config, 'gamer-companion.gameWatcherEnabled', true)
  const watcherIntervalSeconds = configNum(config, 'gamer-companion.gameWatcherIntervalSeconds', 30)
  const backseatEnabled = configBool(config, 'gamer-companion.backseatGamingEnabled', false)
  const backseatAggressiveness = configNum(config, 'gamer-companion.backseatAggressiveness', 40)
  const favoriteGames = configStrArray(config, 'gamer-companion.favoriteGames')
  const favoriteAnime = configStrArray(config, 'gamer-companion.favoriteAnime')

  const saveFavoriteGames = async (games: string[]) => {
    try {
      await onSave('gamer-companion.favoriteGames', games.join(','))
    } catch (err) {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('sdkUi.saveError')))
    }
  }

  const saveFavoriteAnime = async (titles: string[]) => {
    try {
      await onSave('gamer-companion.favoriteAnime', titles.join(','))
    } catch (err) {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('sdkUi.saveError')))
    }
  }

  return (
    <GlassSection
      title={t('settings.gamerCompanion.panel.title')}
      icon={Gamepad2}
      badge={t('settings.gamerCompanion.tab.group')}
    >
      <BentoGrid cols="auto">
        <GlassCard span="full">
          <GamerPartnerLoopPanel
            enabled={enabled}
            backseatEnabled={backseatEnabled}
            watcherEnabled={watcherEnabled}
            intervalSeconds={watcherIntervalSeconds}
          />
        </GlassCard>

        <GamerCompanionDetection
          t={t}
          watcherEnabled={watcherEnabled}
          watcherIntervalSeconds={watcherIntervalSeconds}
          backseatEnabled={backseatEnabled}
          backseatAggressiveness={backseatAggressiveness}
          onSave={onSave}
        />

        {/* One card, two columns: four narrow cards clipped the tag inputs' placeholders. */}
        <GlassCard span="full">
          <div className="grid gap-6 md:grid-cols-2">
            <div>
              <SectionHeader title={t('settings.user.field.favoriteGames.label')} className="mb-3" />
              <SettingsTagInput
                label={t('settings.gamerCompanion.panel.favoriteGamesTagLabel')}
                tags={favoriteGames}
                onChange={saveFavoriteGames}
                placeholder={t('settings.gamerCompanion.panel.tagPlaceholder')}
              />
            </div>
            <div>
              <SectionHeader title={t('settings.user.field.favoriteAnime.label')} className="mb-3" />
              <SettingsTagInput
                label={t('settings.gamerCompanion.panel.favoriteAnimeTagLabel')}
                tags={favoriteAnime}
                onChange={saveFavoriteAnime}
                placeholder={t('settings.gamerCompanion.panel.tagPlaceholder')}
              />
            </div>
          </div>
        </GlassCard>

        <GamerCustomGames t={t} config={config} onSave={onSave} />

        <GamerCompanionCooldowns t={t} config={config} onSave={onSave} />

        <GamerCompanionSteam t={t} config={config} onSave={onSave} />

        <GlassCard span="full" hover={false}>
          <SettingsRiskBanner
            title={t('settings.gamerCompanion.panel.riskBannerTitle')}
            detail={t('settings.gamerCompanion.panel.riskBannerDetail')}
            tone="neutral"
          />
        </GlassCard>
      </BentoGrid>
    </GlassSection>
  )
}
