import { describe, expect, it } from 'vitest'
import { trapTarget } from '../useFocusTrap'

const el = (name: string, disabled = false) => ({ name, disabled })

describe('useFocusTrap — trapTarget', () => {
  const container = { name: 'card' }
  const link = el('link')
  const toggle = el('toggle')
  const quit = el('quit')
  const accept = el('accept', true)

  it('Tab no último focável volta ao primeiro', () => {
    expect(trapTarget([link, toggle, quit], quit, container, false)).toBe(link)
  })

  it('o botão desabilitado no fim da ordem de DOM não é o último focável: Tab no anterior fecha o ciclo', () => {
    // O "Continuar" fica por último e desabilitado até o aceite: se contasse, o foco escaparia do diálogo.
    expect(trapTarget([link, toggle, quit, accept], quit, container, false)).toBe(link)
  })

  it('Shift+Tab no primeiro (ou no próprio cartão) vai ao último focável habilitado', () => {
    expect(trapTarget([link, toggle, quit, accept], link, container, true)).toBe(quit)
    expect(trapTarget([link, toggle, quit, accept], container, container, true)).toBe(quit)
  })

  it('no meio da ordem deixa o navegador andar sozinho', () => {
    expect(trapTarget([link, toggle, quit], toggle, container, false)).toBeNull()
    expect(trapTarget([link, toggle, quit], toggle, container, true)).toBeNull()
  })

  it('sem nenhum focável habilitado não há para onde levar o foco', () => {
    expect(trapTarget([accept], container, container, false)).toBeNull()
    expect(trapTarget([], container, container, true)).toBeNull()
  })
})
