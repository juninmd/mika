import { describe, expect, it } from 'vitest'
import type { AppMessage } from '../../components/app/app-shell.types'
import { appendAssistantMessage, isFailedTurn, toShortErrorMessage } from '../shell-message-utils'

describe('toShortErrorMessage — a bolha de erro precisa de texto estável e curto', () => {
  it('erro de rede sempre vira a mesma frase amigável, não o stack cru', () => {
    expect(toShortErrorMessage('TypeError: fetch failed')).toBe(
      'Sem conexão com a internet. Verifica sua rede e tenta de novo.',
    )
  })

  it('mensagem curta e não-rede passa direto', () => {
    expect(toShortErrorMessage('falha ao chamar tool')).toBe('falha ao chamar tool')
  })

  it('mensagem longa e não-rede é truncada para não estourar a bolha do chat', () => {
    const long = 'x'.repeat(200)
    expect(toShortErrorMessage(long)).toBe('Falha ao enviar. Tente novamente.')
  })
})

describe('appendAssistantMessage — dedupe de resposta duplicada no mesmo turno', () => {
  const base: AppMessage[] = [{ id: '1', role: 'user', content: 'oi', timestamp: 1 }]

  it('conteúdo novo é anexado ao final', () => {
    const result = appendAssistantMessage(base, { id: '2', content: 'olá!' })
    expect(result).toHaveLength(2)
    expect(result[1]).toMatchObject({ role: 'assistant', content: 'olá!' })
  })

  it('resposta repetida (retry de IPC) não duplica a última bolha do assistente', () => {
    const withAssistant: AppMessage[] = [...base, { id: '2', role: 'assistant', content: 'olá!', timestamp: 2 }]
    const result = appendAssistantMessage(withAssistant, { id: '3', content: 'olá!' })
    expect(result).toBe(withAssistant)
    expect(result).toHaveLength(2)
  })

  it('mesmo conteúdo mas a última mensagem é do usuário: não é dedupe — anexa normalmente', () => {
    const result = appendAssistantMessage(base, { id: '2', content: 'oi' })
    expect(result).toHaveLength(2)
  })

  it('sem id na resposta, gera um novo — nunca deixa a mensagem sem identidade', () => {
    const result = appendAssistantMessage(base, { content: 'sem id' })
    expect(result[1]?.id).toBeTruthy()
  })
})

describe('isFailedTurn — o chat-send devolve a falha do turno como role "system", não como erro de IPC', () => {
  it('role system é turno que falhou: o texto é o erro, não uma fala da Mika', () => {
    expect(isFailedTurn({ role: 'system' })).toBe(true)
  })

  it('role assistant, ausente ou desconhecido é resposta normal', () => {
    expect(isFailedTurn({ role: 'assistant' })).toBe(false)
    expect(isFailedTurn({})).toBe(false)
  })
})
