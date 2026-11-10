import type { Translate } from '../../src/hooks/useI18n'

export const PROVIDERS = [
  { id: 'google', label: 'Google AI', desc: 'Gemini 3.5 Flash-Lite · 3.8 Flash' },
  { id: 'openrouter', label: 'OpenRouter', desc: 'Catálogo universal' },
  { id: 'ollama', label: 'Ollama', desc: 'Modelos locais' },
  { id: 'openaiCompatible', label: 'OpenAI Compatible', desc: 'LiteLLM, NVIDIA, proxies' },
  { id: 'unsloth', label: 'Unsloth', desc: 'GGUF locais via API OpenAI' },
] as const

/** Ollama e Unsloth rodam na máquina do usuário e levam o sufixo; os demais são serviços remotos. */
export function providerDisplayLabel(id: string, label: string, t: Translate): string {
  return id === 'ollama' || id === 'unsloth' ? `${label} (${t('settings.llm.provider.localSuffix')})` : label
}
export type LlmConnectionProps = {
  provider: string
  googleModel: string
  openrouterModel: string
  ollamaModel: string
  openaiCompatibleModel: string
  unslothModel: string
  temperature: number
  topP: number
  maxTokens: number
  historyLimit: number
  maxToolSteps: number
  contextWindow: number
  enableMonologue: boolean
  enableSummarization: boolean
  matchUserLanguage: boolean
  enableThinking: boolean
  thinkingMaxTokens: number
  systemPrompt: string
  onSave: (path: string, value: unknown) => Promise<void>
}
