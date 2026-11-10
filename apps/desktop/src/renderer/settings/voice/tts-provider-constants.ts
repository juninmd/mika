import type { TranslationDict } from '@mika/sdk/i18n'
import { choiceOption, type ProviderChoice } from '@mika/sdk-ui/settings'
import {
  AudioWaveform,
  Cloud,
  Cpu,
  Feather,
  Gauge,
  Gem,
  Mars,
  Mic2,
  Rocket,
  Sparkles,
  Venus,
  VolumeX,
} from 'lucide-react'
import { type BadgeKind, providerBadges } from '../../src/components/settings/provider-badges'
import type { Translate } from '../../src/hooks/useI18n'

export type TTSProvider = 'elevenlabs' | 'azure-tts' | 'google-tts' | 'openai-tts' | 'piper' | 'supertonic' | 'system'

type TKey = keyof TranslationDict

// Nome próprio literal + sufixo entre parênteses traduzido — o nome não muda de idioma, o sufixo muda.
function tagged(name: string, tagKey: TKey, t: Translate): string {
  return `${name} (${t(tagKey)})`
}

type SelectOption = ReturnType<typeof choiceOption>

type VoiceData = { name: string; tagKey: TKey; value: string }

/** Voz como card: o nome é o título, o timbre traduzido vira descrição e escolhe o ícone. */
function voiceOptions(data: VoiceData[], t: Translate) {
  return data.map((v) =>
    choiceOption(v.name, v.value, t(v.tagKey), v.tagKey.startsWith('settings.voiceTts.voiceTag.female') ? Venus : Mars),
  )
}

/**
 * Valor salvo que não está em lista nenhuma (voz Piper aposentada, `pt-BR-Standard-A` de uma versão
 * anterior, voz digitada à mão) vira uma opção "personalizada": o seletor mostra o que está de fato
 * gravado em vez de ficar em branco, e o usuário só o perde se escolher outra.
 */
function withSavedValue(options: SelectOption[], current: string | undefined, t: Translate): SelectOption[] {
  if (!current || options.some((o) => o.value === current)) return options
  return [...options, choiceOption(current, current, t('settings.voiceTts.voiceTag.custom'), Mic2)]
}

export const PROVIDERS: { id: TTSProvider; name: string; tagKey?: TKey; desc: string }[] = [
  {
    id: 'supertonic',
    name: 'Supertonic',
    tagKey: 'settings.voiceTts.voiceTag.embedded',
    desc: 'Local, 44 kHz, o mais natural em pt-BR',
  },
  { id: 'piper', name: 'Piper', tagKey: 'settings.voiceTts.voiceTag.local', desc: 'Offline, sem enviar nada' },
  { id: 'elevenlabs', name: 'ElevenLabs', desc: 'Vozes premium com emoção' },
  { id: 'azure-tts', name: 'Azure Speech', desc: 'Vozes Microsoft cognitivas' },
  { id: 'google-tts', name: 'Google Cloud', desc: 'Vozes Google AI' },
  { id: 'openai-tts', name: 'OpenAI TTS', desc: 'Vozes OpenAI / Kokoro' },
  { id: 'system', name: 'Sistema', desc: 'Não gera áudio' },
]

const LOOK: Record<TTSProvider, { icon: ProviderChoice['icon']; badges: BadgeKind[] }> = {
  supertonic: { icon: Sparkles, badges: ['recommended', 'local', 'gpu'] },
  piper: { icon: Feather, badges: ['local'] },
  elevenlabs: { icon: AudioWaveform, badges: ['cloud', 'apiKey'] },
  'azure-tts': { icon: Cloud, badges: ['cloud', 'apiKey'] },
  'google-tts': { icon: Mic2, badges: ['cloud', 'apiKey'] },
  'openai-tts': { icon: Cpu, badges: ['cloud', 'apiKey'] },
  system: { icon: VolumeX, badges: [] },
}

