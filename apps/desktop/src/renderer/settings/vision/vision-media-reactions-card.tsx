import { GlassCard, GlassSlider, GlassToggle, SectionHeader } from '@mika/sdk-ui/settings'
import type { Translate } from '../../src/hooks/useI18n'

/** Opt-in for real-time reactions to the video, live or song in focus (`vision::media_reaction`). */
export function VisionMediaReactionsCard({
  t,
  enabled,
  cooldownSec,
  boolSave,
  numSave,
}: {
  t: Translate
  enabled: boolean
  cooldownSec: number
  boolSave: (path: string, value: boolean) => Promise<void>
  numSave: (path: string, value: number) => Promise<void>
}) {
  return (
    <GlassCard span="full">
      <div className="space-y-3">
        <SectionHeader title={t('settings.vision.panel.mediaReactionsHeader')} />
        <GlassToggle
          label={t('settings.vision.field.mediaReactions.label')}
          description={t('settings.vision.field.mediaReactions.description')}
          configPath="vision.mediaReactions"
          value={enabled}
          onSave={boolSave}
        />
        {enabled && (
          <div className="pl-4 border-l border-border-subtle">
            <GlassSlider
              label={t('settings.vision.field.mediaReactionCooldownSec.label')}
              description={t('settings.vision.field.mediaReactionCooldownSec.description')}
              configPath="vision.mediaReactionCooldownSec"
              value={cooldownSec}
              min={60}
              max={1800}
              step={60}
              showValue={(v: number) => `${Math.round(v / 60)} min`}
              onSave={numSave}
            />
          </div>
        )}
      </div>
    </GlassCard>
  )
}
