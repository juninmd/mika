import { describe, expect, it } from 'vitest'
import { DEFAULT_SLANG_DICTIONARY, SlangDictionarySchema } from '../../src/contracts/slang'

describe('Slang Contract - Fuzzy Tests', () => {
  it('DEFAULT_SLANG_DICTIONARY should match the schema perfectly', () => {
    const result = SlangDictionarySchema.safeParse(DEFAULT_SLANG_DICTIONARY)
    expect(result.success).toBe(true)
  })

  it('DEFAULT_SLANG_DICTIONARY entries should have valid shapes', () => {
    ;['technical', 'combat', 'idle'].forEach((mode) => {
      const entries = DEFAULT_SLANG_DICTIONARY[mode as keyof typeof DEFAULT_SLANG_DICTIONARY]
      entries.forEach((entry) => {
        expect(typeof entry.term).toBe('string')
        expect(entry.term.length).toBeGreaterThan(1)
        expect(typeof entry.meaning).toBe('string')
        expect(['cyberpunk', 'noir']).toContain(entry.register)
      })
    })
  })
})
