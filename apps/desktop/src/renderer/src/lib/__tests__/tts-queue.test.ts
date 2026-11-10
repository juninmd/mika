import { describe, expect, it } from 'vitest'
import { admitToTtsQueue, MAX_TTS_QUEUE } from '../tts-queue'

/** Trava a regressão: fila cheia deve descartar o chunk NOVO, nunca pular frase do meio da resposta. */

const drain = (queue: string[], count: number) => queue.splice(0, count)

describe('admitToTtsQueue', () => {
  it('uma resposta de seis frases é falada inteira, na ordem', () => {
    const queue: string[] = []
    const frases = ['uma', 'duas', 'tres', 'quatro', 'cinco', 'seis']

    for (const frase of frases) admitToTtsQueue(queue, frase)

    expect(queue).toEqual(frases)
  })

  it('a fila cheia descarta o chunk NOVO — nunca o próximo a ser dito', () => {
    const queue = ['proxima', 'depois']

    const resultado = admitToTtsQueue(queue, 'recem-chegada', 2)

    expect(resultado).toBe('dropped')
    expect(queue).toEqual(['proxima', 'depois'])
  })

  it('o descarte é informado, para não ser silencioso como antes', () => {
    const queue = ['a']

    expect(admitToTtsQueue(queue, 'b', 1)).toBe('dropped')
    expect(admitToTtsQueue([], 'b', 1)).toBe('admitted')
  })

  it('vaga aberta pela reprodução volta a aceitar', () => {
    const queue = ['a', 'b']
    expect(admitToTtsQueue(queue, 'c', 2)).toBe('dropped')

    drain(queue, 1)

    expect(admitToTtsQueue(queue, 'c', 2)).toBe('admitted')
    expect(queue).toEqual(['b', 'c'])
  })

  it('o teto padrão não é alcançável por resposta de tamanho real', () => {
    // Chunk tem no máximo 180 caracteres (`MAX_CHUNK_CHARS`): 64 deles são minutos de fala.
    expect(MAX_TTS_QUEUE).toBeGreaterThanOrEqual(32)
  })
})
