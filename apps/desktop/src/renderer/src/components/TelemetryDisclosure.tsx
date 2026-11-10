import { Shield, ShieldOff } from 'lucide-react'
import { useI18n } from '../hooks/useI18n'

export function TelemetryDisclosure() {
  const { t } = useI18n()
  return (
    <div className="mt-5 space-y-4">
      <div className="rounded-xl border border-ok-rim/30 bg-surface-raise p-4">
        <h3 className="mb-2 flex items-center gap-2 text-xs font-semibold text-ok">
          <Shield size={14} aria-hidden="true" />
          {t('shell.telemetry.collected')}
        </h3>
        <ul className="space-y-1 text-caption leading-relaxed text-text-muted">
          <li>{t('shell.telemetry.c1')}</li>
          <li>{t('shell.telemetry.c2')}</li>
          <li>{t('shell.telemetry.c3')}</li>
          <li>{t('shell.telemetry.c4')}</li>
          <li>{t('shell.telemetry.c5')}</li>
        </ul>
      </div>

      <div className="rounded-xl border border-danger-rim/30 bg-surface-raise p-4">
        <h3 className="mb-2 flex items-center gap-2 text-xs font-semibold text-danger">
          <ShieldOff size={14} aria-hidden="true" />
          {t('shell.telemetry.never')}
        </h3>
        <ul className="space-y-1 text-caption leading-relaxed text-text-muted">
          <li>{t('shell.telemetry.n1')}</li>
          <li>{t('shell.telemetry.n2')}</li>
          <li>{t('shell.telemetry.n3')}</li>
          <li>{t('shell.telemetry.n4')}</li>
          <li>{t('shell.telemetry.n5')}</li>
        </ul>
      </div>

      <p className="text-caption leading-relaxed text-text-muted">{t('shell.telemetry.note')}</p>
    </div>
  )
}
