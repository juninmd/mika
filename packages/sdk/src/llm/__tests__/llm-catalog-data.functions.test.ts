import { describe, expect, it } from 'vitest'
import { ConfigKey } from '../../contracts'
import {
  DEFAULT_LLM_PROVIDER_ID,
  getLLMProviderEntry,
  isLLMProviderId,
  LLM_PROVIDER_CATALOG,
  LLM_PROVIDER_OPTIONS,
} from '../llm-catalog-data'

// `getLLMProviderEntry` is a privacy fallback, not just a lookup: an unknown id must resolve to
// the LOCAL provider (ollama), never to a cloud default that would send the prompt off-machine.
describe('isLLMProviderId', () => {
  it('returns true for every catalogued id', () => {
    for (const entry of LLM_PROVIDER_CATALOG) {
      expect(isLLMProviderId(entry.id)).toBe(true)
    }
  })

  it('returns false for an unknown id', () => {
    expect(isLLMProviderId('not-a-provider')).toBe(false)
  })

  it('returns false for undefined', () => {
    expect(isLLMProviderId(undefined)).toBe(false)
  })

  it('returns false for an empty string', () => {
    expect(isLLMProviderId('')).toBe(false)
  })
})

describe('getLLMProviderEntry', () => {
  it('resolves a known id to its catalog entry', () => {
    expect(getLLMProviderEntry('google').id).toBe('google')
  })

  it('falls back to the default (ollama) provider for an unknown id', () => {
    expect(getLLMProviderEntry('totally-unknown').id).toBe(DEFAULT_LLM_PROVIDER_ID)
  })

  it('falls back to the default provider for undefined', () => {
    expect(getLLMProviderEntry(undefined).id).toBe(DEFAULT_LLM_PROVIDER_ID)
  })

  it('the default provider itself is local (ollama) — a privacy invariant, not incidental', () => {
    expect(DEFAULT_LLM_PROVIDER_ID).toBe('ollama')
  })
})

describe('LLM_PROVIDER_OPTIONS', () => {
  it('mirrors the catalog 1:1 as {label, value} pairs', () => {
    expect(LLM_PROVIDER_OPTIONS).toHaveLength(LLM_PROVIDER_CATALOG.length)
    for (const [i, option] of LLM_PROVIDER_OPTIONS.entries()) {
      expect(option.value).toBe(LLM_PROVIDER_CATALOG[i].id)
      expect(option.label).toBe(LLM_PROVIDER_CATALOG[i].label)
    }
  })
})

// `unsloth` shares the OpenAI-compatible resolver but is NOT the same provider: own address, own key,
// own model slot. Sharing any of the three would send one server's credential to the other.
describe('unsloth provider entry', () => {
  const unsloth = getLLMProviderEntry('unsloth')
  const compatible = getLLMProviderEntry('openaiCompatible')

  it('is a catalogued provider with the documented local default address', () => {
    expect(isLLMProviderId('unsloth')).toBe(true)
    expect(unsloth.id).toBe('unsloth')
    expect(unsloth.baseUrlPlaceholder).toBe('http://localhost:8888/v1')
  })

  it('has its own credential and base URL keys, never the openaiCompatible ones', () => {
    expect(unsloth.credentialConfigKey).toBe(ConfigKey.UnslothApiKey)
    expect(unsloth.baseUrlConfigKey).toBe(ConfigKey.UnslothBaseUrl)
    expect(unsloth.credentialConfigKey).not.toBe(compatible.credentialConfigKey)
    expect(unsloth.baseUrlConfigKey).not.toBe(compatible.baseUrlConfigKey)
  })

  it('ships no invented model: the id is whatever the server has loaded', () => {
    expect(unsloth.defaultModel).toBe('')
    expect(unsloth.models).toHaveLength(0)
  })
})
