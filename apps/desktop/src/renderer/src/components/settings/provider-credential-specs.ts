import type { TranslationDict } from '@mika/sdk/i18n'
import type { CredentialTestKind } from './provider-credential-tests'

export type TKey = keyof TranslationDict

export type CredentialField = {
  label: string
  configPath: string
  placeholder: string
  type: 'password' | 'text' | 'url'
  accentHue: number
  description?: string
  // Expected key prefix (only for providers with a confirmed, stable format); warning is visual-only, never blocks save.
  expectedPrefix?: string
  test?: CredentialTestKind
  keyUrl?: string
}

export type CredentialFieldSpec = {
  labelKey: TKey
  configPath: string
  placeholderKey?: TKey
  placeholder?: string
  type: 'password' | 'text' | 'url'
  accentHue: number
  descriptionKey?: TKey
  expectedPrefix?: string
  /** Which real check the card's "Testar" runs — see `provider-credential-tests.ts`. */
  test?: CredentialTestKind
  /** Where to create the key; must be a host in `shell_actions::EXTERNAL_HOSTS` (Rust). */
  keyUrl?: string
}

export const LLM_FIELD_SPECS: CredentialFieldSpec[] = [
  {
    labelKey: 'settings.providerCredentials.field.googleAI.label',
    configPath: 'apiKeys.googleAI',
    test: 'google',
    keyUrl: 'https://aistudio.google.com/apikey',
    placeholder: 'AIza...',
    type: 'password',
    accentHue: 210,
    descriptionKey: 'settings.providerCredentials.field.googleAI.description',
    expectedPrefix: 'AIza',
  },
  {
    labelKey: 'settings.providerCredentials.field.openrouter.label',
    configPath: 'apiKeys.openrouter',
    test: 'openrouter',
    keyUrl: 'https://openrouter.ai/settings/keys',
    placeholder: 'sk-or-v1-...',
    type: 'password',
    accentHue: 38,
    descriptionKey: 'settings.providerCredentials.field.openrouter.description',
    expectedPrefix: 'sk-or-v1-',
  },
  {
    labelKey: 'settings.providerCredentials.field.openaiCompatibleApiKey.label',
    configPath: 'apiKeys.openaiCompatibleApiKey',
    test: 'openaiCompatible',
    placeholder: 'sk-...',
    type: 'password',
    accentHue: 300,
    descriptionKey: 'settings.providerCredentials.field.openaiCompatibleApiKey.description',
  },
  {
    labelKey: 'settings.providerCredentials.field.unslothApiKey.label',
    configPath: 'apiKeys.unslothApiKey',
    test: 'unsloth',
    keyUrl: 'https://unsloth.ai/docs/basics/api',
    placeholder: 'sk-unsloth-...',
    type: 'password',
    accentHue: 140,
    descriptionKey: 'settings.providerCredentials.field.unslothApiKey.description',
    expectedPrefix: 'sk-unsloth-',
  },
  {
    labelKey: 'settings.providerCredentials.field.ollamaBaseUrl.label',
    configPath: 'apiKeys.ollamaBaseUrl',
    test: 'ollama',
    placeholder: 'http://localhost:11434',
    type: 'url',
    accentHue: 155,
    descriptionKey: 'settings.providerCredentials.field.ollamaBaseUrl.description',
  },
  {
    labelKey: 'settings.providerCredentials.field.ollamaApiKey.label',
    configPath: 'apiKeys.ollamaApiKey',
    keyUrl: 'https://ollama.com/settings/keys',
    placeholderKey: 'settings.providerCredentials.field.ollamaApiKey.placeholder',
    type: 'password',
    accentHue: 155,
    descriptionKey: 'settings.providerCredentials.field.ollamaApiKey.description',
  },
  {
    labelKey: 'settings.providerCredentials.field.openaiCompatibleBaseUrl.label',
    configPath: 'apiKeys.openaiCompatibleBaseUrl',
    placeholder: 'http://localhost:4000/v1',
    type: 'url',
    accentHue: 300,
    descriptionKey: 'settings.providerCredentials.field.openaiCompatibleBaseUrl.description',
  },
  {
    labelKey: 'settings.providerCredentials.field.unslothBaseUrl.label',
    configPath: 'apiKeys.unslothBaseUrl',
    placeholder: 'http://localhost:8888/v1',
    type: 'url',
    accentHue: 140,
    descriptionKey: 'settings.providerCredentials.field.unslothBaseUrl.description',
  },
]

