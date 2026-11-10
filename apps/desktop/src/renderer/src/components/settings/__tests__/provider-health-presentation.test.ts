import { describe, expect, it } from 'vitest'
import type { LLMProviderHealth } from '../../../../../shared/electron-api-system-types'
import { isVisionOnlyFailure, providerHealthPresentation } from '../ProviderHealthBanner'

const entry = (fatal: boolean, surfaces: string[] = []): LLMProviderHealth => ({
  key: 'ollama:llama3',
  provider: 'ollama',
  model: 'llama3',
  state: 'disabled',
  since: 0,
  failureCount: 3,
  fatal,
  surfaces,
})

describe('providerHealthPresentation', () => {
  it('marks a fatal entry as needing user action (wrong key/model, no auto-retry)', () => {
    expect(providerHealthPresentation(entry(true))).toEqual({
      variant: 'error',
      badgeKey: 'shell.providerHealth.fatalBadge',
      hintKey: 'shell.providerHealth.fatalHint',
    })
  })

  it('marks a transient entry as retrying automatically with cooldown', () => {
    expect(providerHealthPresentation(entry(false))).toEqual({
      variant: 'warn',
      badgeKey: 'shell.providerHealth.transientBadge',
      hintKey: 'shell.providerHealth.transientHint',
    })
  })
})

// WP9 item 6: uma falha só de visão precisa ser identificável separada de uma falha de chat quando
// o mesmo par provider:model serve as duas superfícies (defects.md vision P2).
describe('isVisionOnlyFailure', () => {
  it('is true when every surface that failed is vision', () => {
    expect(isVisionOnlyFailure(entry(false, ['vision']))).toBe(true)
  })

  it('is false when chat also failed on the same pair', () => {
    expect(isVisionOnlyFailure(entry(false, ['vision', 'chat']))).toBe(false)
  })

  it('is false when only chat failed', () => {
    expect(isVisionOnlyFailure(entry(false, ['chat']))).toBe(false)
  })

  it('is false with no surfaces recorded (older entry shape)', () => {
    expect(isVisionOnlyFailure(entry(false, []))).toBe(false)
  })
})
