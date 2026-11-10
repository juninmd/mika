import { describe, expect, it } from 'vitest'
import { reconcileModel } from '../wizard-data'

const installed = [{ id: 'llama3.2:3b' }, { id: 'qwen3.5:9b' }]

describe('reconcileModel', () => {
  it('troca o padrão que não está instalado pelo primeiro modelo da lista viva', () => {
    expect(reconcileModel('qwen3.5:4b', installed, false)).toBe('llama3.2:3b')
  })

  it('mantém a escolha que está na lista', () => {
    expect(reconcileModel('qwen3.5:9b', installed, false)).toBe('qwen3.5:9b')
  })

  it('não mexe enquanto a lista recarrega (o modelo recém-baixado ainda não chegou)', () => {
    expect(reconcileModel('qwen3.5:4b', installed, true)).toBe('qwen3.5:4b')
  })

  it('sem lista nenhuma, mantém o que há', () => {
    expect(reconcileModel('qwen3.5:4b', [], false)).toBe('qwen3.5:4b')
  })

  it('provedor sem modelo de fábrica (Unsloth) começa no primeiro carregado', () => {
    expect(reconcileModel('', installed, false)).toBe('llama3.2:3b')
  })
})
