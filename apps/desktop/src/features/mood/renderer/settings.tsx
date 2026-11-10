import {
  BentoGrid,
  GlassCard,
  GlassSection,
  GlassSlider,
  GlassToggle,
  SectionHeader,
  StatusPill,
} from '@mika/sdk-ui/settings'
import { Heart } from 'lucide-react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { configBool, configNum } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { LiveMoodCard } from './live-mood-card'
import { HabitsCard, NeedsCard } from './routine-cards'

export function MoodPanel({ config, onSave }: FeatureComponentProps) {
  const { t } = useI18n()
  const saveBool = onSave as (path: string, value: boolean) => Promise<void>
  const saveNum = onSave as (path: string, value: number) => Promise<void>
  const enabled = configBool(config, 'mood.enabled', true)
  const energyDecay = configNum(config, 'mood.energyDecayRate', 0.02)
  const affinityGain = configNum(config, 'mood.affinityGainRate', 1)
  const minInteraction = configNum(config, 'mood.minAffinityInteractionTime', 30)

  return (
    <GlassSection title={t('settings.mood.tab.label')} icon={Heart} badge={t('settings.mood.panel.badge')}>
      <BentoGrid cols="auto">
        <GlassCard span="full" hover={false}>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="min-w-0 flex-1">
              <GlassToggle
                label={t('settings.mood.panel.toggleLabel')}
                description={t('settings.mood.panel.toggleDescription')}
                configPath="mood.enabled"
                value={enabled}
                onSave={(_p, v) => saveBool('mood.enabled', v as boolean)}
              />
            </div>
            <StatusPill
              variant={enabled ? 'ok' : 'inactive'}
              label={enabled ? t('settings.mood.panel.statusOn') : t('settings.mood.panel.statusOff')}
              dot
            />
          </div>
        </GlassCard>

        {enabled ? (
          <LiveMoodCard />
        ) : (
          <div
            role="status"
            className="col-span-full basis-full rounded-xl border border-warn-rim bg-warn-soft p-4 text-caption leading-relaxed text-warn"
          >
            {t('settings.mood.panel.disabledNotice')}
          </div>
        )}

        <GlassCard span="full">
          <SectionHeader
            title={t('settings.mood.panel.dynamicsTitle')}
            description={t('settings.mood.panel.dynamicsDescription')}
            className="mb-4"
          />
          <div className="grid gap-4 sm:grid-cols-3">
            <GlassSlider
              label={t('settings.mood.panel.decaySliderLabel')}
              description={t('settings.mood.panel.decaySliderDescription')}
              configPath="mood.energyDecayRate"
              value={energyDecay}
              min={0}
              max={1}
              step={0.01}
              showValue={(v) => `${(v * 100).toFixed(0)}${t('settings.mood.panel.perMin')}`}
              onSave={saveNum}
              disabled={!enabled}
            />

            <GlassSlider
              label={t('settings.mood.panel.gainSliderLabel')}
              description={t('settings.mood.panel.gainSliderDescription')}
              configPath="mood.affinityGainRate"
              value={affinityGain}
              min={0}
              max={10}
              step={0.1}
              showValue={(v) => `+${v.toFixed(1)}`}
              onSave={saveNum}
              disabled={!enabled}
            />

            <GlassSlider
              label={t('settings.mood.panel.minTimeSliderLabel')}
              description={t('settings.mood.panel.minTimeSliderDescription')}
              configPath="mood.minAffinityInteractionTime"
              value={minInteraction}
              min={0}
              max={3600}
              step={10}
              showValue={(v) => `${v}s`}
              onSave={saveNum}
              disabled={!enabled}
            />
          </div>
        </GlassCard>

        <NeedsCard config={config} masterEnabled={enabled} saveBool={saveBool} saveNum={saveNum} />
        <HabitsCard config={config} masterEnabled={enabled} saveBool={saveBool} />
      </BentoGrid>
    </GlassSection>
  )
}
