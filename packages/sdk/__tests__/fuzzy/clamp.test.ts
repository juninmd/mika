import { describe, expect, it } from 'vitest'
import { bipolarToPercent, clamp, clamp01, clampAffinity, clampPercent, clampUnit } from '../../src/utils/clamp'
import { seeded } from './prng'

// Semeado: mesma semente, mesma entrada — uma falha do fuzz reproduz na máquina de quem a vê.
const random = seeded(0xc1a3)

function randomNumber(): number {
  // Generate random values including edge cases
  const edgeCases = [
    NaN,
    Infinity,
    -Infinity,
    0,
    -0,
    Number.MAX_VALUE,
    -Number.MAX_VALUE,
    Number.MIN_VALUE,
    -Number.MIN_VALUE,
  ]
  if (random() < 0.1) {
    return edgeCases[Math.floor(random() * edgeCases.length)]
  }
  return (random() - 0.5) * 10 ** (random() * 5)
}

describe('Clamp Utilities - Fuzzy Tests', () => {
  it('should always return a finite number within bounds or fallback when clamping random numbers', () => {
    for (let i = 0; i < 1000; i++) {
      const value = randomNumber()
      const min = random() * -100
      const max = random() * 100

      const result = clamp(value, min, max)
      expect(Number.isFinite(result)).toBe(true)
      expect(result).toBeGreaterThanOrEqual(min)
      expect(result).toBeLessThanOrEqual(max)
    }
  })

  it('clampPercent should keep values between 0 and 100', () => {
    for (let i = 0; i < 1000; i++) {
      const value = randomNumber()
      const result = clampPercent(value)
      expect(Number.isFinite(result)).toBe(true)
      expect(result).toBeGreaterThanOrEqual(0)
      expect(result).toBeLessThanOrEqual(100)
    }
  })

  it('clampAffinity should keep values between -100 and 100', () => {
    for (let i = 0; i < 1000; i++) {
      const value = randomNumber()
      const result = clampAffinity(value)
      expect(Number.isFinite(result)).toBe(true)
      expect(result).toBeGreaterThanOrEqual(-100)
      expect(result).toBeLessThanOrEqual(100)
    }
  })

  it('clampUnit should keep values between -1 and 1', () => {
    for (let i = 0; i < 1000; i++) {
      const value = randomNumber()
      const result = clampUnit(value)
      expect(Number.isFinite(result)).toBe(true)
      expect(result).toBeGreaterThanOrEqual(-1)
      expect(result).toBeLessThanOrEqual(1)
    }
  })

  it('clamp01 should keep values between 0 and 1', () => {
    for (let i = 0; i < 1000; i++) {
      const value = randomNumber()
      const result = clamp01(value)
      expect(Number.isFinite(result)).toBe(true)
      expect(result).toBeGreaterThanOrEqual(0)
      expect(result).toBeLessThanOrEqual(1)
    }
  })

  it('bipolarToPercent should keep values between 0 and 100', () => {
    for (let i = 0; i < 1000; i++) {
      const value = randomNumber()
      const result = bipolarToPercent(value)
      expect(Number.isFinite(result)).toBe(true)
      expect(result).toBeGreaterThanOrEqual(0)
      expect(result).toBeLessThanOrEqual(100)
    }
  })
})
