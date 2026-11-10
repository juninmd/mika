import { BentoGrid, GlassCard, GlassSection } from '@mika/sdk-ui/settings'
import { AlarmClock, Bell } from 'lucide-react'
import { useCallback, useState } from 'react'
import { configBool } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { ReminderForm } from './reminder-form'
import { ReminderList } from './reminder-list'
import type { ReminderDraft } from './reminder-types'

type Props = {
  config: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
}

export function SchedulerPanel({ config }: Props) {
  const { t } = useI18n()
  const [refreshToken, setRefreshToken] = useState(0)
  const [selectedPreset, setSelectedPreset] = useState<ReminderDraft | null>(null)
  const enabled = configBool(config, 'scheduler.enabled', true)

  const handleCreated = useCallback(async () => {
    setSelectedPreset(null)
    setRefreshToken((n) => n + 1)
  }, [])

  return (
    <GlassSection
      title={t('settings.scheduler.tab.label')}
      icon={AlarmClock}
      badge={t('settings.scheduler.panel.badge')}
      description={t('settings.scheduler.panel.description')}
    >
      <BentoGrid cols="auto">
        {!enabled && (
          <div
            role="status"
            className="col-span-full basis-full rounded-xl border border-warn-rim bg-warn-soft p-3 text-xs leading-relaxed text-warn"
          >
            {t('settings.scheduler.panel.disabledNotice')}
          </div>
        )}

        <ReminderForm
          onCreated={handleCreated}
          disabled={!enabled}
          preset={selectedPreset}
          onCancelEdit={() => setSelectedPreset(null)}
        />
        <ReminderList key={refreshToken} disabled={!enabled} onSelectPreset={setSelectedPreset} />

        <GlassCard span="full" hover={false}>
          <div className="flex items-start gap-3">
            <Bell size={16} className="mt-0.5 shrink-0 text-text-muted" aria-hidden="true" />
            <div>
              <h4 className="text-sm font-semibold text-text-main">{t('settings.scheduler.panel.infoTitle')}</h4>
              <p className="mt-1 text-xs leading-relaxed text-text-muted">
                {t('settings.scheduler.panel.infoText')} {t('settings.scheduler.panel.infoDelivery')}
              </p>
            </div>
          </div>
        </GlassCard>
      </BentoGrid>
    </GlassSection>
  )
}
