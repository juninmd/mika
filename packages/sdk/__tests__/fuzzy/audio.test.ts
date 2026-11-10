import { describe, expect, it } from 'vitest'
import { float32ArrayToInt16, float32ToInt16Sample } from '../../src/utils/audio'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0xa0d10)

describe('Audio Utilities - Fuzzy Tests', () => {
  it('float32ToInt16Sample should map correctly and clamp values', () => {
    for (let i = 0; i < 10000; i++) {
      // Generate random float between -2.0 and 2.0 to test clamping
      const floatVal = random() * 4 - 2
      const intVal = float32ToInt16Sample(floatVal)

      expect(Number.isInteger(intVal)).toBe(true)
      expect(intVal).toBeGreaterThanOrEqual(-32768)
      expect(intVal).toBeLessThanOrEqual(32767)

      if (floatVal >= 1) {
        expect(intVal).toBe(32767)
      } else if (floatVal <= -1) {
        expect(intVal).toBe(-32768)
      }
    }
  })

  it('float32ArrayToInt16 should process entire array correctly', () => {
    for (let i = 0; i < 100; i++) {
      const length = Math.floor(random() * 1000)
      const floatArray = new Float32Array(length)

      for (let j = 0; j < length; j++) {
        floatArray[j] = random() * 4 - 2
      }

      const intArray = float32ArrayToInt16(floatArray)

      expect(intArray.length).toBe(length)
      expect(intArray instanceof Int16Array).toBe(true)

      for (let j = 0; j < length; j++) {
        // Handle -0 properly by just ensuring it matches +0
        const expected = float32ToInt16Sample(floatArray[j] ?? 0)
        expect(intArray[j]).toBe(expected === 0 ? 0 : expected)
      }
    }
  })
})
