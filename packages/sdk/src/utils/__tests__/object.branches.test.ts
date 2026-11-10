import { describe, expect, it } from 'vitest'
import { readPath, setPath, toErrorMessage, typedEntries, writePathImmutable } from '../object'

describe('toErrorMessage — cauda de fallbacks', () => {
  it('erro sem mensagem útil mas com causa útil usa a causa', () => {
    const err = Object.assign(new Error(''), { cause: new Error('conexão recusada') })

    expect(toErrorMessage(err)).toBe('conexão recusada')
  })

  it('causa também vazia não vira mensagem vazia — sobra ao menos o nome da classe', () => {
    const err = Object.assign(new Error(''), { cause: new Error('') })

    expect(toErrorMessage(err)).toBe('Error (sem mensagem)')
  })

  it('objeto anônimo sem nada aproveitável ainda diz que houve erro', () => {
    expect(toErrorMessage(Object.create(null))).toBe('erro desconhecido (sem mensagem)')
  })

  it('corpo de resposta grande é truncado — mensagem de erro não pode virar um dump', () => {
    const message = toErrorMessage({ responseBody: { texto: 'x'.repeat(2000) } })

    expect(message.length).toBeLessThan(500)
    expect(message).not.toContain('x'.repeat(1000))
  })

  it('objeto cíclico não derruba a formatação do erro', () => {
    const cyclic: Record<string, unknown> = { nome: 'ciclo' }
    cyclic.self = cyclic

    expect(() => toErrorMessage({ data: cyclic })).not.toThrow()
  })

  it('status numérico vira HTTP quando não há texto algum', () => {
    expect(toErrorMessage({ statusCode: 503 })).toBe('HTTP 503')
    expect(toErrorMessage({ status: 404 })).toBe('HTTP 404')
  })

  it('null e undefined são reportados como tal, não como "erro desconhecido"', () => {
    expect(toErrorMessage(null)).toBe('null')
    expect(toErrorMessage(undefined)).toBe('undefined')
    expect(toErrorMessage('falhou')).toBe('falhou')
  })
})

describe('readPath', () => {
  it('caminho que atravessa valor primitivo devolve undefined em vez de estourar', () => {
    expect(readPath({ a: 1 }, ['a', 'b'])).toBeUndefined()
    expect(readPath({ a: null }, ['a', 'b'])).toBeUndefined()
  })

  it('caminho vazio devolve o próprio objeto', () => {
    const obj = { a: 1 }
    expect(readPath(obj, [])).toBe(obj)
  })

  it('caminho válido chega ao valor aninhado', () => {
    expect(readPath({ a: { b: { c: 7 } } }, ['a', 'b', 'c'])).toBe(7)
  })
})

describe('writePathImmutable — escrita sem mutar o original', () => {
  it('caminho vazio devolve o objeto sem cópia', () => {
    const obj = { a: 1 }
    expect(writePathImmutable(obj, [], 9)).toBe(obj)
  })

  it('chave de prototype é recusada — config do usuário não pode envenenar o protótipo', () => {
    for (const key of ['__proto__', 'constructor', 'prototype']) {
      expect(() => writePathImmutable({}, ['voice', key], 1)).toThrow(/perigosa/)
    }
  })

  it('valor undefined apaga a chave em vez de gravar undefined', () => {
    expect(writePathImmutable({ voice: { provider: 'kokoro' } }, ['voice', 'provider'], undefined)).toEqual({
      voice: {},
    })
  })

  it('array no caminho é copiado, não compartilhado com o original', () => {
    const original = { lista: [1, 2] }

    const next = writePathImmutable(original, ['lista', '0'], 9) as { lista: number[] }

    expect(next.lista).not.toBe(original.lista)
    expect(original.lista[0]).toBe(1)
  })

  it('primitivo no meio do caminho é substituído por objeto novo', () => {
    expect(writePathImmutable({ voice: 'texto' }, ['voice', 'provider'], 'kokoro')).toEqual({
      voice: { provider: 'kokoro' },
    })
  })

  it('setPath é o mesmo contrato e não muta a entrada', () => {
    const original = { a: { b: 1 } }

    const next = setPath(original, ['a', 'b'], 2)

    expect(next).toEqual({ a: { b: 2 } })
    expect(original.a.b).toBe(1)
  })
})

describe('typedEntries', () => {
  it('objeto vazio devolve lista vazia', () => {
    expect(typedEntries({})).toEqual([])
  })

  it('preserva pares chave/valor', () => {
    expect(typedEntries({ a: 1, b: 2 })).toEqual([
      ['a', 1],
      ['b', 2],
    ])
  })
})
