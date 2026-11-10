export type AppStatus = 'idle' | 'listening' | 'thinking' | 'acting' | 'confirming' | 'speaking' | 'error' | 'offline'

/** Um passo do turno corrente: qual ferramenta a Mika chamou e como terminou. */
export type ThinkingStep = {
  id: string
  label: string
  status: 'pending' | 'active' | 'done' | 'error'
  detail?: string
}

export type ActiveToolState = {
  name: string
  startedAt: number
  finishedAt?: number
  durationMs?: number
  success?: boolean
} | null

/** Como a mensagem entrou — voz e texto convergem no mesmo `sendMessage`, o histórico precisa distinguir. */
export type MessageSource = 'voice' | 'text'

export type AppMessage = {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: number
  source?: MessageSource
}

export type HealthCapabilities = {
  hasLLM: boolean
  canSpeak: boolean
  canListenAudio: boolean
  canSeeScreen: boolean
  canUseTerminal: boolean
  canLearn: boolean
  plugins: number
  tools: number
}

export type SessionUsage = {
  promptTokens: number
  completionTokens: number
  totalTokens: number
  costUsd: number
  turns: number

  lastPromptTokens: number

  contextWindow: number

  unknownCostTurns: number
}

export const DEFAULT_SESSION_USAGE: SessionUsage = {
  promptTokens: 0,
  completionTokens: 0,
  totalTokens: 0,
  costUsd: 0,
  turns: 0,
  lastPromptTokens: 0,
  contextWindow: 0,
  unknownCostTurns: 0,
}

export const DEFAULT_CAPABILITIES: HealthCapabilities = {
  hasLLM: false,
  canSpeak: false,
  canListenAudio: false,
  canSeeScreen: false,
  canUseTerminal: false,
  canLearn: false,
  plugins: 0,
  tools: 0,
}
