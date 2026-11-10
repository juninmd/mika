import type { TranslationDict } from '@mika/sdk/i18n'
import { PIPER_LANGUAGES, SUPERTONIC_LANGUAGES, type TTSProvider } from './tts-provider-constants'

export type CoverageNoticeKey = Extract<keyof TranslationDict, `settings.voiceTts.coverage.${string}`>

/** Cloud providers are multilingual; `system` makes no audio — neither can fail to cover the UI language. */
const LOCAL_PROVIDERS: readonly TTSProvider[] = ['supertonic', 'piper']

/** Voice keys are `<language>_<REGION>-<speaker>-<quality>` (`de_DE-thorsten-medium`). */
const piperVoiceLanguage = (voice: string): string => voice.slice(0, 2)

/**
 * Which notice (if any) the voice tab owes the user: the active local provider cannot speak the
 * language the whole app talks in. Mirrors the Rust side — `supertonic::supports_language` and the
 * Piper fallback of `tts::dispatch::generate_local` — so what the notice promises is what happens.
 */
export function ttsCoverageNotice(provider: TTSProvider, locale: string, piperVoice: string): CoverageNoticeKey | null {
  if (!LOCAL_PROVIDERS.includes(provider)) return null
  const language = locale.split('-')[0] ?? locale
  const hasPiperVoice = PIPER_LANGUAGES.includes(language)
  if (provider === 'supertonic') {
    if ((SUPERTONIC_LANGUAGES as readonly string[]).includes(language)) return null
    return hasPiperVoice ? 'settings.voiceTts.coverage.piperFallback' : 'settings.voiceTts.coverage.unsupported'
  }
  if (piperVoiceLanguage(piperVoice) === language) return null
  return hasPiperVoice ? 'settings.voiceTts.coverage.piperVoice' : 'settings.voiceTts.coverage.unsupported'
}

/** The language's name in the interface language (`Deutsch`, `ドイツ語`); the bare code if the runtime has no names. */
export function languageDisplayName(locale: string): string {
  const language = locale.split('-')[0] ?? locale
  try {
    return new Intl.DisplayNames([locale], { type: 'language' }).of(language) ?? language
  } catch {
    return language
  }
}
