import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { Activity, CheckCircle2, ExternalLink, XCircle } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'

export type FeatureTestResult = { ok: boolean; message?: string; latencyMs?: number }

const ICON_BTN =
  'p-1.5 rounded-lg bg-surface-raise border border-border-subtle text-text-muted hover:text-text-main hover:border-border-strong hover:bg-surface-raise-strong disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus'

function StatusCell({ result }: { result?: FeatureTestResult }) {
  const { t } = useI18n()
  if (!result) return <span className="text-text-subtle">{t('settings.homologation.featureRow.untested')}</span>
  const Icon = result.ok ? CheckCircle2 : XCircle
  return (
    <div className={`flex min-w-0 flex-col gap-0.5 ${result.ok ? 'text-ok' : 'text-danger'}`}>
      <span className="flex items-center gap-1.5 font-bold">
        <Icon size={12} aria-hidden="true" />
        {result.ok ? t('settings.homologation.featureRow.synced') : t('settings.homologation.featureRow.failed')}
        {typeof result.latencyMs === 'number' && (
          <span className="font-mono font-normal text-text-muted">{result.latencyMs} ms</span>
        )}
      </span>
      {result.message && (
        <span className="max-w-[28rem] truncate text-caption text-text-muted" title={result.message}>
          {result.message}
        </span>
      )}
    </div>
  )
}

export function FeatureRow({
  tab,
  testResult,
  isTesting,
  onTest,
  onSelect,
}: {
  tab: FeatureSettingsDeclaration
  testResult?: FeatureTestResult
  isTesting: boolean
  onTest: () => void
  onSelect: () => void
}) {
  const { t } = useI18n()
  return (
    <tr className="group hover:bg-surface-raise transition-colors">
      <td className="px-4 py-3">
        <div className="flex flex-col">
          <span className="font-bold text-text-main">{tab.tabLabel}</span>
          {tab.tabGroup && <span className="text-caption text-text-subtle">{tab.tabGroup}</span>}
        </div>
      </td>
      <td className="px-4 py-3">
        {isTesting ? (
          <span className="animate-pulse text-text-muted">{t('settings.tabHero.test.running')}</span>
        ) : (
          <StatusCell result={testResult} />
        )}
      </td>
      <td className="px-4 py-3 text-right">
        <div className="flex items-center justify-end gap-2">
          <button
            type="button"
            onClick={onTest}
            disabled={isTesting}
            aria-label={t('settings.homologation.featureRow.testConnectionAriaLabel').replace(
              '{{label}}',
              tab.tabLabel,
            )}
            className={ICON_BTN}
            title={t('button.test')}
          >
            <Activity size={14} className={isTesting ? 'animate-pulse' : ''} aria-hidden="true" />
          </button>
          <button
            type="button"
            onClick={onSelect}
            aria-label={t('settings.homologation.featureRow.goToSettingsAriaLabel').replace('{{label}}', tab.tabLabel)}
            className={ICON_BTN}
            title={t('settings.homologation.featureRow.goToSettings')}
          >
            <ExternalLink size={14} aria-hidden="true" />
          </button>
        </div>
      </td>
    </tr>
  )
}
