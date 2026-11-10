import { getLocale } from '@mika/sdk/i18n'
import { formatLocalNumber } from '@mika/sdk-ui'
import type { OpenRouterKeyStatus } from '../../../../shared/electron-api-system-types'
import { useI18n } from '../../hooks/useI18n'

// OpenRouter bills in USD whatever the UI language is; the locale only picks separators and the symbol's placement.
const money = (value: number) => formatLocalNumber(value, getLocale(), { style: 'currency', currency: 'USD' })

export function OpenRouterQuotaLine({ quota }: { quota: OpenRouterKeyStatus }) {
  const { t } = useI18n()
  if (quota.limit === null) {
    return (
      <p className="mt-1 text-xs text-text-muted">
        {t('shell.providerHealth.noSpendCap')} {t('shell.providerHealth.used')} {money(quota.usage)}.
      </p>
    )
  }
  const remaining = quota.limitRemaining ?? 0
  return (
    <p className={`mt-1 text-xs ${quota.exhausted ? 'font-bold text-danger' : 'text-text-muted'}`}>
      {quota.exhausted ? t('shell.providerHealth.keyExhausted') : t('shell.providerHealth.keyBalance')}
      {t('shell.providerHealth.used')} {money(quota.usage)} {t('shell.providerHealth.of')} {money(quota.limit)} ·{' '}
      {t('shell.providerHealth.remaining')} {money(remaining)}
      {quota.limitReset ? ` · ${t('shell.providerHealth.resets')} ${quota.limitReset}` : ''}
      {quota.isFreeTier ? ` · ${t('shell.providerHealth.freeTier')}` : ''}
    </p>
  )
}
