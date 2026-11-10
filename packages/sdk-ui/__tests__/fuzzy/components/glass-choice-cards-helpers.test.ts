import { describe, expect, it } from 'vitest'
import { seeded } from '../../../../sdk/__tests__/fuzzy/prng'
import {
  choiceFocusIndex,
  choiceGridCols,
  choiceTargetIndex,
  isDenseChoice,
} from '../../../src/components/glass-choice-cards-helpers'

const rand = seeded(1234)

describe('Glass Choice Cards Helpers - Fuzzy Tests', () => {
  it('choiceTargetIndex should correctly navigate indexes without throwing errors', () => {
    const keys = ['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'Enter', 'Escape', 'A', ' ']

    for (let i = 0; i < 1000; i++) {
      const count = Math.floor(rand() * 10) + 1 // 1 to 10 options
      const currentIndex = Math.floor(rand() * count)
      const key = keys[Math.floor(rand() * keys.length)] as string

      const nextIndex = choiceTargetIndex(key, currentIndex, count)

      if (['Enter', 'Escape', 'A', ' '].includes(key)) {
        expect(nextIndex).toBeNull()
      } else {
        expect(nextIndex).toBeGreaterThanOrEqual(0)
        expect(nextIndex).toBeLessThan(count)
      }
    }
  })

  it('choiceFocusIndex should return 0 if no match, or the matched index', () => {
    for (let i = 0; i < 500; i++) {
      const optionsCount = Math.floor(rand() * 10) + 1
      const options = Array.from({ length: optionsCount }, (_, i) => ({ value: `val-${i}` }))

      const targetIndex = rand() > 0.5 ? Math.floor(rand() * optionsCount) : -1
      const searchVal = targetIndex >= 0 ? `val-${targetIndex}` : 'not-found'

      const focusIndex = choiceFocusIndex(options, searchVal)

      expect(focusIndex).toBeGreaterThanOrEqual(0)
      expect(focusIndex).toBeLessThan(optionsCount)

      if (targetIndex >= 0) {
        expect(focusIndex).toBe(targetIndex)
      } else {
        expect(focusIndex).toBe(0)
      }
    }
  })

  it('isDenseChoice should return true only if NO option has a description', () => {
    for (let i = 0; i < 500; i++) {
      const optionsCount = Math.floor(rand() * 10) + 1
      const withDescriptions = rand() > 0.5

      const options = Array.from({ length: optionsCount }, () => ({
        description: withDescriptions && rand() > 0.5 ? 'desc' : undefined,
      }))

      // Force at least one description if `withDescriptions` is true to guarantee behavior
      if (withDescriptions) {
        const target = options[Math.floor(rand() * optionsCount)]
        if (target) target.description = 'desc'
      }

      const result = isDenseChoice(options)

      expect(result).toBe(!withDescriptions)
    }
  })

  it('choiceGridCols should return valid tailwind grid classes', () => {
    for (let i = 0; i < 500; i++) {
      const count = Math.floor(rand() * 20)
      const dense = rand() > 0.5

      const result = choiceGridCols(count, dense)

      expect(typeof result).toBe('string')
      expect(result).toContain('grid-cols')
    }
  })
})
