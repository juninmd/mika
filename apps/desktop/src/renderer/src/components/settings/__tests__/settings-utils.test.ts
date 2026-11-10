import { describe, expect, it } from 'vitest'
import {
  filterDeclarations,
  getByPath,
  getResolvedModel,
  groupDeclarations,
  setByPath,
  validateField,
} from '../settings-utils'

describe('settings-utils', () => {
  describe('getByPath', () => {
    it('retrieves nested value by dot path', () => {
      const obj = { a: { b: { c: 'deep' } } }
      expect(getByPath(obj, 'a.b.c')).toBe('deep')
    })

    it('returns undefined for missing path', () => {
      expect(getByPath({}, 'missing.path')).toBeUndefined()
    })
  })

  describe('setByPath', () => {
    it('sets nested value by dot path', () => {
      const obj = {}
      const result = setByPath(obj, 'a.b.c', 'value')
      expect((result as Record<string, unknown>).a).toEqual({ b: { c: 'value' } })
      expect((obj as Record<string, unknown>).a).toBeUndefined()
    })
  })

  describe('validateField', () => {
    it('validates number field', () => {
      const field = { configPath: 'test.path', type: 'number' as const, label: 'test' }
      expect(validateField(field, 42)).toBeNull()
      expect(validateField(field, 'abc')).toBe('Informe um número válido.')
    })

    it('validates number min', () => {
      const field = { configPath: 'test.path', type: 'number' as const, label: 'test', min: 10 }
      expect(validateField(field, 5)).toBe('Valor mínimo: 10.')
    })

    it('validates text minLength', () => {
      const field = { configPath: 'test.path', type: 'text' as const, label: 'test', minLength: 3 }
      expect(validateField(field, 'ab')).toBe('Mínimo de 3 caracteres.')
    })

    it('validates pattern', () => {
      const field = {
        configPath: 'test.path',
        type: 'text' as const,
        label: 'test',
        pattern: '^\\d{3}$',
        patternError: 'Must be 3 digits',
      }
      expect(validateField(field, 'abc')).toBe('Must be 3 digits')
      expect(validateField(field, '123')).toBeNull()
    })

    it('returns null for valid input', () => {
      const field = { configPath: 'test.path', type: 'text' as const, label: 'test' }
      expect(validateField(field, 'anything')).toBeNull()
    })
  })

  describe('filterDeclarations', () => {
    it('filters by tab label', () => {
      const decls = [
        { tabId: 'a', tabLabel: 'Alpha', tabGroup: 'G', groups: [] },
        { tabId: 'b', tabLabel: 'Beta', tabGroup: 'G', groups: [] },
      ]
      expect(filterDeclarations(decls as never, 'alpha')).toHaveLength(1)
    })

    it('returns all when search is empty', () => {
      const decls = [{ tabId: 'a', tabLabel: 'A', tabGroup: 'G', groups: [] }]
      expect(filterDeclarations(decls as never, '')).toHaveLength(1)
    })
  })

  describe('getResolvedModel', () => {
    it('reads the provider-specific model key, not the generic llm.model', () => {
      const config = { llm: { provider: 'openrouter', model: 'stale-model', openrouterModel: 'anthropic/claude' } }
      expect(getResolvedModel(config, 'openrouter')).toBe('anthropic/claude')
    })

    it('falls back to llm.model when the provider-specific key is empty', () => {
      const config = { llm: { provider: 'google', model: 'gemini-fallback' } }
      expect(getResolvedModel(config, 'google')).toBe('gemini-fallback')
    })

    it('reads the unsloth slot and never borrows llm.model, which belongs to Ollama', () => {
      const loaded = { llm: { model: 'qwen3.5:4b', unslothModel: 'gemma-4-26B-A4B-it-GGUF' } }
      expect(getResolvedModel(loaded, 'unsloth')).toBe('gemma-4-26B-A4B-it-GGUF')
      expect(getResolvedModel({ llm: { model: 'qwen3.5:4b' } }, 'unsloth')).toBe('')
    })

    it('falls back to llm.model for an unknown provider', () => {
      const config = { llm: { model: 'generic-model' } }
      expect(getResolvedModel(config, 'unknown')).toBe('generic-model')
    })
  })

  describe('groupDeclarations', () => {
    it('groups by tabGroup', () => {
      const decls = [
        { tabId: 'a', tabLabel: 'A', tabGroup: 'General', groups: [] },
        { tabId: 'b', tabLabel: 'B', tabGroup: 'Advanced', groups: [] },
      ]
      const grouped = groupDeclarations(decls as never)
      expect(Object.keys(grouped)).toHaveLength(2)
      expect(grouped.General).toHaveLength(1)
    })
  })
})
