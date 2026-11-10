import { describe, expect, it } from 'vitest'
import { capitalize, formatFixed } from '../../src/utils/locale-text'
import { seeded } from './prng'

const random = seeded(0x10ca)

describe('Locale Text Utilities - Fuzzy Tests', () => {
  it('formatFixed should format random numbers without crashing', () => {
    for (let i = 0; i < 1000; i++) {
      const value = (random() - 0.5) * 10 ** (random() * 10)
      const digits = Math.floor(random() * 20) // toFixed throws if digits > 100 usually, but we test valid range
      const locale = random() > 0.5 ? 'en-US' : 'pt-BR'

      const result = formatFixed(value, digits, locale)
      expect(typeof result).toBe('string')
    }
  })

  it('capitalize should handle any string without crashing', () => {
    for (let i = 0; i < 1000; i++) {
      // Generate some random strings including empty and special chars
      const chars = ['a', 'B', '1', '!', ' ', '', 'é', 'ñ', '😊']
      let text = ''
      const len = Math.floor(random() * 20)
      for (let j = 0; j < len; j++) {
        text += chars[Math.floor(random() * chars.length)]
      }

      const locale = random() > 0.5 ? 'en-US' : 'tr-TR'
      const result = capitalize(text, locale)

      expect(typeof result).toBe('string')
      if (text.length > 0) {
        expect(result.length).toBeGreaterThanOrEqual(text.length) // length might change with some unicode chars but usually >=
      } else {
        expect(result).toBe('')
      }
    }
  })
})
