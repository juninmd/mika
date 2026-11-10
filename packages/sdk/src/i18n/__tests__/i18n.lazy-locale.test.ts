import { afterEach, describe, expect, it } from 'vitest'
import { getLocale, isLocaleLoaded, setLocale, subscribeLocale, t, whenLocaleReady } from '../index'

// Only pt-BR (default) and en-US (fallback) ship in the entry bundle; the rest load on demand.
describe('lazy locale catalogs', () => {
  afterEach(async () => {
    setLocale('pt-BR')
    await whenLocaleReady()
  })

  it('has the default and fallback catalogs available synchronously', () => {
    expect(isLocaleLoaded('pt-BR')).toBe(true)
    expect(isLocaleLoaded('en-US')).toBe(true)
    expect(t('button.save', 'pt-BR')).toBe('Salvar')
  })

  it('switches only once the target catalog is loaded, notifying listeners once', async () => {
    const seen: string[] = []
    const off = subscribeLocale(() => seen.push(`${getLocale()}:${t('button.confirm')}`))

    expect(setLocale('ja-JP')).toBe(true)
    await whenLocaleReady()

    expect(getLocale()).toBe('ja-JP')
    expect(seen).toEqual(['ja-JP:確認'])
    off()
  })

  it('never renders a raw key for a catalog still in flight — explicit locale falls back to en-US', () => {
    if (isLocaleLoaded('zh-CN')) return
    expect(t('button.cancel', 'zh-CN')).toBe('Cancel')
  })

  it('last request wins when a slower catalog resolves after a newer choice', async () => {
    setLocale('fr-FR')
    setLocale('en-US')
    await whenLocaleReady()
    expect(getLocale()).toBe('en-US')
  })
})
