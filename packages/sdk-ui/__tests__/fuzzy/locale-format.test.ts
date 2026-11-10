import { describe, expect, it } from 'vitest'
import { seeded } from '../../../sdk/__tests__/fuzzy/prng'
import { formatLocalDate, formatLocalDuration, formatLocalNumber, formatLocalTime } from '../../src/utils/locale-format'

// Semente fixa: o fuzz roda igual em toda execução (uma falha sempre reproduz).
const rand = seeded(44)

describe('Locale Format Utilities - Fuzzy Tests', () => {
  it('formatLocalDate should handle random dates without crashing', () => {
    for (let i = 0; i < 1000; i++) {
      // random date between year 1970 and 2070
      const ms = rand() * 1000 * 60 * 60 * 24 * 365 * 100
      const date = rand() > 0.5 ? new Date(ms) : ms

      const result = formatLocalDate(date)
      expect(typeof result).toBe('string')
      expect(result.length).toBeGreaterThan(0)
    }
  })

  it('formatLocalTime should handle random dates without crashing', () => {
    for (let i = 0; i < 1000; i++) {
      // random date between year 1970 and 2070
      const ms = rand() * 1000 * 60 * 60 * 24 * 365 * 100
      const date = rand() > 0.5 ? new Date(ms) : ms

      const result = formatLocalTime(date)
      expect(typeof result).toBe('string')
      expect(result.length).toBeGreaterThan(0)
    }
  })

  it('formatLocalNumber should handle random numbers without crashing', () => {
    for (let i = 0; i < 1000; i++) {
      const num = (rand() - 0.5) * 10 ** (rand() * 10)

      const result = formatLocalNumber(num)
      expect(typeof result).toBe('string')
      expect(result.length).toBeGreaterThan(0)
    }
  })

  it('formatLocalDuration should handle random seconds without crashing', () => {
    for (let i = 0; i < 1000; i++) {
      const seconds = (rand() - 0.5) * 10 ** (rand() * 10)

      const result = formatLocalDuration(seconds)
      expect(typeof result).toBe('string')
      expect(result.length).toBeGreaterThan(0)
    }
  })
})
