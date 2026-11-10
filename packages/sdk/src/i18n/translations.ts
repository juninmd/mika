import type { SupportedLocale } from '../contracts'
import { enUS } from './locales/en-US'
import { ptBR } from './locales/pt-BR'
import type { TranslationDict } from './translation-dict'

export type { TranslationDict } from './translation-dict'

export type TranslationRecord = Record<SupportedLocale, TranslationDict>

/**
 * Só o padrão (pt-BR) e o fallback (en-US) vão no bundle de entrada; os outros oito catálogos
 * (~170 KB cada) viram chunks carregados sob demanda. Antes todos iam no `index.js` de toda
 * janela — 1,15 MB dos 1,25 MB do entry eram dicionários que a janela nunca ia mostrar.
 */
export const translations: Partial<TranslationRecord> = {
  'en-US': enUS,
  'pt-BR': ptBR,
}

const LOADERS: Record<SupportedLocale, () => Promise<TranslationDict>> = {
  'en-US': async () => enUS,
  'pt-BR': async () => ptBR,
  'es-ES': () => import('./locales/es-ES').then((m) => m.esES),
  'fr-FR': () => import('./locales/fr-FR').then((m) => m.frFR),
  'it-IT': () => import('./locales/it-IT').then((m) => m.itIT),
  'ja-JP': () => import('./locales/ja-JP').then((m) => m.jaJP),
  'zh-CN': () => import('./locales/zh-CN').then((m) => m.zhCN),
  'ko-KR': () => import('./locales/ko-KR').then((m) => m.koKR),
  'ru-RU': () => import('./locales/ru-RU').then((m) => m.ruRU),
  'de-DE': () => import('./locales/de-DE').then((m) => m.deDE),
}

export function isLocaleLoaded(locale: SupportedLocale): boolean {
  return translations[locale] !== undefined
}

export async function loadLocale(locale: SupportedLocale): Promise<TranslationDict> {
  const loaded = translations[locale]
  if (loaded) return loaded
  const dict = await LOADERS[locale]()
  translations[locale] = dict
  return dict
}
