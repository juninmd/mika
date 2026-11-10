import { BentoGrid, choiceOption, GlassCard, GlassSection, GlassSelect, SectionHeader } from '@mika/sdk-ui/settings'
import { Archive, CalendarDays, CalendarRange, Hourglass, Infinity as InfinityIcon, Shield } from 'lucide-react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { SettingsRiskBanner } from '../../../renderer/src/components/settings/RichSettingBlocks'
import { configBool, configStr } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { PrivacyDataActionsCard } from './privacy-data-actions-card'
import { PrivacyDataLocationCard } from './privacy-info-cards'
import { PrivacyMemoryCard } from './privacy-memory-card'
import { PrivacyPermissionsCard } from './privacy-permissions-card'

export function PrivacyPanel({ config, onSave }: FeatureComponentProps) {
  const { t } = useI18n()
  const saveBool = onSave as (path: string, value: boolean) => Promise<void>
  const screenCapture = configBool(config, 'privacy.screenCapture')
  const clipboardLogging = configBool(config, 'privacy.clipboardLogging')
  const conversationRetention = configStr(config, 'privacy.conversationRetention', 'forever')
  const offlineMode = configBool(config, 'privacy.offlineMode')
  const localRiskCount = Number(screenCapture) + Number(clipboardLogging)
  const RETENTION_OPTIONS = [
    choiceOption(
      t('settings.privacy.retention.oneDay'),
      '1day',
      t('settings.privacy.retention.hint.oneDay'),
      Hourglass,
    ),
    choiceOption(
      t('settings.privacy.retention.sevenDays'),
      '7days',
      t('settings.privacy.retention.hint.sevenDays'),
      CalendarDays,
    ),
    choiceOption(
      t('settings.privacy.retention.thirtyDays'),
      '30days',
      t('settings.privacy.retention.hint.thirtyDays'),
      CalendarRange,
    ),
    choiceOption(
      t('settings.privacy.retention.ninetyDays'),
      '90days',
      t('settings.privacy.retention.hint.ninetyDays'),
      Archive,
    ),
    choiceOption(
      t('settings.privacy.retention.forever'),
      'forever',
      t('settings.privacy.retention.hint.forever'),
      InfinityIcon,
    ),
  ]

  return (
    <GlassSection title={t('settings.privacy.tab.label')} icon={Shield} badge={t('settings.privacy.panel.badge')}>
      <BentoGrid cols="auto">
        <div className="col-span-full basis-full">
          <SettingsRiskBanner
            title={
              localRiskCount === 0
                ? t('settings.privacy.panel.riskBannerTitle')
                : `${t('settings.privacy.panel.localPermissionsLabel')}: ${localRiskCount}/2 ${t('settings.privacy.panel.activeSuffix')}`
            }
            detail={t('settings.privacy.panel.heroDescription')}
            tone={localRiskCount === 0 ? 'ok' : 'warn'}
          />
        </div>

        <PrivacyPermissionsCard
          screenCapture={screenCapture}
          clipboardLogging={clipboardLogging}
          offlineMode={offlineMode}
          saveBool={saveBool}
        />

        <GlassCard span="full" hover={false}>
          <SectionHeader
            title={t('settings.privacy.panel.retentionSectionTitle')}
            description={t('settings.privacy.panel.retentionSectionDescription')}
            className="mb-4"
          />
          <GlassSelect
            label={t('settings.privacy.field.conversationRetention.label')}
            configPath="privacy.conversationRetention"
            config={{ privacy: { conversationRetention } }}
            onSave={onSave as (path: string, value: string) => Promise<void>}
            options={RETENTION_OPTIONS}
          />
        </GlassCard>

        <PrivacyDataActionsCard />

        <PrivacyMemoryCard />

        <PrivacyDataLocationCard />
      </BentoGrid>
    </GlassSection>
  )
}
