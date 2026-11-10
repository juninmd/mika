import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import {
  DEFAULT_LLM_PROVIDER_ID,
  getLLMProviderEntry,
  LLM_PROVIDER_CATALOG,
  type LLMProviderId,
} from '@mika/sdk/llm/llm-catalog-data'

export type Step = 'language' | 'avatar' | 'llm' | 'tts' | 'stt' | 'embedding' | 'privacy' | 'identity' | 'done'

export const LLM_PROVIDERS = LLM_PROVIDER_CATALOG.map((provider) => ({
  id: provider.id,
  name: provider.label,
  description: provider.description,
  modelDefault: provider.defaultModel,
  models: provider.models,

  credentialConfigKey: provider.credentialConfigKey,
  credentialLabel: provider.credentialLabel,
  credentialPlaceholder: provider.credentialPlaceholder,
  credentialLink: provider.credentialLink,
}))

export const DEFAULT_WIZARD_PROVIDER_ID: LLMProviderId = DEFAULT_LLM_PROVIDER_ID
export type ProviderId = LLMProviderId
export type TtsId = 'none' | 'supertonic' | 'elevenlabs'
/** Valores de `voice.asrProvider` que o passo de escuta oferece (schema Rust `stt_fields`). */
export type SttId = 'nemotron' | 'electron' | 'browser' | 'groq-whisper' | 'openai-whisper'
export type LlmProvider = (typeof LLM_PROVIDERS)[number]

// Placar estático da tela: alimenta o `GlassSelect` só até a lista viva do Ollama (`useModelCatalog`)
// responder — sem rede, ou com o provedor ainda não instalado, é isto que o passo mostra.
export function wizardFallbackModels(provider: LlmProvider): LLMModelCatalogItem[] {
  return provider.models.map((model) => ({
    id: model.value,
    name: model.label,
    provider: provider.id,
    source: 'static' as const,
  }))
}

export function getWizardProvider(provider: ProviderId): LlmProvider {
  const fallback = getLLMProviderEntry(DEFAULT_LLM_PROVIDER_ID).id
  return (
    LLM_PROVIDERS.find((item) => item.id === provider) ??
    LLM_PROVIDERS.find((item) => item.id === fallback) ??
    LLM_PROVIDERS[0]
  )
}

/**
 * O modelo da tela tem de existir na lista que o usuário vê: com o Ollama no ar, a lista viva só traz
 * o que está instalado e o padrão de fábrica (`qwen3.5:4b`) pode não estar — o select ficava em
 * "Selecione…" e o passo salvava um modelo que o primeiro chat não achava. Não age durante o
 * recarregamento: o modelo que o guia acabou de baixar ainda não chegou na lista.
 */
export function reconcileModel(model: string, catalog: readonly { id: string }[], loading: boolean): string {
  if (loading || catalog.length === 0) return model
  return catalog.some((item) => item.id === model) ? model : catalog[0].id
}
