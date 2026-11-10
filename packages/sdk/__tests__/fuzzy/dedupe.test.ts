import { describe, expect, it } from 'vitest'
import { dedupeBy } from '../../src/utils/dedupe'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0xded0)

describe('Dedupe Utilities - Fuzzy Tests', () => {
  it('dedupeBy should remove duplicates and maintain order', () => {
    for (let i = 0; i < 100; i++) {
      const itemsCount = Math.floor(random() * 500)
      const items = Array.from({ length: itemsCount }, () => ({
        id: Math.floor(random() * 50), // High chance of duplicates
        value: random(),
      }))

      const result = dedupeBy(items, (item) => item.id)

      const seenIds = new Set<number>()
      for (const item of result) {
        expect(seenIds.has(item.id)).toBe(false)
        seenIds.add(item.id)
      }

      // Check if order is maintained (first occurrence kept)
      const expectedResult = []
      const expectedSeen = new Set<number>()
      for (const item of items) {
        if (!expectedSeen.has(item.id)) {
          expectedSeen.add(item.id)
          expectedResult.push(item)
        }
      }

      expect(result).toEqual(expectedResult)
    }
  })
})
