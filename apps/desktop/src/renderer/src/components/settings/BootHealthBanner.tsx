import { AlertTriangle } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'

export function BootHealthBanner({
  health,
  onSelectTab,
}: {
  health: { status: string; bootstrap?: { errors: string[] } } | null
  onSelectTab: (tabId: string) => void
}) {
  const { t } = useI18n()

  if (health?.status !== 'degraded') return null

  const errors = health.bootstrap?.errors ?? []

  return (
    <div
      role="alert"
      className="mb-4 flex items-start gap-3 rounded-xl border border-warn-rim bg-warn-soft p-4 shadow-sm shadow-warn/10"
    >
      <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-warn" aria-hidden="true" />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-bold text-warn">{t('shell.bootHealth.title')}</p>
        <p className="mt-0.5 text-xs text-warn leading-relaxed">{t('shell.bootHealth.description')}</p>
        {errors.length > 0 && (
          <details className="mt-2">
            <summary className="cursor-pointer text-caption font-bold text-warn underline-offset-2 hover:underline">
              {t('shell.bootHealth.viewErrors')} ({errors.length})
            </summary>
            <ul className="mt-1.5 space-y-1 rounded-lg border border-warn-rim/60 bg-warn-soft/60 p-2">
              {errors.map((error, index) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: erros de boot não têm identificador estável
                <li key={index} className="break-words text-caption text-warn/90">
                  {error}
                </li>
              ))}
            </ul>
          </details>
        )}
      </div>
      <button
        type="button"
        onClick={() => onSelectTab('modules')}
        className="shrink-0 rounded-lg border border-warn-rim bg-warn-soft/80 px-2.5 py-1.5 text-xs font-bold text-warn transition active:scale-95 hover:bg-warn hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        {t('shell.bootHealth.viewModules')}
      </button>
    </div>
  )
}
