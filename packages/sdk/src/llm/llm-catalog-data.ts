import type { SettingsFieldOption } from '../contracts'
import { ConfigKey } from '../contracts'
import { OLLAMA_DEFAULT_BASE_URL, UNSLOTH_DEFAULT_BASE_URL } from './llm-constants'

export const LLM_PROVIDER_IDS = ['google', 'openrouter', 'ollama', 'openaiCompatible', 'unsloth'] as const
export type LLMProviderId = (typeof LLM_PROVIDER_IDS)[number]

export const DEFAULT_LLM_PROVIDER_ID: LLMProviderId = 'ollama'

export const EMBEDDING_PROVIDER_IDS = [...LLM_PROVIDER_IDS] as const

export type LLMProviderCatalogEntry = {
  id: LLMProviderId
  label: string
  description: string

  credentialConfigKey?:
    | ConfigKey.GoogleAI
    | ConfigKey.OpenRouter
    | ConfigKey.OllamaApiKey
    | ConfigKey.OpenAiCompatibleApiKey
    | ConfigKey.UnslothApiKey

  baseUrlConfigKey?: ConfigKey.OllamaBaseUrl | ConfigKey.OpenAiCompatibleBaseUrl | ConfigKey.UnslothBaseUrl
  baseUrlPlaceholder?: string
  credentialLabel?: string
  credentialPlaceholder?: string
  credentialLink?: string
  defaultModel: string
  // Só o fallback offline do wizard: preço, contexto e capacidades vêm do catálogo vivo (`llm.rs`).
  models: readonly SettingsFieldOption[]
}