export function providerOptions(t: Translate): (ProviderChoice & { id: TTSProvider })[] {
  return PROVIDERS.map((p) => ({
    id: p.id,
    label: p.id === 'system' ? t('settings.voiceTts.provider.system') : p.tagKey ? tagged(p.name, p.tagKey, t) : p.name,
    desc: p.desc,
    icon: LOOK[p.id].icon,
    badges: providerBadges(t, ...LOOK[p.id].badges),
  }))
}

/** Só vozes do catálogo Rust (`piper/catalog.rs`): SHA fixo e licença do dataset conferida. */
const PIPER_VOICE_DATA = [
  { name: 'Faber', tagKey: 'settings.voiceTts.voiceTag.maleBr' as const, value: 'pt_BR-faber-medium' },
  { name: 'Edresson', tagKey: 'settings.voiceTts.voiceTag.maleBr' as const, value: 'pt_BR-edresson-low' },
  { name: 'Thorsten', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'de_DE-thorsten-medium' },
  { name: 'Paola', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'it_IT-paola-medium' },
  { name: 'Denis', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ru_RU-denis-medium' },
]
/** Idiomas com voz Piper no catálogo (`pt_BR-…` → `pt`). */
export const PIPER_LANGUAGES: readonly string[] = [...new Set(PIPER_VOICE_DATA.map((v) => v.value.slice(0, 2)))]
export function piperVoices(t: Translate, current?: string) {
  return withSavedValue(voiceOptions(PIPER_VOICE_DATA, t), current, t)
}

const SUPERTONIC_VOICE_DATA = [
  { name: 'F1', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'F1' },
  { name: 'F2', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'F2' },
  { name: 'F3', tagKey: 'settings.voiceTts.voiceTag.femaleDefault' as const, value: 'F3' },
  { name: 'F4', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'F4' },
  { name: 'F5', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'F5' },
  { name: 'M1', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'M1' },
  { name: 'M2', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'M2' },
  { name: 'M3', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'M3' },
  { name: 'M4', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'M4' },
  { name: 'M5', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'M5' },
]
/** Idiomas que o modelo Supertonic pronuncia (`supertonic::SUPPORTED_LANGUAGES`), no próprio idioma. */
export const SUPERTONIC_LANGUAGES = ['pt', 'en', 'es', 'fr', 'ko'] as const
export function supertonicLanguages(t: Translate) {
  return SUPERTONIC_LANGUAGES.map((code) => ({
    label: t(`settings.voiceTts.language.${code}`),
    value: code,
    icon: code,
  }))
}

export function supertonicVoices(t: Translate) {
  return voiceOptions(SUPERTONIC_VOICE_DATA, t)
}

/** Nomes de voz da documentação oficial da Azure (Neural) — o valor é o identificador que a API recebe. */
const AZURE_VOICE_DATA = [
  { name: 'Jenny', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'en-US-JennyNeural' },
  { name: 'Aria', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'en-US-AriaNeural' },
  { name: 'Guy', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'en-US-GuyNeural' },
  { name: 'Davis', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'en-US-DavisNeural' },
  { name: 'Francisca', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'pt-BR-FranciscaNeural' },
  { name: 'Thalita', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'pt-BR-ThalitaNeural' },
  { name: 'Antonio', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'pt-BR-AntonioNeural' },
  { name: 'Donato', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'pt-BR-DonatoNeural' },
  { name: 'Elvira', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'es-ES-ElviraNeural' },
  { name: 'Abril', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'es-ES-AbrilNeural' },
  { name: 'Alvaro', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'es-ES-AlvaroNeural' },
  { name: 'Arnau', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'es-ES-ArnauNeural' },
  { name: 'Denise', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'fr-FR-DeniseNeural' },
  { name: 'Brigitte', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'fr-FR-BrigitteNeural' },
  { name: 'Henri', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'fr-FR-HenriNeural' },
  { name: 'Alain', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'fr-FR-AlainNeural' },
  { name: 'Elsa', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'it-IT-ElsaNeural' },
  { name: 'Isabella', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'it-IT-IsabellaNeural' },
  { name: 'Diego', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'it-IT-DiegoNeural' },
  { name: 'Benigno', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'it-IT-BenignoNeural' },
  { name: 'Nanami', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ja-JP-NanamiNeural' },
  { name: 'Aoi', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ja-JP-AoiNeural' },
  { name: 'Keita', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ja-JP-KeitaNeural' },
  { name: 'Daichi', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ja-JP-DaichiNeural' },
  { name: 'Xiaoxiao', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'zh-CN-XiaoxiaoNeural' },
  { name: 'Xiaoyi', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'zh-CN-XiaoyiNeural' },
  { name: 'Yunxi', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'zh-CN-YunxiNeural' },
  { name: 'Yunjian', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'zh-CN-YunjianNeural' },
  { name: 'SunHi', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ko-KR-SunHiNeural' },
  { name: 'JiMin', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ko-KR-JiMinNeural' },
  { name: 'InJoon', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ko-KR-InJoonNeural' },
  { name: 'BongJin', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ko-KR-BongJinNeural' },
  { name: 'Svetlana', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ru-RU-SvetlanaNeural' },
  { name: 'Dariya', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ru-RU-DariyaNeural' },
  { name: 'Dmitry', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ru-RU-DmitryNeural' },
  { name: 'Katja', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'de-DE-KatjaNeural' },
  { name: 'Amala', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'de-DE-AmalaNeural' },
  { name: 'Conrad', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'de-DE-ConradNeural' },
  { name: 'Bernd', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'de-DE-BerndNeural' },
]

/** Google Cloud TTS (WaveNet) — o idioma da China é `cmn-CN` na API, não `zh-CN`. */
const GOOGLE_VOICE_DATA = [
  { name: 'Wavenet-C', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'en-US-Wavenet-C' },
  { name: 'Wavenet-F', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'en-US-Wavenet-F' },
  { name: 'Wavenet-D', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'en-US-Wavenet-D' },
  { name: 'Wavenet-B', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'en-US-Wavenet-B' },
  { name: 'Wavenet-A', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'pt-BR-Wavenet-A' },
  { name: 'Wavenet-C', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'pt-BR-Wavenet-C' },
  { name: 'Wavenet-B', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'pt-BR-Wavenet-B' },
  { name: 'Wavenet-E', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'pt-BR-Wavenet-E' },
  { name: 'Wavenet-F', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'es-ES-Wavenet-F' },
  { name: 'Wavenet-H', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'es-ES-Wavenet-H' },
  { name: 'Wavenet-E', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'es-ES-Wavenet-E' },
  { name: 'Wavenet-G', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'es-ES-Wavenet-G' },
  { name: 'Wavenet-F', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'fr-FR-Wavenet-F' },
  { name: 'Wavenet-G', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'fr-FR-Wavenet-G' },
  { name: 'Wavenet-E', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'it-IT-Wavenet-E' },
  { name: 'Wavenet-F', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'it-IT-Wavenet-F' },
  { name: 'Wavenet-A', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ja-JP-Wavenet-A' },
  { name: 'Wavenet-B', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ja-JP-Wavenet-B' },
  { name: 'Wavenet-C', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ja-JP-Wavenet-C' },
  { name: 'Wavenet-D', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ja-JP-Wavenet-D' },
  { name: 'Wavenet-A', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'cmn-CN-Wavenet-A' },
  { name: 'Wavenet-D', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'cmn-CN-Wavenet-D' },
  { name: 'Wavenet-B', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'cmn-CN-Wavenet-B' },
  { name: 'Wavenet-C', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'cmn-CN-Wavenet-C' },
  { name: 'Wavenet-A', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ko-KR-Wavenet-A' },
  { name: 'Wavenet-B', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ko-KR-Wavenet-B' },
  { name: 'Wavenet-C', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ko-KR-Wavenet-C' },
  { name: 'Wavenet-D', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ko-KR-Wavenet-D' },
  { name: 'Wavenet-A', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ru-RU-Wavenet-A' },
  { name: 'Wavenet-C', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'ru-RU-Wavenet-C' },
  { name: 'Wavenet-B', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ru-RU-Wavenet-B' },
  { name: 'Wavenet-D', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'ru-RU-Wavenet-D' },
  { name: 'Wavenet-G', tagKey: 'settings.voiceTts.voiceTag.female' as const, value: 'de-DE-Wavenet-G' },
  { name: 'Wavenet-H', tagKey: 'settings.voiceTts.voiceTag.male' as const, value: 'de-DE-Wavenet-H' },
]

const AZURE_PT_DEFAULT = 'pt-BR-FranciscaNeural'
const GOOGLE_PT_DEFAULT = 'pt-BR-Wavenet-A'

type CloudProvider = 'azure' | 'google'

function voicesOf(data: VoiceData[], provider: CloudProvider, locale: string): VoiceData[] {
  const prefix = provider === 'google' && locale === 'zh-CN' ? 'cmn-CN' : locale
  return data.filter((v) => v.value.startsWith(`${prefix}-`))
}

/** Vozes do idioma da interface; a voz salva de outro idioma entra junto, senão o seletor ficaria em branco. */
function localeVoiceOptions(data: VoiceData[], provider: CloudProvider, locale: string, current: string, t: Translate) {
  const own = voicesOf(data, provider, locale)
  const kept = own.some((v) => v.value === current) ? undefined : data.find((v) => v.value === current)
  return withSavedValue(voiceOptions(kept ? [...own, kept] : own, t), current, t)
}

/** Espelha `follow_locale` do Rust: a voz ainda no default português segue o idioma da interface. */
function followLocaleVoice(
  stored: string,
  ptDefault: string,
  data: VoiceData[],
  provider: CloudProvider,
  locale: string,
) {
  if (stored !== ptDefault || locale === 'pt-BR') return stored
  return voicesOf(data, provider, locale)[0]?.value ?? stored
}

export const azureVoiceForLocale = (stored: string, locale: string) =>
  followLocaleVoice(stored, AZURE_PT_DEFAULT, AZURE_VOICE_DATA, 'azure', locale)
export const googleVoiceForLocale = (stored: string, locale: string) =>
  followLocaleVoice(stored, GOOGLE_PT_DEFAULT, GOOGLE_VOICE_DATA, 'google', locale)

/** `googleTtsLanguageCode` ainda no default `pt-BR` segue a interface (o chinês é `cmn-CN` no Google). */
export function googleLanguageCodeForLocale(stored: string, locale: string): string {
  if (stored !== 'pt-BR' || locale === 'pt-BR') return stored
  return locale === 'zh-CN' ? 'cmn-CN' : locale
}

export function azureVoices(t: Translate, locale: string, current: string) {
  return localeVoiceOptions(AZURE_VOICE_DATA, 'azure', locale, current, t)
}

export function googleVoices(t: Translate, locale: string, current: string) {
  return localeVoiceOptions(GOOGLE_VOICE_DATA, 'google', locale, current, t)
}

const ELEVEN_MODEL_DATA = [
  {
    name: 'Multilingual v2',
    tagKey: 'settings.voiceTts.quality.high' as const,
    value: 'eleven_multilingual_v2',
    icon: Gem,
  },
  {
    name: 'Turbo v2.5',
    tagKey: 'settings.voiceTts.quality.lowLatency' as const,
    value: 'eleven_turbo_v2_5',
    icon: Gauge,
  },
  { name: 'Flash v2.5', tagKey: 'settings.voiceTts.quality.fast' as const, value: 'eleven_flash_v2_5', icon: Rocket },
]
export function elevenModels(t: Translate) {
  return ELEVEN_MODEL_DATA.map((v) => choiceOption(v.name, v.value, t(v.tagKey), v.icon))
}

export const OPENAI_TTS_VOICE_HINT =
  'Kokoro: af_heart, af_bella, am_michael, pf_dora (pt-BR). OpenAI: alloy, echo, fable, onyx, nova, shimmer.'

export function audioMimeFromBase64(audio: string): string {
  return audio.startsWith('UklGR') ? 'audio/wav' : 'audio/mpeg'
}
