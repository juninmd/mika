export const OLLAMA_DEFAULT_BASE_URL = 'http://localhost:11434'

/**
 * Route by LATENCY instead of OpenRouter's default price/uptime balancing: measured first-token
 * times swung 1.7s-14.9s across endpoints for the same model/prompt size — unacceptable for voice.
 */
export const OPENROUTER_ROUTING = {
  sort: 'latency',
} as const

/** Unsloth Studio/Desktop expõe a API OpenAI-compatível sob `/v1`, tipicamente na porta 8888. */
export const UNSLOTH_DEFAULT_BASE_URL = 'http://localhost:8888/v1'
