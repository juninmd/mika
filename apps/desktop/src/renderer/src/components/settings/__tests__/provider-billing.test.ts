import { describe, expect, it } from 'vitest'
import type { ProviderBilling } from '../../../../../shared/electron-api-system-types'
import { billingVerdict } from '../provider-billing'

const base: ProviderBilling = { provider: 'x', local: false, free: false, api: { status: 'unavailable' }, usage: null }
const balance = (used: number, limit: number | null, remaining: number | null, exhausted = false) => ({
  ...base,
  api: {
    status: 'ok' as const,
    balance: { unit: 'usd' as const, used, limit, remaining, resetsAt: null, plan: null, exhausted },
  },
})

// O veredito responde a pergunta da tela — "este provedor ainda consegue atender?" — sem inventar
// saldo onde a API não informa.
describe('billingVerdict', () => {
  it('esgotado quando a API diz que acabou, mesmo com saldo residual arredondado', () => {
    expect(billingVerdict(balance(10, 10, 0, true))).toBe('exhausted')
  })

  it('baixo abaixo de 10% do limite, para avisar antes de o provedor recusar', () => {
    expect(billingVerdict(balance(95, 100, 5))).toBe('low')
    expect(billingVerdict(balance(50, 100, 50))).toBe('ok')
  })

  it('chave sem teto de gasto pode atender (não é "desconhecido")', () => {
    expect(billingVerdict(balance(3, null, null))).toBe('ok')
  })

  it('provedor sem API de saldo nunca vira "ok" por omissão', () => {
    expect(billingVerdict(base)).toBe('unknown')
  })

  it('local/grátis, chave ausente e erro de consulta são estados distintos', () => {
    expect(billingVerdict({ ...base, local: true, free: true, api: { status: 'notApplicable' } })).toBe('free')
    expect(billingVerdict({ ...base, api: { status: 'missingKey' } })).toBe('missingKey')
    expect(billingVerdict({ ...base, api: { status: 'error', error: 'HTTP 500' } })).toBe('error')
  })
})
