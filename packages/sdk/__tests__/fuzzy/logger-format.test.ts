import { describe, expect, it } from 'vitest'
import { scopeEmoji, ts } from '../../src/core/logger-format'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0x106)

function randomString(length: number): string {
  const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-'
  let result = ''
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(random() * chars.length))
  }
  return result
}

describe('Logger Format - Fuzzy Tests', () => {
  it('scopeEmoji should never crash and always return a string emoji or fallback', () => {
    for (let i = 0; i < 1000; i++) {
      const scope = randomString(Math.floor(random() * 20))
      const result = scopeEmoji(scope)
      expect(typeof result).toBe('string')
      expect(result.length).toBeGreaterThan(0)
    }
  })

  it('ts should always return a formatted time string representing current time', () => {
    const result = ts()
    expect(typeof result).toBe('string')
    expect(result).toMatch(/^\d{2}:\d{2}:\d{2}\.\d{3}$/)
  })
})
