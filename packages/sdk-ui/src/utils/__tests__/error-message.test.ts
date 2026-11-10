import { setLocale } from '@mika/sdk/i18n'
import { afterEach, describe, expect, it } from 'vitest'
import { isNetworkError, toDisplayError } from '../error-message'

describe('toDisplayError', () => {
  it('extracts the message from a real Error instance', () => {
    expect(toDisplayError(new Error('boom'), 'fallback')).toBe('boom')
  })

  // Tauri's `invoke` rejects with a bare STRING, and IPC errors cross the bridge as plain `{ message }`.
  it('accepts a non-empty string rejection (Tauri invoke) and a `{ message }` object', () => {
    expect(toDisplayError('plain string', 'fallback')).toBe('plain string')
    expect(toDisplayError({ message: 'looks like an error' }, 'fallback')).toBe('looks like an error')
    expect(toDisplayError('nope')).toBe('nope')
  })

  it('uses the fallback for blank strings, empty messages and non-message values', () => {
    for (const value of [undefined, null, '', '   ', { message: '' }, { message: 42 }, {}, 42, true, []]) {
      expect(toDisplayError(value, 'fallback')).toBe('fallback')
    }
  })

  it('returns undefined when no fallback is given and the value carries no message', () => {
    expect(toDisplayError(undefined)).toBeUndefined()
    expect(toDisplayError({ code: 1 })).toBeUndefined()
  })

  it('an Error subclass with an empty message still reports the empty message, not the fallback', () => {
    expect(toDisplayError(new Error(''), 'fallback')).toBe('')
  })
})

// O Rust rejeita com texto em pt-BR fixo (`Err(String)`): para quem usa outro idioma isso é lixo
// ilegível na notificação — o detalhe vai para o log e a tela mostra o texto localizado do chamador.
describe('toDisplayError — texto do backend em idioma que não é o dele', () => {
  const rust = "mika:chat-report: 'content' passa de 20000 caracteres"
  afterEach(() => setLocale('pt-BR'))

  it('pt-BR é o idioma do backend: o detalhe cru continua sendo mostrado', () => {
    setLocale('pt-BR')
    expect(toDisplayError('mika:wizard-react: etapa desconhecida "x"', 'fallback')).toBe(
      'mika:wizard-react: etapa desconhecida "x"',
    )
  })

  it('outro idioma: string desconhecida do Rust vira o fallback localizado do chamador', () => {
    setLocale('en-US')
    expect(toDisplayError('mika:wizard-react: etapa desconhecida "x"', 'Could not save.')).toBe('Could not save.')
    expect(toDisplayError({ message: 'não leu a master.key' }, 'Could not save.')).toBe('Could not save.')
    expect(toDisplayError('não leu a master.key')).toBeUndefined()
  })

  it('erro conhecido do backend é traduzido pela chave i18n, com o limite', () => {
    setLocale('en-US')
    const shown = toDisplayError(rust, 'fallback')
    expect(shown).toContain('20000')
    expect(shown).not.toContain('passa de')
    expect(shown).not.toBe('fallback')
  })

  it('erro de rede passa cru: quem decide o texto é o chamador, por isNetworkError', () => {
    setLocale('en-US')
    expect(toDisplayError('fetch failed', 'fallback')).toBe('fetch failed')
    expect(toDisplayError('connect ECONNREFUSED 127.0.0.1:11434', 'fallback')).toContain('ECONNREFUSED')
  })

  it('Error de JavaScript não é texto do backend: a mensagem é mantida em qualquer idioma', () => {
    setLocale('en-US')
    expect(toDisplayError(new Error('boom'), 'fallback')).toBe('boom')
  })

  it('isNetworkError reconhece as assinaturas de rede, e só elas', () => {
    expect(isNetworkError('fetch failed')).toBe(true)
    expect(isNetworkError('Error: connect ECONNREFUSED 127.0.0.1:11434')).toBe(true)
    expect(isNetworkError('modelo indisponível: qwen128k')).toBe(false)
  })
})
