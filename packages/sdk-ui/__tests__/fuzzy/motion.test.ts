import { describe, expect, it } from 'vitest'
import { seeded } from '../../../sdk/__tests__/fuzzy/prng'
import { staggerDelay } from '../../src/motion'

const rand = seeded(66)

describe('Motion Utilities - Fuzzy Tests', () => {
  describe('staggerDelay', () => {
    it('should calculate stagger delay correctly with max limit for random inputs', () => {
      for (let i = 0; i < 1000; i++) {
        // can be negative, 0, or very large
        const index = (rand() - 0.2) * 100
        const step = rand()

        const result = staggerDelay(index, step)

        expect(typeof result).toBe('number')
        expect(result).toBeGreaterThanOrEqual(0)

        // it caps at STAGGER_MAX_ITEMS (8)
        expect(result).toBeLessThanOrEqual(8 * step)

        if (index <= 0 || !Number.isFinite(index)) {
          expect(result).toBe(0)
        }
      }
    })
  })
})
