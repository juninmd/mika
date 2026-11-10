import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import type { Translate } from '../../src/hooks/useI18n'

export type EmbeddingConfigInput = {
  embeddingProvider: string
  embeddingModel: string
  embeddingTaskType: string
  embeddingOutputDimensionality: number
}

export function buildEmbeddingConfig(input: EmbeddingConfigInput): Record<string, unknown> {
  return {
    'llm.embeddingProvider': input.embeddingProvider,
    'llm.embeddingModel': input.embeddingModel,
    'llm.embeddingTaskType': input.embeddingTaskType,
    'llm.embeddingOutputDimensionality': input.embeddingOutputDimensionality,
  }
}

export type ModelMeta = { id: string; dims: number; note?: string }

export const PROVIDERS = [
  { id: 'local', label: 'e5', desc: 'e5 no seu PC, offline' },
  { id: 'google', label: 'Google AI', desc: 'Gemini Embedding' },
  { id: 'openrouter', label: 'OpenRouter', desc: 'Catálogo universal' },
  { id: 'ollama', label: 'Ollama', desc: 'Modelos locais' },
  { id: 'openaiCompatible', label: 'OpenAI Compatible', desc: 'LiteLLM / OpenAI / host custom' },
  { id: 'unsloth', label: 'Unsloth', desc: 'GGUF de embedding local' },
] as const

/** Ollama e Unsloth rodam num servidor local à parte e levam sufixo; `local` não é marca, então vem do dicionário. */
export function providerDisplayLabel(id: string, label: string, t: Translate): string {
  if (id === 'local') return t('settings.embeddings.field.embeddingProvider.option.local')
  return id === 'ollama' || id === 'unsloth' ? `${label} (${t('settings.llm.provider.localSuffix')})` : label
}

// Revisado em 2026-09-28. Fontes: ai.google.dev/gemini-api/docs/embeddings (gemini-embedding-001
// aceita `taskType`; gemini-embedding-2 é multimodal, NÃO aceita `taskType` e tem espaço vetorial
// incompatível com o 001; text-embedding-004 foi desligado), openrouter.ai/<id> de cada entrada,
// ollama.com/library/embeddinggemma (300M, 768d, 622 MB), ollama.com/library/qwen3-embedding
// (0.6b, até 1024d), ollama.com/library/nomic-embed-text (768d, 274 MB).
export const FALLBACK_MODELS: Record<string, ModelMeta[]> = {
  google: [
    { id: 'gemini-embedding-001', dims: 3072, note: 'Recomendado' },
    { id: 'gemini-embedding-2', dims: 3072, note: 'Multimodal, sem taskType' },
  ],
  openrouter: [
    { id: 'google/gemini-embedding-001', dims: 3072, note: 'Recomendado' },
    { id: 'qwen/qwen3-embedding-8b', dims: 4096, note: 'Multilingual' },
    { id: 'openai/text-embedding-3-small', dims: 1536 },
    { id: 'openai/text-embedding-3-large', dims: 3072 },
  ],
  ollama: [
    { id: 'embeddinggemma', dims: 768, note: 'Recomendado' },
    { id: 'qwen3-embedding:0.6b', dims: 1024, note: 'Multilingual' },
    { id: 'nomic-embed-text', dims: 768, note: 'Leve' },
    { id: 'mxbai-embed-large', dims: 1024 },
  ],
  openaiCompatible: [
    { id: 'text-embedding-3-small', dims: 1536, note: 'Econômico' },
    { id: 'text-embedding-3-large', dims: 3072, note: 'Alta precisão' },
  ],
  // Sem lista embutida: o Unsloth serve o embedding GGUF que o usuário carregou (`GET /v1/models`).
  unsloth: [],
}

export function fallbackCatalog(provider: string): LLMModelCatalogItem[] {
  return (FALLBACK_MODELS[provider] ?? []).map((model) => ({
    id: model.id,
    name: model.note ? `${model.id} · ${model.dims}d · ${model.note}` : `${model.id} · ${model.dims}d`,
    provider: provider as LLMModelCatalogItem['provider'],
    source: 'static' as const,
  }))
}

export const DEFAULT_MODEL: Record<string, string> = {
  local: 'multilingual-e5-small',
  google: 'gemini-embedding-001',
  openrouter: 'google/gemini-embedding-001',
  ollama: 'embeddinggemma',
  openaiCompatible: 'text-embedding-3-small',
  unsloth: '',
}
