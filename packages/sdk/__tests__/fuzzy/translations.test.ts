import { describe, expect, it } from 'vitest'
import { SUPPORTED_LOCALES } from '../../src/contracts'
import { isLocaleLoaded, translations } from '../../src/i18n/translations'
import { seeded } from './prng'

const rand = seeded(0x4d494b41)

describe('Translations - Fuzzy Tests', () => {
  it('isLocaleLoaded should return correct status for randomly manipulated states', () => {
    for (let i = 0; i < 500; i++) {
      const locale = SUPPORTED_LOCALES[Math.floor(rand() * SUPPORTED_LOCALES.length)] as keyof typeof translations
      const isLoaded = rand() > 0.5

      const original = translations[locale]
      if (isLoaded) {
        // biome-ignore lint/suspicious/noExplicitAny: fuzzy testing arbitrary inputs
        translations[locale] = {} as any
        expect(isLocaleLoaded(locale)).toBe(true)
      } else {
        translations[locale] = undefined
        expect(isLocaleLoaded(locale)).toBe(false)
      }

      // Restore
      translations[locale] = original
    }
  })
})
