import { describe, expect, it } from 'vitest'
import { setBounded } from '../bounded-cache'

// `setBounded` is the only guard against unbounded cache growth; behavior at the exact bound is what matters.
describe('setBounded', () => {
  it('abaixo do limite, apenas insere sem evictar nada', () => {
    const cache = new Map<string, number>()
    setBounded(cache, 'a', 1, 3)
    setBounded(cache, 'b', 2, 3)

    expect(cache).toEqual(
      new Map([
        ['a', 1],
        ['b', 2],
      ]),
    )
  })

  it('exatamente no limite, não evicta (size <= max é permitido)', () => {
    const cache = new Map<string, number>()
    setBounded(cache, 'a', 1, 2)
    setBounded(cache, 'b', 2, 2)

    expect(cache.size).toBe(2)
    expect(cache.has('a')).toBe(true)
  })

  it('passar do limite por um evicta exatamente o mais antigo (ordem de inserção do Map)', () => {
    const cache = new Map<string, number>()
    setBounded(cache, 'a', 1, 2)
    setBounded(cache, 'b', 2, 2)
    setBounded(cache, 'c', 3, 2)

    expect(cache.size).toBe(2)
    expect(cache.has('a')).toBe(false)
    expect(cache.has('b')).toBe(true)
    expect(cache.has('c')).toBe(true)
  })

  it('max=1 mantém só a última chave escrita', () => {
    const cache = new Map<string, number>()
    setBounded(cache, 'a', 1, 1)
    setBounded(cache, 'b', 2, 1)

    expect(Array.from(cache.entries())).toEqual([['b', 2]])
  })

  it('max=0 evicta tudo, inclusive o item recém-inserido', () => {
    const cache = new Map<string, number>()
    setBounded(cache, 'a', 1, 0)

    expect(cache.size).toBe(0)
  })

  it('quando o mapa já excede o limite por mais de um, o laço evicta mais de uma vez antes de parar', () => {
    // Pre-seed beyond `max` directly (bypassing setBounded) so a single call must loop past the
    // first eviction — exercises the `continue` path of the `if (size <= max) break` guard.
    const cache = new Map<string, number>([
      ['a', 1],
      ['b', 2],
      ['c', 3],
    ])
    setBounded(cache, 'd', 4, 1)

    expect(cache.size).toBe(1)
    expect(Array.from(cache.entries())).toEqual([['d', 4]])
  })

  it('regravar uma chave existente não conta como novo item (Map.set não muda a ordem de inserção)', () => {
    const cache = new Map<string, number>()
    setBounded(cache, 'a', 1, 2)
    setBounded(cache, 'b', 2, 2)
    setBounded(cache, 'a', 99, 2)

    expect(cache.get('a')).toBe(99)
    expect(cache.size).toBe(2)
  })
})
