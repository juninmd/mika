import type { TranslationDict } from '@mika/sdk/i18n'
import { getLocale } from '@mika/sdk/i18n'
import { configBool, configNum, configStr, pick } from '../../src/components/settings/settings-utils'
import {
  azureVoiceForLocale,
  googleLanguageCodeForLocale,
  googleVoiceForLocale,
  type TTSProvider,
} from './tts-provider-constants'

type Props = {
  ttsProvider: TTSProvider
  ttsEnabled: boolean
  volume: number
  autoSpeak: boolean
  autoMuteDuringTts: boolean
  interruptionEnabled: boolean
  interruptionThreshold: number
  interruptionTriggerStreak: number
  interruptionCooldownMs: number
  ttsIgnoreBrackets: boolean
  ttsIgnoreParentheses: boolean
  ttsIgnoreAsterisks: boolean
  ttsIgnoreAngleBrackets: boolean
  ttsRemoveEmoji: boolean
  elevenLabsApiKey: string
  elevenLabsVoiceId: string
  elevenLabsModel: string
  ttsStability: number
  elevenLabsSimilarityBoost: number
  elevenLabsStyle: number
  elevenLabsSpeakerBoost: boolean
  elevenLabsSpeed: number
  elevenLabsTextLimit: number
  elevenLabsTimeoutMs: number
  azureTtsVoice: string
  azureTtsRate: number
  azureTtsPitch: number
  azureTtsTextLimit: number
  azureTtsTimeoutMs: number
  piperVoice: string
  piperBinaryPath: string
  piperRate: number
  supertonicVoice: string
  supertonicRate: number
  supertonicSteps: number
  supertonicLanguage: string
  openaiTtsVoice: string
  openaiTtsModel: string
  openaiTtsSpeed: number
  googleTtsVoice: string
  googleTtsLanguageCode: string
  googleTtsSpeakingRate: number
  googleTtsPitch: number
  googleTtsTextLimit: number
  googleTtsTimeoutMs: number
  onSave: (path: string, value: unknown) => Promise<void>
}

export type TTSProviderPanelProps = {
  config?: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
} & Partial<Props>

export function resolveTestActionLabel(
  t: (key: keyof TranslationDict) => string,
  testStatus: string,
  isTestAudioPlaying: boolean,
  testResult: string | null,
): string {
  if (testStatus === 'testing') return t('settings.voiceTts.panel.testing')
  if (isTestAudioPlaying) return t('settings.voiceTts.panel.playing')
  return testResult ?? t('settings.voiceTts.panel.testVoice')
}

