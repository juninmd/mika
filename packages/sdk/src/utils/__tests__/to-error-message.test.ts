import { describe, expect, it } from 'vitest'
import { toErrorMessage } from '../object'

describe('toErrorMessage', () => {
  it('mantém a mensagem quando ela diz alguma coisa', () => {
    expect(toErrorMessage(new Error('rate limited'))).toBe('rate limited')
    expect(toErrorMessage('falha simples')).toBe('falha simples')
  })

  it('nunca devolve [object Object] para objeto solto', () => {
    const message = toErrorMessage({ foo: 'bar' })

    expect(message).not.toBe('[object Object]')
    expect(message).toContain('bar')
  })

  it('extrai o motivo de um Error cuja mensagem virou [object Object]', () => {
    const error = Object.assign(new Error({ foo: 1 } as never), {
      statusCode: 502,
      responseBody: '{"error":"upstream indisponivel"}',
    })

    expect(toErrorMessage(error)).toContain('upstream indisponivel')
  })

  it('usa o erro aninhado do provedor quando existe', () => {
    expect(toErrorMessage({ error: { message: 'model is overloaded' } })).toBe('model is overloaded')
    expect(toErrorMessage({ error: 'provider returned error' })).toBe('provider returned error')
  })

  it('cai para o status quando não há texto nenhum', () => {
    expect(toErrorMessage(Object.assign(new Error(''), { statusCode: 429 }))).toBe('HTTP 429')
  })

  it('segue a cause quando o erro de cima não diz nada', () => {
    const error = new Error('')
    error.cause = new Error('conexão recusada')

    expect(toErrorMessage(error)).toBe('conexão recusada')
  })

  it('redige segredo antes de serializar o objeto', () => {
    const message = toErrorMessage({ authorization: 'Bearer sk-super-secreto', detalhe: 'falhou' })

    expect(message).not.toContain('sk-super-secreto')
    expect(message).toContain('REDACTED')
  })

  it('não quebra com referência circular', () => {
    const circular: Record<string, unknown> = { nome: 'loop' }
    circular.self = circular

    expect(() => toErrorMessage(circular)).not.toThrow()
    expect(toErrorMessage(circular)).not.toBe('[object Object]')
  })

  it('diz o nome do construtor quando não sobra mais nada', () => {
    class FalhaEstranha {}

    expect(toErrorMessage(new FalhaEstranha())).toBe('FalhaEstranha (sem mensagem)')
  })

  it('preserva null e undefined legíveis', () => {
    expect(toErrorMessage(null)).toBe('null')
    expect(toErrorMessage(undefined)).toBe('undefined')
  })
})
