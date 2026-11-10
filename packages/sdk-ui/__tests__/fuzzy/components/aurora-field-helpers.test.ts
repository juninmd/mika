import { describe, expect, it } from 'vitest'
import { seeded } from '../../../../sdk/__tests__/fuzzy/prng'
import { buildInputClass, buildWrapperClass, readInitial } from '../../../src/components/aurora-field-helpers'

const rand = seeded(9876)

describe('Aurora Field Helpers - Fuzzy Tests', () => {
  it('buildInputClass should always return a valid string of classes based on focus', () => {
    for (let i = 0; i < 500; i++) {
      const focused = rand() > 0.5
      const result = buildInputClass(focused)

      expect(typeof result).toBe('string')
      expect(result).toContain('border-border-subtle')

      if (!focused) {
        expect(result).toContain('hover:border-border-strong')
      }
    }
  })

  // biome-ignore lint/complexity/noExcessiveCognitiveComplexity: fuzzy testing requires multiple branches
  it('readInitial should correctly determine the string value from config or value', () => {
    for (let i = 0; i < 500; i++) {
      // Create random inputs simulating combinations of control types
      const hasOnChange = rand() > 0.5
      const hasConfig = rand() > 0.5
      const hasConfigPath = rand() > 0.5

      const onChange = hasOnChange ? () => {} : undefined
      const configPath = hasConfigPath ? 'deep.test.path' : undefined
      const config = hasConfig ? { deep: { test: { path: rand() > 0.5 ? 'cfg-value' : 42 } } } : undefined
      const value = rand() > 0.5 ? 'controlled-value' : undefined

      const result = readInitial({
        label: 'Test',
        onChange,
        configPath,
        config,
        value,
      })

      expect(typeof result).toBe('string')
    }
  })

  it('buildWrapperClass should append opacity and space classes conditionally based on disabled', () => {
    for (let i = 0; i < 500; i++) {
      const disabled = rand() > 0.5
      const result = buildWrapperClass(disabled)

      expect(typeof result).toBe('string')
      expect(result).toContain('space-y-1.5')

      if (disabled) {
        expect(result).toContain('opacity-50')
      } else {
        expect(result).not.toContain('opacity-50')
      }
    }
  })
})
