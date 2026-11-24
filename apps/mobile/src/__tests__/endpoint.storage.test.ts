import { describe, expect, it } from 'vitest'
import { DEFAULT_ENDPOINT, loadEndpoint, saveEndpoint } from '../endpoint.storage'

function memoryStore(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial))
  return { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v), data }
}

describe('endpoint storage', () => {
  it('never persists the API key, so a WebView storage dump does not leak it', () => {
    const store = memoryStore()
    saveEndpoint(store, { baseUrl: 'https://api.example/v1', model: 'm', apiKey: 'sk-secret' })
    expect([...store.data.values()].join()).not.toContain('sk-secret')
    expect(loadEndpoint(store)).toEqual({ baseUrl: 'https://api.example/v1', model: 'm', apiKey: '' })
  })

  it('drops a key written by an older build instead of sending it to the provider', () => {
    const store = memoryStore({ 'mika.mobile.endpoint': JSON.stringify({ apiKey: 'sk-old' }) })
    expect(loadEndpoint(store).apiKey).toBe('')
  })

  it('falls back to the emulator default when storage holds corrupt JSON', () => {
    expect(loadEndpoint(memoryStore({ 'mika.mobile.endpoint': '{' }))).toEqual(DEFAULT_ENDPOINT)
  })
})
