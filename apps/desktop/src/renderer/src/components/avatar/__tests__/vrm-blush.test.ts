import { describe, expect, it } from 'vitest'
import { blushTarget, RESTING_BLUSH } from '../vrm-blush'

// The VRM ships no blush shape: shy and love read as a plain smile without the anime cheek flush.
describe('blush de anime', () => {
  it('cora enquanto a emoção é de vergonha ou carinho', () => {
    for (const expression of ['shy', 'embarrassed', 'love']) {
      expect(blushTarget(expression, '', 0), expression).toBeGreaterThan(0.5)
    }
  })

  it('acompanha o pico de uma ação tímida e some no fim dela', () => {
    expect(blushTarget('neutral', 'heart', 1)).toBeGreaterThan(0.5)
    expect(blushTarget('neutral', 'shy', 0.5)).toBeLessThan(blushTarget('neutral', 'shy', 1))
    expect(blushTarget('neutral', 'shy', 0)).toBe(RESTING_BLUSH)
  })

  // The Live2D Haru keeps a faint pink on her cheeks at rest; a bare cheek read as a porcelain doll.
  it('fora da vergonha fica só o rosado leve de repouso, como na Haru Live2D', () => {
    expect(RESTING_BLUSH).toBeGreaterThan(0.05)
    expect(RESTING_BLUSH).toBeLessThan(0.25)
    for (const expression of ['angry', 'surprised', 'neutral', 'happy']) {
      expect(blushTarget(expression, 'wave', 1), expression).toBe(RESTING_BLUSH)
    }
  })

  it('pulso inválido nunca vaza para a opacidade', () => {
    expect(blushTarget('neutral', 'heart', Number.NaN)).toBe(RESTING_BLUSH)
    expect(blushTarget('neutral', 'heart', 5)).toBeLessThanOrEqual(1)
  })
})
