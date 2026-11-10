import type { TranslationDict } from '@mika/sdk/i18n'
import { renderAzureFields, renderElevenLabsFields, renderGoogleFields } from './tts-cloud-provider-fields'
import {
  renderOpenAiFields,
  renderPiperFields,
  renderSupertonicFields,
  renderSystemFields,
} from './tts-local-provider-fields'
import type { TTSProvider } from './tts-provider-constants'

export type ProviderSpecificFieldsProps = {
  t: (key: keyof TranslationDict) => string
  ttsProvider: TTSProvider
  cfg: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
  numSave: (path: string, value: number) => Promise<void>
  strSave: (path: string, value: string) => Promise<void>
  doSave: (path: string, value: unknown) => Promise<void>
  elevenLabsApiKey: string
  elevenLabsVoiceId: string
  ttsStability: number
  elevenLabsSimilarityBoost: number
  elevenLabsStyle: number
  elevenLabsSpeakerBoost: boolean
  elevenLabsSpeed: number
  elevenLabsTextLimit: number
  elevenLabsTimeoutMs: number
  azureTtsRate: number
  azureTtsPitch: number
  azureTtsTextLimit: number
  azureTtsTimeoutMs: number
  piperRate: number
  supertonicRate: number
  supertonicSteps: number
  openaiTtsSpeed: number
  googleTtsSpeakingRate: number
  googleTtsPitch: number
  googleTtsTextLimit: number
  googleTtsTimeoutMs: number
}

/** Card de campos específicos do provedor de TTS selecionado — isolado para manter a complexidade do painel principal sob controle. */
export function ProviderSpecificFields(props: ProviderSpecificFieldsProps) {
  const { ttsProvider } = props
  if (ttsProvider === 'elevenlabs') return renderElevenLabsFields(props)
  if (ttsProvider === 'azure-tts') return renderAzureFields(props)
  if (ttsProvider === 'piper') return renderPiperFields(props)
  if (ttsProvider === 'supertonic') return renderSupertonicFields(props)
  if (ttsProvider === 'google-tts') return renderGoogleFields(props)
  if (ttsProvider === 'openai-tts') return renderOpenAiFields(props)
  if (ttsProvider === 'system') return renderSystemFields(props)
  return null
}
