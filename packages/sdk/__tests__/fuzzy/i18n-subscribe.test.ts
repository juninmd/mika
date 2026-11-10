import { describe, expect, it } from 'vitest'
import { setLocale, subscribeLocale } from '../../src/i18n/index'

describe('i18n subscribe - Fuzzy Tests', () => {
  it('subscribeLocale should allow subscription and unsubscription without errors', () => {
    let callCount = 0
    const unsubscribe = subscribeLocale(() => {
      callCount++
    })

    // Trigger some valid state changes
    const _original = 'pt-BR'
    setLocale('en-US')
    setLocale('pt-BR')

    unsubscribe()
    const oldCallCount = callCount

    setLocale('en-US')

    expect(callCount).toBe(oldCallCount) // Should not increase after unsubscribe
  })
})