// Catálogo revisado em 2026-11-07 — cada id conferido na fonte oficial do dia (não inventar nome):
// - Gemini: ai.google.dev/gemini-api/docs/models + changelog (4 Argon GA em 2026-10-13 é o mais novo
//   de texto, mas é modelo de raciocínio de 1M de contexto: lento e caro demais para fala em tempo real;
//   o padrão é o 3.8 Flash, "Flash mais inteligente" e recomendado pela página para projetos novos),
//   3.5 Flash-Lite,
//   3.1 Flash-Lite com shutdown só em 2027-05-07). `gemini-2.5-*` saiu: 404 para contas novas desde
//   2026-07-09 e shutdown em 2026-10-16 (ai.google.dev/gemini-api/docs/deprecations); `gemini-1.5-*`
//   já não existe.
// - OpenRouter: openrouter.ai/api/v1/models de cada entrada (google/gemini-4-argon,
//   google/gemini-3.5-flash-lite, google/gemini-3.8-flash, openai/gpt-5.4-mini,
//   anthropic/claude-sonnet-5.5, anthropic/claude-haiku-5.5, qwen/qwen3.8-27b).
// - Ollama: ollama.com/library/qwen3.5 (tools, visão, thinking; 4b = 3.3 GB, 9b = 6.6 GB),
//   ollama.com/library/gemma4 (12b = 7.7 GB, visão), ollama.com/library/ministral-3 (8b = 6.0 GB).
// - Unsloth: unsloth.ai/docs/basics/api (API OpenAI-compatível em localhost:8888, chave sk-unsloth-…
//   criada em Settings → API; `GET /v1/models` lista o que está carregado, por isso sem modelo fixo).
// - OpenAI: developers.openai.com/api/docs/models/gpt-5.4-mini e /gpt-5.4-nano — esforço de
//   raciocínio padrão `none`, o único em que `temperature`/`top_p` são aceitos (o turno manda
//   `llm.temperature`); por isso não um modelo cujo padrão raciocina. `gpt-5.6-terra` (faixa mini) e
//   `gpt-5.6-luna` (faixa nano) existem (/api/docs/models/gpt-5.6-*), mas o esforço padrão deles é
//   `medium`: ficam FORA do catálogo até o turno poder mandar `reasoning_effort: none` ou omitir
//   `temperature` — oferecê-los hoje seria oferecer uma opção que tende a falhar.
export const LLM_PROVIDER_CATALOG: readonly LLMProviderCatalogEntry[] = [
  {
    id: 'google',
    label: 'Google AI (Gemini)',
    description: 'Modelos Gemini via AI SDK com foco em multimodal e baixa latencia.',
    credentialConfigKey: ConfigKey.GoogleAI,
    credentialLabel: 'Google AI API Key',
    credentialPlaceholder: 'AIzaSy...',
    credentialLink: 'https://aistudio.google.com/app/apikey',
    defaultModel: 'gemini-3.8-flash',
    models: [
      { label: 'Gemini 3.8 Flash', value: 'gemini-3.8-flash' },
      { label: 'Gemini 4 Argon', value: 'gemini-4-argon' },
      { label: 'Gemini 3.5 Flash-Lite', value: 'gemini-3.5-flash-lite' },
      { label: 'Gemini 3.1 Flash-Lite', value: 'gemini-3.1-flash-lite' },
    ],
  },
  {
    id: 'openrouter',
    label: 'OpenRouter',
    description: 'Catalogo multi-provider via OpenRouter, seguindo o fluxo do vtuber-ai.',
    credentialConfigKey: ConfigKey.OpenRouter,
    credentialLabel: 'OpenRouter API Key',
    credentialPlaceholder: 'sk-or-v1-...',
    credentialLink: 'https://openrouter.ai/keys',
    defaultModel: 'google/gemini-3.8-flash',
    models: [
      { label: 'Google Gemini 3.8 Flash', value: 'google/gemini-3.8-flash' },
      { label: 'Google Gemini 4 Argon', value: 'google/gemini-4-argon' },
      { label: 'Google Gemini 3.5 Flash-Lite', value: 'google/gemini-3.5-flash-lite' },
      { label: 'OpenAI GPT-5.4 Mini', value: 'openai/gpt-5.4-mini' },
      { label: 'Anthropic Claude Haiku 5.5', value: 'anthropic/claude-haiku-5.5' },
      { label: 'Anthropic Claude Sonnet 5.5', value: 'anthropic/claude-sonnet-5.5' },
      { label: 'Qwen3.8 27B', value: 'qwen/qwen3.8-27b' },
    ],
  },
  {
    id: 'ollama',
    label: 'Ollama (local)',
    description: 'Modelos locais servidos por Ollama para uso offline ou privado.',
    credentialConfigKey: ConfigKey.OllamaApiKey,
    baseUrlConfigKey: ConfigKey.OllamaBaseUrl,
    baseUrlPlaceholder: OLLAMA_DEFAULT_BASE_URL,
    credentialLabel: 'Ollama API Key (opcional)',
    credentialPlaceholder: 'deixe vazio para Ollama local',
    credentialLink: 'https://ollama.com',
    // 3.3 GB, cabe em GPU de 6 GB; tools + visão + thinking (a visão usa o mesmo modelo por padrão).
    defaultModel: 'qwen3.5:4b',
    models: [
      { label: 'Qwen3.5 4B · 3.3 GB', value: 'qwen3.5:4b' },
      { label: 'Qwen3.5 9B · 6.6 GB', value: 'qwen3.5:9b' },
      { label: 'Gemma 4 12B · 7.7 GB', value: 'gemma4:12b' },
      { label: 'Ministral 3 8B · 6.0 GB', value: 'ministral-3:8b' },
    ],
  },
  {
    id: 'openaiCompatible',
    label: 'OpenAI Compatible',
    description:
      'Qualquer endpoint que fale o protocolo da OpenAI: LiteLLM, vLLM, Ollama (/v1), a própria OpenAI, NVIDIA NIM.',
    credentialConfigKey: ConfigKey.OpenAiCompatibleApiKey,
    baseUrlConfigKey: ConfigKey.OpenAiCompatibleBaseUrl,
    credentialLabel: 'Authorization Key',
    credentialPlaceholder: 'sk-...',
    credentialLink: 'https://docs.litellm.ai/docs/proxy/quick_start',
    defaultModel: 'gpt-5.4-mini',
    models: [
      { label: 'GPT-5.4 Mini', value: 'gpt-5.4-mini' },
      { label: 'GPT-5.4 Nano', value: 'gpt-5.4-nano' },
    ],
  },
  {
    id: 'unsloth',
    label: 'Unsloth (local)',
    description:
      'Modelos GGUF servidos pelo Unsloth (Studio/Desktop) na sua máquina, via API compatível com OpenAI. Crie a chave em Settings → API.',
    credentialConfigKey: ConfigKey.UnslothApiKey,
    baseUrlConfigKey: ConfigKey.UnslothBaseUrl,
    baseUrlPlaceholder: UNSLOTH_DEFAULT_BASE_URL,
    credentialLabel: 'Unsloth API Key',
    credentialPlaceholder: 'sk-unsloth-...',
    credentialLink: 'https://unsloth.ai/docs/basics/api',
    // Sem modelo de fábrica: o id é o que o Unsloth tem carregado (`GET /v1/models`), e a lista vem
    // do catálogo vivo (`llm.rs`), nunca de um nome embutido.
    defaultModel: '',
    models: [],
  },
]

// Consultas puras sobre o catálogo. Moram aqui, e não em `llm-catalog.ts`, porque o renderer
// precisa delas e `llm-catalog.ts` arrasta `createLoggedFetch` — código de processo main.
export const LLM_PROVIDER_OPTIONS: readonly SettingsFieldOption[] = LLM_PROVIDER_CATALOG.map((provider) => ({
  label: provider.label,
  value: provider.id,
}))

export function isLLMProviderId(value: string | undefined): value is LLMProviderId {
  return Boolean(value) && LLM_PROVIDER_IDS.includes(value as LLMProviderId)
}

export function getLLMProviderEntry(provider: string | undefined): LLMProviderCatalogEntry {
  const byId = (id: string | undefined) => LLM_PROVIDER_CATALOG.find((entry) => entry.id === id)
  // Unknown ids fall back to the local provider: a cloud default would send the prompt off the machine.
  return byId(provider) ?? byId(DEFAULT_LLM_PROVIDER_ID) ?? LLM_PROVIDER_CATALOG[0]
}
