import { describe, expect, it } from 'vitest'
import { SUPPORTED_LOCALES } from '../../contracts'
import { loadLocale, translations } from '../index'

// One concept, one name: the bond level shown on Mood › Afinidade and the relationship status
// picked on Character › Status de Relacionamento are the same ladder. Mika is the female character,
// so "Amigo" on one screen and "Amiga" on the other is a defect in every gendered language.
const SHARED_LEVELS = [
  ['mood.affinity.level.known', 'settings.relationship.acquaintanceLabel'],
  ['mood.affinity.level.friend', 'settings.relationship.friendLabel'],
  ['mood.affinity.level.new', 'settings.mood.live.affinityNew'],
] as const

describe('affinity and relationship terms agree per locale', () => {
  for (const locale of SUPPORTED_LOCALES) {
    it(`${locale}: shared levels have one name`, async () => {
      await loadLocale(locale)
      const dict: Record<string, string | undefined> = translations[locale] ?? {}
      for (const [moodKey, otherKey] of SHARED_LEVELS) {
        expect(dict[moodKey], moodKey).toBeTruthy()
        expect(dict[moodKey], `${moodKey} vs ${otherKey}`).toBe(dict[otherKey])
      }
    })
  }
})
