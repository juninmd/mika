import type { LlmEndpoint } from './chat.service'

const STORAGE_KEY = 'mika.mobile.endpoint'
// 10.0.2.2 is the host machine as seen from the Android emulator (Ollama on the PC).
export const DEFAULT_ENDPOINT: LlmEndpoint = { baseUrl: 'http://10.0.2.2:11434/v1', model: 'qwen2.5:1.5b', apiKey: '' }

type KeyValueStore = Pick<Storage, 'getItem' | 'setItem'>

export function loadEndpoint(store: KeyValueStore): LlmEndpoint {
  try {
    const saved = JSON.parse(store.getItem(STORAGE_KEY) ?? '{}') as Partial<LlmEndpoint>
    // The API key is never read back from WebView storage, even if an older build wrote it.
    return { ...DEFAULT_ENDPOINT, ...saved, apiKey: '' }
  } catch {
    return DEFAULT_ENDPOINT
  }
}

export function saveEndpoint(store: KeyValueStore, value: LlmEndpoint): void {
  try {
    store.setItem(STORAGE_KEY, JSON.stringify({ baseUrl: value.baseUrl, model: value.model }))
  } catch {
    // Storage blocked: settings stay for this session only.
  }
}
