import type { ProviderBilling } from '../../../../shared/electron-api-system-types'

export type BillingVerdict = 'ok' | 'low' | 'exhausted' | 'unknown' | 'free' | 'missingKey' | 'error'

/** Abaixo disto do limite o card avisa antes de o provedor começar a recusar pedidos. */
const LOW_FRACTION = 0.1

/** Responde "o provedor ainda consegue atender?" só com o que a API informou — sem saldo, `unknown`. */
export function billingVerdict(billing: ProviderBilling): BillingVerdict {
  const { status, balance } = billing.api
  if (status === 'notApplicable') return 'free'
  if (status === 'missingKey' || status === 'error') return status
  if (status !== 'ok' || !balance) return 'unknown'
  if (balance.exhausted) return 'exhausted'
  if (balance.limit && balance.remaining !== null && balance.remaining / balance.limit < LOW_FRACTION) return 'low'
  return 'ok'
}
