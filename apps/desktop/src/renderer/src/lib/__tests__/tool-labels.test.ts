import { setLocale, whenLocaleReady } from '@mika/sdk/i18n'
import { afterEach, describe, expect, it } from 'vitest'
import { getFriendlyToolLabel } from '../tool-labels'

describe('tool-labels — getFriendlyToolLabel', () => {
  afterEach(async () => {
    setLocale('pt-BR')
    await whenLocaleReady()
  })

  // Regression: the labels were a Portuguese constant, so the thinking HUD stayed Portuguese in every locale.
  it('acompanha o idioma ativo', async () => {
    setLocale('en-US')
    await whenLocaleReady()
    expect(getFriendlyToolLabel('vision_capture')).toBe('Looking at the screen...')
    setLocale('ja-JP')
    await whenLocaleReady()
    expect(getFriendlyToolLabel('vision_capture')).not.toMatch(/Olhando|Looking/)
  })

  it('devolve o rótulo amigável de uma tool conhecida', () => {
    expect(getFriendlyToolLabel('vision_capture')).toBe('Olhando a tela...')
  })

  it('cai no fallback genérico com o nome cru quando a tool é desconhecida', () => {
    expect(getFriendlyToolLabel('some_unmapped_tool')).toBe('some_unmapped_tool')
  })

  it('não confunde tool desconhecida com string vazia (edge case)', () => {
    expect(getFriendlyToolLabel('')).toBe('')
  })

  it('é sensível a maiúsculas/minúsculas — não normaliza o nome da tool', () => {
    expect(getFriendlyToolLabel('Vision_Capture')).toBe('Vision_Capture')
  })
})
