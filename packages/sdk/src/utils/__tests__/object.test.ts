import { describe, expect, it } from 'vitest'
import { readPath, setPath, toErrorMessage, typedEntries, writePathImmutable } from '../object'

describe('typedEntries', () => {
  it('behaves like Object.entries with typed keys', () => {
    expect(typedEntries({ a: 1, b: 'x' })).toEqual([
      ['a', 1],
      ['b', 'x'],
    ])
  })
})

describe('setPath', () => {
  it('is an alias for writePathImmutable', () => {
    const result = setPath({}, ['a', 'b'], 1)
    expect(result).toEqual({ a: { b: 1 } })
  })
})

describe('readPath', () => {
  it('reads a nested value by key path', () => {
    expect(readPath({ a: { b: { c: 42 } } }, ['a', 'b', 'c'])).toBe(42)
  })

  it('returns undefined when the path traverses through a non-object', () => {
    expect(readPath({ a: 'not-an-object' }, ['a', 'b'])).toBeUndefined()
  })

  it('returns undefined when a middle key is missing', () => {
    expect(readPath({ a: {} }, ['a', 'b', 'c'])).toBeUndefined()
  })

  it('returns the root object for an empty key path', () => {
    const obj = { a: 1 }
    expect(readPath(obj, [])).toBe(obj)
  })
})

describe('writePathImmutable — arrays and deletion', () => {
  it('clones an existing array along the path instead of mutating the original', () => {
    const original = { list: [1, 2, 3] }
    const result = writePathImmutable(original, ['list', '0'], 99) as { list: number[] }
    expect(result.list[0]).toBe(99)
    expect(original.list[0]).toBe(1)
  })

  it('deletes the key when value is undefined', () => {
    const result = writePathImmutable({ a: { b: 1, c: 2 } }, ['a', 'b'], undefined) as { a: Record<string, unknown> }
    expect('b' in result.a).toBe(false)
    expect(result.a.c).toBe(2)
  })

  it('does not mutate the original object (structural sharing check)', () => {
    const original = { a: { b: 1 } }
    const result = writePathImmutable(original, ['a', 'b'], 2)
    expect(original.a.b).toBe(1)
    expect((result.a as Record<string, unknown>).b).toBe(2)
  })
})

describe('toErrorMessage — nested object stringify and safe fallback', () => {
  it('stringifies a nested non-string detail object when no plain message is available', () => {
    const message = toErrorMessage({ data: { code: 'ERR_X', info: 'context here' } })
    expect(message).toContain('ERR_X')
  })

  it('falls back safely when the nested detail object cannot be stringified (e.g. BigInt)', () => {
    const message = toErrorMessage({ data: { amount: 10n }, status: 400 })
    expect(message).toBe('HTTP 400')
  })

  it('falls back safely when a nested detail object stringifies to an empty/uninformative result', () => {
    // `data` is an empty object: not a useful message and its stringified form is "{}", so it is skipped.
    const message = toErrorMessage({ data: {}, status: 503 })
    expect(message).toBe('HTTP 503')
  })
})
