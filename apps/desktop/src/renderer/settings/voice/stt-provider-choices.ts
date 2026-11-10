import type { ProviderChoice } from '@mika/sdk-ui/settings'
import { Activity, AudioLines, Brain, Cloud, Globe, HardDrive, Zap } from 'lucide-react'
import { providerBadges } from '../../src/components/settings/provider-badges'
import type { Translate } from '../../src/hooks/useI18n'

/** `local-whisper` is the legacy alias of `electron`; both light the same card. */
export function sttCardId(provider: string): string {
  return provider === 'local-whisper' ? 'electron' : provider
}

export function sttProviderChoices(t: Translate): ProviderChoice[] {
  return [
    {
      id: 'nemotron',
      label: t('settings.stt.provider.nemotron'),
      desc: t('settings.stt.provider.nemotron.desc'),
      icon: AudioLines,
      badges: providerBadges(t, 'recommended', 'local', 'streaming'),
    },
    {
      id: 'electron',
      label: t('settings.stt.provider.localWhisper'),
      desc: t('settings.stt.provider.localWhisper.desc'),
      icon: HardDrive,
      badges: providerBadges(t, 'local', 'gpu'),
    },
    {
      id: 'browser',
      label: t('settings.stt.provider.browser'),
      desc: t('settings.stt.provider.browser.desc'),
      icon: Globe,
      badges: providerBadges(t, 'cloud'),
    },
    {
      id: 'groq-whisper',
      label: t('settings.stt.provider.groqWhisper'),
      desc: t('settings.stt.provider.groqWhisper.desc'),
      icon: Zap,
      badges: providerBadges(t, 'cloud', 'apiKey'),
    },
    {
      id: 'openai-whisper',
      label: t('settings.stt.provider.openaiWhisper'),
      desc: t('settings.stt.provider.openaiWhisper.desc'),
      icon: Cloud,
      badges: providerBadges(t, 'cloud', 'apiKey'),
    },
  ]
}

export function vadEngineChoices(t: Translate): ProviderChoice[] {
  return [
    {
      id: 'energy',
      label: t('settings.voiceStt.field.vadEngine.option.energy'),
      desc: t('settings.voiceStt.vadEngine.energy.desc'),
      icon: Activity,
      badges: providerBadges(t, 'local'),
    },
    {
      id: 'silero',
      label: t('settings.voiceStt.field.vadEngine.option.silero'),
      desc: t('settings.voiceStt.vadEngine.silero.desc'),
      icon: Brain,
      badges: providerBadges(t, 'local', 'neural'),
    },
  ]
}

// Códigos que o STT resolve de fato (`inference/nemotron/lang.rs`; o Whisper aceita os mesmos ISO 639-1).
const ASR_LANGUAGE_CODES = [
  'pt',
  'en',
  'es',
  'fr',
  'it',
  'de',
  'nl',
  'pl',
  'ru',
  'uk',
  'tr',
  'ar',
  'hi',
  'ja',
  'ko',
  'zh',
]

/** Nome nativo vindo do próprio `Intl` — nenhuma string de idioma escrita à mão. */
function nativeLanguageName(code: string): string {
  try {
    const name = new Intl.DisplayNames([code], { type: 'language' }).of(code)
    return name ? name.charAt(0).toLocaleUpperCase(code) + name.slice(1) : code
  } catch {
    return code
  }
}

/** Opções do idioma de escuta; um código salvo fora da lista continua selecionável. */
export function asrLanguageOptions(current: string): { label: string; value: string; description: string }[] {
  const codes = current && !ASR_LANGUAGE_CODES.includes(current) ? [current, ...ASR_LANGUAGE_CODES] : ASR_LANGUAGE_CODES
  return codes.map((code) => ({ label: nativeLanguageName(code), value: code, description: code }))
}
