import { describe, expect, it } from 'vitest'
import { SUPPORTED_LOCALES, SupportedLocaleSchema } from '../../contracts'
import { getLocale, loadLocale, setLocale, subscribeLocale, translations, whenLocaleReady } from '../index'

describe('i18n locale contract', () => {
  it('supports every commercial launch locale', () => {
    expect(SUPPORTED_LOCALES).toEqual([
      'en-US',
      'pt-BR',
      'es-ES',
      'fr-FR',
      'it-IT',
      'ja-JP',
      'zh-CN',
      'ko-KR',
      'ru-RU',
      'de-DE',
    ])

    for (const locale of SUPPORTED_LOCALES) {
      expect(SupportedLocaleSchema.safeParse(locale).success).toBe(true)
    }
  })

  it('reacts only to supported locale changes', async () => {
    setLocale('pt-BR')
    const changes: string[] = []
    const unsubscribe = subscribeLocale(() => changes.push(getLocale()))

    expect(setLocale('ja-JP')).toBe(true)
    expect(setLocale('unsupported')).toBe(false)
    await whenLocaleReady()

    expect(getLocale()).toBe('ja-JP')
    expect(changes).toEqual(['ja-JP'])

    unsubscribe()
    setLocale('pt-BR')
  })

  it('ships a complete catalog for every launch locale', async () => {
    await Promise.all(SUPPORTED_LOCALES.map((locale) => loadLocale(locale)))
    // `<chave>.one/.few/.many` são variantes de plural, que cada idioma tem ou não (ver i18n.plural.test.ts).
    const withoutPluralVariants = (locale: (typeof SUPPORTED_LOCALES)[number]) =>
      Object.keys(translations[locale] ?? {})
        .filter((key) => !/\.(one|few|many)$/.test(key))
        .sort()
    const referenceKeys = withoutPluralVariants('en-US')

    for (const locale of SUPPORTED_LOCALES) {
      expect(withoutPluralVariants(locale)).toEqual(referenceKeys)
    }

    expect(translations['es-ES']?.['button.save']).toBe('Guardar')
    expect(translations['fr-FR']?.['button.retry']).toBe('Réessayer')
    expect(translations['it-IT']?.['status.loading']).toBe('Caricamento...')
    expect(translations['ja-JP']?.['button.confirm']).toBe('確認')
    expect(translations['zh-CN']?.['button.cancel']).toBe('取消')
  })
})

describe('Diagnóstico tab copy', () => {
  // "Homologação" and "Sincronizado" are words from the build pipeline, not from the person using
  // the app: the tab is "Diagnóstico", a passing row "works", and the embeddings card is a test.
  const JARGON = /omolog|ホモロゲ|sincroniz|synchron|\bsynced\b|同期|同步/i

  it('speaks plain words in every locale', async () => {
    await Promise.all(SUPPORTED_LOCALES.map((locale) => loadLocale(locale)))
    const leaks = SUPPORTED_LOCALES.flatMap((locale) =>
      Object.entries(translations[locale] ?? {})
        .filter(([key, value]) => /^settings\.(homologation|diagnostics)\./.test(key) && JARGON.test(String(value)))
        .map(([key, value]) => `${locale} ${key}: ${value}`),
    )
    expect(leaks).toEqual([])
  })
})
