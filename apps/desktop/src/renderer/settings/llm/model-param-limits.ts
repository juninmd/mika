import type { LlmModelProfile } from '../../../shared/electron-api-system-types'

/** Teto do slider de saída quando o provedor não informou o do modelo (o backend ainda corta). */
const UNKNOWN_MAX_OUTPUT = 32768
const UNKNOWN_THINKING_BUDGET = 100000
/** Só Google e OpenRouter recebem orçamento de raciocínio; Ollama/llama.cpp só ligam ou desligam. */
const THINKING_BUDGET_PROVIDERS = new Set(['google', 'openrouter'])

export type ModelParamLimits = {
  showTopP: boolean
  showThinking: boolean
  showThinkingBudget: boolean
  thinkingBudgetMax: number
  maxTokensMax: number
  contextMax: number | undefined
  contextEditable: boolean
}

/** Mesmas regras de `provider_params.rs`: `false` do perfil esconde, `null` (não informado) mantém. */
export function modelParamLimits(provider: string, profile: LlmModelProfile): ModelParamLimits {
  return {
    showTopP: profile.topP !== false,
    showThinking: profile.reasoning !== false,
    showThinkingBudget: profile.reasoning === true && THINKING_BUDGET_PROVIDERS.has(provider),
    thinkingBudgetMax: profile.thinkingBudgetMax ?? UNKNOWN_THINKING_BUDGET,
    maxTokensMax: profile.maxOutputTokens ?? UNKNOWN_MAX_OUTPUT,
    contextMax: profile.contextWindow ?? undefined,
    contextEditable: provider === 'ollama',
  }
}
