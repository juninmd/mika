import { toDisplayError } from '@mika/sdk-ui'
import { ConfirmButton, GlassCard, SectionHeader } from '@mika/sdk-ui/settings'
import { Download, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'
import { useUIStore } from '../../../renderer/src/store/useUIStore'

export function PrivacyDataActionsCard() {
  const { t } = useI18n()
  const [exporting, setExporting] = useState(false)
  const [clearing, setClearing] = useState(false)

  const handleExportData = async () => {
    setExporting(true)
    try {
      const result = await getElectronApi().runtime.executeTool<{ archive?: string }>('export_data', {})
      if (result?.success === false) {
        useUIStore.getState().addNotification('error', result.error ?? t('settings.privacy.panel.exportError'))
        return
      }
      const archive = result?.data?.archive
      useUIStore
        .getState()
        .addNotification(
          'success',
          archive
            ? `${t('settings.privacy.panel.exportSuccess')} ${archive}`
            : t('settings.privacy.panel.exportSuccess'),
        )
    } catch (err) {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('settings.privacy.panel.exportError')))
    } finally {
      setExporting(false)
    }
  }

  const handleClearData = async () => {
    setClearing(true)
    try {
      const result = await getElectronApi().runtime.executeTool<{ skipped?: string[] }>('clear_history', {
        confirm: true,
      })
      if (result?.success === false) {
        useUIStore.getState().addNotification('error', result.error ?? t('settings.privacy.panel.clearError'))
        return
      }
      // The tool names what it did NOT erase: saying only "cleared" would promise more than it did.
      const skipped = result?.data?.skipped ?? []
      const notCovered = skipped.length
        ? ` ${t('settings.privacy.panel.clearSkipped', { stores: skipped.join(', ') })}`
        : ''
      useUIStore.getState().addNotification('success', `${t('settings.privacy.panel.clearSuccess')}${notCovered}`)
    } catch (err) {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('settings.privacy.panel.clearError')))
    } finally {
      setClearing(false)
    }
  }

  return (
    <GlassCard span="full" hover={false}>
      <SectionHeader title={t('settings.privacy.panel.dataActionsTitle')} className="mb-4" />
      <div className="grid gap-4 md:grid-cols-2">
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border-subtle bg-surface-raise/40 p-3">
          <div className="min-w-[12rem] flex-1">
            <p className="text-[13px] font-semibold text-text-main">{t('settings.privacy.panel.exportButton')}</p>
            <p className="mt-1 text-xs text-text-muted">{t('settings.privacy.panel.exportDescription')}</p>
          </div>
          <button
            type="button"
            onClick={() => void handleExportData()}
            disabled={exporting}
            aria-label={t('settings.privacy.panel.exportButton')}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2 text-xs font-bold text-text-main transition hover:border-accent-rim hover:bg-accent-soft hover:text-accent disabled:opacity-50"
          >
            <Download size={13} aria-hidden="true" />
            {t('settings.privacy.panel.exportButton')}
          </button>
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border-subtle bg-surface-raise/40 p-3">
          <div className="min-w-[12rem] flex-1">
            <p className="text-[13px] font-semibold text-text-main">{t('settings.privacy.panel.clearButton')}</p>
            <p className="mt-1 text-xs text-text-muted">{t('settings.privacy.panel.clearDescription')}</p>
          </div>
          <ConfirmButton
            icon={Trash2}
            label={t('settings.privacy.panel.clearButton')}
            confirmLabel={t('settings.privacy.panel.clearConfirmLabel')}
            cancelLabel={t('settings.privacy.panel.clearCancelLabel')}
            ariaLabel={t('settings.privacy.panel.clearButton')}
            disabled={clearing}
            onConfirm={() => void handleClearData()}
          />
        </div>
      </div>
    </GlassCard>
  )
}
