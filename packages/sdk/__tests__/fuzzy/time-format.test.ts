import { describe, expect, it } from 'vitest'
import { formatDurationMs, formatRelativeTime } from '../../src/utils/time-format'
import { seeded } from './prng'

const random = seeded(0x71e)

function randomDuration(): number {
  return (random() - 0.2) * 1000 * 60 * 60 * 24 * 7
}

describe('Time Format - Fuzzy Tests', () => {
  it('should format any random duration without crashing', () => {
    for (let i = 0; i < 1000; i++) {
      const ms = randomDuration()
      const formatted = formatDurationMs(ms)

      expect(typeof formatted).toBe('string')
      if (ms <= 0) {
        expect(formatted).toBe('0min')
      } else {
        expect(formatted).toMatch(/min|h/)
      }
    }
  })

  it('should format any random relative time without crashing', () => {
    for (let i = 0; i < 1000; i++) {
      const epochMs = Date.now() - (random() - 0.2) * 1000 * 60 * 60 * 24 * 365
      const mockTranslate = (key: unknown) => key as string

      const formatted = formatRelativeTime(epochMs, mockTranslate)

      expect(typeof formatted).toBe('string')
    }
  })
})
