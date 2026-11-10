import { describe, expect, it } from 'vitest'
import { seeded } from '../../../sdk/__tests__/fuzzy/prng'
import { setBounded } from '../../src/utils/bounded-cache'

// Modelo de referência: mapa com ordem de inserção (regravar mantém a posição) e corte FIFO em `max`.
function referenceSet(model: Array<[number, number]>, key: number, value: number, max: number): void {
  const at = model.findIndex(([k]) => k === key)
  if (at >= 0) (model[at] as [number, number])[1] = value
  else model.push([key, value])
  while (model.length > max) model.shift()
}

describe('setBounded (fuzzy, seeded)', () => {
  it('acompanha o modelo FIFO em sequências aleatórias com regravações e nunca passa de max', () => {
    const rand = seeded(0x5eed)
    for (let run = 0; run < 200; run++) {
      const max = 1 + Math.floor(rand() * 8)
      const keySpace = 1 + Math.floor(rand() * 15)
      const cache = new Map<number, number>()
      const model: Array<[number, number]> = []
      for (let step = 0; step < 100; step++) {
        const key = Math.floor(rand() * keySpace)
        setBounded(cache, key, step, max)
        referenceSet(model, key, step, max)
        expect(cache.size).toBeLessThanOrEqual(max)
        expect([...cache.entries()]).toEqual(model)
      }
    }
  })
})
