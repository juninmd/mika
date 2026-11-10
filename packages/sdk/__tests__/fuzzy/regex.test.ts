import { describe, expect, it } from 'vitest'
import { escapeRegExp } from '../../src/utils/regex'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0x4e6e)

function randomString(length: number): string {
  let result = ''
  const characters = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?\\/'
  const charactersLength = characters.length
  for (let i = 0; i < length; i++) {
    result += characters.charAt(Math.floor(random() * charactersLength))
  }
  return result
}

describe('Regex Utilities - Fuzzy Tests', () => {
  it('should escape metacharacters preventing RegExp construction errors', () => {
    for (let i = 0; i < 1000; i++) {
      const input = randomString(Math.floor(random() * 100))
      const escaped = escapeRegExp(input)

      // Should not throw Error
      const rx = new RegExp(escaped)
      expect(rx.test(input)).toBe(true)
    }
  })

  it('escapeRegExp should allow literal matching of any string embedded within other text', () => {
    for (let i = 0; i < 500; i++) {
      const core = randomString(Math.floor(random() * 50) + 1)
      const prefix = randomString(Math.floor(random() * 20))
      const suffix = randomString(Math.floor(random() * 20))

      const escapedCore = escapeRegExp(core)
      const fullString = prefix + core + suffix

      const regex = new RegExp(escapedCore)
      expect(regex.test(fullString)).toBe(true)

      // Strict match check
      const strictRegex = new RegExp(`^${escapedCore}$`)
      expect(strictRegex.test(core)).toBe(true)
      expect(strictRegex.test(fullString)).toBe(prefix === '' && suffix === '')
    }
  })

  it('escapeRegExp should handle empty strings and extremely long strings without crashing', () => {
    for (let i = 0; i < 100; i++) {
      const length = i === 0 ? 0 : Math.floor(random() * 10000)
      const input = randomString(length)
      const escaped = escapeRegExp(input)
      expect(typeof escaped).toBe('string')
      if (length === 0) expect(escaped).toBe('')
    }
  })
})
