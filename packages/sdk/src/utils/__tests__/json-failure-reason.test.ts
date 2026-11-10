import { describe, expect, it } from 'vitest'
import { jsonFailureReason } from '../redaction'

describe('jsonFailureReason', () => {
  it('não devolve nada do conteúdo do documento', () => {
    let caught: unknown
    try {
      JSON.parse('{"nome":"Fulano de Tal","apiKey":"sk_muito_secreto","x":}')
    } catch (error) {
      caught = error
    }

    const reason = jsonFailureReason(caught)

    expect(reason).not.toContain('Fulano')
    expect(reason).not.toContain('sk_muito_secreto')
    expect(reason).not.toContain('nome')
  })

  it('mantém o que diagnostica: tipo do erro e posição', () => {
    const error = new SyntaxError('Unexpected token \'}\', ..."m SP","x":}" is not valid JSON at position 42')

    expect(jsonFailureReason(error)).toBe('SyntaxError na posição 42')
  })

  it('sem posição na mensagem, sobra o tipo', () => {
    expect(jsonFailureReason(new SyntaxError('is not valid JSON'))).toBe('SyntaxError')
  })

  it('valor que não é Error não vira texto cru', () => {
    expect(jsonFailureReason({ segredo: 'sk_123' })).toBe('erro desconhecido ao ler JSON')
  })
})
