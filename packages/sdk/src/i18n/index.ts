import { SUPPORTED_LOCALES, type SupportedLocale } from '../contracts'
import type { TranslationDict, TranslationRecord } from './translations'
import { isLocaleLoaded, loadLocale, translations } from './translations'

export const DEFAULT_LOCALE: SupportedLocale = 'pt-BR'
export const FALLBACK_LOCALE: SupportedLocale = 'en-US'

export interface LocaleOption {
  locale: SupportedLocale
  nativeName: string
  englishName: string
}

export const LOCALE_OPTIONS: readonly LocaleOption[] = [
  { locale: 'en-US', nativeName: 'English', englishName: 'English' },
  { locale: 'pt-BR', nativeName: 'Português (Brasil)', englishName: 'Portuguese (Brazil)' },
  { locale: 'es-ES', nativeName: 'Español', englishName: 'Spanish' },
  { locale: 'fr-FR', nativeName: 'Français', englishName: 'French' },
  { locale: 'it-IT', nativeName: 'Italiano', englishName: 'Italian' },
  { locale: 'ja-JP', nativeName: '日本語', englishName: 'Japanese' },
  { locale: 'zh-CN', nativeName: '简体中文', englishName: 'Simplified Chinese' },
  { locale: 'ko-KR', nativeName: '한국어', englishName: 'Korean' },
  { locale: 'ru-RU', nativeName: 'Русский', englishName: 'Russian' },
  { locale: 'de-DE', nativeName: 'Deutsch', englishName: 'German' },
]

let currentLocale: SupportedLocale = DEFAULT_LOCALE
// Latest requested locale: a slower catalog that resolves after a newer choice must not win.
let requestedLocale: SupportedLocale = DEFAULT_LOCALE
let pending: Promise<void> = Promise.resolve()
const localeListeners = new Set<() => void>()

function applyLocale(locale: SupportedLocale): void {
  if (locale === currentLocale) return
  currentLocale = locale
  for (const listener of localeListeners) listener()
}

export function isSupportedLocale(locale: unknown): locale is SupportedLocale {
  return typeof locale === 'string' && SUPPORTED_LOCALES.includes(locale as SupportedLocale)
}

/**
 * Accepts the locale now and switches once its catalog is in memory — synchronously for the
 * bundled ones, after the chunk loads for the rest. Switching before the load would paint a frame
 * of en-US fallback text between the two languages.
 */
export function setLocale(locale: string): boolean {
  if (!isSupportedLocale(locale)) return false
  requestedLocale = locale
  if (isLocaleLoaded(locale)) {
    applyLocale(locale)
    return true
  }
  pending = loadLocale(locale).then(
    () => {
      if (requestedLocale === locale) applyLocale(locale)
    },
    () => {
      // Chunk missing (should never happen in a packaged build): stay on the current locale.
    },
  )
  return true
}

/** Resolves once the last `setLocale` has been applied (or given up). */
export function whenLocaleReady(): Promise<void> {
  return pending
}

export function getLocale(): SupportedLocale {
  return currentLocale
}

export function subscribeLocale(listener: () => void): () => void {
  localeListeners.add(listener)
  return () => localeListeners.delete(listener)
}

export type TranslateParams = { count?: number } & Record<string, string | number | undefined>

type Catalog = Readonly<Record<string, string | undefined>>

/**
 * Forma de plural de `key` para `count`: `<key>.<categoria>` (one/few/many, pela regra do idioma em
 * `Intl.PluralRules`) e, na falta dela, a própria `<key>` — a forma `other`, que todo locale tem.
 * Idioma sem plural (ja/ko/zh) só define `<key>`.
 */
export function resolvePlural(dict: Catalog, key: string, count: number, locale: string): string | undefined {
  const category = new Intl.PluralRules(locale).select(count)
  return dict[`${key}.${category}`] ?? dict[key]
}

function interpolate(text: string, params: TranslateParams): string {
  return text.replace(/\{\{(\w+)\}\}/g, (match, name: string) => {
    const value = params[name]
    return value === undefined ? match : String(value)
  })
}

function lookup(key: string, locale: SupportedLocale, count: number | undefined): string | undefined {
  for (const target of [locale, FALLBACK_LOCALE]) {
    const dict = translations[target] as Catalog | undefined
    if (!dict) continue
    const text = count === undefined ? dict[key] : resolvePlural(dict, key, count, locale)
    if (text !== undefined) return text
  }
  return undefined
}

function resolveLocale(locale: string | undefined): SupportedLocale {
  if (locale === undefined) return currentLocale
  return isSupportedLocale(locale) ? locale : FALLBACK_LOCALE
}

/**
 * `t(key)` / `t(key, locale)` — texto do catálogo. `t(key, { count, ...vars }, locale?)` escolhe a
 * forma de plural por `count` e troca cada `{{nome}}` pelo valor (inclusive `{{count}}`).
 * Chave ausente em todo catálogo devolve a própria chave.
 */
export function t(key: keyof TranslationDict, locale?: string): string
export function t(key: keyof TranslationDict, params: TranslateParams, locale?: string): string
export function t(key: keyof TranslationDict, second?: string | TranslateParams, third?: string): string {
  const params = typeof second === 'object' ? second : undefined
  const locale = resolveLocale(params ? third : (second as string | undefined))
  const text = lookup(key, locale, params?.count)
  if (text === undefined) return key
  return params ? interpolate(text, params) : text
}

export type { TranslationDict, TranslationRecord }
export { isLocaleLoaded, loadLocale, translations }
