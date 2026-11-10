import { describe, expect, it } from 'vitest'
import { shouldFinishUtterance } from '../speech-transcribe'

describe('shouldFinishUtterance', () => {
  it('finishes once silence reaches the live hangover value — not the old hardcoded 1200ms', () => {
    // Regression: the main listening path used to compare against a hardcoded constant no matter
    // what "Duração de Silêncio para Finalizar" (voice.vadSilenceMs) was set to.
    expect(shouldFinishUtterance(3_000, 5_000, 3_000, true)).toBe(true)
    expect(shouldFinishUtterance(2_999, 5_000, 3_000, true)).toBe(false)
  })

  it('never finishes on silence when endpointing is disabled, only at the hard ceiling', () => {
    expect(shouldFinishUtterance(999_999, 10_000, 500, false)).toBe(false)
    expect(shouldFinishUtterance(0, 45_000, 500, false)).toBe(true)
  })

  it('the 45s ceiling always wins, endpointing on or off', () => {
    expect(shouldFinishUtterance(0, 45_000, 5_000, true)).toBe(true)
    expect(shouldFinishUtterance(0, 45_000, 5_000, false)).toBe(true)
  })
})
