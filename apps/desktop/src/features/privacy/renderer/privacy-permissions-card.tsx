import { ConfigKey } from '@mika/sdk/contracts'
import { GlassCard, GlassToggle, SectionHeader } from '@mika/sdk-ui/settings'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { useUIStore } from '../../../renderer/src/store/useUIStore'

export function PrivacyPermissionsCard({
  screenCapture,
  clipboardLogging,
  offlineMode,
  saveBool,
}: {
  screenCapture: boolean
  clipboardLogging: boolean
  offlineMode: boolean
  saveBool: (path: string, value: boolean) => Promise<void>
}) {
  const { t } = useI18n()
  const privacyToggles = [
    {
      label: t('settings.privacy.field.offlineMode.label'),
      description: t('settings.privacy.field.offlineMode.description'),
      configPath: 'privacy.offlineMode',
      value: offlineMode,
    },
    {
      label: t('settings.privacy.field.screenCapture.label'),
      description: t('settings.privacy.field.screenCapture.description'),
      configPath: 'privacy.screenCapture',
      value: screenCapture,
    },
    {
      label: t('settings.privacy.field.clipboardLogging.label'),
      description: t('settings.privacy.field.clipboardLogging.description'),
      configPath: 'privacy.clipboardLogging',
      value: clipboardLogging,
    },
  ]

  return (
    <GlassCard span="full">
      <SectionHeader title={t('settings.privacy.panel.localPermissionsLabel')} className="mb-4" />
      <div className="grid gap-4 sm:grid-cols-3">
        {privacyToggles.map((toggle) => (
          <div key={toggle.configPath} className="rounded-xl border border-border-subtle bg-surface-raise/40 p-3">
            <GlassToggle
              {...toggle}
              onSave={
                toggle.configPath === 'privacy.screenCapture'
                  ? async (path, value) => {
                      const previous = !value
                      await saveBool(ConfigKey.VisionEnabled, value)
                      try {
                        await saveBool(path, value)
                      } catch (error) {
                        try {
                          await saveBool(ConfigKey.VisionEnabled, previous)
                        } catch {
                          useUIStore
                            .getState()
                            .addNotification('error', t('settings.privacy.panel.screenCaptureRollbackError'))
                        }
                        throw error
                      }
                    }
                  : saveBool
              }
            />
          </div>
        ))}
      </div>
    </GlassCard>
  )
}
