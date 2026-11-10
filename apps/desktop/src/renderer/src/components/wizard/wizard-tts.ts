import { ConfigKey } from '@mika/sdk/contracts'
import type { TtsId } from './wizard-data'

export const TTS_OPTION_IDS: readonly TtsId[] = ['none', 'supertonic', 'elevenlabs']

// null = the saved provider is not a wizard option (piper, azure, kokoro...) and must be kept as is.
export type TtsSelection = TtsId | null

export function readSavedTts(config: Record<string, unknown>): TtsSelection {
  const voice = config.voice as Record<string, unknown> | undefined
  if (voice?.ttsEnabled === false) return 'none'
  const provider = voice?.ttsProvider
  return TTS_OPTION_IDS.find((id) => id !== 'none' && id === provider) ?? null
}

export function ttsSettingsToSave(selection: TtsSelection): Array<[string, unknown]> {
  if (selection === null) return []
  if (selection === 'none') return [[ConfigKey.VoiceTtsEnabled, false]]
  return [
    [ConfigKey.VoiceTtsEnabled, true],
    [ConfigKey.VoiceTtsProvider, selection],
  ]
}
