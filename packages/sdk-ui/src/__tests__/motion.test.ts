import { describe, expect, it } from 'vitest'
import { DURATION, MOTION, staggerDelay } from '../motion'

describe('motion tokens', () => {
  it('staggers entries but caps the wait so long lists do not trickle in', () => {
    expect(staggerDelay(0)).toBe(0)
    expect(staggerDelay(2)).toBeCloseTo(0.06)
    expect(staggerDelay(40)).toBe(staggerDelay(8))
    expect(staggerDelay(-3)).toBe(0)
    expect(staggerDelay(Number.NaN)).toBe(0)
  })

  it('page transition has no exit — the new tab never waits for the old one to leave', () => {
    expect('exit' in MOTION.page).toBe(false)
    expect(MOTION.page.transition.duration).toBeLessThanOrEqual(DURATION.base)
  })

  it('presets animate only compositor properties (transform/opacity)', () => {
    const allowed = new Set(['opacity', 'x', 'y', 'scale', 'transition'])
    for (const preset of Object.values(MOTION)) {
      for (const phase of ['initial', 'animate', 'exit'] as const) {
        const values = (preset as Record<string, object | undefined>)[phase] ?? {}
        for (const key of Object.keys(values)) expect(allowed.has(key), `${key}`).toBe(true)
      }
    }
  })
})
