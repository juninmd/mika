import { GlassBox, GlassCard } from '@mika/sdk-ui/settings'
import { Eye, History, MessageCircle, Trophy } from 'lucide-react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { GamerNowPlaying } from './gamer-now-playing'

type Props = {
  enabled: boolean
  backseatEnabled: boolean
  watcherEnabled: boolean
  intervalSeconds: number
}

export function GamerPartnerLoopPanel({ enabled, backseatEnabled, watcherEnabled, intervalSeconds }: Props) {
  const { t } = useI18n()
  const loopItems = [
    {
      label: t('settings.gamerCompanion.panel.loopObserveLabel'),
      detail: t('settings.gamerCompanion.panel.loopObserveDetail'),
      icon: Eye,
      tone: 'text-info',
    },
    {
      label: t('settings.gamerCompanion.panel.loopTeaseLabel'),
      detail: t('settings.gamerCompanion.panel.loopTeaseDetail'),
      icon: MessageCircle,
      tone: 'text-accent',
    },
    {
      label: t('settings.gamerCompanion.panel.loopRememberLabel'),
      detail: t('settings.gamerCompanion.panel.loopRememberDetail'),
      icon: History,
      tone: 'text-ok',
    },
    {
      label: t('settings.gamerCompanion.panel.loopCelebrateLabel'),
      detail: t('settings.gamerCompanion.panel.loopCelebrateDetail'),
      icon: Trophy,
      tone: 'text-warn',
    },
  ]
  const loopActive = enabled && backseatEnabled

  const pauseReason = enabled
    ? t('settings.gamerCompanion.panel.loopPauseReasonBackseat')
    : t('settings.gamerCompanion.panel.loopPauseReasonEnable')

  return (
    <GlassCard span="full" accent>
      <div className="grid gap-4 lg:grid-cols-[1fr_0.85fr]">
        <div className="space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-caption font-black uppercase tracking-[0.16em] text-accent">
                {t('settings.gamerCompanion.panel.loopModeLabel')}
              </p>
              <h4 className="mt-1 text-base font-semibold text-text-main">
                {t('settings.gamerCompanion.panel.loopModeDescription')}
              </h4>
            </div>
            <span
              role="status"
              className={`rounded-lg border px-3 py-2 text-caption font-semibold ${
                loopActive
                  ? 'border-ok-rim bg-ok-soft uppercase tracking-[0.12em] text-ok'
                  : 'border-border-subtle bg-surface-raise text-text-muted'
              }`}
            >
              {loopActive
                ? t('settings.gamerCompanion.panel.loopOnline')
                : `${t('settings.gamerCompanion.panel.loopPaused')} — ${pauseReason}`}
            </span>
          </div>

          <div className="grid gap-2 sm:grid-cols-2">
            {loopItems.map((item) => {
              const Icon = item.icon
              return (
                <GlassBox key={item.label} variant="compact">
                  <div className="flex items-center gap-2">
                    <Icon size={14} className={item.tone} aria-hidden="true" />
                    <span className="text-[12px] font-semibold text-text-main">{item.label}</span>
                  </div>
                  <p className="mt-2 text-caption leading-snug text-text-muted">{item.detail}</p>
                </GlassBox>
              )
            })}
          </div>
        </div>

        <GamerNowPlaying watcherEnabled={watcherEnabled} intervalSeconds={intervalSeconds} />
      </div>
    </GlassCard>
  )
}