export const TTS_FIELD_SPECS: CredentialFieldSpec[] = [
  {
    labelKey: 'settings.providerCredentials.field.elevenLabs.label',
    configPath: 'apiKeys.elevenLabs',
    test: 'elevenLabs',
    keyUrl: 'https://elevenlabs.io/app/settings/api-keys',
    placeholderKey: 'settings.providerCredentials.field.elevenLabs.placeholder',
    type: 'password',
    accentHue: 300,
  },
  {
    labelKey: 'settings.providerCredentials.field.azureTtsSubscriptionKey.label',
    configPath: 'voice.azureTtsSubscriptionKey',
    test: 'azure',
    keyUrl: 'https://portal.azure.com/#create/Microsoft.CognitiveServicesSpeechServices',
    placeholderKey: 'settings.providerCredentials.field.azureTtsSubscriptionKey.placeholder',
    type: 'password',
    accentHue: 215,
  },
  {
    labelKey: 'settings.providerCredentials.field.azureTtsRegion.label',
    configPath: 'voice.azureTtsRegion',
    placeholder: 'brazilsouth',
    type: 'text',
    accentHue: 215,
    descriptionKey: 'settings.providerCredentials.field.azureTtsRegion.description',
  },
  {
    labelKey: 'settings.providerCredentials.field.googleTtsApiKey.label',
    configPath: 'voice.googleTtsApiKey',
    test: 'googleTts',
    keyUrl: 'https://console.cloud.google.com/apis/credentials',
    placeholder: 'AIza...',
    type: 'password',
    accentHue: 210,
    expectedPrefix: 'AIza',
  },
  {
    labelKey: 'settings.providerCredentials.field.openaiTtsApiKey.label',
    configPath: 'voice.openaiTtsApiKey',
    keyUrl: 'https://platform.openai.com/api-keys',
    placeholder: 'sk-...',
    type: 'password',
    accentHue: 120,
    descriptionKey: 'settings.providerCredentials.field.openaiTtsApiKey.description',
  },
  {
    labelKey: 'settings.providerCredentials.field.openaiTtsBaseUrl.label',
    configPath: 'voice.openaiTtsBaseUrl',
    placeholder: 'http://localhost:8880/v1',
    type: 'url',
    accentHue: 120,
    descriptionKey: 'settings.providerCredentials.field.openaiTtsBaseUrl.description',
  },
]

export const STT_FIELD_SPECS: CredentialFieldSpec[] = [
  {
    labelKey: 'settings.providerCredentials.field.groqApiKey.label',
    configPath: 'voice.groqApiKey',
    test: 'groq',
    keyUrl: 'https://console.groq.com/keys',
    placeholder: 'gsk_...',
    type: 'password',
    accentHue: 155,
    descriptionKey: 'settings.providerCredentials.field.groqApiKey.description',
    expectedPrefix: 'gsk_',
  },
  {
    labelKey: 'settings.providerCredentials.field.openaiSttApiKey.label',
    configPath: 'voice.openaiApiKey',
    test: 'openaiStt',
    keyUrl: 'https://platform.openai.com/api-keys',
    placeholder: 'sk-...',
    type: 'password',
    accentHue: 120,
    descriptionKey: 'settings.providerCredentials.field.openaiSttApiKey.description',
    expectedPrefix: 'sk-',
  },
]

export const INTEGRATION_FIELD_SPECS: CredentialFieldSpec[] = [
  {
    labelKey: 'settings.providerCredentials.field.steamApiKey.label',
    configPath: 'gamer-companion.steamApiKey',
    test: 'steam',
    keyUrl: 'https://steamcommunity.com/dev/apikey',
    placeholderKey: 'settings.providerCredentials.field.steamApiKey.placeholder',
    type: 'password',
    accentHue: 28,
    descriptionKey: 'settings.providerCredentials.field.steamApiKey.description',
  },
]
