import { BentoGrid, GlassSection, PageHero, StatusPill } from '@mika/sdk-ui/settings'
import { Radar, ScanEye } from 'lucide-react'
import { DecisionProviderCard } from '../../../settings/vision/decision-provider-card'
import { VisionDecisionCard } from '../../../settings/vision/vision-decision-card'
import { useI18n } from '../../hooks/useI18n'
import { configBool, configNum, configStr } from './settings-utils'

/** Own tab for the decision-model pre-filter (`vision::decision`); it gates the Vision tab's LLM. */
export function DecisionPanel({
  config,
  onSave,
}: {
  config: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
}) {
  const { t } = useI18n()
  const enabled = configBool(config, 'vision.decisionEnabled')
  const visionEnabled = configBool(config, 'vision.enabled')
  return (
    <div className="space-y-6">
      <PageHero
        icon={ScanEye}
        title={t('settings.decision.panel.title')}
        description={t('settings.decision.panel.description')}
        pills={
          <>
            <StatusPill
              variant={enabled ? 'ok' : 'neutral'}
              label={t(enabled ? 'settings.decision.hero.on' : 'settings.decision.hero.off')}
              dot
            />
            {!visionEnabled && <StatusPill variant="warn" label={t('settings.decision.hero.visionOff')} />}
          </>
        }
      />
      <GlassSection
        title={t('settings.vision.panel.decisionHeader')}
        icon={Radar}
        description={t('settings.decision.section.description')}
      >
        <BentoGrid>
          <VisionDecisionCard
            t={t}
            enabled={enabled}
            threshold={configNum(config, 'vision.decisionThreshold', 0.5)}
            onSave={onSave}
          />
          <DecisionProviderCard
            t={t}
            config={config}
            provider={configStr(config, 'vision.decisionProvider', 'llamaServer')}
            model={configStr(config, 'vision.decisionModel')}
            onSave={onSave}
          />
        </BentoGrid>
      </GlassSection>
    </div>
  )
}
