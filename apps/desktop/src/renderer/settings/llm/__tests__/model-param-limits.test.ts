import { describe, expect, it } from 'vitest'
import type { LlmModelProfile } from '../../../../shared/electron-api-system-types'
import { modelParamLimits } from '../model-param-limits'

const UNKNOWN: LlmModelProfile = {
  contextWindow: null,
  maxOutputTokens: null,
  reasoning: null,
  topP: null,
  thinkingBudgetMax: null,
}

describe('modelParamLimits', () => {
  // O turno descarta top_p/reasoning quando o perfil nega (provider_params.rs): mostrar o controle
  // nesses modelos é oferecer um ajuste que não chega ao modelo.
  it('hides top_p and thinking for a model whose provider says it does not accept them', () => {
    const limits = modelParamLimits('openrouter', { ...UNKNOWN, topP: false, reasoning: false })
    expect(limits.showTopP).toBe(false)
    expect(limits.showThinking).toBe(false)
    expect(limits.showThinkingBudget).toBe(false)
  })

  it('keeps every control when the provider did not answer — unknown is not unsupported', () => {
    const limits = modelParamLimits('ollama', UNKNOWN)
    expect(limits).toMatchObject({ showTopP: true, showThinking: true, maxTokensMax: 32768 })
    expect(limits.contextMax).toBeUndefined()
  })

  it('bounds output and thinking sliders by what the model accepts', () => {
    const flash = {
      contextWindow: 1048576,
      maxOutputTokens: 65536,
      reasoning: true,
      topP: true,
      thinkingBudgetMax: 24576,
    }
    const limits = modelParamLimits('google', flash)
    expect(limits).toMatchObject({
      maxTokensMax: 65536,
      contextMax: 1048576,
      showThinkingBudget: true,
      thinkingBudgetMax: 24576,
    })
  })

  // Ollama/llama.cpp só ligam ou desligam o raciocínio; o turno nunca envia orçamento a eles.
  it('offers a thinking budget only where the turn sends one', () => {
    const reasoning = { ...UNKNOWN, reasoning: true }
    expect(modelParamLimits('ollama', reasoning).showThinkingBudget).toBe(false)
    expect(modelParamLimits('openaiCompatible', reasoning).showThinkingBudget).toBe(false)
    expect(modelParamLimits('openrouter', UNKNOWN).showThinkingBudget).toBe(false)
  })

  // A janela só é escolha do app no Ollama (num_ctx); nos outros é fixa do modelo/servidor.
  it('lets the user pick the context window only on Ollama', () => {
    expect(modelParamLimits('ollama', { ...UNKNOWN, contextWindow: 40960 }).contextEditable).toBe(true)
    expect(modelParamLimits('openaiCompatible', { ...UNKNOWN, contextWindow: 16384 }).contextEditable).toBe(false)
  })
})
