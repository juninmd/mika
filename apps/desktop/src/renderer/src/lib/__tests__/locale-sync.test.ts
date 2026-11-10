import { getLocale, setLocale, whenLocaleReady } from '@mika/sdk/i18n'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const subscriptions: Array<(event: { path: string; value: unknown }) => void> = []
const saved: Array<[string, unknown]> = []
let saveSucceeds = true
let uiPrefs: { locale?: string } = { locale: 'pt-BR' }

vi.mock('../shell-api', () => ({
  getElectronApi: () => ({
    runtime: {
      getUiPrefs: async () => uiPrefs,
      onConfigChanged: (cb: (event: { path: string; value: unknown }) => void) => {
        subscriptions.push(cb)
        return () => {}
      },
      saveSetting: async (path: string, value: unknown) => {
        saved.push([path, value])
        return { success: saveSucceeds }
      },
    },
  }),
}))
vi.mock('../renderer-logger', () => ({ rendererLog: { warn: () => {}, debug: () => {}, error: () => {} } }))

const { changeLocale, ensureLocaleSync } = await import('../locale-sync')

/** Leitura e escuta de `config-changed` acontecem uma vez por janela, não uma por componente. */
describe('sincronia de idioma da janela', () => {
  beforeEach(() => {
    setLocale('pt-BR')
    saveSucceeds = true
    saved.length = 0
    uiPrefs = { locale: 'pt-BR' }
  })

  it('89 consumidores abrem UMA assinatura de IPC, não 89', () => {
    for (let i = 0; i < 89; i++) ensureLocaleSync()

    expect(subscriptions).toHaveLength(1)
  })

  it('a troca vinda de outra janela chega ao idioma corrente', async () => {
    ensureLocaleSync()

    subscriptions[0]?.({ path: 'app.locale', value: 'fr-FR' })
    await whenLocaleReady()

    expect(getLocale()).toBe('fr-FR')
  })

  it('mudança de outra chave não mexe no idioma', () => {
    ensureLocaleSync()

    subscriptions[0]?.({ path: 'character.theme', value: 'neon' })

    expect(getLocale()).toBe('pt-BR')
  })

  it('valor de idioma que não existe é ignorado, e não derruba o corrente', () => {
    ensureLocaleSync()

    subscriptions[0]?.({ path: 'app.locale', value: 'klingon' })

    expect(getLocale()).toBe('pt-BR')
  })

  it('trocar o idioma persiste a escolha', async () => {
    await expect(changeLocale('es-ES')).resolves.toBe(true)

    expect(getLocale()).toBe('es-ES')
    expect(saved).toEqual([['app.locale', 'es-ES']])
  })

  it('disco recusando a escrita volta o idioma — a UI não fica mentindo', async () => {
    saveSucceeds = false

    await expect(changeLocale('it-IT')).resolves.toBe(false)

    expect(getLocale()).toBe('pt-BR')
  })

  it('o idioma salvo no disco vale na abertura da janela', async () => {
    vi.resetModules()
    uiPrefs = { locale: 'ja-JP' }
    // Módulo novo: `started` e o idioma corrente são estado de módulo.
    const i18n = await import('@mika/sdk/i18n')
    const fresh = await import('../locale-sync')
    fresh.ensureLocaleSync()
    await new Promise((resolve) => setTimeout(resolve, 0))
    await i18n.whenLocaleReady()

    expect(i18n.getLocale()).toBe('ja-JP')
    expect(saved).toEqual([])
  })
})
