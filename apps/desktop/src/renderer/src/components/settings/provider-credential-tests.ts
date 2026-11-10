import type { CredentialTestRequest } from '../../../../shared/electron-api-system-types'
import { getElectronApi } from '../../lib/shell-api'
import { getByPath } from './settings-utils'

/**
 * The real check behind each credential card's "Testar". LLM keys go through the same catalog call
 * the model picker uses (`mika:llm-models`, explicit key/URL so an unsaved-elsewhere value is what
 * gets tested), ElevenLabs through `mika:elevenlabs-voices`, and the rest through `credentials:test`
 * (one cheap authenticated GET in Rust, offline-gated, key never logged).
 */
export type CredentialTestKind =
  | 'google'
  | 'openrouter'
  | 'openaiCompatible'
  | 'unsloth'
  | 'ollama'
  | 'elevenLabs'
  | CredentialTestRequest['provider']

export type CredentialTestOutcome = {
  ok: boolean
  latencyMs?: number
  count?: number | null
  unit: 'models' | 'voices' | 'apis'
  error?: string
}

type Config = Record<string, unknown>

const str = (config: Config, path: string): string | undefined => {
  const value = getByPath(config, path)
  return typeof value === 'string' && value.trim() ? value.trim() : undefined
}

const LLM_ARGS: Record<string, (config: Config) => { apiKey?: string; baseUrl?: string }> = {
  google: (c) => ({ apiKey: str(c, 'apiKeys.googleAI') }),
  openrouter: (c) => ({ apiKey: str(c, 'apiKeys.openrouter') }),
  openaiCompatible: (c) => ({
    apiKey: str(c, 'apiKeys.openaiCompatibleApiKey'),
    baseUrl: str(c, 'apiKeys.openaiCompatibleBaseUrl'),
  }),
  unsloth: (c) => ({ apiKey: str(c, 'apiKeys.unslothApiKey'), baseUrl: str(c, 'apiKeys.unslothBaseUrl') }),
  ollama: (c) => ({ apiKey: str(c, 'apiKeys.ollamaApiKey'), baseUrl: str(c, 'apiKeys.ollamaBaseUrl') }),
}

async function timed<T>(run: () => Promise<T>): Promise<{ value: T; latencyMs: number }> {
  const started = performance.now()
  const value = await run()
  return { value, latencyMs: Math.round(performance.now() - started) }
}

async function testLlm(kind: string, config: Config): Promise<CredentialTestOutcome> {
  const args = LLM_ARGS[kind]?.(config) ?? {}
  const { value, latencyMs } = await timed(
    () =>
      getElectronApi().runtime.getLlmModels({ provider: kind, kind: 'chat', ...args }) as Promise<{
        models?: unknown[]
        error?: string
      }>,
  )
  if (value.error) return { ok: false, unit: 'models', latencyMs, error: value.error }
  return { ok: true, unit: 'models', latencyMs, count: value.models?.length ?? 0 }
}

async function testElevenLabs(config: Config): Promise<CredentialTestOutcome> {
  const apiKey = str(config, 'apiKeys.elevenLabs') ?? ''
  const { value, latencyMs } = await timed(() => getElectronApi().runtime.getElevenLabsVoices({ apiKey }))
  if (!value.ok) return { ok: false, unit: 'voices', latencyMs, error: value.error }
  return { ok: true, unit: 'voices', latencyMs, count: value.voices?.length ?? 0 }
}

const NATIVE_UNIT: Record<CredentialTestRequest['provider'], CredentialTestOutcome['unit']> = {
  groq: 'models',
  openaiStt: 'models',
  azure: 'voices',
  googleTts: 'voices',
  steam: 'apis',
}

export async function runCredentialTest(kind: CredentialTestKind, config: Config): Promise<CredentialTestOutcome> {
  if (kind in LLM_ARGS) return testLlm(kind, config)
  if (kind === 'elevenLabs') return testElevenLabs(config)
  const provider = kind as CredentialTestRequest['provider']
  const result = await getElectronApi().runtime.testCredential({ provider })
  return { ...result, unit: NATIVE_UNIT[provider] }
}
