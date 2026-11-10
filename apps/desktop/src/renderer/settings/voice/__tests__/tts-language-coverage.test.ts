import { describe, expect, it } from 'vitest'
import { ttsCoverageNotice } from '../tts-language-coverage'
import {
  azureVoiceForLocale,
  azureVoices,
  googleLanguageCodeForLocale,
  googleVoiceForLocale,
  googleVoices,
  piperVoices,
} from '../tts-provider-constants'

const LOCALES = ['en-US', 'pt-BR', 'es-ES', 'fr-FR', 'it-IT', 'ja-JP', 'zh-CN', 'ko-KR', 'ru-RU', 'de-DE']
const t = (key: string) => key

describe('ttsCoverageNotice', () => {
  it('Supertonic speaks en/pt/es/fr/ko with no notice', () => {
    for (const locale of ['en-US', 'pt-BR', 'es-ES', 'fr-FR', 'ko-KR']) {
      expect(ttsCoverageNotice('supertonic', locale, 'pt_BR-faber-medium')).toBeNull()
    }
  })

  it('Supertonic with de/it/ru announces the Piper fallback (a verified voice exists)', () => {
    for (const locale of ['de-DE', 'it-IT', 'ru-RU']) {
      expect(ttsCoverageNotice('supertonic', locale, 'pt_BR-faber-medium')).toBe(
        'settings.voiceTts.coverage.piperFallback',
      )
    }
  })

  it('ja/zh have no local voice with a verified license: the notice sends the user to the cloud', () => {
    for (const locale of ['ja-JP', 'zh-CN']) {
      expect(ttsCoverageNotice('supertonic', locale, 'x')).toBe('settings.voiceTts.coverage.unsupported')
      expect(ttsCoverageNotice('piper', locale, 'pt_BR-faber-medium')).toBe('settings.voiceTts.coverage.unsupported')
    }
  })

  it('Piper warns only when the selected voice is not in the interface language', () => {
    expect(ttsCoverageNotice('piper', 'de-DE', 'de_DE-thorsten-medium')).toBeNull()
    expect(ttsCoverageNotice('piper', 'de-DE', 'pt_BR-faber-medium')).toBe('settings.voiceTts.coverage.piperVoice')
  })

  it('cloud providers and "system" never raise the notice', () => {
    for (const provider of ['elevenlabs', 'azure-tts', 'google-tts', 'openai-tts', 'system'] as const) {
      expect(ttsCoverageNotice(provider, 'ja-JP', 'x')).toBeNull()
    }
  })
})

describe('cloud voice selects per locale', () => {
  it('every interface language lists Azure and Google voices of that language', () => {
    for (const locale of LOCALES) {
      const prefix = locale
      const azure = azureVoices(t, locale, '').map((o) => String(o.value))
      const google = googleVoices(t, locale, '').map((o) => String(o.value))
      expect(azure.length, `azure ${locale}`).toBeGreaterThan(0)
      expect(google.length, `google ${locale}`).toBeGreaterThan(0)
      expect(azure.every((v) => v.startsWith(`${prefix}-`))).toBe(true)
      expect(google.every((v) => v.startsWith(`${locale === 'zh-CN' ? 'cmn-CN' : locale}-`))).toBe(true)
    }
  })

  it('keeps a saved voice from another language selectable instead of rendering a blank select', () => {
    const values = azureVoices(t, 'ja-JP', 'pt-BR-AntonioNeural').map((o) => String(o.value))
    expect(values).toContain('pt-BR-AntonioNeural')
    expect(values).toContain('ja-JP-NanamiNeural')
  })

  it('shows a saved voice that left every list (retired Google/Piper voice) as a custom option', () => {
    const google = googleVoices(t, 'pt-BR', 'pt-BR-Standard-A').map((o) => String(o.value))
    expect(google).toContain('pt-BR-Standard-A')
    const piper = piperVoices(t, 'en_US-lessac-high').map((o) => String(o.value))
    expect(piper).toContain('en_US-lessac-high')
    expect(piper.filter((v) => v === 'pt_BR-faber-medium')).toHaveLength(1)
    expect(piperVoices(t, 'pt_BR-faber-medium')).toHaveLength(piperVoices(t, '').length)
    expect(piperVoices(t, '').map((o) => String(o.value))).not.toContain('en_US-amy-medium')
  })

  it('an untouched Portuguese default follows the interface language, like the Rust side', () => {
    expect(azureVoiceForLocale('pt-BR-FranciscaNeural', 'ja-JP')).toBe('ja-JP-NanamiNeural')
    expect(googleVoiceForLocale('pt-BR-Wavenet-A', 'zh-CN')).toBe('cmn-CN-Wavenet-A')
    expect(googleLanguageCodeForLocale('pt-BR', 'zh-CN')).toBe('cmn-CN')
    expect(azureVoiceForLocale('ja-JP-AoiNeural', 'ja-JP')).toBe('ja-JP-AoiNeural')
    expect(azureVoiceForLocale('pt-BR-FranciscaNeural', 'pt-BR')).toBe('pt-BR-FranciscaNeural')
  })
})
