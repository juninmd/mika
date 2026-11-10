import { describe, expect, it } from 'vitest'
import { scopeEmoji, ts } from '../../src/core/logger-format'
import { seeded } from './prng'

const rand = seeded(0x4d494b41)

describe('Core Logger Format - Fuzzy Tests', () => {
  it('scopeEmoji should return string for any value', () => {
    for (let i = 0; i < 500; i++) {
      const charCode = Math.floor(rand() * 65535)
      const testStr = String.fromCharCode(charCode)
      const res = scopeEmoji(testStr)
      expect(typeof res).toBe('string')
    }
  })

  it('ts should format current date to string', () => {
    for (let i = 0; i < 50; i++) {
      expect(typeof ts()).toBe('string')
    }
  })
})
