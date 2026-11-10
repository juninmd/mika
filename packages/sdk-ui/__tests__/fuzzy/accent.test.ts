import { describe, expect, it } from 'vitest'
import { seeded } from '../../../sdk/__tests__/fuzzy/prng'
import { accentVars } from '../../src/utils/accent'

// Semente fixa: o fuzz roda igual em toda execução (uma falha sempre reproduz).
const rand = seeded(22)

describe('Accent Utilities - Fuzzy Tests', () => {
  it('accentVars should generate valid CSS properties for any random hue', () => {
    for (let i = 0; i < 1000; i++) {
      const hue = Math.floor(rand() * 360)
      const saturation = rand() > 0.5 ? Math.floor(rand() * 100) : undefined
      const lightness = rand() > 0.5 ? Math.floor(rand() * 100) : undefined

      const result = accentVars(hue, { saturation, lightness })

      expect(result).toBeDefined()
      if (result) {
        expect(result['--accent-h']).toBe(String(hue))

        const expectedS = `${saturation ?? 90}%`
        const expectedL = `${lightness ?? 65}%`

        expect(result['--accent-s']).toBe(expectedS)
        expect(result['--accent-l']).toBe(expectedL)
        expect(result['--accent']).toBe(`hsl(${hue} ${expectedS} ${expectedL})`)
        expect(result['--accent-text']).toBe(`hsl(${hue} ${expectedS} 78%)`)
        expect(result['--accent-soft']).toBe(`hsl(${hue} ${expectedS} ${expectedL} / 0.14)`)
        expect(result['--accent-rim']).toBe(`hsl(${hue} ${expectedS} ${expectedL} / 0.32)`)
        expect(result['--accent-glow']).toBe(`hsl(${hue} ${expectedS} ${expectedL} / 0.2)`)
      }
    }
  })

  it('accentVars should return undefined if hue is undefined', () => {
    expect(accentVars(undefined)).toBeUndefined()
  })
})
