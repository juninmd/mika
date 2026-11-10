import { describe, expect, it } from 'vitest'
import { SUPPORTED_LOCALES } from '../../src/contracts'
import { loadLocale, translations } from '../../src/i18n/translations'

describe('Translations Load - Fuzzy Tests', () => {
  it('loadLocale should not crash on valid requests and return object', async () => {
    for (const locale of SUPPORTED_LOCALES) {
      // With cache
      const loadedCached = await loadLocale(locale)
      expect(typeof loadedCached).toBe('object')
      expect(loadedCached).toBeDefined()

      // Clear cache to force reload logic if applicable
      const original = translations[locale]
      translations[locale] = undefined

      const loaded = await loadLocale(locale)
      expect(typeof loaded).toBe('object')
      expect(loaded).toBeDefined()

      // Restore
      translations[locale] = original
    }
  })
})
