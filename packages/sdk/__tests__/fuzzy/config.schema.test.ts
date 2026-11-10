import { describe, expect, it } from 'vitest'
import { SUPPORTED_LOCALES, SupportedLocaleSchema } from '../../src/contracts/schemas/config.schema'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0xc0f165)

function randomString(length: number): string {
  const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-.'
  let result = ''
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(random() * chars.length))
  }
  return result
}

describe('Config Schema - Fuzzy Tests', () => {
  it('SupportedLocaleSchema should only accept defined locales', () => {
    // Valid cases
    SUPPORTED_LOCALES.forEach((locale) => {
      expect(SupportedLocaleSchema.safeParse(locale).success).toBe(true)
    })

    // Random strings shouldn't pass
    for (let i = 0; i < 500; i++) {
      let rand = randomString(Math.floor(random() * 10) + 2)
      while (SUPPORTED_LOCALES.includes(rand as (typeof SUPPORTED_LOCALES)[number])) {
        rand = randomString(Math.floor(random() * 10) + 2)
      }
      expect(SupportedLocaleSchema.safeParse(rand).success).toBe(false)
    }
  })
})
