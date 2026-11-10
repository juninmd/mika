import { describe, expect, it } from 'vitest'
import { seeded } from '../../../sdk/__tests__/fuzzy/prng'
import { toDisplayError } from '../../src/utils/error-message'

// Semente fixa: o fuzz roda igual em toda execução (uma falha sempre reproduz).
const rand = seeded(33)

describe('toDisplayError - Fuzzy Tests', () => {
  it('should return error message for Error objects', () => {
    for (let i = 0; i < 1000; i++) {
      const randomMessage = rand().toString(36).substring(7)
      const err = new Error(randomMessage)
      const fallback = rand().toString(36).substring(7)

      expect(toDisplayError(err, fallback)).toBe(randomMessage)
      expect(toDisplayError(err)).toBe(randomMessage)
    }
  })

  it('should return fallback or undefined for values that carry no message', () => {
    const edgeCases = [
      null,
      undefined,
      123,
      '',
      { code: 'not a message' },
      [1, 2, 3],
      true,
      false,
      Symbol('test'),
      () => {},
    ]

    for (const testCase of edgeCases) {
      const fallback = rand().toString(36).substring(7)

      expect(toDisplayError(testCase, fallback)).toBe(fallback)
      expect(toDisplayError(testCase)).toBeUndefined()
    }
  })

  // biome-ignore lint/complexity/noExcessiveCognitiveComplexity: fuzzy testing requires multiple branches
  it('should handle completely random inputs', () => {
    for (let i = 0; i < 1000; i++) {
      let randomInput: unknown
      const roll = rand()

      if (roll < 0.2) randomInput = new Error(`Error ${i}`)
      else if (roll < 0.4) randomInput = null
      else if (roll < 0.6) randomInput = undefined
      else if (roll < 0.8) randomInput = `String error ${i}`
      else randomInput = { foo: 'bar' }
      const carriesMessage = randomInput instanceof Error || typeof randomInput === 'string'

      const fallback = `fallback ${i}`
      const resultWithFallback = toDisplayError(randomInput, fallback)
      const resultWithoutFallback = toDisplayError(randomInput)

      if (carriesMessage) {
        const message = randomInput instanceof Error ? randomInput.message : (randomInput as string)
        expect(resultWithFallback).toBe(message)
        expect(resultWithoutFallback).toBe(message)
      } else {
        expect(resultWithFallback).toBe(fallback)
        expect(resultWithoutFallback).toBeUndefined()
      }
    }
  })
})
