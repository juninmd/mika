import type { LegalDocumentId } from '@mika/sdk/contracts'
import type { LogItem } from '@mika/sdk/core/logger-format'

/** Payload of the `llmGetProviderHealth` route and `LlmProviderHealth` event (Rust `modules/llm.rs`). */
export interface LLMProviderHealth {
  key: string
  provider: string
  model: string
  state: 'healthy' | 'degraded' | 'disabled'
  reason?: string
  statusCode?: number
  since: number
  failureCount: number
  fatal: boolean
  /** Superfícies que já reportaram falha para este par (`"chat"`, `"vision"`, …) — WP9 item 6, para
   * o banner distinguir uma falha só-de-visão de uma falha de chat no mesmo `provider:model`. */
  surfaces: string[]
}

/** `quota` of the `llmGetOpenRouterQuota` route: OpenRouter's `/api/v1/key`, camelCased. */
export interface OpenRouterKeyStatus {
  label: string
  limit: number | null
  limitRemaining: number | null
  limitReset: string | null
  usage: number
  usageDaily: number
  usageMonthly: number
  isFreeTier: boolean
  exhausted: boolean
}

/** Response of `llmGetModelProfile` (Rust `modules/llm/profile_route.rs`); `null` = the provider did not say. */
export interface LlmModelProfile {
  contextWindow: number | null
  maxOutputTokens: number | null
  reasoning: boolean | null
  topP: boolean | null
  thinkingBudgetMax: number | null
}

/** One latency series of `LlmStatus.timings` (ms; `null` until a turn measured it). */
export interface LlmTimingSeries {
  last: number | null
  median: number | null
  count: number
}

/** Response of `llmGetStatus` and payload of the `LlmStatus` event (Rust `modules/llm/status.rs`). */
export interface LlmStatus {
  provider: string
  ready: boolean
  ollama: {
    endpoint?: string
    reachable: boolean
    version?: string | null
    installed: {
      name: string
      sizeBytes: number | null
      family: string
      parameterSize?: string | null
      quantization?: string | null
      embedding: boolean
    }[]
    loaded: { name: string; sizeBytes: number | null; sizeVramBytes: number | null; expiresAt?: string | null }[]
    error?: string | null
  }
  localEmbedding: { model: string; dimensions: number; downloaded: boolean; sizeBytes: number | null }
  pullingModel: string | null
  timings: {
    samples: { atMs: number; firstTokenMs: number | null; firstAudioMs: number | null; totalMs: number | null }[]
    firstToken: LlmTimingSeries
    firstAudio: LlmTimingSeries
    total: LlmTimingSeries
  }
}

/** Payload of the `LlmPullProgress` event (Rust `modules/llm.rs::pull_model_with_progress`). */
export interface LlmPullProgress {
  model: string
  status?: string
  completed?: number | null
  total?: number | null
  done: boolean
  ok?: boolean
  error?: string | null
}

export type SystemRuntimeApi = {
  checkForUpdates: () => Promise<{
    success: boolean
    updateAvailable: boolean
    version?: string
    /** Changelog da release (markdown), montado no CI a partir dos commits convencionais. */
    notes?: string
    error?: string
  }>
  downloadUpdate: () => Promise<{ success: boolean; error?: string }>
  quitAndInstall: () => Promise<{ success: boolean; error?: string }>
  getUpdateStatus: () => Promise<{ available: boolean; version?: string; progress?: number }>
  getLegalDocument: (doc: LegalDocumentId) => Promise<{ doc: LegalDocumentId; text: string; version: string }>
  getTelemetryStatus: () => Promise<{ enabled: boolean; optInShown: boolean; sessionId: string }>
  setTelemetryOptIn: (payload: { enabled: boolean; optInShown: boolean }) => Promise<{ success: boolean }>
  getTokenUsage: () => Promise<{
    totalTokens: number
    totalPromptTokens: number
    totalCompletionTokens: number
    totalCostUsd: number
    totalInteractions: number
    firstUsedAt: number
    lastUsedAt: number
  }>
  resetTokenUsage: () => Promise<{ success: boolean }>
  onToolApprovalRequest: (
    callback: (payload: { toolCallId: string; toolName: string; args: Record<string, unknown> }) => void,
  ) => () => void
  sendToolApprovalResponse: (toolCallId: string, approved: boolean, scope?: 'once' | 'session' | 'always') => void

  log: (entry: RendererLogEntry) => Promise<boolean>
  getRecentLogs: () => Promise<{ logs: LogItem[] }>
  onLog: (callback: (log: LogItem) => void) => () => void
  getProviderHealth: () => Promise<{ health: LLMProviderHealth[] }>
  resetProviderCircuit: (provider: string, model?: string) => Promise<{ ok: boolean }>
  onProviderHealth: (callback: (health: LLMProviderHealth) => void) => () => void

  getOpenRouterQuota: () => Promise<{ ok: boolean; quota?: OpenRouterKeyStatus; error?: string }>
  getModelProfile: (provider: string, model: string) => Promise<LlmModelProfile>
  testCredential: (payload: CredentialTestRequest) => Promise<CredentialTestResult>
  getLlmStatus: () => Promise<LlmStatus>
  onLlmStatus: (callback: (status: LlmStatus) => void) => () => void
  pullLlmModel: (model: string) => Promise<{ ok: boolean; error?: string | null }>
  cancelLlmPull: () => Promise<{ ok: boolean }>
  onLlmPullProgress: (callback: (progress: LlmPullProgress) => void) => () => void
}

export interface RendererLogEntry {
  level: 'debug' | 'info' | 'warn' | 'error'
  scope: string
  msg: string
  meta?: Record<string, unknown>
}

export type CredentialTestRequest = {
  provider: 'groq' | 'openaiStt' | 'azure' | 'googleTts' | 'steam'
  apiKey?: string
  region?: string
}

export type CredentialTestResult = {
  ok: boolean
  status?: number
  latencyMs?: number
  count?: number | null
  error?: string
}
