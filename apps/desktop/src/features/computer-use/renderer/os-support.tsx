import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard, StatusPill } from '@mika/sdk-ui/settings'
import { Check, Minus, MonitorSmartphone } from 'lucide-react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'

type Os = 'win32' | 'darwin' | 'linux'
const OSES: { id: Os; labelKey: keyof TranslationDict }[] = [
  { id: 'win32', labelKey: 'settings.computerUse.os.windows' },
  { id: 'darwin', labelKey: 'settings.computerUse.os.macos' },
  { id: 'linux', labelKey: 'settings.computerUse.os.linux' },
]

/**
 * Mirrors `features/computer_use` (Rust): `click`/`type_text`/`key` go through `enigo` on every OS;
 * `find`/`click_element` need Windows UI Automation, and the "never click Mika herself" guard reads
 * the foreground window, which only has a Windows probe today.
 */
const CAPABILITIES: { labelKey: keyof TranslationDict; oses: Os[] }[] = [
  { labelKey: 'settings.computerUse.os.capInput', oses: ['win32', 'darwin', 'linux'] },
  { labelKey: 'settings.computerUse.os.capFind', oses: ['win32'] },
  { labelKey: 'settings.computerUse.os.capGuard', oses: ['win32'] },
]

export function ComputerUseOsSupport() {
  const { t } = useI18n()
  const current = getElectronApi().platform as Os
  return (
    <GlassCard span="full" hover={false}>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <MonitorSmartphone size={15} className="text-accent" aria-hidden="true" />
        <h3 className="text-sm font-black uppercase tracking-[0.12em] text-text-main">
          {t('settings.computerUse.os.title')}
        </h3>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[28rem] border-collapse text-left text-xs">
          <thead>
            <tr>
              <th className="py-2 pr-3 text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">
                {t('settings.computerUse.os.capability')}
              </th>
              {OSES.map((os) => (
                <th key={os.id} className="px-2 py-2 text-center">
                  <span className="inline-flex flex-col items-center gap-1">
                    <span className={`text-caption font-bold ${os.id === current ? 'text-accent' : 'text-text-muted'}`}>
                      {t(os.labelKey)}
                    </span>
                    {os.id === current && <StatusPill variant="accent" label={t('settings.computerUse.os.thisPc')} />}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-border-subtle">
            {CAPABILITIES.map((cap) => (
              <tr key={cap.labelKey}>
                <td className="py-2 pr-3 text-text-main">{t(cap.labelKey)}</td>
                {OSES.map((os) => {
                  const ok = cap.oses.includes(os.id)
                  return (
                    <td key={os.id} className={`px-2 py-2 text-center ${os.id === current ? 'bg-accent-soft/40' : ''}`}>
                      {ok ? (
                        <Check
                          size={14}
                          className="mx-auto text-ok"
                          aria-label={t('settings.computerUse.os.supported')}
                        />
                      ) : (
                        <Minus
                          size={14}
                          className="mx-auto text-text-subtle"
                          aria-label={t('settings.computerUse.os.unsupported')}
                        />
                      )}
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </GlassCard>
  )
}
