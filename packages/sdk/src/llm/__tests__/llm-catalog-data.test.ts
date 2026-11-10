import { describe, expect, it } from 'vitest'
import { EMBEDDING_PROVIDER_IDS, LLM_PROVIDER_CATALOG, LLM_PROVIDER_IDS } from '../llm-catalog-data'

describe('llm-catalog-data - Fuzzy / property-based', () => {
  it('LLM_PROVIDER_CATALOG maintains required invariants for all entries', () => {
    for (let i = 0; i < 50; i++) {
      const entry = LLM_PROVIDER_CATALOG[Math.floor(Math.random() * LLM_PROVIDER_CATALOG.length)]
      expect(entry.id).toBeDefined()
      expect(typeof entry.id).toBe('string')
      expect(entry.label.length).toBeGreaterThan(0)
      expect(Array.isArray(entry.models)).toBe(true)
      expect(entry.defaultModel).toBeDefined()

      if (entry.models.length > 0) {
        const randomModel = entry.models[Math.floor(Math.random() * entry.models.length)]
        expect(randomModel.label).toBeDefined()
        expect(randomModel.value).toBeDefined()
      }
    }
  })

  it('LLM_PROVIDER_IDS contains expected constants', () => {
    expect(LLM_PROVIDER_IDS.length).toBeGreaterThan(0)
    for (const id of LLM_PROVIDER_IDS) {
      expect(typeof id).toBe('string')
    }
  })

  it('EMBEDDING_PROVIDER_IDS matches LLM_PROVIDER_IDS', () => {
    expect([...EMBEDDING_PROVIDER_IDS]).toEqual([...LLM_PROVIDER_IDS])
  })
})
