import type { LLMProviderId } from '../../llm/llm-catalog-data'
import type { SettingsFieldOption } from '../config-ui'
import type { ToolErrorCode } from './tool-error-code'

export const EMBEDDING_TASK_TYPE_OPTIONS: SettingsFieldOption[] = [
  { label: 'Semantic Similarity', value: 'SEMANTIC_SIMILARITY' },
  { label: 'Classification', value: 'CLASSIFICATION' },
  { label: 'Clustering', value: 'CLUSTERING' },
  { label: 'Retrieval Document', value: 'RETRIEVAL_DOCUMENT' },
  { label: 'Retrieval Query', value: 'RETRIEVAL_QUERY' },
  { label: 'Question Answering', value: 'QUESTION_ANSWERING' },
  { label: 'Fact Verification', value: 'FACT_VERIFICATION' },
  { label: 'Code Retrieval Query', value: 'CODE_RETRIEVAL_QUERY' },
]

export interface UsageMetadata {
  promptTokens: number
  completionTokens: number
  totalTokens: number
  costUsd?: number

  cachedPromptTokens?: number

  contextWindow?: number
}

export interface TokenUsageStats {
  totalTokens: number
  totalPromptTokens: number
  totalCompletionTokens: number

  totalCachedPromptTokens?: number
  totalCostUsd: number

  unknownCostTurns?: number
  totalInteractions: number
  firstUsedAt: number
  lastUsedAt: number
}

export interface ChatAttachment {
  type: 'image'
  mimeType: string
  data: string
}

export type ChatMessageSource = 'voice' | 'text'

/** Regions with crisis resources in the chat card; must match `brain/crisis.rs::region` and the `chat.crisis.region.*` keys. */
export const CRISIS_REGIONS = ['br', 'us', 'eu', 'jp', 'cn', 'kr', 'ru'] as const
export type CrisisRegion = (typeof CRISIS_REGIONS)[number]

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant' | 'system' | 'tool'
  content: string
  timestamp: number
  attachments?: ChatAttachment[]
  toolCallId?: string
  toolName?: string
  usage?: UsageMetadata

  toolCalls?: Array<{ toolCallId: string; toolName: string; input: unknown }>

  /** How the user's message came in: spoken or typed. */
  source?: ChatMessageSource
}

export interface ToolResult<T = unknown> {
  success: boolean
  data?: T
  error?: string

  errorCode?: ToolErrorCode
}

export interface ConfigChangeEvent {
  path: string
  value: unknown
  previousValue?: unknown
  source: 'runtime' | 'desktop-shell' | 'renderer' | 'system'
  timestamp: number

  accepted?: boolean

  rejectedReason?: string
}

export type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue }

export interface ScoredVector {
  index: number
  id: string
  score: number
}

export interface ScreenSource {
  id: string
  name: string
  thumbnailDataUrl?: string
}

export type LLMModelProvider = LLMProviderId

export interface LLMModelCatalogItem {
  id: string
  name: string
  provider: LLMModelProvider
  source: 'static' | 'remote'
  vendor?: string
  description?: string
  contextWindow?: number
  promptPrice?: string
  completionPrice?: string
  inputCacheReadPrice?: string
  supportsTools?: boolean
  supportsVision?: boolean
  supportsImageGeneration?: boolean
  isFree?: boolean
  knowledgeCutoff?: string
  created?: number
  ended?: number
  maxCompletionTokens?: number
  inputModalities?: string[]
  outputModalities?: string[]
}