/** Resolve todos os campos do painel de TTS: prop explícita (form controlado) > config lida do store. */
export function resolveTtsFields(props: TTSProviderPanelProps, rawCfg: Record<string, unknown>) {
  return {
    ttsProvider: pick(props.ttsProvider, () => configStr(rawCfg, 'voice.ttsProvider', 'supertonic')) as TTSProvider,
    ttsEnabled: pick(props.ttsEnabled, () => configBool(rawCfg, 'voice.ttsEnabled', true)),
    volume: pick(props.volume, () => configNum(rawCfg, 'voice.volume', 1)),
    autoSpeak: pick(props.autoSpeak, () => configBool(rawCfg, 'voice.autoSpeak', true)),
    autoMuteDuringTts: pick(props.autoMuteDuringTts, () => configBool(rawCfg, 'voice.autoMuteDuringTts', false)),
    interruptionEnabled: pick(props.interruptionEnabled, () => configBool(rawCfg, 'voice.interruptionEnabled', true)),
    interruptionThreshold: pick(props.interruptionThreshold, () =>
      configNum(rawCfg, 'voice.interruptionThreshold', 0.35),
    ),
    interruptionTriggerStreak: pick(props.interruptionTriggerStreak, () =>
      configNum(rawCfg, 'voice.interruptionTriggerStreak', 3),
    ),
    interruptionCooldownMs: pick(props.interruptionCooldownMs, () =>
      configNum(rawCfg, 'voice.interruptionCooldownMs', 1200),
    ),
    ttsIgnoreBrackets: pick(props.ttsIgnoreBrackets, () => configBool(rawCfg, 'voice.ttsIgnoreBrackets', true)),
    ttsIgnoreParentheses: pick(props.ttsIgnoreParentheses, () =>
      configBool(rawCfg, 'voice.ttsIgnoreParentheses', true),
    ),
    ttsIgnoreAsterisks: pick(props.ttsIgnoreAsterisks, () => configBool(rawCfg, 'voice.ttsIgnoreAsterisks', true)),
    ttsIgnoreAngleBrackets: pick(props.ttsIgnoreAngleBrackets, () =>
      configBool(rawCfg, 'voice.ttsIgnoreAngleBrackets', true),
    ),
    ttsRemoveEmoji: pick(props.ttsRemoveEmoji, () => configBool(rawCfg, 'voice.ttsRemoveEmoji', true)),
    elevenLabsApiKey: props.elevenLabsApiKey ?? '',
    elevenLabsVoiceId: pick(props.elevenLabsVoiceId, () =>
      configStr(rawCfg, 'voice.elevenLabsVoiceId', 'EXAVITQu4vr4xnSDxMaL'),
    ),
    elevenLabsModel: pick(props.elevenLabsModel, () => configStr(rawCfg, 'voice.elevenLabsModel', 'eleven_flash_v2_5')),
    ttsStability: pick(props.ttsStability, () => configNum(rawCfg, 'voice.ttsStability', 0.5)),
    elevenLabsSimilarityBoost: pick(props.elevenLabsSimilarityBoost, () =>
      configNum(rawCfg, 'voice.elevenLabsSimilarityBoost', 0.75),
    ),
    elevenLabsStyle: pick(props.elevenLabsStyle, () => configNum(rawCfg, 'voice.elevenLabsStyle', 0)),
    elevenLabsSpeakerBoost: pick(props.elevenLabsSpeakerBoost, () =>
      configBool(rawCfg, 'voice.elevenLabsSpeakerBoost', true),
    ),
    elevenLabsSpeed: pick(props.elevenLabsSpeed, () => configNum(rawCfg, 'voice.elevenLabsSpeed', 1)),
    elevenLabsTextLimit: pick(props.elevenLabsTextLimit, () => configNum(rawCfg, 'voice.elevenLabsTextLimit', 3000)),
    elevenLabsTimeoutMs: pick(props.elevenLabsTimeoutMs, () => configNum(rawCfg, 'voice.elevenLabsTimeoutMs', 30000)),
    azureTtsVoice: pick(props.azureTtsVoice, () =>
      azureVoiceForLocale(configStr(rawCfg, 'voice.azureTtsVoice', 'pt-BR-FranciscaNeural'), getLocale()),
    ),
    azureTtsRate: pick(props.azureTtsRate, () => configNum(rawCfg, 'voice.azureTtsRate', 0)),
    azureTtsPitch: pick(props.azureTtsPitch, () => configNum(rawCfg, 'voice.azureTtsPitch', 0)),
    azureTtsTextLimit: pick(props.azureTtsTextLimit, () => configNum(rawCfg, 'voice.azureTtsTextLimit', 3000)),
    azureTtsTimeoutMs: pick(props.azureTtsTimeoutMs, () => configNum(rawCfg, 'voice.azureTtsTimeoutMs', 25000)),
    piperVoice: pick(props.piperVoice, () => configStr(rawCfg, 'voice.piperVoice', 'pt_BR-faber-medium')),
    piperBinaryPath: pick(props.piperBinaryPath, () => configStr(rawCfg, 'voice.piperBinaryPath')),
    piperRate: pick(props.piperRate, () => configNum(rawCfg, 'voice.piperRate', 1.0)),
    supertonicVoice: pick(props.supertonicVoice, () => configStr(rawCfg, 'voice.supertonicVoice', 'F3')),
    supertonicRate: pick(props.supertonicRate, () => configNum(rawCfg, 'voice.supertonicRate', 1.0)),
    supertonicSteps: pick(props.supertonicSteps, () => configNum(rawCfg, 'voice.supertonicSteps', 20)),
    supertonicLanguage: pick(props.supertonicLanguage, () => configStr(rawCfg, 'voice.supertonicLanguage', 'pt')),
    openaiTtsVoice: pick(props.openaiTtsVoice, () => configStr(rawCfg, 'voice.openaiTtsVoice', 'af_heart')),
    openaiTtsModel: pick(props.openaiTtsModel, () => configStr(rawCfg, 'voice.openaiTtsModel', 'kokoro')),
    openaiTtsSpeed: pick(props.openaiTtsSpeed, () => configNum(rawCfg, 'voice.openaiTtsSpeed', 1)),
    googleTtsVoice: pick(props.googleTtsVoice, () =>
      googleVoiceForLocale(configStr(rawCfg, 'voice.googleTtsVoice', 'pt-BR-Wavenet-A'), getLocale()),
    ),
    googleTtsLanguageCode: pick(props.googleTtsLanguageCode, () =>
      googleLanguageCodeForLocale(configStr(rawCfg, 'voice.googleTtsLanguageCode', 'pt-BR'), getLocale()),
    ),
    googleTtsSpeakingRate: pick(props.googleTtsSpeakingRate, () =>
      configNum(rawCfg, 'voice.googleTtsSpeakingRate', 1.0),
    ),
    googleTtsPitch: pick(props.googleTtsPitch, () => configNum(rawCfg, 'voice.googleTtsPitch', 0.0)),
    googleTtsTextLimit: pick(props.googleTtsTextLimit, () => configNum(rawCfg, 'voice.googleTtsTextLimit', 3000)),
    googleTtsTimeoutMs: pick(props.googleTtsTimeoutMs, () => configNum(rawCfg, 'voice.googleTtsTimeoutMs', 25000)),
  }
}

export type TtsFields = ReturnType<typeof resolveTtsFields>

export function buildTtsCfg(fields: TtsFields): Record<string, unknown> {
  return {
    'voice.elevenLabsModel': fields.elevenLabsModel,
    'voice.azureTtsVoice': fields.azureTtsVoice,
    'voice.azureTtsTextLimit': fields.azureTtsTextLimit,
    'voice.azureTtsTimeoutMs': fields.azureTtsTimeoutMs,
    'voice.googleTtsVoice': fields.googleTtsVoice,
    'voice.googleTtsLanguageCode': fields.googleTtsLanguageCode,
    'voice.piperVoice': fields.piperVoice,
    'voice.piperBinaryPath': fields.piperBinaryPath,
    'voice.supertonicVoice': fields.supertonicVoice,
    'voice.supertonicRate': fields.supertonicRate,
    'voice.supertonicSteps': fields.supertonicSteps,
    'voice.supertonicLanguage': fields.supertonicLanguage,
    'voice.openaiTtsVoice': fields.openaiTtsVoice,
    'voice.openaiTtsModel': fields.openaiTtsModel,
    'voice.elevenLabsTextLimit': fields.elevenLabsTextLimit,
    'voice.elevenLabsTimeoutMs': fields.elevenLabsTimeoutMs,
  }
}
